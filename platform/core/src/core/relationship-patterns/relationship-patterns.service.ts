import { forwardRef, Inject, Injectable, Logger } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import axios from 'axios'
import { Transaction } from 'neo4j-driver'

import { AiService } from '@/core/ai/ai.service'
import { OntologyItem } from '@/core/ai/ai.types'
import { estimateTokens } from '@/core/ai/embedding.utils'
import { RUSHDB_RELATION_DEFAULT } from '@/core/common/constants'
import { EntityService } from '@/core/entity/entity.service'
import { TRelationDirection } from '@/core/entity/entity.types'
import { KuOperation } from '@/core/ku-events/ku-events.constants'
import { KuEventsService } from '@/core/ku-events/ku-events.service'
import { ProjectRepository } from '@/dashboard/project/model/project.repository'
import { NeogmaService } from '@/database/neogma/neogma.service'

import { createHash } from 'crypto'

import { RelationshipPatternsRepository } from './relationship-patterns.repository'
import {
  RelationshipPatternCandidate,
  RelationshipPatternDto,
  RelationshipPatternEndpoint,
  RelationshipPatternListResponse,
  RelationshipPatternMode,
  RelationshipPatternStatus
} from './relationship-patterns.types'

import type { RelationshipPatternRow } from '@/database/sql/schema/types'

const ANALYSIS_DEBOUNCE_MS = 60_000
const MAX_LLM_CANDIDATES = 20

@Injectable()
export class RelationshipPatternsService {
  private readonly logger = new Logger(RelationshipPatternsService.name)
  private readonly runningAnalysis = new Set<string>()
  private readonly runningApply = new Set<string>()

  constructor(
    private readonly repository: RelationshipPatternsRepository,
    private readonly configService: ConfigService,
    private readonly aiService: AiService,
    private readonly neogmaService: NeogmaService,
    private readonly kuEventsService: KuEventsService,
    private readonly projectRepository: ProjectRepository,
    @Inject(forwardRef(() => EntityService))
    private readonly entityService: EntityService
  ) {}

  async list(projectId: string, transaction: Transaction): Promise<RelationshipPatternListResponse> {
    const [patterns, ontology, analysis] = await Promise.all([
      this.repository.findByProjectId(projectId),
      this.aiService.getOntology({ projectId, transaction }),
      this.repository.getQueue(projectId)
    ])

    return {
      patterns: patterns.map((row) => this.toDto(row)),
      relationships: this.summarizeExistingRelationships(ontology),
      analysis:
        analysis ?
          {
            status: analysis.status,
            requestedAt: analysis.requestedAt,
            notBefore: analysis.notBefore,
            lastRunAt: analysis.lastRunAt,
            lastError: analysis.lastError
          }
        : undefined
    }
  }

  private summarizeExistingRelationships(
    ontology: OntologyItem[]
  ): RelationshipPatternListResponse['relationships'] {
    const grouped = new Map<
      string,
      RelationshipPatternListResponse['relationships'][number]['relationships']
    >()
    const seen = new Set<string>()

    for (const item of ontology) {
      for (const relationship of item.relationships) {
        const sourceLabel = relationship.direction === 'in' ? relationship.label : item.label
        const targetLabel = relationship.direction === 'in' ? item.label : relationship.label
        const key = `${sourceLabel}:${relationship.type}:${targetLabel}`

        if (seen.has(key)) {
          continue
        }

        seen.add(key)
        const relationships = grouped.get(sourceLabel) ?? []
        relationships.push({
          label: targetLabel,
          type: relationship.type,
          direction: 'out'
        })
        grouped.set(sourceLabel, relationships)
      }
    }

    return [...grouped.entries()].map(([label, relationships]) => ({ label, relationships }))
  }

