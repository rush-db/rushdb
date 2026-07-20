import { forwardRef, Inject, Injectable, Logger } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import axios from 'axios'
import { Transaction } from 'neo4j-driver'

import { AiService } from '@/core/ai/ai.service'
import { SchemaItem } from '@/core/ai/ai.types'
import { estimateTokens } from '@/core/ai/embedding.utils'
import { RUSHDB_RELATION_DEFAULT } from '@/core/common/constants'
import { EntityService } from '@/core/entity/entity.service'
import { TRelationDirection } from '@/core/entity/entity.types'
import { KuOperation } from '@/core/ku-events/ku-events.constants'
import { KuEventsService } from '@/core/ku-events/ku-events.service'
import { ProjectRepository } from '@/dashboard/project/model/project.repository'
import { NeogmaService } from '@/database/neogma/neogma.service'
import { DEFAULT_TRANSACTION_TIMEOUT_MS } from '@/database/transaction.constants'

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
// Reference-suffix tokens that never identify the entity a chain flows through.
const CHAIN_GENERIC_NAME_TOKENS = new Set(['id', 'ref', 'key', 'code'])
// Pairs to count per candidate when probing the live graph; one matched pair is
// already real evidence, the cap only bounds probe cost on large graphs.
const PROBE_MATCH_LIMIT = 100

@Injectable()
export class RelationshipPatternsService {
  private readonly logger = new Logger(RelationshipPatternsService.name)
  private readonly runningAnalysis = new Set<string>()
  private readonly runningApply = new Set<string>()
  /** Hash of (schema + stored patterns) per project at the last completed run, to skip redundant LLM calls. */
  private readonly lastAnalysisSignature = new Map<string, string>()

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
    // allowStale: the dashboard polls this endpoint every few seconds while an analysis
    // runs — a synchronous schema recompute here would pile concurrent full-graph scans
    // on top of the analysis's own forced recompute, which refreshes the cache anyway.
    const [patterns, schema, analysis] = await Promise.all([
      this.repository.findByProjectId(projectId),
      this.aiService.getSchema({ projectId, allowStale: true }),
      this.repository.getQueue(projectId)
    ])

    return {
      patterns: patterns.map((row) => this.toDto(row)),
      relationships: this.summarizeExistingRelationships(schema),
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
    schema: SchemaItem[]
  ): RelationshipPatternListResponse['relationships'] {
    const grouped = new Map<
      string,
      RelationshipPatternListResponse['relationships'][number]['relationships']
    >()
    const seen = new Set<string>()

    for (const item of schema) {
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
    const notBeforeMs =
      existingNotBefore && existingNotBefore < nextAllowedAt ? existingNotBefore : nextAllowedAt
    await this.repository.enqueueAnalysis(projectId, new Date(notBeforeMs).toISOString())

    // When not debounced, run right away instead of waiting for the once-a-minute cron
    // sweep — otherwise a first import sits up to 60s with "No patterns found".
    // The queue row stays as the cron's fallback; runningAnalysis guards double-starts.
    if (notBeforeMs <= now) {
      this.runAnalysisForProject(projectId).catch((error) => {
        this.logger.error(`[RelationshipAnalysis] project ${projectId} failed`, error)
      })
    }
  }

  async forceAnalysis(projectId: string, workspaceId?: string): Promise<void> {
    if (!this.analysisEnabled()) {
      await this.repository.updateQueue(projectId, {
        status: 'idle',
        lastError: 'Relationship analysis is disabled. Set RUSHDB_LLM_API_KEY and RUSHDB_LLM_MODEL.'
      })
      return
    }

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
        lastError: null,
        // Clear so the pattern reads as "approved, not yet applied" — the dashboard
        // renders an "Applying…" state and polls until the background apply sets it.
        lastAppliedAt: null
      },
      projectId
    )
    if (!pattern) {
      return undefined
    }

    // Connecting the matching records is a heavy graph write (batched MERGE over every
    // matching pair). Run it in the background so the request returns immediately; the
    // list() endpoint reports progress via lastAppliedAt / lastError.
    this.scheduleApprovedPatternApply(projectId)

