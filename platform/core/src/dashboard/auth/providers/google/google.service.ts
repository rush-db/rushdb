import { Injectable, Logger, UnauthorizedException } from '@nestjs/common'
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

type TGoogleTokenData = {
  access_token?: string
}

type TGoogleTokenError = {
  error?: string
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

  getRedirectUri(): string {
    const dashboardUrl = this.configService.get<string>('RUSHDB_DASHBOARD_URL')?.trim()

    if (!dashboardUrl) {
      throw new UnauthorizedException('Google OAuth callback URL is not configured')
    }

    return new URL('/auth/google', dashboardUrl).toString()
  }

  async getGoogleUserInfo(accessToken: string): Promise<TGoogleAuthData> {
    if (!accessToken) {
      throw new UnauthorizedException('Google OAuth access token is missing')
    }

    const { data } = await axios.get<TGoogleAuthData>(GOOGLE_OAUTH_URL, {
      headers: {
        Authorization: `Bearer ${accessToken}`
      }
    })
    return data
  }

  async getAccessTokenFromCode(code: string): Promise<string> {
    const clientId = this.configService.get<string>('GOOGLE_CLIENT_ID')?.trim()
    const clientSecret = this.configService.get<string>('GOOGLE_SECRET')?.trim()

    if (!clientId || !clientSecret) {
      throw new UnauthorizedException('Google OAuth credentials are not configured')
    }

    const body = new URLSearchParams({
      client_id: clientId,
      client_secret: clientSecret,
      redirect_uri: this.getRedirectUri(),
      grant_type: 'authorization_code',
      code
    })

    try {
      const { data } = await axios.post<TGoogleTokenData>(GOOGLE_TOKEN_URL, body, {
        headers: {
          'Content-Type': 'application/x-www-form-urlencoded'
        }
      })

      if (!data.access_token) {
        throw new UnauthorizedException('Google OAuth token response did not include an access token')
      }

      return data.access_token
    } catch (error) {
      if (error instanceof UnauthorizedException) {
        throw error
      }

      const providerError =
        axios.isAxiosError<TGoogleTokenError>(error) && typeof error.response?.data?.error === 'string' ?
          error.response.data.error
        : undefined

      Logger.warn(
        `Google OAuth token exchange failed${providerError ? `: ${providerError}` : ''}`,
        GoogleOAuthService.name
      )
      throw new UnauthorizedException('Google OAuth token exchange failed')
    }
  }

  async googleLogin(code: string, transaction: Transaction): Promise<User | undefined> {
    const token = await this.getAccessTokenFromCode(code)
    const googleData = await this.getGoogleUserInfo(token)

    if (googleData?.email) {
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
