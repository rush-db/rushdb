import { toNative } from '@/database/interceptors/data.interceptor'

describe('DataInterceptor provenance filtering', () => {
  it('strips namespaced sync provenance keys from record maps', () => {
    const record = {
      name: 'ada',
      __RUSHDB__KEY__SYNC__ID__: 'pg.USERS:1',
      __RUSHDB__KEY__SYNCED__AT__: '2026-01-01T00:00:00.000Z'
    }
    const native = toNative(record)
    expect(native.name).toBe('ada')
    expect(native.__RUSHDB__KEY__SYNC__ID__).toBeUndefined()
    expect(native.__RUSHDB__KEY__SYNCED__AT__).toBeUndefined()
  })

  it('keeps ordinary fields untouched', () => {
    const record = { name: 'ada', email: 'ada@example.com', age: 36 }
    const native = toNative(record)
    expect(native).toEqual({ name: 'ada', email: 'ada@example.com', age: 36 })
  })

  it('never hides user-defined syncKey/syncedAt business fields', () => {
    // A user may legitimately have their own fields with these names; they must
    // survive — only the __RUSHDB__-namespaced provenance keys are hidden.
    const record = { syncKey: 'acme-123', syncedAt: '2026-06-01', total: 42 }
    const native = toNative(record)
    expect(native).toEqual({ syncKey: 'acme-123', syncedAt: '2026-06-01', total: 42 })
  })

  it('recursively strips provenance inside nested records and arrays', () => {
    const payload = {
      items: [
        { id: 1, __RUSHDB__KEY__SYNC__ID__: 'x:1', __RUSHDB__KEY__SYNCED__AT__: '2026-01-01T00:00:00.000Z' },
        { id: 2 }
      ],
      meta: { __RUSHDB__KEY__SYNC__ID__: 'y:2', ok: true }
    }
    const native = toNative(payload)
    expect(native.items[0].__RUSHDB__KEY__SYNC__ID__).toBeUndefined()
    expect(native.items[0].__RUSHDB__KEY__SYNCED__AT__).toBeUndefined()
    expect(native.items[1]).toEqual({ id: 2 })
    expect(native.meta.__RUSHDB__KEY__SYNC__ID__).toBeUndefined()
    expect(native.meta.ok).toBe(true)
  })
})
