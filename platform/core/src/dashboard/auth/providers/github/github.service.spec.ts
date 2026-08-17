import { ServiceUnavailableException, UnauthorizedException } from '@nestjs/common'
import axios from 'axios'

import { GithubOAuthService } from './github.service'

jest.mock('axios')

const mockedAxios = jest.mocked(axios)

describe('GithubOAuthService', () => {
  const config: Record<string, string> = {
    GH_CLIENT_ID: 'client-id',
    GH_SECRET: 'client-secret',
    RUSHDB_DASHBOARD_URL: 'http://localhost:3005/'
  }

  const userService = {
    find: jest.fn(),
    create: jest.fn(),
    update: jest.fn()
  }
  const encryptionService = {
    hash: jest.fn(),
    compare: jest.fn()
  }

  let service: GithubOAuthService

  beforeEach(() => {
    jest.clearAllMocks()

    service = new GithubOAuthService(
      userService as never,
      encryptionService as never,
      { get: (key: string) => config[key] } as never
    )
  })

  it('uses one normalized redirect URI for authorization and token exchange', () => {
    expect(service.getRedirectUri()).toBe('http://localhost:3005/auth/github')
  })

  it('exchanges the code with PKCE as form-encoded data', async () => {
    mockedAxios.post.mockResolvedValueOnce({ data: { access_token: 'access-token' } })

    await expect(service.getAccessTokenFromCode('authorization-code', 'code-verifier')).resolves.toBe(
      'access-token'
    )

    expect(mockedAxios.post).toHaveBeenCalledWith(
      'https://github.com/login/oauth/access_token',
      expect.any(URLSearchParams),
      {
        headers: {
          Accept: 'application/json',
          'Content-Type': 'application/x-www-form-urlencoded'
        }
      }
    )

    const body = mockedAxios.post.mock.calls[0][1] as URLSearchParams
    expect(body.get('client_id')).toBe('client-id')
    expect(body.get('client_secret')).toBe('client-secret')
    expect(body.get('redirect_uri')).toBe('http://localhost:3005/auth/github')
    expect(body.get('code')).toBe('authorization-code')
    expect(body.get('code_verifier')).toBe('code-verifier')
  })

  it('selects a verified primary email and sends GitHub REST API headers', async () => {
    mockedAxios.get
      .mockResolvedValueOnce({ data: { id: 42, login: 'octocat', name: 'The Octocat', email: null } })
      .mockResolvedValueOnce({
        data: [
          { email: 'unverified@example.com', primary: true, verified: false, visibility: null },
          { email: 'Verified@Example.com', primary: true, verified: true, visibility: 'private' }
        ]
      })

    await expect(service.getGithubUserInfo('access-token')).resolves.toMatchObject({
      id: 42,
      email: 'verified@example.com'
    })

    expect(mockedAxios.get).toHaveBeenNthCalledWith(
      1,
      'https://api.github.com/user',
      expect.objectContaining({
        headers: {
          Accept: 'application/vnd.github+json',
          Authorization: 'Bearer access-token',
          'User-Agent': 'RushDB',
          'X-GitHub-Api-Version': '2026-03-10'
        }
      })
    )
    expect(mockedAxios.get).toHaveBeenNthCalledWith(
      2,
      'https://api.github.com/user/emails',
      expect.any(Object)
    )
  })

  it('rejects an account without any verified email', async () => {
    mockedAxios.get
      .mockResolvedValueOnce({ data: { id: 42, login: 'octocat', name: null, email: null } })
      .mockResolvedValueOnce({
        data: [{ email: 'unverified@example.com', primary: true, verified: false, visibility: null }]
      })

    await expect(service.getGithubUserInfo('access-token')).rejects.toBeInstanceOf(UnauthorizedException)
  })

  it('does not call user-info when token exchange fails', async () => {
    mockedAxios.post.mockResolvedValueOnce({ data: { error: 'bad_verification_code' } })
    const userInfo = jest.spyOn(service, 'getGithubUserInfo')

    await expect(service.githubLogin('used-code', 'verifier', {} as never)).rejects.toBeInstanceOf(
      UnauthorizedException
    )
    expect(userInfo).not.toHaveBeenCalled()
  })

  it('rejects incomplete server configuration before contacting GitHub', async () => {
    const unconfigured = new GithubOAuthService(
      userService as never,
      encryptionService as never,
      { get: () => undefined } as never
    )

    await expect(unconfigured.getAccessTokenFromCode('code', 'verifier')).rejects.toBeInstanceOf(
      ServiceUnavailableException
    )
    expect(mockedAxios.post).not.toHaveBeenCalled()
  })

  it('will not replace a link to a different GitHub identity', async () => {
    const existingUser = {
      getGithubAuth: () => 'existing-hash',
      getId: () => 'user-id'
    }
    jest.spyOn(service, 'getAccessTokenFromCode').mockResolvedValueOnce('access-token')
    jest.spyOn(service, 'getGithubUserInfo').mockResolvedValueOnce({
      id: 42,
      login: 'octocat',
      name: 'The Octocat',
      email: 'verified@example.com'
    })
    userService.find.mockResolvedValueOnce(existingUser)
    encryptionService.compare.mockResolvedValueOnce(false)

    await expect(service.githubLogin('code', 'verifier', {} as never)).rejects.toBeInstanceOf(
      UnauthorizedException
    )
    expect(userService.update).not.toHaveBeenCalled()
  })
})