  async markAfterWrite(projectId: string): Promise<void> {
    if (!this.analysisEnabled()) {
      return
    }
    const queue = await this.repository.getQueue(projectId)
    const now = Date.now()
    const lastRunAt = queue?.lastRunAt ? new Date(queue.lastRunAt).getTime() : 0
    const nextAllowedAt =
      lastRunAt && now - lastRunAt < ANALYSIS_DEBOUNCE_MS ? lastRunAt + ANALYSIS_DEBOUNCE_MS : now
    const existingNotBefore = queue?.status === 'pending' ? new Date(queue.notBefore).getTime() : undefined
    const notBefore = new Date(
      existingNotBefore && existingNotBefore < nextAllowedAt ? existingNotBefore : nextAllowedAt
    ).toISOString()
    await this.repository.enqueueAnalysis(projectId, notBefore)
  }

  async forceAnalysis(projectId: string, workspaceId?: string): Promise<void> {
    await this.repository.enqueueAnalysis(projectId, new Date().toISOString())
    this.runAnalysisForProject(projectId, workspaceId, true).catch((error) => {
      this.logger.error(`[RelationshipAnalysis] project ${projectId} failed`, error)
    })
  }

  async approve(projectId: string, id: string): Promise<RelationshipPatternDto | undefined> {
    const pattern = await this.repository.updatePattern(
      id,
      {
        status: 'approved',
        lastError: null
      },
      projectId
    )
    if (!pattern) {
      return undefined
    }

    await this.applyPatternInFreshTransaction(pattern)

    const appliedPattern = await this.repository.findById(id, projectId)
    return this.toDto(appliedPattern ?? pattern)
  }

  async ignore(projectId: string, id: string): Promise<RelationshipPatternDto | undefined> {
    const pattern = await this.repository.updatePattern(
      id,
      {
        status: 'ignored',
        lastError: null
      },
      projectId
    )
    return pattern ? this.toDto(pattern) : undefined
  }

  async delete(projectId: string, id: string, deleteExisting = false): Promise<void> {
    const pattern = await this.repository.findById(id, projectId)
    if (!pattern) {
      return
    }

    await this.repository.deletePattern(id, projectId)

    if (deleteExisting) {
      this.deleteRelationshipsInFreshTransaction(pattern).catch((error) => {
        this.logger.error(`[RelationshipPattern] relationship cleanup failed for pattern ${id}`, error)
      })
    }
  }

  async applyApprovedPatterns(projectId: string, transaction: Transaction): Promise<void> {
    if (this.runningApply.has(projectId)) {
      return
    }

    this.runningApply.add(projectId)
    try {
      const patterns = await this.repository.findApproved(projectId)
      for (const pattern of patterns) {
        await this.applyPattern(pattern, transaction)
      }
    } finally {
      this.runningApply.delete(projectId)
    }
  }

  async processDueAnalysis(): Promise<void> {
    const due = await this.repository.findDueAnalysis(new Date().toISOString())
    await Promise.allSettled(due.map((queueRow) => this.runAnalysisForProject(queueRow.projectId)))
  }

