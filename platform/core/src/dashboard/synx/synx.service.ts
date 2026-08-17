import { ForbiddenException, HttpException, Injectable, Logger } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import {
  CONTRACT_VERSION,
  SynxAckStatusV1,
  SynxAcknowledgementV1,
  SynxEnvelopeV1
} from '@rushdb/synx-contract'
import { Transaction } from 'neo4j-driver'
import { uuidv7 } from 'uuidv7'

import { AiService } from '@/core/ai/ai.service'
import { RUSHDB_KEY_SYNC_DELETED_AT, RUSHDB_KEY_SYNC_ID } from '@/core/common/constants'
import { EntityService } from '@/core/entity/entity.service'
import { ImportService } from '@/core/entity/import-export/import.service'
import { RelationshipPatternsService } from '@/core/relationship-patterns/relationship-patterns.service'
import { ConnectorRepository } from '@/dashboard/connector/model/connector.repository'
import { ProjectService } from '@/dashboard/project/project.service'
import { NeogmaService } from '@/database/neogma/neogma.service'
import { DEFAULT_TRANSACTION_TIMEOUT_MS } from '@/database/transaction.constants'

import { createHash } from 'node:crypto'

import {
  SynxOffsetCheckpoint,
  decideSequence,
  parseOffsetPosition,
  stringifyOffsetCheckpoint,
  synxPartition
} from './synx.offsets'

import type { ConnectorRow } from '@/database/sql/schema/types'

const SYNX_MAX_OPERATIONS = 500

/** Opaque, stable hash of a source identity — never the raw id/payload. */
function hashSourceId(bindingId: string, streamId: string): string {
  return createHash('sha256').update(`${bindingId}:${streamId}`).digest('hex').slice(0, 32)
}

/** Escape a label for inline Cypher (backtick-doubling). */
function escapeLabel(label: string): string {
  return label.replace(/`/g, '``')
}

/** Escape a property key for inline Cypher (backtick-doubling). */
function escapeIdent(ident: string): string {
  return ident.replace(/`/g, '``')
}

interface SynxErrorBody {
  code: string
  message: string
  retryable: boolean
  details?: Record<string, unknown>
}

/**
 * Private Cloud/enterprise destination for managed synx connectors
 * (`POST /api/v1/_internal/synx/batches`).
 *
 * The envelope's `source.bindingId` maps to an existing `connectors` row
 * (control-plane provisioned binding) that carries the owning project. The
 * worker authenticates with `x-synx-control-token` (RUSHDB_SYNX_CONTROL_TOKEN).
 * Operations are applied to that project's graph via the import-merge path,
 * and per-stream ordering is enforced with a checkpoint in `connector_offsets`.
 */
@Injectable()
export class SynxService {
  private readonly logger = new Logger(SynxService.name)

  constructor(
    private readonly configService: ConfigService,
    private readonly connectorRepository: ConnectorRepository,
    private readonly importService: ImportService,
    private readonly entityService: EntityService,
    private readonly neogmaService: NeogmaService,
    private readonly relationshipPatternsService: RelationshipPatternsService,
    private readonly projectService: ProjectService,
    private readonly aiService: AiService
  ) {}

  assertWorkerToken(tokenHeader?: string) {
    const token = this.configService.get<string>('RUSHDB_SYNX_CONTROL_TOKEN')
    if (!token) {
      throw new ForbiddenException('Synx control token is not configured')
    }
    if (tokenHeader !== token) {
      throw new ForbiddenException('Invalid synx control token')
    }
  }

  private throwError(
    status: number,
    code: string,
    message: string,
    retryable: boolean,
    details?: Record<string, unknown>
  ): never {
    const body: SynxErrorBody = { code, message, retryable, details }
    throw new HttpException(body, status)
  }

  private assertIntactEnvelope(envelope: SynxEnvelopeV1) {
    if (
      !envelope ||
      typeof envelope.batch?.sequence !== 'number' ||
      !Array.isArray(envelope.operations) ||
      !envelope.source?.bindingId ||
      !envelope.stream?.id
    ) {
      this.throwError(400, 'MALFORMED_OPERATION', 'Invalid envelope shape', false)
    }
    if (envelope.version !== CONTRACT_VERSION) {
      this.throwError(
        422,
        'INVALID_CONTRACT_VERSION',
        `Unsupported contract version ${String(envelope.version)}`,
        false
      )
    }
    if (envelope.operations.length > SYNX_MAX_OPERATIONS) {
      this.throwError(
        413,
        'LIMIT_EXCEEDED',
        `Batch exceeds maxOperationsPerBatch = ${SYNX_MAX_OPERATIONS}`,
        false
      )
    }
  }

