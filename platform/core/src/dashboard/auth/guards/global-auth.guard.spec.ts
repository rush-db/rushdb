import { ExecutionContext, ForbiddenException } from '@nestjs/common'
import { Reflector } from '@nestjs/core'

import { TOKEN_READ_SAFE_KEY } from '@/dashboard/auth/decorators/token-read-access.decorator'
import { UserService } from '@/dashboard/user/user.service'

import { GlobalAuthGuard } from './global-auth.guard'

// Opaque API tokens have no dots; JWTs have three segments
const API_TOKEN = 'a1b2c3d4e5f6'
const JWT_TOKEN = 'header.payload.signature'

const createContext = ({
  bearerToken,
  raw = {},
  metadata = {}
}: {
  bearerToken?: string
  raw?: Record<string, unknown>
  metadata?: Record<string, unknown>
}) => {
  const request: any = {
    headers: bearerToken ? { authorization: `Bearer ${bearerToken}` } : {},
    raw
  }

  const reflector = {
    get: jest.fn((key: string) => metadata[key]),
    getAllAndOverride: jest.fn((key: string) => metadata[key])
  } as unknown as Reflector

  const context = {
    switchToHttp: () => ({ getRequest: () => request }),
    getHandler: () => ({}),
    getClass: () => ({})
  } as unknown as ExecutionContext

  return { context, reflector, request }
}

describe('GlobalAuthGuard (SDK token shortcut)', () => {
  const userService = {
    hasMinimalAccessLevel: jest.fn()
  } as unknown as UserService

  const sdkRaw = (tokenCanWrite: boolean) => ({
    projectId: 'project-1',
    workspaceId: 'workspace-1',
    tokenCanWrite
  })

  it('allows a write token on a route without read access metadata', async () => {
    const { context, reflector, request } = createContext({
      bearerToken: API_TOKEN,
      raw: sdkRaw(true)
    })
    const guard = new GlobalAuthGuard(reflector, userService)

    await expect(guard.canActivate(context)).resolves.toBe(true)
    expect(request.projectId).toBe('project-1')
    expect(request.workspaceId).toBe('workspace-1')
  })

  it('rejects a read-only token on a route without read access metadata', async () => {
    const { context, reflector } = createContext({
      bearerToken: API_TOKEN,
      raw: sdkRaw(false)
    })
    const guard = new GlobalAuthGuard(reflector, userService)

    await expect(guard.canActivate(context)).rejects.toThrow(ForbiddenException)
  })

  it('allows a read-only token on a route marked with @TokenReadAccess()', async () => {
    const { context, reflector, request } = createContext({
      bearerToken: API_TOKEN,
      raw: sdkRaw(false),
      metadata: { [TOKEN_READ_SAFE_KEY]: true }
    })
    const guard = new GlobalAuthGuard(reflector, userService)

    await expect(guard.canActivate(context)).resolves.toBe(true)
    expect(request.projectId).toBe('project-1')
  })

  it('does not apply token-level checks to JWT (dashboard) requests', async () => {
    const { context, reflector } = createContext({
      bearerToken: JWT_TOKEN,
      raw: { user: { id: 'user-1' }, workspaceId: 'workspace-1' },
      metadata: { dashboardTargetType: null }
    })
    const guard = new GlobalAuthGuard(reflector, userService)

    // dashboardTargetType === null with a user short-circuits to true
    await expect(guard.canActivate(context)).resolves.toBe(true)
  })
})
