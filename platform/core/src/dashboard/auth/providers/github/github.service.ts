import { Injectable, Logger, ServiceUnavailableException, UnauthorizedException } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import axios from 'axios'
import { Transaction } from 'neo4j-driver'

import { EncryptionService } from '@/dashboard/auth/encryption/encryption.service'
import { User } from '@/dashboard/user/user.entity'
import { UserService } from '@/dashboard/user/user.service'

import { randomBytes } from 'node:crypto'

type TGithubUserData = {
  id: number
  email: string | null
  login: string
  name: string | null
}

type TGithubEmailData = {
  email: string
  primary: boolean
  verified: boolean
  visibility: 'private' | 'public' | null
}

type TGithubAuthData = TGithubUserData & {
  email: string
}

type TGithubTokenData = {
  access_token?: string
  error?: string
  error_description?: string
}

const GITHUB_TOKEN_URL = 'https://github.com/login/oauth/access_token'
const GITHUB_USER_URL = 'https://api.github.com/user'
const GITHUB_EMAILS_URL = 'https://api.github.com/user/emails'
const GITHUB_API_VERSION = '2026-03-10'

@Injectable()
export class GithubOAuthService {
  constructor(
    private readonly userService: UserService,
    private readonly encryptionService: EncryptionService,
    private readonly configService: ConfigService
  ) {}

  getClientId(): string {
    const clientId = this.configService.get<string>('GH_CLIENT_ID')?.trim()

    if (!clientId) {
      throw new ServiceUnavailableException('GitHub OAuth client ID is not configured')
    }

    return clientId
  }

  getRedirectUri(): string {
    const dashboardUrl = this.configService.get<string>('RUSHDB_DASHBOARD_URL')?.trim()

    if (!dashboardUrl) {
      throw new ServiceUnavailableException('GitHub OAuth callback URL is not configured')
    }

    return new URL('/auth/github', dashboardUrl).toString()
  }

  private getApiHeaders(accessToken: string) {
    if (!accessToken) {
      throw new UnauthorizedException('GitHub OAuth access token is missing')
    }

    return {
      Accept: 'application/vnd.github+json',
      Authorization: `Bearer ${accessToken}`,
      'X-GitHub-Api-Version': GITHUB_API_VERSION,
      'User-Agent': 'RushDB'
    }
  }

  async getGithubUserInfo(accessToken: string): Promise<TGithubAuthData> {
    const headers = this.getApiHeaders(accessToken)
    const [{ data: user }, { data: emails }] = await Promise.all([
      axios.get<TGithubUserData>(GITHUB_USER_URL, { headers }),
      axios.get<TGithubEmailData[]>(GITHUB_EMAILS_URL, { headers })
    ])

    // RushDB links accounts by email. Only a provider-verified address is safe
    // for that purpose; a merely primary or public address is not sufficient.
    const verifiedEmail =
      emails.find(({ primary, verified }) => primary && verified)?.email ??
      emails.find(({ verified }) => verified)?.email

    if (!verifiedEmail) {
      throw new UnauthorizedException('GitHub account has no verified email address')
    }

    return {
      ...user,
      email: verifiedEmail.trim().toLowerCase()
    }
  }

  async getAccessTokenFromCode(code: string, codeVerifier: string): Promise<string> {
    const clientSecret = this.configService.get<string>('GH_SECRET')?.trim()

    if (!clientSecret) {
      throw new ServiceUnavailableException('GitHub OAuth client secret is not configured')
    }

    if (!code || !codeVerifier) {
      throw new UnauthorizedException('GitHub OAuth callback is incomplete')
    }

    const body = new URLSearchParams({
      client_id: this.getClientId(),
      client_secret: clientSecret,
      code,
      redirect_uri: this.getRedirectUri(),
      code_verifier: codeVerifier
    })

    try {
      const { data } = await axios.post<TGithubTokenData>(GITHUB_TOKEN_URL, body, {
        headers: {
          Accept: 'application/json',
          'Content-Type': 'application/x-www-form-urlencoded'
        }
      })

      if (!data.access_token) {
        const providerError = data.error ? `: ${data.error}` : ''
        Logger.warn(`GitHub OAuth token exchange failed${providerError}`, GithubOAuthService.name)
        throw new UnauthorizedException('GitHub OAuth token exchange failed')
      }

      return data.access_token
    } catch (error) {
      if (error instanceof UnauthorizedException) {
        throw error
      }

      const providerError =
        axios.isAxiosError<TGithubTokenData>(error) && typeof error.response?.data?.error === 'string' ?
          error.response.data.error
        : undefined

      Logger.warn(
        `GitHub OAuth token exchange failed${providerError ? `: ${providerError}` : ''}`,
        GithubOAuthService.name
      )
      throw new UnauthorizedException('GitHub OAuth token exchange failed')
    }
  }

  async githubLogin(code: string, codeVerifier: string, transaction: Transaction): Promise<User> {
    const token = await this.getAccessTokenFromCode(code, codeVerifier)
    const githubData = await this.getGithubUserInfo(token)
    let user = await this.userService.find(githubData.email, transaction)

    if (!user) {
      const hash = await this.encryptionService.hash(String(githubData.id))
      const nameParts = githubData.name?.trim().split(/\s+/).filter(Boolean) ?? []
      const firstName = nameParts.shift() ?? githubData.login
      const lastName = nameParts.join(' ') || undefined

      const { userData } = await this.userService.create(
        {
          login: githubData.email,
          password: randomBytes(32).toString('base64url'),
          firstName,
          lastName,
          githubAuth: hash,
          confirmed: true
        },
        transaction
      )

      return userData
    }

    const linkedGithubAuth = user.getGithubAuth()

    if (linkedGithubAuth) {
      const matchesLinkedAccount = await this.encryptionService.compare(
        String(githubData.id),
        linkedGithubAuth
      )

      if (!matchesLinkedAccount) {
        throw new UnauthorizedException('A different GitHub account is already linked')
      }

      return user
    }

    const hash = await this.encryptionService.hash(String(githubData.id))
    user = await this.userService.update(user.getId(), { githubAuth: hash, confirmed: true }, transaction)

    return user
  }
}