  async runAnalysisForProject(projectId: string, workspaceId?: string, isManual = false): Promise<void> {
    if (this.runningAnalysis.has(projectId)) {
      return
    }
    this.runningAnalysis.add(projectId)

    await this.repository.updateQueue(projectId, { status: 'running', lastError: null })

    const session = this.neogmaService.createSession('relationship-analysis')
    const transaction = session.beginTransaction({ timeout: 60_000 })

    try {
      const ontology = await this.aiService.getOntology({ projectId, force: true, transaction })
      await transaction.commit()
      await this.neogmaService.closeSession(session, 'relationship-analysis')

      // Resolve workspaceId for KU billing — supplied by the controller for manual
      // refreshes; looked up from the project record for scheduler-triggered runs.
      const resolvedWorkspaceId =
        workspaceId ?? (await this.projectRepository.findById(projectId))?.workspaceId

      const existingPatterns = await this.repository.findByProjectId(projectId)
      const { candidates, promptTokens, completionTokens, totalTokens } = await this.suggestCandidates(
        ontology,
        existingPatterns
      )
      const deterministicCandidates = this.suggestDeterministicCandidates(ontology)
      const validCandidates = this.dedupeInverseCandidates(
        [...deterministicCandidates, ...candidates]
          .map((candidate) => this.validateCandidate(candidate, ontology))
          .filter((candidate): candidate is RelationshipPatternCandidate => Boolean(candidate))
      )
        .filter((candidate) => !this.hasExistingPatternForJoin(candidate, existingPatterns))
        .slice(0, MAX_LLM_CANDIDATES)

      for (const candidate of validCandidates) {
        const row = this.candidateToInsert(projectId, candidate)
        await this.repository.upsertCandidate(row)
      }

      if (resolvedWorkspaceId) {
        this.kuEventsService.emit(resolvedWorkspaceId, projectId, KuOperation.RELATIONSHIP_ANALYSIS, {
          model: this.configService.get<string>('RUSHDB_LLM_MODEL'),
          promptTokens,
          completionTokens,
          totalTokens,
          candidateCount: validCandidates.length,
          trigger: isManual ? 'manual' : 'scheduler'
        })
      }

      await this.repository.updateQueue(projectId, {
        status: 'idle',
        lastRunAt: new Date().toISOString(),
        lastError: null
      })
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      await this.repository.updateQueue(projectId, {
        status: 'error',
        lastRunAt: new Date().toISOString(),
        lastError: message
      })
      try {
        if (transaction.isOpen()) {
          await transaction.rollback()
        }
      } catch {
        /* empty */
      }
      try {
        await this.neogmaService.closeSession(session, 'relationship-analysis')
      } catch {
        /* empty */
      }
      throw error
    } finally {
      this.runningAnalysis.delete(projectId)
    }
  }

  private suggestDeterministicCandidates(ontology: OntologyItem[]): RelationshipPatternCandidate[] {
    const agent = this.findOntologyItemWithProperty(ontology, ['AGENT'], 'agentId')
    const run = this.findOntologyItemWithProperty(ontology, ['RUN'], 'agentId')

    if (!agent || !run || agent.label === run.label) {
      return []
    }

    return [
      {
        source: { label: agent.label, key: 'agentId' },
        target: { label: run.label, key: 'agentId' },
        direction: 'out',
        type: 'HAS_RUN',
        mode: 'join_pattern',
        confidence: 0.92,
        rationale: `${run.label}.agentId references ${agent.label}.agentId, linking evaluation runs to the agent that produced them.`
      }
    ]
  }

  private findOntologyItemWithProperty(
    ontology: OntologyItem[],
    preferredLabels: string[],
    propertyName: string
  ): OntologyItem | undefined {
    const normalizedPreferred = preferredLabels.map((label) => label.toUpperCase())
    const candidates = ontology.filter((item) =>
      item.properties.some((property) => property.name.toLowerCase() === propertyName.toLowerCase())
    )

    return (
      candidates.find((item) => normalizedPreferred.includes(item.label.toUpperCase())) ??
      candidates.find((item) => normalizedPreferred.some((label) => item.label.toUpperCase().includes(label)))
    )
  }

