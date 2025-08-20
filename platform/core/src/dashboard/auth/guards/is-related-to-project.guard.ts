import { CanActivate, ExecutionContext, Injectable, mixin } from '@nestjs/common'
import { Transaction } from 'neo4j-driver'

import { collectValuesByKeysFromObject } from '@/common/utils/collectValuesByKeysFromObject'
import { AuthService } from '@/dashboard/auth/auth.service'
import { TVerifyOwnershipConfig } from '@/dashboard/auth/auth.types'

export const IsRelatedToProjectGuard = (keysToCheck?: string[], config?: TVerifyOwnershipConfig) => {
  @Injectable()
  class IsRelatedToProjectMixin implements CanActivate {
    constructor(readonly authService: AuthService) {}

    async canActivate(context: ExecutionContext): Promise<boolean> {
      const request = context.switchToHttp().getRequest()

      const projectId = request.projectId

      const transaction: Transaction =
        request.raw.userDefinedTransaction ?? request.raw.externalTransaction ?? request.raw.transaction

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

        const [recordIdsVerificationResult, otherIdsVerificationResult] = await Promise.all([
          this.authService.verifyRecordsIds(recordIdsToVerify, projectId, transaction),
          this.authService.verifyOtherIds(otherIdsToVerify, projectId, config, transaction)
        ])
        return recordIdsVerificationResult && otherIdsVerificationResult
      } catch (e) {
        return false
      }
    }
  }

  return mixin(IsRelatedToProjectMixin)
}
