import { describe, it, expect } from 'vitest'
import { z } from 'zod'
import { zodToStandard } from '../src/index'
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('')

const createMany = db.records.createMany.bind(db)

describe('zod adapter', () => {
  it('infers types and validates', async () => {
    const User = z.object({ id: z.number(), name: z.string() })
    const schema = zodToStandard(User)

    const ok = await createMany({ label: 'user', data: [{ id: 1, name: 'Ada' }], schema })
    expect(ok.success).toBe(true)
    expect(ok.value?.[0].name).toBe('Ada')

    const bad = await createMany({ label: 'user', data: [{ id: 'x', name: 'Ada' } as any], schema })
    expect(bad.success).toBe(false)
    expect(bad.errors?.[0].index).toBe(0)
  })
})