  private async suggestCandidates(
    ontology: OntologyItem[],
    existingPatterns: RelationshipPatternRow[]
  ): Promise<{
    candidates: RelationshipPatternCandidate[]
    promptTokens: number
    completionTokens: number
    totalTokens: number
  }> {
    const empty = { candidates: [], promptTokens: 0, completionTokens: 0, totalTokens: 0 }
    const apiKey = this.configService.get<string>('RUSHDB_LLM_API_KEY')
    const model = this.configService.get<string>('RUSHDB_LLM_MODEL')
    const baseUrl = this.configService.get<string>('RUSHDB_LLM_BASE_URL') ?? 'https://api.openai.com/v1'

    if (!apiKey || !model) {
      return empty
    }

    const response = await axios.post(
      `${baseUrl.replace(/\/$/, '')}/chat/completions`,
      {
        model,
        temperature: 0,
        response_format: { type: 'json_object' },
        messages: [
          {
            role: 'system',
            content:
              'You infer graph relationship patterns for RushDB. Return only JSON: {"candidates":[...]}. ' +
              'Every candidate must include mode: "join_pattern" or "retype_existing_relationship". ' +
              'Use "join_pattern" only for high-confidence foreign-key-like joins where one label has a reference field matching another label key. ' +
              'Use "retype_existing_relationship" when ontology already shows a RUSHDB_DEFAULT_RELATION between labels; in that case source.key and target.key are optional and the task is to rename existing structure semantically. ' +
              'For retype_existing_relationship, infer the semantic relationship from the existing graph structure and label meanings, not from same-named descriptive properties. ' +
              'Do not suggest joins based only on common descriptive fields such as name, title, label, description, status, country, or type. ' +
              'If a default relationship already connects two labels, prefer retype_existing_relationship over join_pattern. ' +
              'Return ONE canonical candidate per semantic relationship; never return both A->B and B->A for the same relationship. ' +
              'Do not suggest a relationship when the same mode and label/key pair already appears in existingPatterns, even if you would use a different synonym or inverse type. ' +
              'Choose source as the natural actor/owner/parent and target as the natural object/action/child. ' +
              'If both labels represent equal peers and direction is semantically ambiguous, choose a neutral symmetric type and use deterministic alphabetical label/key ordering for source and target. ' +
              'Relationship type must read naturally from source to target. Do not choose a direction where the target would appear to act on or create the source. ' +
              'Each candidate must include source.label, target.label, mode, direction "out", type, confidence 0..1, and rationale. join_pattern must also include source.key and target.key. Return at most 8 candidates.'
          },
          {
            role: 'user',
            content: JSON.stringify({
              ontology: this.compactOntologyForRelationshipAnalysis(ontology),
              existingPatterns: this.compactExistingPatternsForRelationshipAnalysis(existingPatterns),
              relationshipTypeRules:
                'Use uppercase Neo4j-safe verb phrases from source to target. Do not use inverse duplicate types for the same relationship.'
            })
          }
        ]
      },
      {
        headers: {
          Authorization: `Bearer ${apiKey}`,
          'Content-Type': 'application/json'
        },
        timeout: 30_000
      }
    )

    const content = response.data?.choices?.[0]?.message?.content
    if (!content) {
      return empty
    }

    const usage = response.data?.usage
    const systemContent =
      'You infer graph relationship patterns for RushDB. Return only JSON: {"candidates":[...]}. '
    const userContent = JSON.stringify({
      ontology: this.compactOntologyForRelationshipAnalysis(ontology),
      existingPatterns: this.compactExistingPatternsForRelationshipAnalysis(existingPatterns)
    })
    const promptTokens = usage?.prompt_tokens ?? estimateTokens(systemContent + userContent)
    const completionTokens = usage?.completion_tokens ?? estimateTokens(content)
    const totalTokens = usage?.total_tokens ?? promptTokens + completionTokens

    const parsed = JSON.parse(content)
    return {
      candidates: Array.isArray(parsed?.candidates) ? parsed.candidates : [],
      promptTokens,
      completionTokens,
      totalTokens
    }
  }

  private compactOntologyForRelationshipAnalysis(ontology: OntologyItem[]) {
    return ontology.map((item) => ({
      label: item.label,
      count: item.count,
      properties: item.properties.map((property) => ({
        name: property.name,
        type: property.type,
        values: property.values?.slice(0, 3)
      })),
      relationships: item.relationships
    }))
  }

  private compactExistingPatternsForRelationshipAnalysis(patterns: RelationshipPatternRow[]) {
    return patterns.map((pattern) => ({
      status: pattern.status,
      mode: pattern.mode,
      source: { label: pattern.sourceLabel, key: pattern.sourceKey },
      target: { label: pattern.targetLabel, key: pattern.targetKey },
      direction: pattern.direction,
      type: pattern.type
    }))
  }

