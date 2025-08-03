import { CanActivate, ExecutionContext, Injectable, Logger, mixin } from '@nestjs/common'
import { Session, Transaction } from 'neo4j-driver'

import { collectValuesByKeysFromObject } from '@/common/utils/collectValuesByKeysFromObject'
import { isDevMode } from '@/common/utils/isDevMode'
import { TransactionService } from '@/core/transactions/transaction.service'
import { AuthService } from '@/dashboard/auth/auth.service'
import { TVerifyOwnershipConfig } from '@/dashboard/auth/auth.types'
import { TokenService } from '@/dashboard/token/token.service'
import { DEFAULT_INSTANCE_CONNECTION_LITERAL } from '@/database/db-connection/db-connection.constants'
import { dbContextStorage } from '@/database/db-context'
import { NeogmaService } from '@/database/neogma/neogma.service'
import { CompositeNeogmaService } from '@/database/neogma-dynamic/composite-neogma.service'

import * as fs from 'node:fs'

export const IsRelatedToProjectGuard = (keysToCheck?: string[], config?: TVerifyOwnershipConfig) => {
  @Injectable()
  class IsRelatedToProjectMixin implements CanActivate {
    constructor(
      readonly tokenService: TokenService,
      readonly authService: AuthService,
      readonly neogmaService: NeogmaService,
      readonly transactionService: TransactionService,
      readonly compositeNeogmaService: CompositeNeogmaService
    ) {}

    async canActivate(context: ExecutionContext): Promise<boolean> {
      const request = context.switchToHttp().getRequest()
      const txId = request.headers['x-transaction-id']

      const dbContext = dbContextStorage.getStore()
      const hasCustomDbContext =
        dbContext?.projectId && dbContext?.projectId !== DEFAULT_INSTANCE_CONNECTION_LITERAL
      const projectId = request.projectId ?? request.headers['x-project-id']

      let transaction: Transaction
      let session: Session

      let customSession: Session
      let customTransaction: Transaction

      if (!txId && !request.transaction) {
        session = this.neogmaService.createSession('related-to-project-guard')
        transaction = session.beginTransaction()
      } else {
        const clientTransaction = this.transactionService.getTransaction(txId)
        transaction = clientTransaction?.transaction ?? request.transaction
        session = clientTransaction?.session ?? request.session
      }

      if (hasCustomDbContext) {
        isDevMode(() =>
          Logger.log(`[RTP GUARD]: Custom transaction created for RTP guard for project ${projectId}`)
        )
        customSession = this.compositeNeogmaService.createSession()
        customTransaction = customSession.beginTransaction()
      }

      try {
        // @FYI: Records IDS (as they get verified against n.${RUSHDB_KEY_PROJECT_ID} condition
        const recordIds = [request.params?.entityId].filter(Boolean)

        const recordIdsToVerify = []
        for (const candidate of recordIds) {
          recordIdsToVerify.push(candidate)
        }

        // @FYI: Other IDS (as they get verified against n.projectId condition
        const idsToVerify = keysToCheck ? collectValuesByKeysFromObject(request?.body ?? {}, keysToCheck) : []
        const otherIds = [
          ...idsToVerify.flat(),
          request.params?.propertyId,
          request.params?.tokenId,
          request.params?.id
        ].filter(Boolean)
        const otherIdsToVerify = []
        for (const candidate of otherIds) {
          otherIdsToVerify.push(candidate)
        }

        const targetTransaction = hasCustomDbContext ? customTransaction : transaction

        const [recordIdsVerificationResult, otherIdsVerificationResult] = await Promise.all([
          this.authService.verifyRecordsIds(recordIdsToVerify, projectId, targetTransaction),
          this.authService.verifyOtherIds(otherIdsToVerify, projectId, config, targetTransaction)
        ])

        return recordIdsVerificationResult && otherIdsVerificationResult
      } catch (e) {
        return false
      } finally {
        // Close only if tx was begun locally (in this particular guard)
        if (!txId) {
          isDevMode(() => Logger.log('[RTP GUARD]: Close tx and session'))

          await transaction.close()
          await this.neogmaService.closeSession(session, 'is-related-to-project-guard')
        }

        if (hasCustomDbContext && customTransaction?.isOpen()) {
          isDevMode(() => Logger.log('[RTP GUARD]: Close custom tx and session'))

          await customTransaction.close()
          await this.compositeNeogmaService.closeSession(session)
        }
      }
    }
  }

  return mixin(IsRelatedToProjectMixin)
}