  async applyBatch(envelope: SynxEnvelopeV1, controlToken?: string): Promise<SynxAcknowledgementV1> {
    this.assertWorkerToken(controlToken)
    this.assertIntactEnvelope(envelope)

    const binding = await this.connectorRepository.findById(envelope.source.bindingId)
    if (!binding) {
      this.throwError(404, 'INVALID_BINDING', 'Unknown source binding', true, {
        connectionId: envelope.source.connectionId
      })
    }

    // Fencing: a batch stamped with a generation older than the connector's
    // current generation belongs to a superseded snapshot (e.g. after a
    // resnapshot). Reject it so a stale worker cannot write into the graph.
    // The worker re-claims the connector, learns the new generation, and
    // restarts from a fresh snapshot. A newer generation is accepted (the
    // worker simply got the latest claim).
    if (envelope.batch.generation !== undefined && binding.generation !== undefined) {
      if (envelope.batch.generation < binding.generation) {
        this.throwError(
          409,
          'STALE_GENERATION',
          `Batch generation ${envelope.batch.generation} is older than the connector's current ` +
            `generation ${binding.generation}; a resnapshot superseded this worker's lease. ` +
            'Re-claim the connector to resume from the new snapshot.',
          true,
          { currentGeneration: binding.generation }
        )
      }
    }

    const currentOffset = await this.readOffset(binding.id, envelope.stream.id)

    const decision = decideSequence(currentOffset, envelope.batch.sequence)
    if (decision === 'duplicate') {
      return {
        batchId: envelope.batch.id,
        status: 'duplicate' satisfies SynxAckStatusV1,
        accepted: 0,
        rejected: 0,
        duplicate: true,
        checkpointAccepted: true,
        schemaVersion: CONTRACT_VERSION,
        errors: []
      }
    }
    if (decision === 'gap') {
      this.throwError(
        409,
        'SEQUENCE_GAP',
        `Batch sequence ${envelope.batch.sequence} does not follow the committed sequence ${
          currentOffset?.sequence ?? -1
        }`,
        true,
        { nextExpectedSequence: currentOffset === null ? 0 : currentOffset.sequence + 1 }
      )
    }

    try {
      await this.applyToGraph(binding.projectId, envelope)
    } catch (error) {
      // Failure quarantine: persist a metadata-only rejection ledger entry
      // (stable code + hashed binding + redacted message — never the payload)
      // so the failure is observable and replayable without touching source
      // data. Fire-and-forget: ledger write failures must not mask the error.
      this.recordRejection(
        binding,
        envelope,
        'APPLY_FAILED',
        error instanceof Error ? error.message : 'failed to apply batch to graph'
      ).catch((ledgerError) => {
        this.logger.warn(
          `[SynxService] rejection ledger write failed for connector ${binding.id}`,
          ledgerError
        )
      })
      throw error
    }

    const checkpoint: SynxOffsetCheckpoint = {
      sequence: envelope.batch.sequence,
      updatedAt: new Date().toISOString()
    }
    await this.connectorRepository.upsertOffset({
      connectorId: binding.id,
      partition: synxPartition(binding.id, envelope.stream.id),
      position: stringifyOffsetCheckpoint(checkpoint),
      updatedAt: checkpoint.updatedAt
    })

    // Sync writes are graph writes too: re-apply approved relationship patterns
    // against the newly arrived records, and queue a fresh analysis so new
    // suggestions can surface for the new data (mirrors the import write path).
    // Fire-and-forget so a slow pattern apply or analysis never delays the ack;
    // both methods are internally guarded (per-project locks, debounce).
    this.runPostCommitSideEffects(binding.projectId).catch((error) => {
      this.logger.error(
        `[SynxService] post-commit side effects failed for project ${binding.projectId}`,
        error
      )
    })

    return {
      batchId: envelope.batch.id,
      status: 'committed' satisfies SynxAckStatusV1,
      accepted: envelope.operations.length,
      rejected: 0,
      duplicate: false,
      checkpointAccepted: true,
      schemaVersion: CONTRACT_VERSION,
      committedAt: checkpoint.updatedAt,
      errors: []
    }
  }

