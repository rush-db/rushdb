import { Injectable, Logger, UnprocessableEntityException } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import axios from 'axios'

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

  /**
   * Generate an embedding vector for a single text input.
   * Returns a float32 array whose length equals RUSHDB_EMBEDDING_DIMENSIONS.
   */
  async embed(text: string): Promise<number[]> {
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

    const url = `${baseUrl.replace(/\/$/, '')}/embeddings`

    this.logger.debug(`[embed] POST ${url} model=${model} inputLen=${text.length}`)

    try {
      const { data } = await axios.post<{
        data: Array<{ embedding: number[] }>
      }>(
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

      this.logger.debug(`[embed] received vector length=${embedding.length}`)
      return embedding
    } catch (err: any) {
      const status = err?.response?.status
      const message = err?.response?.data?.error?.message ?? err?.message ?? 'unknown error'
      this.logger.error(`Embedding API call failed (HTTP ${status ?? '?'}): ${message}`)
      throw new UnprocessableEntityException(`Embedding provider error: ${message}`)
    }
  }

  /**
   * Embed multiple texts in a single API call (batch).
   * Falls back to individual calls for providers that don't support array input.
   */
  async embedBatch(texts: string[]): Promise<number[][]> {
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

    const url = `${baseUrl.replace(/\/$/, '')}/embeddings`

    this.logger.debug(`[embedBatch] POST ${url} model=${model} count=${texts.length}`)

    try {
      const { data } = await axios.post<{
        data: Array<{ embedding: number[]; index: number }>
      }>(
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
      const result = sorted.map((item) => item.embedding)
      this.logger.debug(`[embedBatch] received ${result.length} vectors, dims=${result[0]?.length ?? 0}`)
      return result
    } catch (err: any) {
      const status = err?.response?.status
      const body = err?.response?.data
      const message = body?.error?.message ?? err?.message ?? 'unknown error'
      this.logger.error(
        `Batch embedding API call failed (HTTP ${status ?? '?'}): ${message}` +
          (body ? `\nResponse body: ${JSON.stringify(body)}` : '')
      )
      throw new UnprocessableEntityException(`Embedding provider error: ${message}`)
    }
  }
}
