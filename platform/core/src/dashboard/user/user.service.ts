import { BadRequestException, forwardRef, Inject, Injectable, Logger } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Transaction } from 'neo4j-driver'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { toBoolean } from '@/common/utils/toBolean'
import { removeUndefinedKeys } from '@/core/property/property.utils'
import { IDecodedResetToken } from '@/dashboard/auth/auth.types'
import { ResetPasswordAuthDto } from '@/dashboard/auth/dto/reset-password-auth.dto'
import { EncryptionService } from '@/dashboard/auth/encryption/encryption.service'
import { StripeService } from '@/dashboard/billing/stripe/stripe.service'
import {
  RUSHDB_LABEL_PROJECT,
  RUSHDB_LABEL_USER,
  RUSHDB_LABEL_WORKSPACE,
  RUSHDB_RELATION_HAS_ACCESS,
  RUSHDB_RELATION_MEMBER_OF
} from '@/dashboard/common/constants'
import { ProjectService } from '@/dashboard/project/project.service'
import { ICreatedUserData } from '@/dashboard/user/interfaces/authenticated-user.interface'
import { sanitizeSettings, validateEmail } from '@/dashboard/user/user.utils'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

import { TUserInstance, TUserProperties } from './model/user.interface'
import { UserRepository } from './model/user.repository'
import { User } from './user.entity'
import { isDevMode } from '@/common/utils/isDevMode'
import { TWorkSpaceInviteToken } from '@/dashboard/workspace/workspace.types'
import * as crypto from 'node:crypto'

@Injectable()
export class UserService {
  constructor(
    private readonly configService: ConfigService,
    private readonly neogmaService: NeogmaService,
    private readonly encryptionService: EncryptionService,
    private readonly userRepository: UserRepository,
    @Inject(forwardRef(() => WorkspaceService))
    private readonly workspaceService: WorkspaceService,
    @Inject(forwardRef(() => StripeService))
    private readonly stripeService: StripeService,
    @Inject(forwardRef(() => ProjectService))
    private readonly projectService: ProjectService
  ) {}

  normalize(node?: TUserInstance) {
    return node ? new User(node) : undefined
  }

  async find(login: string, transaction: Transaction): Promise<User | undefined> {
    const node = await this.userRepository.model.findOne({
      where: { login },
      throwIfNotFound: false,
      session: transaction
    })

    return this.normalize(node)
  }

  async findById(id: string, transaction: Transaction): Promise<User | undefined> {
    const node = await this.userRepository.model.findOne({
      where: { id },
      throwIfNotFound: false,
      session: transaction
    })

    return this.normalize(node)
  }

  async findUserNodeById(id: string, transaction: Transaction) {
    const queryRunner = this.neogmaService.createRunner()

    const findUser = await this.neogmaService
      .createBuilder()
      .match({
        model: this.userRepository.model,
        where: { id },
        identifier: 'i'
      })
      .return('i')
      .run(queryRunner, transaction)

    const user = findUser.records[0]?.get('i')

    if (!user) {
      return
    }

    return this.userRepository.model.buildFromRecord(user)
  }

  async markEmailAsConfirmed(login: string, transaction: Transaction): Promise<User> {
    const queryRunner = this.neogmaService.createRunner()

    const result = await queryRunner.run(
      `
                MATCH (u:${RUSHDB_LABEL_USER} { login: $login })
                SET u.confirmed = true
                RETURN u`,
      {
        login
      },
      transaction
    )

    return result?.records?.map((v) => v.get('u'))[0]
  }

  async create(
    properties: Omit<TUserProperties, 'id' | 'isEmail'>,
    transaction: Transaction
  ): Promise<ICreatedUserData> {
    const allowedLogins = JSON.parse(this.configService.get('RUSHDB_ALLOWED_LOGINS') || '[]') ?? []

    if (allowedLogins.length === 0 || (allowedLogins.length && allowedLogins.includes(properties.login))) {
      const userNode = await this.createUserNode(properties, transaction)
      // Add Default Workspace on registration
      await this.workspaceService.createWorkspace({ name: 'Default Workspace' }, userNode.id, transaction)

      if (!toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))) {
        await this.stripeService.createCustomer(properties.login)
      }

