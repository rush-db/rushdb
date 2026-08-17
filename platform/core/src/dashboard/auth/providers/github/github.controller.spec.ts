import { UnauthorizedException } from '@nestjs/common'

import { GithubOAuthController } from './github.controller'

describe('GithubOAuthController', () => {
  const authService = { createToken: jest.fn(() => 'rushdb-token') }
  const githubOAuthService = {
    getClientId: jest.fn(() => 'client-id'),
    getRedirectUri: jest.fn(() => 'http://localhost:3005/auth/github'),
    githubLogin: jest.fn()
  }

  let controller: GithubOAuthController

  beforeEach(() => {
    jest.clearAllMocks()
    controller = new GithubOAuthController(authService as never, githubOAuthService as never)
  })

  it('binds state and a PKCE verifier to a short-lived HttpOnly cookie', async () => {
    const reply = { header: jest.fn() }
    const result = await controller.githubAuth(reply as never)
    const authorizationUrl = new URL(result.url)
    const setCookie = reply.header.mock.calls[0][1] as string

    expect(authorizationUrl.searchParams.get('state')).toHaveLength(43)
    expect(authorizationUrl.searchParams.get('code_challenge_method')).toBe('S256')
    expect(authorizationUrl.searchParams.get('code_challenge')).toBeTruthy()
    expect(setCookie).toContain('HttpOnly')
    expect(setCookie).toContain('SameSite=Lax')
    expect(setCookie).toContain('Max-Age=600')
  })

  it('validates state, uses the stored verifier once, and clears the cookie', async () => {
    const authorizationReply = { header: jest.fn() }
    const authorization = await controller.githubAuth(authorizationReply as never)
    const state = new URL(authorization.url).searchParams.get('state') as string
    const cookie = (authorizationReply.header.mock.calls[0][1] as string).split(';')[0]
    const user = { toJson: () => ({ id: 'user-id', login: 'verified@example.com' }) }
    githubOAuthService.githubLogin.mockResolvedValueOnce(user)

    const callbackReply = { header: jest.fn() }
    await expect(
      controller.githubAuthRedirect(
        {} as never,
        { code: 'authorization-code', state },
        { headers: { cookie } } as never,
        callbackReply as never
      )
    ).resolves.toEqual({
      id: 'user-id',
      login: 'verified@example.com',
      token: 'rushdb-token'
    })

    expect(githubOAuthService.githubLogin).toHaveBeenCalledWith(
      'authorization-code',
      expect.any(String),
      expect.any(Object)
    )
    expect(callbackReply.header).toHaveBeenCalledWith('Set-Cookie', expect.stringContaining('Max-Age=0'))
  })

  it('rejects a callback whose state is missing or mismatched', async () => {
    const reply = { header: jest.fn() }

    await expect(
      controller.githubAuthRedirect(
        {} as never,
        { code: 'authorization-code', state: 'attacker-state' },
        { headers: {} } as never,
        reply as never
      )
    ).rejects.toBeInstanceOf(UnauthorizedException)

    expect(githubOAuthService.githubLogin).not.toHaveBeenCalled()
    expect(reply.header).toHaveBeenCalledWith('Set-Cookie', expect.stringContaining('Max-Age=0'))
  })
})