  private dedupeInverseCandidates(
    candidates: RelationshipPatternCandidate[]
  ): RelationshipPatternCandidate[] {
    const byJoin = new Map<string, RelationshipPatternCandidate>()

    for (const candidate of candidates) {
      const mode = this.normalizePatternMode(candidate.mode)
      const endpoints =
        mode === 'retype_existing_relationship' ?
          [candidate.source.label, candidate.target.label].sort()
        : [
            `${candidate.source.label}.${candidate.source.key}`,
            `${candidate.target.label}.${candidate.target.key}`
          ].sort()
      const key = `${mode}|${endpoints.join('|')}`
      const normalizedCandidate =
        this.isSymmetricRelationship(candidate) ? this.normalizeSymmetricCandidate(candidate) : candidate
      const existing = byJoin.get(key)

      if (!existing || this.scoreCandidate(normalizedCandidate) > this.scoreCandidate(existing)) {
        byJoin.set(key, normalizedCandidate)
      }
    }

    return [...byJoin.values()]
  }

  private isSymmetricRelationship(candidate: RelationshipPatternCandidate): boolean {
    const type = this.normalizeRelationType(candidate.type)
    return [
      'FRIEND',
      'FRIENDS',
      'FRIEND_OF',
      'CONNECTED_TO',
      'RELATED_TO',
      'PEER_OF',
      'COLLEAGUE_OF'
    ].includes(type)
  }

  private normalizeSymmetricCandidate(candidate: RelationshipPatternCandidate): RelationshipPatternCandidate {
    const left = `${candidate.source.label}.${candidate.source.key}`
    const right = `${candidate.target.label}.${candidate.target.key}`
    if (left.localeCompare(right) <= 0) {
      return { ...candidate, direction: 'out' }
    }

    return {
      ...candidate,
      source: candidate.target,
      target: candidate.source,
      direction: 'out'
    }
  }

  private hasExistingPatternForJoin(
    candidate: RelationshipPatternCandidate,
    patterns: RelationshipPatternRow[]
  ): boolean {
    const mode = this.normalizePatternMode(candidate.mode)
    const candidateKey =
      mode === 'retype_existing_relationship' ?
        this.unorderedLabelKey(candidate.source.label, candidate.target.label)
      : this.unorderedJoinKey(
          candidate.source.label,
          candidate.source.key,
          candidate.target.label,
          candidate.target.key
        )

    return patterns.some(
      (pattern) =>
        pattern.mode === mode &&
        (mode === 'retype_existing_relationship' ?
          this.unorderedLabelKey(pattern.sourceLabel, pattern.targetLabel) === candidateKey
        : this.unorderedJoinKey(
            pattern.sourceLabel,
            pattern.sourceKey,
            pattern.targetLabel,
            pattern.targetKey
          ) === candidateKey)
    )
  }

  private unorderedLabelKey(sourceLabel: string, targetLabel: string): string {
    return [sourceLabel, targetLabel].sort().join('|')
  }

  private unorderedJoinKey(
    sourceLabel: string,
    sourceKey: string | null | undefined,
    targetLabel: string,
    targetKey: string | null | undefined
  ): string {
    return [`${sourceLabel}.${sourceKey ?? ''}`, `${targetLabel}.${targetKey ?? ''}`].sort().join('|')
  }

  private scoreCandidate(candidate: RelationshipPatternCandidate): number {
    let score = this.normalizeConfidence(candidate.confidence)
    const type = this.normalizeRelationType(candidate.type)
    const source = candidate.source.label.toUpperCase()
    const target = candidate.target.label.toUpperCase()

    if (type.includes(target) || type.includes(this.singularize(target))) {
      score += 0.2
    }
    if (type.includes(source) && !type.includes(target)) {
      score -= 0.2
    }

    return score
  }

  private singularize(value: string): string {
    return value.endsWith('S') ? value.slice(0, -1) : value
  }

  private analysisEnabled(): boolean {
    return (
      Boolean(this.configService.get<string>('RUSHDB_LLM_API_KEY')) &&
      Boolean(this.configService.get<string>('RUSHDB_LLM_MODEL'))
    )
  }