    return this.toDto(pattern)
  }

  async approveMany(projectId: string, ids: string[]): Promise<RelationshipPatternDto[]> {
    const uniqueIds = [...new Set(ids)].filter(Boolean)
    if (!uniqueIds.length) {
      return []
    }

    const updated = await this.repository.approveManyByIds(uniqueIds, projectId)

    if (updated.length) {
      this.scheduleApprovedPatternApply(projectId)
    }

    return updated.map((row) => this.toDto(row))
  }

  /**
   * Applies approved-but-unapplied patterns for a project in the background, one per
   * fresh transaction, until none remain. Shares runningApply with the write-path
   * applyApprovedPatterns so only one apply touches a project's graph at a time —
   * avoiding concurrent MERGE contention. Re-queries after each pass so patterns
   * approved mid-run (e.g. a bulk approve) are still picked up. Fire-and-forget:
   * per-pattern errors are recorded on the row and surfaced through list().
   */
  private scheduleApprovedPatternApply(projectId: string): void {
    if (this.runningApply.has(projectId)) {
      return
    }
    this.runningApply.add(projectId)

    void (async () => {
      let totalApplied = 0
      try {
        for (;;) {
          const pending = (await this.repository.findApproved(projectId)).filter(
            (pattern) => !pattern.lastAppliedAt && !pattern.lastError
          )
          if (!pending.length) {
            break
          }
          for (const pattern of pending) {
            try {
              totalApplied += await this.applyPatternInFreshTransaction(pattern)
            } catch (error) {
              // applyPattern already recorded lastError on the row; log and continue
              // so one failing pattern doesn't stall the rest of the batch.
              this.logger.error(
                `[RelationshipPattern] background apply failed for pattern ${pattern.id}`,
                error
              )
            }
          }
        }

        // Refresh the schema cache once, after the whole batch, on its own
        // transaction — rather than per-pattern inside applyPattern.
        if (totalApplied > 0) {
          await this.recomputeSchemaInFreshTransaction(projectId)
        }
      } catch (error) {
        this.logger.error(
          `[RelationshipPattern] background apply loop failed for project ${projectId}`,
          error
        )
      } finally {
        this.runningApply.delete(projectId)
      }
    })()
  }

  /**
   * Forces a schema cache recompute. getSchema manages its own short-lived sessions,
   * so the heavy scan never shares a write transaction's timeout budget.
   */
  private async recomputeSchemaInFreshTransaction(projectId: string): Promise<void> {
    await this.aiService.getSchema({ projectId, force: true })
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

  /**
   * Applies approved patterns on the caller's transaction and returns the total number
   * of relationships materialized. The caller is responsible for recomputing the schema
   * cache once (on its own transaction) when the returned count is > 0 — this keeps the
   * heavy full-graph scan out of the write transaction's timeout budget.
   */
  async applyApprovedPatterns(projectId: string, transaction: Transaction): Promise<number> {
    if (this.runningApply.has(projectId)) {
      return 0
    }

    this.runningApply.add(projectId)
    try {
      const patterns = await this.repository.findApproved(projectId)
      let totalApplied = 0
      for (const pattern of patterns) {
        totalApplied += await this.applyPattern(pattern, transaction)
      }
      return totalApplied
    } finally {
      this.runningApply.delete(projectId)
    }
  }

  async processDueAnalysis(): Promise<void> {
    if (!this.analysisEnabled()) {
      return
    }

    const due = await this.repository.findDueAnalysis(new Date().toISOString())
    await Promise.allSettled(due.map((queueRow) => this.runAnalysisForProject(queueRow.projectId)))
  }

  async runAnalysisForProject(projectId: string, workspaceId?: string, isManual = false): Promise<void> {
    if (!this.analysisEnabled()) {
      await this.repository.updateQueue(projectId, {
        status: 'idle',
        lastError: 'Relationship analysis is disabled. Set RUSHDB_LLM_API_KEY and RUSHDB_LLM_MODEL.'
      })
      return
    }

    if (this.runningAnalysis.has(projectId)) {
      return
    }
    this.runningAnalysis.add(projectId)

    await this.repository.updateQueue(projectId, { status: 'running', lastError: null })

    try {
      // No force: every write endpoint that triggers this analysis also runs the
      // RECALCULATE_SCHEMA_CACHE side effect first, so the cache is already fresh for
      // the data that queued us — forcing here recomputed the same schema twice in a row.
      // A stale/missing cache (TTL expired, old project) still recomputes.
      const tSchema = Date.now()
      const schema = await this.aiService.getSchema({ projectId })
      this.logger.log(
        `[RelationshipAnalysis] project ${projectId} schema loaded in ${Date.now() - tSchema}ms`
      )

      // Resolve workspaceId for KU billing — supplied by the controller for manual
      // refreshes; looked up from the project record for scheduler-triggered runs.
      const resolvedWorkspaceId =
        workspaceId ?? (await this.projectRepository.findById(projectId))?.workspaceId

      const existingPatterns = await this.pruneOrphanedJoinSuggestions(
        projectId,
        await this.repository.findByProjectId(projectId),
        schema
      )

      // Neither the schema nor the stored patterns changed since the last completed
      // run in this process — the LLM would see the identical prompt, so skip it.
      // Makes repeated Refresh clicks near-instant instead of re-billing a full analysis.
      const analysisSignature = this.signatureHash({
        schema,
        patterns: this.compactExistingPatternsForRelationshipAnalysis(existingPatterns)
      })
      if (this.lastAnalysisSignature.get(projectId) === analysisSignature) {
        this.logger.log(
          `[RelationshipAnalysis] project ${projectId} unchanged since last run — skipping LLM call`
        )
        await this.repository.updateQueue(projectId, {
          status: 'idle',
          lastRunAt: new Date().toISOString(),
          lastError: null
        })
        return
      }

      // Names propose, the LLM judges, the graph verifies: deterministic name analysis
      // only produces *hints* the LLM evaluates alongside the schema — sampled values
      // never gate anything (tiny samples of high-cardinality data carry no evidence) —
      // and every surviving join is probed against the live graph before it is stored.
      const candidateHints = this.buildCandidateHints(schema)
      const tLlm = Date.now()
      const { candidates, promptTokens, completionTokens, totalTokens } = await this.suggestCandidates(
        schema,
        existingPatterns,
        candidateHints
      )
      this.logger.log(
        `[RelationshipAnalysis] project ${projectId} LLM returned ${candidates.length} candidates in ${Date.now() - tLlm}ms`
      )
      const validCandidates = this.dedupeInverseCandidates(
        candidates
          .map((candidate) => this.validateCandidate(candidate, schema))
          .filter((candidate): candidate is RelationshipPatternCandidate => Boolean(candidate))
      )
        .filter((candidate) => !this.hasExistingPatternForJoin(candidate, existingPatterns))
        .sort((a, b) => this.scoreCandidate(b) - this.scoreCandidate(a))
        .slice(0, MAX_LLM_CANDIDATES)

      const evidencedCandidates = await this.probeJoinCandidates(projectId, validCandidates)

      for (const candidate of evidencedCandidates) {
        const row = this.candidateToInsert(projectId, candidate)
        await this.repository.upsertCandidate(row)
      }

      if (resolvedWorkspaceId) {
        this.kuEventsService.emit(resolvedWorkspaceId, projectId, KuOperation.RELATIONSHIP_ANALYSIS, {
          model: this.configService.get<string>('RUSHDB_LLM_MODEL'),
          promptTokens,
          completionTokens,
          totalTokens,
          candidateCount: evidencedCandidates.length,
          trigger: isManual ? 'manual' : 'scheduler'
        })
      }

      this.lastAnalysisSignature.set(projectId, analysisSignature)
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
      throw error
    } finally {
      this.runningAnalysis.delete(projectId)
    }
  }

  /**
   * Deletes previously stored auto-suggestions whose endpoints no longer make sense
   * against the current schema (label or key gone, or the property type became
   * non-joinable). Only 'suggested' rows are touched; approved/ignored rows reflect
   * an explicit user decision and are kept.
   */
  private async pruneOrphanedJoinSuggestions(
    projectId: string,
    patterns: RelationshipPatternRow[],
    schema: SchemaItem[]
  ): Promise<RelationshipPatternRow[]> {
    const kept: RelationshipPatternRow[] = []
    for (const pattern of patterns) {
      const orphaned =
        pattern.status === 'suggested' &&
        this.normalizePatternMode(pattern.mode) === 'join_pattern' &&
        !this.isPlausibleJoinCandidate(
          {
            source: { label: pattern.sourceLabel, key: pattern.sourceKey ?? undefined },
            target: { label: pattern.targetLabel, key: pattern.targetKey ?? undefined }
          },
          schema
        )

      if (orphaned) {
        await this.repository.deletePattern(pattern.id, projectId)
        this.logger.log(
          `[RelationshipAnalysis] pruned orphaned join suggestion ${pattern.id}: ` +
            `${pattern.sourceLabel}.${pattern.sourceKey} -> ${pattern.targetLabel}.${pattern.targetKey} (${pattern.type})`
        )
      } else {
        kept.push(pattern)
      }
    }
    return kept
  }

  /**
   * Name-derived join hypotheses handed to the LLM for semantic vetting. Built from
   * the schema's structure only (labels, property names, types) — never from sampled
   * values, whose overlap carries no signal on high-cardinality real data. The LLM
   * decides which hints reference an entity (vs. attribute coincidences like paired
   * country columns), and the graph probe then supplies the actual evidence.
   */
  private buildCandidateHints(schema: SchemaItem[]): RelationshipPatternCandidate[] {
    return [
      ...this.suggestNameBackedReferenceCandidates(schema),
      ...this.suggestSameLabelChainCandidates(schema)
    ]
  }

  private suggestNameBackedReferenceCandidates(schema: SchemaItem[]): RelationshipPatternCandidate[] {
    const candidates: RelationshipPatternCandidate[] = []

    for (const source of schema) {
      for (const target of schema) {
        for (const sourceProperty of source.properties) {
          for (const targetProperty of target.properties) {
            if (source.label === target.label && sourceProperty.name === targetProperty.name) {
              continue
            }

            if (
              targetProperty.isArray ||
              !this.propertyNameReferencesLabelAndKey(sourceProperty.name, target.label, targetProperty.name)
            ) {
              continue
            }

            candidates.push({
              source: { label: source.label, key: sourceProperty.name },
              target: { label: target.label, key: targetProperty.name },
              direction: 'out',
              type: this.referencePropertyRelationshipType(sourceProperty.name),
              mode: 'join_pattern',
              confidence: sourceProperty.isArray ? 0.9 : 0.86,
              rationale: `${source.label}.${sourceProperty.name} names ${target.label}.${targetProperty.name} as a reference field.`
            })
          }
        }
      }
    }

    return candidates
  }

  // Same-label chain/flow detection: two reference-like properties on ONE label that
  // identify the same kind of entity (receiver_account / sender_account, to_station /
  // from_station) chain records into a sequence. Detected purely from property names —
  // shared entity token plus opposing direction words — since sampled values from
  // high-cardinality columns (thousands of accounts) practically never overlap.
  private suggestSameLabelChainCandidates(schema: SchemaItem[]): RelationshipPatternCandidate[] {
    const candidates: RelationshipPatternCandidate[] = []

    for (const item of schema) {
      for (let leftIndex = 0; leftIndex < item.properties.length; leftIndex += 1) {
        for (let rightIndex = leftIndex + 1; rightIndex < item.properties.length; rightIndex += 1) {
          const candidate = this.buildSameLabelChainCandidate(
            item,
            item.properties[leftIndex],
            item.properties[rightIndex]
          )
          if (candidate) {
            candidates.push(candidate)
          }
        }
      }
    }

    return candidates
  }

  private buildSameLabelChainCandidate(
    item: SchemaItem,
    leftProperty: SchemaItem['properties'][number],
    rightProperty: SchemaItem['properties'][number]
  ): RelationshipPatternCandidate | undefined {
    if (!this.isSameLabelChainPair(leftProperty, rightProperty)) {
      return undefined
    }

    // A chain hint is only emitted when the column names disambiguate flow direction;
    // ambiguous pairs are left to the LLM pass (isPlausibleJoinCandidate still admits
    // them there). Edge semantics: the record whose destination-side value is X points
    // at the record whose origin-side value is X — (hop N)-[:FLOW]->(hop N+1).
    const leftRole = this.chainDirectionRole(leftProperty.name)
    const rightRole = this.chainDirectionRole(rightProperty.name)
    if (!leftRole || !rightRole || leftRole === rightRole) {
      return undefined
    }

    const sourceProperty = leftRole === 'destination' ? leftProperty : rightProperty
    const targetProperty = leftRole === 'destination' ? rightProperty : leftProperty
    const sharedTokens = [...this.nameTokens(sourceProperty.name)]
      .filter((token) => this.nameTokens(targetProperty.name).has(token))
      .sort()

    return {
      source: { label: item.label, key: sourceProperty.name },
      target: { label: item.label, key: targetProperty.name },
      direction: 'out',
      type: this.chainRelationshipType(sharedTokens),
      mode: 'join_pattern',
      confidence: 0.82,
      rationale:
        `${item.label}.${sourceProperty.name} and ${item.label}.${targetProperty.name} appear to identify ` +
        `the same kind of entity — records chain into a flow where one record's ` +
        `${sourceProperty.name} is the next record's ${targetProperty.name}.`
    }
  }

  private isSameLabelChainPair(
    leftProperty: SchemaItem['properties'][number],
    rightProperty: SchemaItem['properties'][number]
  ): boolean {
    if (leftProperty.name === rightProperty.name) {
      return false
    }
    if (this.isNonReferenceProperty(leftProperty) || this.isNonReferenceProperty(rightProperty)) {
      return false
    }
    if (leftProperty.isArray || rightProperty.isArray) {
      return false
    }

    // The two columns must talk about the same entity: their names share a token
    // (receiver_ACCOUNT / sender_ACCOUNT) beyond the direction word — generic reference
    // suffixes (id/ref/key/code) don't count, or ownerRef/regionRef would pair up.
    const leftTokens = this.nameTokens(leftProperty.name)
    return [...this.nameTokens(rightProperty.name)].some(
      (token) => leftTokens.has(token) && !CHAIN_GENERIC_NAME_TOKENS.has(token)
    )
  }

  private chainDirectionRole(propertyName: string): 'destination' | 'origin' | undefined {
    // Tokens as emitted by nameTokens (lowercased, crudely singularized).
    const DESTINATION_TOKENS = new Set([
      'to',
      'receiver',
      'recipient',
      'dest',
      'destination',
      'next',
      'child',
      'successor',
      'output',
      'sink',
      'head'
    ])
    const ORIGIN_TOKENS = new Set([
      'from',
      'sender',
      'origin',
      'prev',
      'previou',
      'parent',
      'predecessor',
      'input',
      'tail'
    ])

    let role: 'destination' | 'origin' | undefined
    for (const token of this.nameTokens(propertyName)) {
      const tokenRole =
        DESTINATION_TOKENS.has(token) ? ('destination' as const)
        : ORIGIN_TOKENS.has(token) ? ('origin' as const)
        : undefined
      if (!tokenRole) {
        continue
      }
      if (role && role !== tokenRole) {
        return undefined
      }
      role = tokenRole
    }
    return role
  }

  private chainRelationshipType(sharedTokens: string[]): string {
    const entity = sharedTokens
      .filter((token) => !CHAIN_GENERIC_NAME_TOKENS.has(token))
      .map((token) => token.toUpperCase())
      .join('_')

    return entity ? `FLOWS_TO_${entity}` : 'FLOWS_TO'
  }

  private referencePropertyRelationshipType(propertyName: string): string {
    const token = propertyName
      .replace(/([a-z])([A-Z])/g, '$1_$2')
      .replace(/[^a-zA-Z0-9]+/g, '_')
      .replace(/^_+|_+$/g, '')
      .toUpperCase()

    const singular =
      token.endsWith('IES') ? `${token.slice(0, -3)}Y`
      : token.endsWith('S') ? token.slice(0, -1)
      : token

    return `HAS_${singular || 'RELATED_RECORD'}`
  }

  private async suggestCandidates(
    schema: SchemaItem[],
    existingPatterns: RelationshipPatternRow[],
    candidateHints: RelationshipPatternCandidate[] = []
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

    const systemContent =
      'You infer graph relationship patterns for RushDB. Return only JSON: {"candidates":[...]}. ' +
      'Every candidate must include mode: "join_pattern" or "retype_existing_relationship". ' +
      'Judge joinability from label semantics, property names, and property types — the schema is your substrate. ' +
      'Sampled property values, when present, are illustrative only: they are tiny samples of potentially huge datasets, so a lack of overlap between samples is NEVER evidence against a join. ' +
      'Every join_pattern you return is verified against the live graph before it is suggested to the user, so propose every join you judge semantically correct. ' +
      'Only propose joins between properties that reference an entity (identifiers, names of things, reference fields). Never join mere attribute columns (statuses, countries, currencies, categories) whose values may coincide without referencing an entity. ' +
      'candidateHints in the user message are join hypotheses derived mechanically from property names. Evaluate each hint: include it (refining type and direction if needed) when it references an entity; omit it when the shared naming is an attribute coincidence rather than an entity reference. ' +
      'Array/list fields and comma-separated reference fields may join scalar fields. ' +
      'Return at most ONE join_pattern per label pair; if several key pairs could join the same two labels, pick the single most semantically correct one. ' +
      'Use "retype_existing_relationship" when schema already shows a RUSHDB_DEFAULT_RELATION between labels; in that case source.key and target.key are optional and the task is to rename existing structure semantically. ' +
      'For retype_existing_relationship, infer the semantic relationship from the existing graph structure and label meanings. ' +
      'If a default relationship already connects two labels, prefer retype_existing_relationship over join_pattern. ' +
      'Return ONE canonical candidate per semantic relationship; never return both A->B and B->A for the same relationship. ' +
      'Do not suggest a relationship when the same mode and label/key pair already appears in existingPatterns, even if you would use a different synonym or inverse type. ' +
      'Choose source as the natural actor/owner/parent and target as the natural object/action/child. ' +
      'For same-label relationships, never join a property to itself. Choose exactly one canonical orientation from the schema context. ' +
      'Same-label chain/flow patterns are valid join_patterns: when two different reference-like properties on ONE label identify the same kind of entity (e.g. receiver_account of one record equals sender_account of the next; to_station / from_station), suggest source.label === target.label with source.key = the destination-side property and target.key = the origin-side property, chaining records into a sequence. ' +
      'Relationship type must read naturally from source to target. Do not choose a direction where the target would appear to act on or create the source. ' +
      'Each candidate must include source.label, target.label, mode, direction "out", type, confidence 0..1, and rationale. join_pattern must also include source.key and target.key. Return at most 12 candidates.'
    const userContent = JSON.stringify({
      schema,
      existingPatterns: this.compactExistingPatternsForRelationshipAnalysis(existingPatterns),
      candidateHints: candidateHints.map((hint) => ({
        source: hint.source,
        target: hint.target,
        direction: hint.direction,
        type: hint.type,
        rationale: hint.rationale
      })),
      relationshipTypeRules:
        'Use uppercase Neo4j-safe verb phrases from source to target. Do not use inverse duplicate types for the same relationship.'
    })

    const response = await axios.post(
      `${baseUrl.replace(/\/$/, '')}/chat/completions`,
      {
        model,
        temperature: 0,
        response_format: { type: 'json_object' },
        messages: [
          { role: 'system', content: systemContent },
          { role: 'user', content: userContent }
        ]
      },
      {
        headers: {
          Authorization: `Bearer ${apiKey}`,
          'Content-Type': 'application/json'
        },
        timeout: DEFAULT_TRANSACTION_TIMEOUT_MS
      }
    )

    const content = response.data?.choices?.[0]?.message?.content
    if (!content) {
      return empty
    }

    const usage = response.data?.usage
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
      const existing = byJoin.get(key)

      if (!existing || this.scoreCandidate(candidate) > this.scoreCandidate(existing)) {
        byJoin.set(key, candidate)
      }
    }

    return [...byJoin.values()]
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
    return this.normalizeConfidence(candidate.confidence) + (candidate.sampleMatchCount ?? 0) / 10_000
  }

  private analysisEnabled(): boolean {
    return (
      Boolean(this.configService.get<string>('RUSHDB_LLM_API_KEY')) &&
      Boolean(this.configService.get<string>('RUSHDB_LLM_MODEL'))
    )
  }

  private validateCandidate(
    candidate: RelationshipPatternCandidate,
    schema: SchemaItem[]
  ): RelationshipPatternCandidate | undefined {
    if (!candidate?.source?.label || !candidate?.target?.label) {
      return undefined
    }

    const source = schema.find((item) => item.label === candidate.source.label)
    const target = schema.find((item) => item.label === candidate.target.label)
    if (!source || !target) {
      return undefined
    }

    const mode = this.normalizePatternMode(candidate.mode)
    const isSameLabel = source.label === target.label
    if (isSameLabel && mode !== 'join_pattern') {
      return undefined
    }

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
      if (!sourceHasKey || !targetHasKey || !this.isPlausibleJoinCandidate(candidate, schema)) {
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

  private hasDefaultRelationshipBetween(source: SchemaItem, target: SchemaItem): boolean {
    return (
      source.relationships.some(
        (relationship) => relationship.label === target.label && this.isDefaultRelationType(relationship.type)
      ) ||
      target.relationships.some(
        (relationship) => relationship.label === source.label && this.isDefaultRelationType(relationship.type)
      )
    )
  }

  private hasSemanticRelationshipBetween(source: SchemaItem, target: SchemaItem): boolean {
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

  /**
   * Structural plausibility only — the semantic judgment belongs to the LLM and the
   * factual evidence to the graph probe. Rejects joins that can never be meaningful
   * regardless of data: a property joined to itself, missing properties, non-reference
   * value types, and list-to-list joins.
   */
  private isPlausibleJoinCandidate(
    candidate: RelationshipPatternCandidate,
    schema: SchemaItem[] = []
  ): boolean {
    if (candidate.source.label === candidate.target.label && candidate.source.key === candidate.target.key) {
      return false
    }

    const sourceProperty = this.findProperty(schema, candidate.source.label, candidate.source.key ?? '')
    const targetProperty = this.findProperty(schema, candidate.target.label, candidate.target.key ?? '')
    if (!sourceProperty || !targetProperty) {
      return false
    }

    if (this.isNonReferenceProperty(sourceProperty) || this.isNonReferenceProperty(targetProperty)) {
      return false
    }

    return !(sourceProperty.isArray && targetProperty.isArray)
  }

  /**
   * Verifies join candidates against the live graph: each join_pattern is kept only
   * when at least one real record pair matches its join, and sampleMatchCount is set
   * to the probed pair count (capped at PROBE_MATCH_LIMIT). This is the evidence
   * gate — schema samples never decide, the actual data does. Retype candidates pass
   * through untouched: their evidence is the existing relationship in the schema.
   */
  private async probeJoinCandidates(
    projectId: string,
    candidates: RelationshipPatternCandidate[]
  ): Promise<RelationshipPatternCandidate[]> {
    if (!candidates.some((candidate) => this.normalizePatternMode(candidate.mode) === 'join_pattern')) {
      return candidates
    }

    const evidenced: RelationshipPatternCandidate[] = []
    const session = this.neogmaService.createSession('relationship-pattern-probe')
    const transaction = session.beginTransaction({ timeout: 60_000 })
    try {
      for (const candidate of candidates) {
        if (this.normalizePatternMode(candidate.mode) !== 'join_pattern') {
          evidenced.push(candidate)
          continue
        }

        try {
          const matchCount = await this.entityService.countRelationCandidatesByKeys({
            source: candidate.source,
            target: candidate.target,
            projectId,
            transaction,
            limit: PROBE_MATCH_LIMIT
          })

          if (matchCount > 0) {
            evidenced.push({ ...candidate, sampleMatchCount: matchCount })
          } else {
            this.logger.log(
              `[RelationshipAnalysis] probe found no matching pairs for ` +
                `${candidate.source.label}.${candidate.source.key} -> ` +
                `${candidate.target.label}.${candidate.target.key} (${candidate.type}) — dropped`
            )
          }
        } catch (error) {
          // No evidence gathered — drop rather than suggest unverified structure.
          this.logger.error(
            `[RelationshipAnalysis] probe failed for ${candidate.source.label}.${candidate.source.key} -> ` +
              `${candidate.target.label}.${candidate.target.key}`,
            error
          )
        }
      }
      await transaction.commit()
    } finally {
      if (transaction.isOpen()) {
        await transaction.rollback()
      }
      await this.neogmaService.closeSession(session, 'relationship-pattern-probe')
    }

    return evidenced
  }

  private calibrateConfidence(
    candidate: RelationshipPatternCandidate,
    mode: RelationshipPatternMode
  ): number {
    const confidence = this.normalizeConfidence(candidate.confidence)
    if (mode !== 'join_pattern') {
      return confidence
    }

    return Math.min(confidence, 0.95)
  }

  private findProperty(schema: SchemaItem[], label: string, key: string) {
    return schema.find((item) => item.label === label)?.properties.find((property) => property.name === key)
  }

  private propertyNameReferencesLabelAndKey(
    referencePropertyName: string,
    targetLabel: string,
    targetPropertyName: string
  ): boolean {
    const referenceTokens = this.nameTokens(referencePropertyName)
    const labelTokens = this.nameTokens(targetLabel)
    const keyTokens = this.nameTokens(targetPropertyName)

    if (!referenceTokens.size || !labelTokens.size || !keyTokens.size) {
      return false
    }

    return (
      [...labelTokens].every((token) => referenceTokens.has(token)) &&
      [...keyTokens].every((token) => referenceTokens.has(token))
    )
  }

  private nameTokens(value: string): Set<string> {
    return new Set(
      String(value)
        .replace(/([a-z])([A-Z])/g, '$1_$2')
        .split(/[^a-zA-Z0-9]+/)
        .map((part) => part.trim().toLowerCase())
        .filter(Boolean)
        .map((part) =>
          part.endsWith('ies') ? `${part.slice(0, -3)}y`
          : part.endsWith('s') && part.length > 1 ? part.slice(0, -1)
          : part
        )
    )
  }

  private isNonReferenceProperty(property: SchemaItem['properties'][number]): boolean {
    return property.type === 'boolean' || property.type === 'datetime'
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

  private async applyPattern(pattern: RelationshipPatternRow, transaction: Transaction): Promise<number> {
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

      // Track KU for the relationships materialized by applying this pattern.
      // The graph write here is equivalent to creating relationships via import,
      // which is metered the same way.
      if (appliedCount > 0) {
        const workspaceId = (await this.projectRepository.findById(pattern.projectId))?.workspaceId
        if (workspaceId) {
          this.kuEventsService.emitBulk(
            workspaceId,
            pattern.projectId,
            KuOperation.RELATIONSHIP_CREATED,
            appliedCount,
            { type: pattern.type, mode: pattern.mode, trigger: 'pattern_apply' }
          )
        }
      }

      // NOTE: the schema cache is intentionally NOT recomputed here. Recomputing
      // per-pattern meant N full-graph scans in a single transaction, which blew the
      // side-effect transaction timeout. Callers refresh the cache once, after all
      // patterns are applied, on their own transaction (see applyApprovedPatterns and
      // scheduleApprovedPatternApply).
      await this.repository.updatePattern(pattern.id, {
        lastAppliedAt: new Date().toISOString(),
        sampleMatchCount: appliedCount,
        lastError: null
      })
      return appliedCount
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

  private async applyPatternInFreshTransaction(pattern: RelationshipPatternRow): Promise<number> {
    const session = this.neogmaService.createSession('relationship-pattern-apply')
    const transaction = session.beginTransaction({ timeout: 60_000 })
    try {
      const appliedCount = await this.applyPattern(pattern, transaction)
      await transaction.commit()
      return appliedCount
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
      await transaction.commit()
    } catch (error) {
      if (transaction.isOpen()) {
        await transaction.rollback()
      }
      throw error
    } finally {
      await this.neogmaService.closeSession(session, 'relationship-pattern-delete')
    }
    // After the delete is committed: the recompute reads on its own sessions, so it
    // must run post-commit to see the removed relationships.
    await this.aiService.getSchema({
      projectId: pattern.projectId,
      force: true
    })
  }
}
