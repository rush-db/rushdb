import { createHash } from 'node:crypto'
import {
  AGENT_MEMORY_SCHEMA_VERSION,
  type AgentMemoryEpisodeInput,
  type AgentMemoryEpisodeV1,
  type AgentMemoryFactInput,
  type AgentMemoryFactV1
} from './types.js'

function sortValue(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(sortValue)
  }
  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .filter(([, child]) => child !== undefined)
        .sort(([left], [right]) =>
          left < right ? -1
          : left > right ? 1
          : 0
        )
        .map(([key, child]) => [key, sortValue(child)])
    )
  }
  return value
}

export function stableStringify(value: unknown): string {
  return JSON.stringify(sortValue(value))
}

export function sha256(value: string): string {
  return createHash('sha256').update(value).digest('hex')
}

export function hashScope(parts: ReadonlyArray<string | undefined>, salt = ''): string {
  const normalized = parts.map((part) => part?.trim() ?? '').join('\u001f')
  return sha256(`${salt}\u001e${normalized}`)
}

export function episodeEventId(input: AgentMemoryEpisodeInput): string {
  return sha256(
    stableStringify({
      schemaVersion: AGENT_MEMORY_SCHEMA_VERSION,
      eventType: 'episode',
      runtime: input.runtime,
      agentId: input.agentId,
      profileId: input.profileId,
      externalSessionId: input.externalSessionId,
      sourceEventId: input.sourceEventId ?? '',
      turnIndex: input.turnIndex,
      userText: input.userText,
      assistantText: input.assistantText
    })
  )
}

export function factEventId(input: AgentMemoryFactInput): string {
  return sha256(
    stableStringify({
      schemaVersion: AGENT_MEMORY_SCHEMA_VERSION,
      eventType: 'fact',
      runtime: input.runtime,
      agentId: input.agentId,
      profileId: input.profileId,
      participantScopeHash: input.participantScopeHash,
      subjectKey: input.subjectKey,
      kind: input.kind,
      sourceEventId: input.sourceEventId,
      text: input.text
    })
  )
}

export function createEpisodeEvent(input: AgentMemoryEpisodeInput): AgentMemoryEpisodeV1 {
  return {
    ...input,
    schemaVersion: AGENT_MEMORY_SCHEMA_VERSION,
    eventType: 'episode',
    eventId: episodeEventId(input)
  }
}

export function createFactEvent(input: AgentMemoryFactInput): AgentMemoryFactV1 {
  return {
    ...input,
    schemaVersion: AGENT_MEMORY_SCHEMA_VERSION,
    eventType: 'fact',
    factId: factEventId(input)
  }
}