  private validateCandidate(
    candidate: RelationshipPatternCandidate,
    ontology: OntologyItem[]
  ): RelationshipPatternCandidate | undefined {
    if (!candidate?.source?.label || !candidate?.target?.label) {
      return undefined
    }

    const source = ontology.find((item) => item.label === candidate.source.label)
    const target = ontology.find((item) => item.label === candidate.target.label)
    if (!source || !target || source.label === target.label) {
      return undefined
    }

    const mode = this.normalizePatternMode(candidate.mode)
    const hasDefaultRelationship = this.hasDefaultRelationshipBetween(source, target)

    if (mode === 'retype_existing_relationship') {
      if (!hasDefaultRelationship) {
        return undefined
      }
    } else {
      if (hasDefaultRelationship) {
        return undefined
      }

      if (this.hasSemanticRelationshipBetween(source, target)) {
        return undefined
      }

      if (!candidate.source.key || !candidate.target.key) {
        return undefined
      }

      const sourceHasKey = source.properties.some((property) => property.name === candidate.source.key)
      const targetHasKey = target.properties.some((property) => property.name === candidate.target.key)
      if (!sourceHasKey || !targetHasKey || !this.isSafeJoinCandidate(candidate)) {
        return undefined
      }
    }

    return {
      source: this.normalizeEndpoint(candidate.source),
      target: this.normalizeEndpoint(candidate.target),
      direction: candidate.direction === 'in' ? 'in' : 'out',
      type: this.normalizeRelationType(candidate.type),
      mode,
      confidence: this.calibrateConfidence(candidate, mode),
      rationale: typeof candidate.rationale === 'string' ? candidate.rationale.slice(0, 500) : undefined,
      sampleMatchCount:
        typeof candidate.sampleMatchCount === 'number' ? Math.max(0, candidate.sampleMatchCount) : undefined
    }
  }

  private normalizePatternMode(mode?: string) {
    return mode === 'retype_existing_relationship' ? 'retype_existing_relationship' : 'join_pattern'
  }

  private isDefaultRelationType(type: string): boolean {
    return type === RUSHDB_RELATION_DEFAULT || type === 'RUSHDB_DEFAULT_RELATION'
  }

  private hasDefaultRelationshipBetween(source: OntologyItem, target: OntologyItem): boolean {
    return (
      source.relationships.some(
        (relationship) => relationship.label === target.label && this.isDefaultRelationType(relationship.type)
      ) ||
      target.relationships.some(
        (relationship) => relationship.label === source.label && this.isDefaultRelationType(relationship.type)
      )
    )
  }

  private hasSemanticRelationshipBetween(source: OntologyItem, target: OntologyItem): boolean {
    return (
      source.relationships.some(
        (relationship) =>
          relationship.label === target.label && !this.isDefaultRelationType(relationship.type)
      ) ||
      target.relationships.some(
        (relationship) =>
          relationship.label === source.label && !this.isDefaultRelationType(relationship.type)
      )
    )
  }

  private isSafeJoinCandidate(candidate: RelationshipPatternCandidate): boolean {
    const sourceKey = candidate.source.key ?? ''
    const targetKey = candidate.target.key ?? ''
    const generic = new Set(['name', 'title', 'label', 'description', 'status', 'country', 'type'])
    if (sourceKey === targetKey && generic.has(sourceKey.toLowerCase())) {
      return false
    }

    const sourceLabel = this.normalizeReferenceToken(candidate.source.label)
    const targetLabel = this.normalizeReferenceToken(candidate.target.label)
    const sourceKeyToken = this.normalizeReferenceToken(sourceKey)
    const targetKeyToken = this.normalizeReferenceToken(targetKey)
    const identityLike = /(id|ref|key|token|email)$/i
    return (
      this.hasLabelSpecificReference(candidate) ||
      identityLike.test(sourceKey) ||
      identityLike.test(targetKey)
    )
  }

  private calibrateConfidence(
    candidate: RelationshipPatternCandidate,
    mode: RelationshipPatternMode
  ): number {
    const confidence = this.normalizeConfidence(candidate.confidence)
    if (mode !== 'join_pattern') {
      return confidence
    }

    return this.hasLabelSpecificReference(candidate) ? Math.min(confidence, 0.95) : Math.min(confidence, 0.75)
  }

