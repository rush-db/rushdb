import { UnauthorizedException } from '@nestjs/common'
import axios from 'axios'

import { GoogleOAuthService } from './google.service'

jest.mock('axios')

const mockedAxios = jest.mocked(axios)

describe('GoogleOAuthService', () => {
  const config: Record<string, string> = {
    GOOGLE_CLIENT_ID: 'client-id',
    GOOGLE_SECRET: 'client-secret',
    RUSHDB_DASHBOARD_URL: 'http://localhost:3005/'
  }

  let service: GoogleOAuthService

  beforeEach(() => {
    jest.clearAllMocks()

    service = new GoogleOAuthService(
      {} as never,
      {} as never,
      {} as never,
      { get: (key: string) => config[key] } as never
    )
  })

  it('uses one normalized redirect URI for the OAuth flow', () => {
    expect(service.getRedirectUri()).toBe('http://localhost:3005/auth/google')
  })

  it('exchanges the authorization code as form-encoded data', async () => {
    mockedAxios.post.mockResolvedValueOnce({ data: { access_token: 'access-token' } })

    await expect(service.getAccessTokenFromCode('authorization-code')).resolves.toBe('access-token')

    expect(mockedAxios.post).toHaveBeenCalledWith(
      'https://oauth2.googleapis.com/token',
      expect.any(URLSearchParams),
      { headers: { 'Content-Type': 'application/x-www-form-urlencoded' } }
    )

    const body = mockedAxios.post.mock.calls[0][1] as URLSearchParams
    expect(body.get('client_id')).toBe('client-id')
    expect(body.get('client_secret')).toBe('client-secret')
    expect(body.get('redirect_uri')).toBe('http://localhost:3005/auth/google')
    expect(body.get('grant_type')).toBe('authorization_code')
    expect(body.get('code')).toBe('authorization-code')
  })

  it('does not call user-info when the token exchange fails', async () => {
    mockedAxios.post.mockRejectedValueOnce({
      isAxiosError: true,
      response: { data: { error: 'invalid_grant' } }
    })
    const userInfo = jest.spyOn(service, 'getGoogleUserInfo')

    await expect(service.googleLogin('used-code', {} as never)).rejects.toBeInstanceOf(UnauthorizedException)
    expect(userInfo).not.toHaveBeenCalled()
  })

  it('rejects a missing access token before calling Google user-info', async () => {
    await expect(service.getGoogleUserInfo('')).rejects.toBeInstanceOf(UnauthorizedException)
    expect(mockedAxios.get).not.toHaveBeenCalled()
  })
})
