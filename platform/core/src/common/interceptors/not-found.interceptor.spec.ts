import { of, throwError } from 'rxjs'
import { tap } from 'rxjs/operators'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'

describe('NotFoundInterceptor', () => {
  // Replicates the interceptor's own decision: poll routes bypass the 404 tap.
  const interceptor = new NotFoundInterceptor()

  function makeCtx(path: string) {
    return {
      switchToHttp: () => ({ getRequest: () => ({ route: { path }, url: path }) })
    } as never
  }

  function collect(observable: any): Promise<any> {
    return new Promise((resolve, reject) => {
      let value: any
      observable.subscribe({
        next: (v: any) => (value = v),
        error: (e: any) => reject(e),
        complete: () => resolve(value)
      })
    })
  }

  it('does not 404 when a poll route returns null', async () => {
    const ctx = makeCtx('/api/v1/connectors/_internal/claim')
    const next = { handle: () => of(null) }
    const result = await collect(interceptor.intercept(ctx as never, next as never))
    expect(result).toBeNull()
  })

  it('does not 404 when the command claim route returns null', async () => {
    const ctx = makeCtx('/api/v1/connectors/_internal/commands/claim')
    const next = { handle: () => of(null) }
    const result = await collect(interceptor.intercept(ctx as never, next as never))
    expect(result).toBeNull()
  })

  it('turns null into a 404 for ordinary routes', async () => {
    const ctx = makeCtx('/connectors/some-id')
    const next = { handle: () => of(null) }
    await expect(collect(interceptor.intercept(ctx as never, next as never))).rejects.toMatchObject({
      status: 404
    })
  })

  it('passes non-null values through untouched', async () => {
    const ctx = makeCtx('/connectors/some-id')
    const next = { handle: () => of({ ok: true }) }
    const result = await collect(interceptor.intercept(ctx as never, next as never))
    expect(result).toEqual({ ok: true })
  })
})