  private hasLabelSpecificReference(candidate: RelationshipPatternCandidate): boolean {
    const sourceLabel = this.normalizeReferenceToken(candidate.source.label)
    const targetLabel = this.normalizeReferenceToken(candidate.target.label)
    const sourceKeyToken = this.normalizeReferenceToken(candidate.source.key ?? '')
    const targetKeyToken = this.normalizeReferenceToken(candidate.target.key ?? '')

    return sourceKeyToken.includes(targetLabel) || targetKeyToken.includes(sourceLabel)
  }

  private normalizeReferenceToken(value: string): string {
    return value
      .toLowerCase()
      .replace(/[^a-z0-9]/g, '')
      .replace(/s$/, '')
  }

  private normalizeEndpoint(endpoint: RelationshipPatternEndpoint): RelationshipPatternEndpoint {
    return {
      label: String(endpoint.label).trim(),
      key: endpoint.key ? String(endpoint.key).trim() : undefined,
      where: endpoint.where
    }
  }

  private normalizeRelationType(type?: string): string {
    const normalized = String(type || RUSHDB_RELATION_DEFAULT)
      .trim()
      .toUpperCase()
      .replace(/\s+/g, '_')
      .replace(/[^A-Z0-9_]/g, '')
    return normalized || RUSHDB_RELATION_DEFAULT
  }

  private normalizeConfidence(confidence?: number): number {
    if (typeof confidence !== 'number' || Number.isNaN(confidence)) {
      return 0
    }
    return Math.max(0, Math.min(1, confidence))
  }

  private candidateToInsert(projectId: string, candidate: RelationshipPatternCandidate) {
    const now = new Date().toISOString()
    const mode = this.normalizePatternMode(candidate.mode)
    const sourceWhere = candidate.source.where ? JSON.stringify(candidate.source.where) : null
    const targetWhere = candidate.target.where ? JSON.stringify(candidate.target.where) : null
    const signatureHash = this.signatureHash({
      mode,
      sourceLabel: candidate.source.label,
      sourceKey: mode === 'join_pattern' ? candidate.source.key : null,
      sourceWhere,
      targetLabel: candidate.target.label,
      targetKey: mode === 'join_pattern' ? candidate.target.key : null,
      targetWhere,
      direction: candidate.direction,
      type: candidate.type
    })

    return {
      projectId,
      sourceLabel: candidate.source.label,
      sourceKey: mode === 'join_pattern' ? candidate.source.key : null,
      sourceWhere,
      targetLabel: candidate.target.label,
      targetKey: mode === 'join_pattern' ? candidate.target.key : null,
      targetWhere,
      direction: candidate.direction ?? 'out',
      type: candidate.type ?? RUSHDB_RELATION_DEFAULT,
      mode,
      confidence: Math.round(this.normalizeConfidence(candidate.confidence) * 10_000),
      status: 'suggested' as RelationshipPatternStatus,
      origin: 'llm',
      signatureHash,
      rationale: candidate.rationale,
      sampleMatchCount: candidate.sampleMatchCount,
      lastAnalyzedAt: now,
      lastError: null
    }
  }

  private signatureHash(value: unknown): string {
    return createHash('sha256').update(JSON.stringify(value)).digest('hex')
  }

  private toDto(row: RelationshipPatternRow): RelationshipPatternDto {
    return {
      id: row.id,
      status: row.status as RelationshipPatternDto['status'],
      origin: row.origin as RelationshipPatternDto['origin'],
      source: {
        label: row.sourceLabel,
        key: row.sourceKey ?? undefined,
        where: row.sourceWhere ? JSON.parse(row.sourceWhere) : undefined
      },
      target: {
        label: row.targetLabel,
        key: row.targetKey ?? undefined,
        where: row.targetWhere ? JSON.parse(row.targetWhere) : undefined
      },
      direction: row.direction as RelationshipPatternDto['direction'],
      type: row.type,
      mode: this.normalizePatternMode(row.mode),
      confidence: row.confidence / 10_000,
      rationale: row.rationale ?? undefined,
      sampleMatchCount: row.sampleMatchCount ?? undefined,
      lastAppliedAt: row.lastAppliedAt ?? undefined,
      lastAnalyzedAt: row.lastAnalyzedAt ?? undefined,
      lastError: row.lastError ?? undefined,
      createdAt: row.createdAt,
      updatedAt: row.updatedAt
    }
  }

