import { Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { JwtService } from '@nestjs/jwt'
import { Transaction } from 'neo4j-driver'

import { RUSHDB_KEY_ID, RUSHDB_KEY_PROJECT_ID } from '@/core/common/constants'
import { TVerifyOwnershipConfig } from '@/dashboard/auth/auth.types'
import {
  RUSHDB_LABEL_PROJECT,
  RUSHDB_LABEL_USER,
  RUSHDB_RELATION_HAS_ACCESS
} from '@/dashboard/common/constants'
import { IUserClaims } from '@/dashboard/user/interfaces/user-claims.interface'
import { User } from '@/dashboard/user/user.entity'
import { UserService } from '@/dashboard/user/user.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

import { EncryptionService } from './encryption/encryption.service'

@Injectable()
export class AuthService {
  constructor(
    private readonly userService: UserService,
    private readonly encryptionService: EncryptionService,
    private readonly jwtService: JwtService,
    private readonly configService: ConfigService,
    private readonly neogmaService: NeogmaService
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

  verifyJwt(payload: string): IUserClaims {
    return this.jwtService.verify(payload)
  }

  // async isOwnerMatch(
  //     idsToVerify: string[],
  //     userId: string,
  //     neogmaTransaction?: Transaction
  // ): Promise<boolean> {
  //     const queryRunner = this.neogmaService.createRunner();
  //
  //     const result = await queryRunner.run(
  //         `
  //             UNWIND $idsToVerify as entityId
  //             MATCH (n) WHERE n.id = entityId RETURN collect(n.ownerId) as ownershipList
  //         `,
  //         {
  //             idsToVerify,
  //         },
  //         neogmaTransaction
  //     );
  //
  //     const ownershipList = result.records[0].get('ownershipList');
  //
  //     return (
  //         ownershipList.length === idsToVerify.length &&
  //         ownershipList.every((owner) => owner === userId)
  //     );
  // }

  async hasProjectAccess(projectId: string, userId: string, transaction: Transaction): Promise<boolean> {
    const queryRunner = this.neogmaService.createRunner()

    const result = await queryRunner.run(
      `
                MATCH (p:${RUSHDB_LABEL_PROJECT} { id: $projectId })<-[rel:${RUSHDB_RELATION_HAS_ACCESS}]-(u:${RUSHDB_LABEL_USER} { id: $userId }) 
                RETURN COUNT(rel) as accessRelationCount
            `,
      {
        projectId,
        userId
      },
      transaction
    )

    const relations = result.records[0].get('accessRelationCount')

    return relations?.low > 0 || relations?.high > 0
  }

  async verifyRecordsIds(
    idsToVerify: string[],
    projectId: string,
    transaction: Transaction
  ): Promise<boolean> {
    const queryRunner = this.neogmaService.createRunner()

    const projectIdsList = await queryRunner
      .run(
        `
                    UNWIND $idsToVerify as entityId
                    MATCH (n) WHERE n.${RUSHDB_KEY_ID} = entityId RETURN collect(n.${RUSHDB_KEY_PROJECT_ID}) as projectIdsList
                `,
        {
          idsToVerify
        },
        transaction
      )
      .then((result) => result.records[0].get('projectIdsList'))

    const result =
      projectIdsList.length === idsToVerify.length && projectIdsList.every((pid) => pid === projectId)

    return result
  }

  async verifyOtherIds(
    idsToVerify: string[],
    projectId: string,
    config: TVerifyOwnershipConfig,
    transaction: Transaction
  ): Promise<boolean> {
    const queryRunner = this.neogmaService.createRunner()
    const { nodeProperty, projectIdProperty } = config ?? {
      nodeProperty: 'id',
      projectIdProperty: 'projectId'
    }
    const projectIdsList = await queryRunner
      .run(
        `
                    UNWIND $idsToVerify as entityId
                    MATCH (n) WHERE n.${nodeProperty} = entityId RETURN collect(n.${projectIdProperty}) as projectIdsList
                `,
        {
          idsToVerify
        },
        transaction
      )
      .then((result) => result.records[0].get('projectIdsList'))

    const result =
      projectIdsList.length === idsToVerify.length && projectIdsList.every((pid) => pid === projectId)

    return result
  }
}
