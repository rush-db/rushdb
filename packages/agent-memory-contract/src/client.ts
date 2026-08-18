import RushDB from '@rushdb/javascript-sdk'
import type { DBRecordInstance, DBRecordsArrayInstance } from '@rushdb/javascript-sdk'
import { createEpisodeEvent, createFactEvent } from './canonical.js'
import { boundedText } from './format.js'
import { buildScopeWhere } from './scope.js'
import type {
  AgentMemoryEpisodeInput,
  AgentMemoryEpisodeV1,
  AgentMemoryFactInput,
  AgentMemoryFactV1,
  RecallOptions,
  RecalledMemory
} from './types.js'

type UnknownRecord = DBRecordInstance<any>
type UnknownRecords = DBRecordsArrayInstance<any>

export interface RushDBMemoryClientOptions {
  apiKey?: string
  url?: string
  db?: RushDB
  recentLimit?: number
}

export class RushDBAgentMemory {
  private readonly db: RushDB
  private readonly recent = new Map<string, AgentMemoryEpisodeV1>()
  private readonly recentLimit: number

  public constructor(options: RushDBMemoryClientOptions) {
    if (options.db) {
      this.db = options.db
    } else {
      if (!options.apiKey) {
        throw new Error('RUSHDB_API_KEY is required')
      }
      this.db = new RushDB(options.apiKey, options.url ? { url: options.url } : undefined)
    }
    this.recentLimit = Math.max(1, options.recentLimit ?? 128)
  }

  public async ensureIndexes(): Promise<void> {
    const existing = await this.db.ai.indexes.find()
    const indexes = existing.data ?? []
    const wanted = [
      { label: 'EPISODE', propertyName: 'summary' },
      { label: 'MEMORY_FACT', propertyName: 'text' }
    ]

    for (const candidate of wanted) {
      const found = indexes.some(
        (index) => index.label === candidate.label && index.propertyName === candidate.propertyName
      )
      if (!found) {
        await this.db.ai.indexes.create(candidate)
      }
    }
  }

  public rememberRecent(input: AgentMemoryEpisodeInput | AgentMemoryEpisodeV1): AgentMemoryEpisodeV1 {
    const episode = 'eventId' in input ? input : createEpisodeEvent(input)
    this.recent.delete(episode.eventId)
    this.recent.set(episode.eventId, episode)
    while (this.recent.size > this.recentLimit) {
      const oldest = this.recent.keys().next().value as string | undefined
      if (!oldest) break
      this.recent.delete(oldest)
    }
    return episode
  }

  public async persistEpisode(
    input: AgentMemoryEpisodeInput | AgentMemoryEpisodeV1
  ): Promise<AgentMemoryEpisodeV1> {
    const episode = this.rememberRecent(input)
    await this.db.records.upsert({
      label: 'EPISODE',
      data: { ...episode },
      options: { mergeBy: ['eventId'], mergeStrategy: 'append' }
    })
    return episode
  }

  public async persistFact(input: AgentMemoryFactInput | AgentMemoryFactV1): Promise<AgentMemoryFactV1> {
    const fact = 'factId' in input ? input : createFactEvent(input)
    await this.db.records.upsert({
      label: 'MEMORY_FACT',
      data: { ...fact },
      options: { mergeBy: ['factId'], mergeStrategy: 'append' }
    })
    return fact
  }

  public async recall(options: RecallOptions): Promise<RecalledMemory[]> {
    const limit = Math.max(1, Math.min(options.limit ?? 8, 20))
    const episodeWhere = buildScopeWhere(options, {
      excludeSessionId: options.excludeSessionId
    })
    const factWhere = buildScopeWhere(options, { activeOnly: true })

    const searches: Array<Promise<RecalledMemory[]>> = [
      this.vectorRecall('EPISODE', 'summary', episodeWhere, options.query, limit)
    ]
    if (options.includeFacts !== false) {
      searches.push(this.vectorRecall('MEMORY_FACT', 'text', factWhere, options.query, limit))
    }

    const remote = (await Promise.allSettled(searches)).flatMap((result) =>
      result.status === 'fulfilled' ? result.value : []
    )
    const recent = this.recallRecent(options, limit)
    const merged = new Map<string, RecalledMemory>()

    for (const memory of [...recent, ...remote]) {
      const previous = merged.get(memory.id)
      if (!previous || memory.score > previous.score) {
        merged.set(memory.id, memory)
      }
    }

    return [...merged.values()].sort((left, right) => right.score - left.score).slice(0, limit)
  }

  public async getEpisode(eventId: string, scope: RecallOptions): Promise<RecalledMemory | null> {
    const result = await this.db.records.find({
      labels: ['EPISODE'],
      where: { ...buildScopeWhere(scope), eventId },
      limit: 1
    })
    const record = result.data[0]
    return record ? this.toRecalledMemory('EPISODE', 'summary', record, 1) : null
  }

  private async vectorRecall(
    label: 'EPISODE' | 'MEMORY_FACT',
    propertyName: 'summary' | 'text',
    where: Record<string, unknown>,
    query: string,
    limit: number
  ): Promise<RecalledMemory[]> {
    const result: UnknownRecords = await this.db.records.vectorSearch({
      labels: [label],
      propertyName,
      query: boundedText(query, 4000),
      where,
      limit
    })
    return result.data
      .map((record) =>
        this.toRecalledMemory(
          label,
          propertyName,
          record,
          Number((record.data as Record<string, unknown>).__score ?? 0)
        )
      )
      .filter((memory) => memory.text.length > 0)
  }

  private toRecalledMemory(
    label: 'EPISODE' | 'MEMORY_FACT',
    propertyName: 'summary' | 'text',
    record: UnknownRecord,
    score: number
  ): RecalledMemory {
    const data = record.data as Record<string, unknown>
    return {
      id: String(data.eventId ?? data.factId ?? data.__id ?? ''),
      label,
      text: String(data[propertyName] ?? ''),
      score,
      observedAt:
        typeof data.observedAt === 'string' ? data.observedAt
        : typeof data.validFrom === 'string' ? data.validFrom
        : undefined,
      provenance: typeof data.provenance === 'string' ? data.provenance : undefined,
      trustClass:
        data.trustClass === 'trusted' || data.trustClass === 'mixed' || data.trustClass === 'untrusted' ?
          data.trustClass
        : undefined
    }
  }

  private recallRecent(options: RecallOptions, limit: number): RecalledMemory[] {
    const terms = options.query
      .toLocaleLowerCase()
      .split(/[^\p{L}\p{N}_-]+/u)
      .filter((term) => term.length > 1)

    return [...this.recent.values()]
      .filter(
        (episode) =>
          episode.agentId === options.agentId &&
          episode.profileId === options.profileId &&
          episode.privacyScope === options.privacyScope &&
          episode.participantScopeHash === options.participantScopeHash &&
          episode.sandboxEligible === options.sandboxEligible &&
          episode.externalSessionId !== options.excludeSessionId
      )
      .map((episode) => {
        const haystack = episode.summary.toLocaleLowerCase()
        const matches = terms.filter((term) => haystack.includes(term)).length
        return {
          id: episode.eventId,
          label: 'EPISODE' as const,
          text: episode.summary,
          score: terms.length === 0 ? 0 : Math.min(1, 0.55 + matches / terms.length / 2),
          observedAt: episode.observedAt,
          provenance: episode.provenance,
          trustClass: episode.trustClass
        }
      })
      .filter((memory) => memory.score > 0.55)
      .sort((left, right) => right.score - left.score)
      .slice(0, limit)
  }
}