      return {
        userData: this.normalize(userNode)
      }
    } else {
      throw new BadRequestException('Provided login is not allowed')
    }
  }

  async acceptWorkspaceInvitation(
    properties: Omit<TUserProperties, 'id' | 'isEmail'>,
    inviteToken: string,
    transaction: Transaction
  ): Promise<ICreatedUserData> {
    const allowedLogins = JSON.parse(this.configService.get('RUSHDB_ALLOWED_LOGINS') || '[]') ?? []

    if (allowedLogins.length === 0 || (allowedLogins.length && allowedLogins.includes(properties.login))) {
      const userNode = await this.createUserNode(properties, transaction)
      const { workspaceId, email, projectIds } = this.decryptInvite(inviteToken)

      if (!workspaceId || !email) {
        throw new BadRequestException('Malformed invite provided')
      }

      if (email !== properties.login) {
        throw new BadRequestException("Provided email doesn't match invitee's email")
      }

      // @TODO pass projectIds to the workspace
      await this.workspaceService.attachUserToWorkspace(workspaceId, userNode.id, transaction)

      if (!toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))) {
        await this.stripeService.createCustomer(properties.login)
      }

      return {
        userData: this.normalize(userNode)
      }
    } else {
      throw new BadRequestException('Provided login is not allowed')
    }
  }

  decryptInvite(encrypted: string): TWorkSpaceInviteToken {
    const encryptionKey = this.configService.get('RUSHDB_AES_256_ENCRYPTION_KEY')
    const iv = encrypted.substring(0, 32)
    const cipherText = encrypted.substring(32)

    const decipher = crypto.createDecipheriv('aes-256-cbc', encryptionKey, Buffer.from(iv, 'hex'))
    const decrypted = decipher.update(cipherText, 'base64', 'utf8')
    const resultString = decrypted + decipher.final('utf8')

    return JSON.parse(resultString) as TWorkSpaceInviteToken
  }

  async createUserNode(properties: Omit<TUserProperties, 'id' | 'isEmail'>, transaction: Transaction) {
    let userSettings = {}
    const currentTime = getCurrentISO()
    const userId = uuidv7()
    const userPassword = await this.encryptionService.hash(properties.password)

    //  process settings only if they're present
    if (properties.settings) {
      userSettings = sanitizeSettings(properties.settings)
    }

    const isEmail = toBoolean(validateEmail(properties.login || ''))

    if (!isEmail) {
      isDevMode(() => Logger.error('[Create user ERROR]: Bad email provided'))
      throw new BadRequestException('Bad email data provided')
    }

    return await this.userRepository.model.createOne(
      {
        ...properties,
        isEmail,
        password: userPassword,
        confirmed: properties.confirmed ?? false,
        created: currentTime,
        id: userId,
        settings: JSON.stringify(userSettings)
      },
      { session: transaction }
    )
  }

  async update(
    id: string,
    userProperties: Partial<TUserProperties>,
    transaction: Transaction
  ): Promise<User | any> {
    const userNode = await this.findUserNodeById(id, transaction)

    if (!userNode && !transaction) {
      throw new BadRequestException(`User ${id} not found`)
    }

    const fieldsToUpdate = removeUndefinedKeys(userProperties)

    const updateField = async (key: string, value: TUserProperties[keyof TUserProperties]) => {
      if (userNode[key] !== value) {
        // hack to change an object in TUserProperties interface
        if (key === 'settings') {
          userNode[key] = sanitizeSettings(value)
        } else {
          userNode[key] = value
        }
      }
    }

    await Promise.all(
      Object.entries<TUserProperties[keyof TUserProperties]>(fieldsToUpdate).map(
        async ([key, value]) => await updateField(key, value)
      )
    )
    userNode['edited'] = getCurrentISO()

    await userNode.save()

    return this.normalize(userNode)
  }

  async resetUserPassword(
    newPasswordData: ResetPasswordAuthDto,
    tokenPayload: IDecodedResetToken,
    transaction: Transaction
  ): Promise<User> {
    const userToValidate = await this.find(newPasswordData.login, transaction).then((user) => user?.toJson())

    if (userToValidate.login !== tokenPayload.login || userToValidate.id !== tokenPayload.id) {
      throw new BadRequestException('Confirmation data malformed')
    }

    const { password: newPassword } = newPasswordData
    const newPasswordEncrypted = await this.encryptionService.hash(newPassword)

    return this.update(
      userToValidate.id,
      {
        password: newPasswordEncrypted
      },
      transaction
    )
  }

  // async setUserAsInactive(id: string, neogmaTransaction?: Transaction): Promise<boolean> {
  //     const transaction = neogmaTransaction;
  //
  //     const currentTime = getCurrentISO();
  //
  //     try {
  //         await this.update(
  //             id,
  //             {
  //                 status: USER_STATUS_DELETED,
  //                 deletedDate: currentTime,
  //             },
  //             transaction
  //         );
  //
  //         return true;
  //     } catch (e) {
  //         return false;
  //     }
  // }

  async hasMinimalAccessLevel({
    userId,
    targetId,
    targetType = 'workspace',
    transaction
  }: {
    userId: string
    targetId: string
    targetType?: 'project' | 'workspace'
    transaction: Transaction
  }): Promise<boolean> {
    const queryRunner = this.neogmaService.createRunner()

    const targetPart =
      targetType === 'workspace' ?
        `-[rel:${RUSHDB_RELATION_MEMBER_OF}]->(:${RUSHDB_LABEL_WORKSPACE} { id: $targetId })`
      : `-[rel:${RUSHDB_RELATION_HAS_ACCESS}]->(:${RUSHDB_LABEL_PROJECT} { id: $targetId }) `

    const query = `
            MATCH (:${RUSHDB_LABEL_USER} { id: $userId })${targetPart}
            RETURN rel.role as accessRole
        `

    const result = await queryRunner.run(
      query,
      {
        targetId,
        userId
      },
      transaction
    )

    if (!result.records[0]) {
      return false
    }

    // if (!currentUserRole) {
    //     return false;
    // }
    //
    // const currentUserRoleWeight = USER_ROLE_WEIGHT[currentUserRole];
    // const minimalAccessLevel = USER_ROLE_WEIGHT[accessLevel];
    //
    // return currentUserRoleWeight >= minimalAccessLevel;

    return Boolean(result.records[0].get('accessRole') as string)
  }

  async delete({ userId, transaction }: { userId: string; transaction: Transaction }) {
    try {
      const userNode = await this.findUserNodeById(userId, transaction)
      if (userNode) {
        const relatedProjects = await userNode.findRelationships({
          alias: 'Projects',
          session: transaction
        })

        const relatedWorkspaces = await userNode.findRelationships({
          alias: 'Workspaces',
          session: transaction
        })

        await Promise.all([
          ...relatedProjects.map(
            async (rp) => await this.projectService.deleteProject(rp.target.dataValues.id, transaction)
          ),
          ...relatedWorkspaces.map(
            async (rws) => await this.workspaceService.deleteWorkspace(rws.target.dataValues.id, transaction)
          ),
          userNode.delete({ detach: true })
        ])
      }
    } catch (e) {
      await transaction.rollback()
    }

    return true
  }

  // async linkUser(id: string, projectId: string, transaction: Transaction): Promise<boolean> {
  //     const userNode = await this.findUserNodeById(id, transaction);
  //
  //     try {
  //         await userNode.relateTo({
  //             alias: 'Projects',
  //             where: { id: projectId },
  //             session: transaction,
  //         });
  //     } catch (e) {
  //         throw new BadRequestException('Cant link user');
  //     }
  //
  //     return true;
  // }
}
