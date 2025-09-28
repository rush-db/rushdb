import { describe, it, expect } from 'vitest'
import * as yup from 'yup'
import { yupToStandard } from '../src'
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('')

const createMany = db.records.createMany.bind(db)

type User = { id: number; name: string }

describe('yup adapter', () => {
  it('validates at runtime, types via generic', async () => {
    const schema = yup.object<User>({
      id: yup.number().required(),
      name: yup.string().required()
    })

    const std = yupToStandard<User>(schema)

    const ok = await createMany(std, [{ id: 1, name: 'Ada' }])
    if (!ok.success) throw new Error('unexpected')
    const result: User[] = ok.value!
    expect(result[0].id).toBe(1)

    const bad = await createMany(std, [{ id: 'x', name: 'Ada' } as any])
    expect(bad.success).toBe(false)
    expect(bad.errors?.length).toBe(1)
  })
})
