import { Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { JwtService } from '@nestjs/jwt'
import axios from 'axios'
import { Transaction } from 'neo4j-driver'

import { randomString } from '@/common/utils/randomString'
import { EncryptionService } from '@/dashboard/auth/encryption/encryption.service'
import { User } from '@/dashboard/user/user.entity'
import { UserService } from '@/dashboard/user/user.service'

type TGoogleAuthData = {
  id: string
  email: string
  verified_email: boolean
  name: string
  given_name: string
  family_name: string
  picture: string
  locale: string
}

const GOOGLE_TOKEN_URL = 'https://oauth2.googleapis.com/token'
const GOOGLE_OAUTH_URL = 'https://www.googleapis.com/oauth2/v2/userinfo'

@Injectable()
export class GoogleOAuthService {
  constructor(
    private readonly userService: UserService,
    private readonly encryptionService: EncryptionService,
    private readonly jwtService: JwtService,
    private readonly configService: ConfigService
  ) {}

  async getGoogleUserInfo(accessToken: string): Promise<TGoogleAuthData> {
    const { data } = await axios.get<TGoogleAuthData>(GOOGLE_OAUTH_URL, {
      headers: {
        Authorization: `Bearer ${accessToken}`
      }
    })
    return data
  }

  async getAccessTokenFromCode(code: string): Promise<string> {
    // { access_token, expires_in, token_type, refresh_token }
    try {
      const { data } = await axios.post<{ access_token: string }>(GOOGLE_TOKEN_URL, {
        client_id: this.configService.get('GOOGLE_CLIENT_ID'),
        client_secret: this.configService.get('GOOGLE_SECRET'),
        redirect_uri: `${this.configService.get('RUSHDB_DASHBOARD_URL')}/auth/google`,
        grant_type: 'authorization_code',
        code
      })
      return data.access_token
    } catch (e) {
      return undefined
    }
  }

  async googleLogin(code: string, transaction: Transaction): Promise<User | undefined> {
    const token = await this.getAccessTokenFromCode(code)
    const googleData = await this.getGoogleUserInfo(token)

    if (token && googleData?.email) {
      let user = await this.userService.find(googleData.email, transaction)

      if (!user) {
        const hash = await this.encryptionService.hash(googleData?.id)
        const { userData } = await this.userService.create(
          {
            login: googleData.email,
            password: randomString(32),
            firstName: googleData.given_name,
            lastName: googleData.family_name,
            googleAuth: hash,
            confirmed: !!googleData?.verified_email
          },
          transaction
        )
        user = userData
        return user
      } else {
        if (
          user.getGoogleAuth() &&
          (await this.encryptionService.compare(googleData?.id, user.getGoogleAuth()))
        ) {
          return user
        } else {
          const hash = await this.encryptionService.hash(googleData?.id)
          return await this.userService.update(user.getId(), { googleAuth: hash }, transaction)
        }
      }
    }

    return undefined
  }
}
