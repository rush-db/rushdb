import { Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Console, Command } from 'nestjs-console'

import { toBoolean } from '@/common/utils/toBolean'
import { EncryptionService } from '@/dashboard/auth/encryption/encryption.service'
import { UserService } from '@/dashboard/user/user.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

@Injectable()
@Console()
export class CliService {
  constructor(
    private readonly configService: ConfigService,
    private readonly userService: UserService,
    private readonly encryptionService: EncryptionService,
    private readonly neogmaService: NeogmaService
  ) {}

  @Command({
    command: 'create-user <login> <password>',
    description: 'Create a new user with the given login and password'
  })
  async createUser(login: string, password: string): Promise<void> {
    const isSelfHosted = toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))
    const session = this.neogmaService.createSession()
    const transaction = session.beginTransaction()

    try {
      if (isSelfHosted) {
        await this.userService.create({ login, password, confirmed: true }, transaction)
        console.log(`User created successfully: ${login}`)
      } else {
        console.error('CLI Error: Creating user within CLI in managed setup is not allowed')
        await transaction.rollback()
      }
    } catch (error) {
      await transaction.rollback()
      console.error('Error creating user:', error.message)
    } finally {
      if (transaction.isOpen()) {
        await transaction.commit()
        await transaction.close()
      }
      await session.close()
    }
  }

  @Command({
    command: 'update-password <login> <newPassword>',
    description: 'Update the password for an existing user'
  })
  async updatePassword(login: string, newPassword: string): Promise<void> {
    const isSelfHosted = toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))
    const session = this.neogmaService.createSession()
    const transaction = session.beginTransaction()

    try {
      if (isSelfHosted) {
        const user = await this.userService.find(login, transaction)
        if (!user) {
          console.error(`User not found: ${login}`)
          await transaction.rollback()
          return
        }

        const newPasswordEncrypted = await this.encryptionService.hash(newPassword)
        await this.userService.update(user.getId(), { password: newPasswordEncrypted }, transaction)
        console.log(`Password updated successfully for user: ${login}`)
      } else {
        console.error('CLI Error: Updating password within CLI in managed setup is not allowed')
        await transaction.rollback()
      }
    } catch (error) {
      await transaction.rollback()
      console.error('Error updating password:', error.message)
    } finally {
      if (transaction.isOpen()) {
        await transaction.commit()
        await transaction.close()
      }
      await session.close()
    }
  }
}
