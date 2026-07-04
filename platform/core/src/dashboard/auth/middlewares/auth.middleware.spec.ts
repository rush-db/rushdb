import { AuthService } from '@/dashboard/auth/auth.service'
import { AuthMiddleware } from '@/dashboard/auth/middlewares/auth.middleware'
import { TokenService } from '@/dashboard/token/token.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

const API_TOKEN = 'a1b2c3d4e5f6'

const createMiddleware = ({ canWrite }: { canWrite: boolean }) => {
  const transaction = { isOpen: () => true, rollback: jest.fn(), close: jest.fn() }
  const session = { beginTransaction: jest.fn(() => transaction), close: jest.fn() }

  const authService = {} as unknown as AuthService

  const tokenService = {
    decrypt: jest.fn(() => 'token-id'),
    validateToken: jest.fn(async () => ({
      hasAccess: true,
      accessLevel: canWrite ? 'write' : 'read',
      canWrite,
      projectId: 'project-1',
      project: { id: 'project-1' },
      workspaceId: 'workspace-1',
      workspace: { id: 'workspace-1' }
    }))
  } as unknown as TokenService

  const neogmaService = {
    createSession: jest.fn(() => session),
    closeSession: jest.fn()
  } as unknown as NeogmaService

  return {
    middleware: new AuthMiddleware(authService, tokenService, neogmaService),
    neogmaService,
    session,
    transaction
  }
}

const createRequest = () => {
  const raw: any = {}
  return {
    method: 'POST',
    headers: { authorization: `Bearer ${API_TOKEN}` },
    raw
  } as any
}

describe('AuthMiddleware (SDK token flow)', () => {
  it('opens a WRITE session for a write token and attaches token flags', async () => {
    const { middleware, neogmaService, session, transaction } = createMiddleware({ canWrite: true })
    const request = createRequest()
    const next = jest.fn()

    await middleware.use(request, {} as any, next)

    expect(neogmaService.createSession).toHaveBeenCalledWith('auth-middleware-sdk', 'WRITE')
    expect(request.raw.session).toBe(session)
    expect(request.raw.transaction).toBe(transaction)
    expect(request.raw.tokenCanWrite).toBe(true)
    expect(request.raw.tokenAccessLevel).toBe('write')
    expect(next).toHaveBeenCalled()
  })

  it('opens a READ session for a read-only token', async () => {
    const { middleware, neogmaService } = createMiddleware({ canWrite: false })
    const request = createRequest()
    const next = jest.fn()

    await middleware.use(request, {} as any, next)

    expect(neogmaService.createSession).toHaveBeenCalledWith('auth-middleware-sdk', 'READ')
    expect(request.raw.tokenCanWrite).toBe(false)
    expect(request.raw.tokenAccessLevel).toBe('read')
    expect(next).toHaveBeenCalled()
  })

  it('does not open a session when the token is invalid', async () => {
    const { middleware, neogmaService } = createMiddleware({ canWrite: true })
    ;(middleware as any).tokenService.validateToken = jest.fn(async () => ({ hasAccess: false }))
    const request = createRequest()
    const next = jest.fn()

    await middleware.use(request, {} as any, next)

    expect(neogmaService.createSession).not.toHaveBeenCalled()
    expect(request.raw.session).toBeUndefined()
    expect(next).toHaveBeenCalled()
  })
})
