import { Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { JwtService } from '@nestjs/jwt'
import axios from 'axios'
import { Transaction } from 'neo4j-driver'

import { randomString } from '@/common/utils/randomString'
import { EncryptionService } from '@/dashboard/auth/encryption/encryption.service'
import { User } from '@/dashboard/user/user.entity'
import { UserService } from '@/dashboard/user/user.service'

type TGithubAuthData = {
  id: number
  email: string
  name: string
}

const GITHUB_TOKEN_URL = 'https://github.com/login/oauth/access_token'
const GITHUB_OAUTH_URL = 'https://api.github.com/user'

@Injectable()
export class GithubOAuthService {
  constructor(
    private readonly userService: UserService,
    private readonly encryptionService: EncryptionService,
    private readonly jwtService: JwtService,
    private readonly configService: ConfigService
  ) {}

  async getGithubUserInfo(accessToken: string): Promise<TGithubAuthData> {
    const { data: userResponse } = await axios.get<TGithubAuthData>(GITHUB_OAUTH_URL, {
      headers: {
        Authorization: `Bearer ${accessToken}`
      }
    })

    const data = userResponse

    // if user email is set to private
    if (!userResponse.email) {
      const { data: emailResponse } = await axios.get<
        {
          email: string
          primary: boolean
          verified: boolean
          visibility: 'private' | null
        }[]
      >('https://api.github.com/user/emails', {
        headers: {
          Authorization: `Bearer ${accessToken}`
        }
      })

      data.email = emailResponse.find((email) => email.primary)?.email
    }

    return data
  }

  async getAccessTokenFromCode(code: string): Promise<string> {
    try {
      const { data } = await axios.post<{ access_token: string }>(
        GITHUB_TOKEN_URL,
        {
          client_id: this.configService.get('GH_CLIENT_ID'),
          client_secret: this.configService.get('GH_SECRET'),
          code
        },
        {
          headers: {
            Accept: 'application/json'
          }
        }
      )

      return data.access_token
    } catch (e) {
      return undefined
    }
  }

  async githubLogin(code: string, transaction: Transaction): Promise<User | undefined> {
    const token = await this.getAccessTokenFromCode(code)

    const githubData = await this.getGithubUserInfo(token)

    if (token && githubData?.email) {
      let user = await this.userService.find(githubData.email, transaction)

      if (!user) {
        const hash = await this.encryptionService.hash(String(githubData?.id))

        const [firstName, lastName] = githubData.name?.split(' ') ?? []

        const { userData } = await this.userService.create(
          {
            login: githubData.email,
            password: randomString(32),
            firstName,
            lastName,
            githubAuth: hash,
            confirmed: !!githubData?.email
          },
          transaction
        )
        user = userData

        return user
      } else {
        if (
          user.getGithubAuth() &&
          (await this.encryptionService.compare(String(githubData?.id), user.getGithubAuth()))
        ) {
          return user
        } else {
          const hash = await this.encryptionService.hash(String(githubData?.id))
          return await this.userService.update(user.getId(), { githubAuth: hash }, transaction)
        }
      }
    }

    return undefined
  }
}
