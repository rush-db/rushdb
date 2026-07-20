import { FastifyRequest } from 'fastify'
import { Session, Transaction } from 'neo4j-driver'

import { IProjectProperties } from '@/dashboard/project/model/project.interface'
import { IUserClaims } from '@/dashboard/user/interfaces/user-claims.interface'
import { IWorkspaceProperties } from '@/dashboard/workspace/model/workspace.interface'

export type PlatformRequest = FastifyRequest & {
  projectId: string
  project: IProjectProperties
  workspaceId?: string
  workspace: IWorkspaceProperties
  user: IUserClaims
  session?: Session
  transaction?: Transaction
  externalSession?: Session
  externalTransaction?: Transaction
  userDefinedTransaction?: Transaction
  // Drained by RequestCleanupInterceptor after the request transaction commits (see
  // run-side-effect.interceptor.ts). Accessed via request.raw.* in the fastify context.
  postCommitHooks?: Array<() => Promise<void> | void>
  // NOTE: In fastify context, interceptors and filters should access properties via request.raw.*
}