  private async applyPattern(pattern: RelationshipPatternRow, transaction: Transaction): Promise<void> {
    try {
      const source = {
        label: pattern.sourceLabel,
        key: pattern.sourceKey,
        where: pattern.sourceWhere ? JSON.parse(pattern.sourceWhere) : undefined
      }
      const target = {
        label: pattern.targetLabel,
        key: pattern.targetKey,
        where: pattern.targetWhere ? JSON.parse(pattern.targetWhere) : undefined
      }
      const direction = pattern.direction as TRelationDirection
      const appliedCount =
        pattern.mode === 'retype_existing_relationship' ?
          await this.entityService.retypeRelationsByLabels({
            source: { label: pattern.sourceLabel },
            target: { label: pattern.targetLabel },
            sourceType: RUSHDB_RELATION_DEFAULT,
            targetType: pattern.type,
            direction,
            projectId: pattern.projectId,
            transaction
          })
        : await this.entityService.createRelationsByKeys({
            source,
            target,
            type: pattern.type,
            direction,
            projectId: pattern.projectId,
            transaction
          })
      if (pattern.mode === 'join_pattern' && pattern.type !== RUSHDB_RELATION_DEFAULT) {
        await this.entityService.deleteRelationsByKeys({
          source,
          target,
          type: RUSHDB_RELATION_DEFAULT,
          projectId: pattern.projectId,
          transaction
        })
      }
      await this.aiService.getOntology({
        projectId: pattern.projectId,
        force: true,
        transaction
      })
      await this.repository.updatePattern(pattern.id, {
        lastAppliedAt: new Date().toISOString(),
        sampleMatchCount: appliedCount,
        lastError: null
      })
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      await this.repository.updatePattern(pattern.id, { lastError: message })
      throw error
    }
  }

  private async deletePatternRelationships(
    pattern: RelationshipPatternRow,
    transaction: Transaction
  ): Promise<void> {
    if (pattern.mode === 'retype_existing_relationship') {
      await this.entityService.deleteRelationsByLabels({
        source: { label: pattern.sourceLabel },
        target: { label: pattern.targetLabel },
        type: pattern.type,
        projectId: pattern.projectId,
        transaction
      })
      return
    }

    await this.entityService.deleteRelationsByKeys({
      source: {
        label: pattern.sourceLabel,
        key: pattern.sourceKey,
        where: pattern.sourceWhere ? JSON.parse(pattern.sourceWhere) : undefined
      },
      target: {
        label: pattern.targetLabel,
        key: pattern.targetKey,
        where: pattern.targetWhere ? JSON.parse(pattern.targetWhere) : undefined
      },
      type: pattern.type,
      direction: pattern.direction as TRelationDirection,
      projectId: pattern.projectId,
      transaction
    })
  }

  private async applyPatternInFreshTransaction(pattern: RelationshipPatternRow): Promise<void> {
    const session = this.neogmaService.createSession('relationship-pattern-apply')
    const transaction = session.beginTransaction({ timeout: 60_000 })
    try {
      await this.applyPattern(pattern, transaction)
      await transaction.commit()
    } catch (error) {
      if (transaction.isOpen()) {
        await transaction.rollback()
      }
      throw error
    } finally {
      await this.neogmaService.closeSession(session, 'relationship-pattern-apply')
    }
  }

  private async deleteRelationshipsInFreshTransaction(pattern: RelationshipPatternRow): Promise<void> {
    const session = this.neogmaService.createSession('relationship-pattern-delete')
    const transaction = session.beginTransaction({ timeout: 60_000 })
    try {
      await this.deletePatternRelationships(pattern, transaction)
      await this.aiService.getOntology({
        projectId: pattern.projectId,
        force: true,
        transaction
      })
      await transaction.commit()
    } catch (error) {
      if (transaction.isOpen()) {
        await transaction.rollback()
      }
      throw error
    } finally {
      await this.neogmaService.closeSession(session, 'relationship-pattern-delete')
    }
  }
}
