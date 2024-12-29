import { FastifyRequest } from 'fastify'

import { IUserClaims } from '@/dashboard/user/interfaces/user-claims.interface'

export type PlatformRequest = FastifyRequest & {
  projectId: string
  user: IUserClaims
}
