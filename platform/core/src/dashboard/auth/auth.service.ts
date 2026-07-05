import { Injectable } from '@nestjs/common'
import { JwtService } from '@nestjs/jwt'
import { Transaction } from 'neo4j-driver'

import { ProjectRepository } from '@/dashboard/project/model/project.repository'
import { IUserClaims } from '@/dashboard/user/interfaces/user-claims.interface'
import { User } from '@/dashboard/user/user.entity'
import { UserService } from '@/dashboard/user/user.service'

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

  async hasProjectAccess(projectId: string, userId: string, _transaction?: Transaction): Promise<boolean> {
    return this.projectRepository.hasAccess(projectId, userId)
  }
}
