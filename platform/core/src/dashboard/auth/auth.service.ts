import { Injectable } from '@nestjs/common'
import { JwtService } from '@nestjs/jwt'
import { Transaction } from 'neo4j-driver'

import { RUSHDB_KEY_ID, RUSHDB_KEY_PROJECT_ID } from '@/core/common/constants'
import { TVerifyOwnershipConfig } from '@/dashboard/auth/auth.types'
import { ProjectRepository } from '@/dashboard/project/model/project.repository'
import { IUserClaims } from '@/dashboard/user/interfaces/user-claims.interface'
import { User } from '@/dashboard/user/user.entity'
import { UserService } from '@/dashboard/user/user.service'
import { QueryBuilder } from '@/database/QueryBuilder'

import { EncryptionService } from './encryption/encryption.service'

@Injectable()
export class AuthService {
  constructor(
    private readonly userService: UserService,
    private readonly encryptionService: EncryptionService,
    private readonly jwtService: JwtService,
    private readonly projectRepository: ProjectRepository
  ) {}

  createToken(user: User): string {
    return this.jwtService.sign(user.getClaims())
  }

  async validateUser(
    { login, password }: { login: string; password: string },
    transaction: Transaction
  ): Promise<User | undefined> {
    const user = await this.userService.find(login, transaction)

    if (user && (await this.encryptionService.compare(password, user.getPassword()))) {
      return user
    }

    return undefined
  }

  verifyJwt(jwt: string): IUserClaims {
    return this.jwtService.verify(jwt)
  }

  // async isOwnerMatch(
  //     idsToVerify: string[],
  //     userId: string,
  //     transaction?: Transaction
  // ): Promise<boolean> {
  //     const result = await transaction.run(
  //         `
  //             UNWIND $idsToVerify as entityId
  //             MATCH (n) WHERE n.id = entityId RETURN collect(n.ownerId) as ownershipList
  //         `,
  //         {
  //             idsToVerify,
  //         }
  //     );
  //
  //     const ownershipList = result.records[0].get('ownershipList');
  //
  //     return (
  //         ownershipList.length === idsToVerify.length &&
  //         ownershipList.every((owner) => owner === userId)
  //     );
  // }

  async hasProjectAccess(projectId: string, userId: string, _transaction?: Transaction): Promise<boolean> {
    return this.projectRepository.hasAccess(projectId, userId)
  }

  async verifyRecordsIds(
    idsToVerify: string[],
    projectId: string,
    transaction: Transaction
  ): Promise<boolean> {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`UNWIND $idsToVerify as entityId`)
      .append(
        `MATCH (n) WHERE n.${RUSHDB_KEY_ID} = entityId RETURN collect(n.${RUSHDB_KEY_PROJECT_ID}) as projectIdsList`
      )

    const projectIdsList = (await transaction
      .run(queryBuilder.getQuery(), {
        idsToVerify
      })
      .then((result) => result.records[0].get('projectIdsList'))) as string[]

    return projectIdsList.length === idsToVerify.length && projectIdsList.every((pid) => pid === projectId)
  }

  async verifyOtherIds(
    idsToVerify: string[],
    projectId: string,
    config: TVerifyOwnershipConfig,
    transaction: Transaction
  ): Promise<boolean> {
    const { nodeProperty, projectIdProperty } = config ?? {
      nodeProperty: 'id',
      projectIdProperty: 'projectId'
    }

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`UNWIND $idsToVerify as entityId`)
      .append(
        `MATCH (n) WHERE n.${nodeProperty} = entityId RETURN collect(n.${projectIdProperty}) as projectIdsList`
      )

    const projectIdsList = (await transaction
      .run(queryBuilder.getQuery(), {
        idsToVerify
      })
      .then((result) => result.records[0].get('projectIdsList'))) as string[]

    return projectIdsList.length === idsToVerify.length && projectIdsList.every((pid) => pid === projectId)
  }
}
