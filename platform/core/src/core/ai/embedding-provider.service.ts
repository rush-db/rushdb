import { Injectable, Logger, UnprocessableEntityException } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import axios from 'axios'

type EmbeddingApiUsage = {
  prompt_tokens?: number
  total_tokens?: number
}

type EmbeddingApiResponse = {
  data: Array<{ embedding: number[]; index?: number }>
  usage?: EmbeddingApiUsage
}

export type EmbedWithUsageResult = {
  embedding: number[]
  tokensUsed?: number
}

export type EmbedBatchWithUsageResult = {
  embeddings: number[][]
  tokensUsed?: number
}

function stringifyProviderErrorBody(body: unknown): string | undefined {
  if (!body) {
    return undefined
  }

  if (typeof body === 'string') {
    return body
  }

  try {
    return JSON.stringify(body)
  } catch {
    return String(body)
  }
}

function extractProviderErrorMessage(err: unknown): { body?: string; message: string; status?: number } {
  if (axios.isAxiosError(err)) {
    const status = err.response?.status
    const body = stringifyProviderErrorBody(err.response?.data)
    const responseMessage =
      typeof err.response?.data?.error === 'string' ? err.response.data.error
      : typeof err.response?.data?.error?.message === 'string' ? err.response.data.error.message
      : typeof err.response?.data?.message === 'string' ? err.response.data.message
      : body

    return {
      body,
      message: responseMessage || err.message || err.code || 'unknown provider error',
      status
    }
  }

  if (err instanceof Error) {
    return { message: err.message }
  }

  return { message: String(err) }
}

/**
 * Thin wrapper around any OpenAI-compatible embeddings endpoint.
 *
 * Configured entirely via env vars:
 *   RUSHDB_EMBEDDING_BASE_URL  – defaults to https://api.openai.com/v1
 *   RUSHDB_EMBEDDING_API_KEY   – Bearer token for the provider
 *   RUSHDB_EMBEDDING_MODEL     – model identifier (provider-specific)
 *
 * Works with OpenAI, OpenRouter, Azure OpenAI, Ollama, etc.
 */
@Injectable()
export class EmbeddingProviderService {
  private readonly logger = new Logger(EmbeddingProviderService.name)

  constructor(private readonly configService: ConfigService) {}

  private getProviderConfig() {
    const baseUrl = this.configService.get<string>('RUSHDB_EMBEDDING_BASE_URL') ?? 'https://api.openai.com/v1'
    const apiKey = this.configService.get<string>('RUSHDB_EMBEDDING_API_KEY') ?? ''
    const model = this.configService.get<string>('RUSHDB_EMBEDDING_MODEL') ?? ''
    const dimensionsRaw = this.configService.get<string>('RUSHDB_EMBEDDING_DIMENSIONS')
    const dimensions = Number.parseInt(dimensionsRaw ?? '0', 10)

    if (!apiKey || !model || !Number.isInteger(dimensions) || dimensions <= 0) {
      throw new UnprocessableEntityException(
        'Embedding provider is not fully configured. Set RUSHDB_EMBEDDING_API_KEY, RUSHDB_EMBEDDING_MODEL, and RUSHDB_EMBEDDING_DIMENSIONS (positive integer).'
      )
    }

    return {
      url: `${baseUrl.replace(/\/$/, '')}/embeddings`,
      apiKey,
      model,
      dimensions
    }
  }

  private parseTokensUsed(usage?: EmbeddingApiUsage): number | undefined {
    const total = usage?.total_tokens
    const prompt = usage?.prompt_tokens
    if (typeof total === 'number' && Number.isFinite(total) && total > 0) {
      return total
    }
    if (typeof prompt === 'number' && Number.isFinite(prompt) && prompt > 0) {
      return prompt
    }
    return undefined
  }

  /**
   * Generate an embedding vector for a single text input.
   * Returns a float32 array whose length equals RUSHDB_EMBEDDING_DIMENSIONS.
   */
  async embed(text: string): Promise<number[]> {
    const result = await this.embedWithUsage(text)
    return result.embedding
  }

  /**
   * Same as embed(), but also returns provider-reported token usage when available.
   */
  async embedWithUsage(text: string): Promise<EmbedWithUsageResult> {
    const { url, apiKey, model, dimensions } = this.getProviderConfig()

    this.logger.debug(`[embed] POST ${url} model=${model} inputLen=${text.length}`)

    try {
      const { data } = await axios.post<EmbeddingApiResponse>(
        url,
        { model, input: text, dimensions },
        {
          headers: {
            Authorization: `Bearer ${apiKey}`,
            'Content-Type': 'application/json'
          }
        }
      )

      const embedding = data?.data?.[0]?.embedding
      if (!Array.isArray(embedding) || embedding.length === 0) {
        throw new Error('Provider returned an empty or malformed embedding')
      }

      const tokensUsed = this.parseTokensUsed(data?.usage)

      this.logger.debug(
        `[embed] received vector length=${embedding.length}` +
          (tokensUsed ? ` tokensUsed=${tokensUsed}` : ' tokensUsed=n/a')
      )

      return { embedding, tokensUsed }
    } catch (err: unknown) {
      const { body, message, status } = extractProviderErrorMessage(err)
      this.logger.error(
        `Embedding API call failed (HTTP ${status ?? '?'}): ${message}` +
          (body ? `\nResponse body: ${body}` : '')
      )
      throw new UnprocessableEntityException(`Embedding provider error: ${message}`)
    }
  }

  /**
   * Embed multiple texts in a single API call (batch).
   * Falls back to individual calls for providers that don't support array input.
   */
  async embedBatch(texts: string[]): Promise<number[][]> {
    const result = await this.embedBatchWithUsage(texts)
    return result.embeddings
  }

  /**
   * Same as embedBatch(), but also returns provider-reported token usage when available.
   */
  async embedBatchWithUsage(texts: string[]): Promise<EmbedBatchWithUsageResult> {
    const { url, apiKey, model, dimensions } = this.getProviderConfig()

    this.logger.debug(`[embedBatch] POST ${url} model=${model} count=${texts.length}`)

    try {
      const { data } = await axios.post<EmbeddingApiResponse>(
        url,
        { model, input: texts, dimensions },
        {
          headers: {
            Authorization: `Bearer ${apiKey}`,
            'Content-Type': 'application/json'
          }
        }
      )

      // Sort by index to preserve order
      const sorted = [...data.data].sort((a, b) => a.index - b.index)
      const embeddings = sorted.map((item) => item.embedding)
      const tokensUsed = this.parseTokensUsed(data?.usage)
      this.logger.debug(
        `[embedBatch] received ${embeddings.length} vectors, dims=${embeddings[0]?.length ?? 0}` +
          (tokensUsed ? ` tokensUsed=${tokensUsed}` : ' tokensUsed=n/a')
      )
      return { embeddings, tokensUsed }
    } catch (err: unknown) {
      const { body, message, status } = extractProviderErrorMessage(err)
      this.logger.error(
        `Batch embedding API call failed (HTTP ${status ?? '?'}): ${message}` +
          (body ? `\nResponse body: ${body}` : '')
      )
      throw new UnprocessableEntityException(`Embedding provider error: ${message}`)
    }
  }
}