  /**
   * Runs the post-commit side effects for a synced project after a committed
   * batch, mirroring the import write path:
   *   1. recount project structure (records/labels counts shown in the UI),
   *   2. apply approved relationship patterns (MERGE, idempotent — new records
   *      get matched),
   *   3. recompute the schema cache so it reflects the new data,
   *   4. queue a fresh analysis for new relationship suggestions.
   * Each step is isolated so one failure never blocks the others, and the
   * whole thing is fire-and-forget so a slow recount never delays the ack.
   */
  private async runPostCommitSideEffects(projectId: string): Promise<void> {
    // ── graph-mutating steps on a fresh transaction ────────────────────────
    const session = this.neogmaService.getDriver().session()
    const transaction: Transaction = session.beginTransaction({
      timeout: DEFAULT_TRANSACTION_TIMEOUT_MS
    })
    try {
      try {
        await this.projectService.recomputeProjectNodes(projectId, transaction)
      } catch (error) {
        this.logger.error(`[SynxService] recount failed for project ${projectId}`, error)
      }
      try {
        await this.relationshipPatternsService.applyApprovedPatterns(projectId, transaction)
      } catch (error) {
        this.logger.error(`[SynxService] relationship apply failed for project ${projectId}`, error)
      }
      await transaction.commit()
    } catch (error) {
      await transaction?.rollback?.().catch(() => undefined)
      throw error
    } finally {
      await session.close()
    }

    // ── schema cache, then analysis (analysis reads the fresh schema) ───────
    try {
      await this.aiService.getSchema({ projectId, force: true })
    } catch (error) {
      this.logger.error(`[SynxService] schema recompute failed for project ${projectId}`, error)
    }
    try {
      await this.relationshipPatternsService.markAfterWrite(projectId)
    } catch (error) {
      this.logger.error(`[SynxService] analysis queue failed for project ${projectId}`, error)
    }
  }

  /**
   * Records a metadata-only entry in the connector rejection ledger. Never
   * persists source payloads: only the connector id, a redacted message, and
   * the stable error code. Repeated identical rejections collapse into one row
   * with an occurrence counter.
   */
  private async recordRejection(
    binding: ConnectorRow,
    envelope: SynxEnvelopeV1,
    code: string,
    message: string
  ): Promise<void> {
    const sourceIdHash = hashSourceId(envelope.source.bindingId, envelope.stream.id)
    await this.connectorRepository.upsertRejection({
      id: uuidv7(),
      connectorId: binding.id,
      projectId: binding.projectId,
      batchId: envelope.batch.id,
      sourceIdHash,
      code,
      message: message.slice(0, 500),
      retryable: 1
    })
  }

  private async readOffset(connectorId: string, streamId: string): Promise<SynxOffsetCheckpoint | null> {
    const partition = synxPartition(connectorId, streamId)
    const offsets = await this.connectorRepository.findOffsets(connectorId)
    const row = offsets.find((o) => o.partition === partition)
    return row ? parseOffsetPosition(row.position) : null
  }

  /**
   * Applies the envelope's operations to the project's graph inside a single
   * transaction. Upserts go through the merge-import path keyed by the
   * mapping identity fields; deletes match those fields to the source id.
   */
  private async applyToGraph(projectId: string, envelope: SynxEnvelopeV1): Promise<void> {
    const session = this.neogmaService.getDriver().session()
    const transaction: Transaction = session.beginTransaction({
      timeout: DEFAULT_TRANSACTION_TIMEOUT_MS
    })
    try {
      const upsertData = envelope.operations
        .filter((op) => op.type === 'upsert')
        .map((op) => {
          const opData = { ...(op as { data: Record<string, unknown> }).data }
          // Stamp the durable source identity onto the record if the worker's
          // transform did not already (defensive: deletes and merges key on
          // this field, so its presence is a correctness invariant).
          opData[RUSHDB_KEY_SYNC_ID] = (op as { sourceId: string }).sourceId
          return opData
        })
      if (upsertData.length > 0) {
        await this.importService.importRecords(
          {
            data: upsertData,
            label: envelope.mapping.targetLabel,
            options: {
              suggestTypes: true,
              returnResult: false,
              mergeStrategy: 'append',
              mergeBy: envelope.mapping.identityFields
            }
          },
          projectId,
          transaction,
          transaction
        )
      }

      if (envelope.mapping.deletionMode !== 'ignore') {
        const idField = envelope.mapping.identityFields[0] ?? RUSHDB_KEY_SYNC_ID
        const deletedAt = new Date().toISOString()
        for (const op of envelope.operations) {
          if (op.type !== 'delete') {
            continue
          }
          const sourceId = (op as { sourceId: string }).sourceId
          if (envelope.mapping.deletionMode === 'soft_delete') {
            // Flag the matching record(s) instead of removing them, so the
            // source deletion is observable in the graph and reversible.
            await transaction.run(
              `MATCH (n:\`${escapeLabel(envelope.mapping.targetLabel)}\`)
               WHERE n.\`${escapeIdent(idField)}\` = $sourceId
               SET n.\`${escapeIdent(RUSHDB_KEY_SYNC_DELETED_AT)}\` = $deletedAt`,
              { sourceId, deletedAt }
            )
          } else {
            await this.entityService.delete({
              projectId,
              transaction,
              searchQuery: {
                where: { [idField]: { $eq: sourceId } }
              }
            })
          }
        }
      }

      await transaction.commit()
    } catch (error) {
      await transaction?.rollback?.().catch(() => undefined)
      throw error
    } finally {
      await session.close()
    }
  }
}
