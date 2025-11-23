import path from 'path'
import dotenv from 'dotenv'

dotenv.config({ path: path.resolve(__dirname, '../.env') })

import RushDB from '../src/index.node'
import { isArray } from '../src/common/utils.js'

jest.setTimeout(60_000)

describe('records.importJson & records.createMany (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY
  const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'

  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => {
      expect(true).toBe(true)
    })
    return
  }

  const db = new RushDB(apiKey, { url: apiUrl })

  it('importJson simple create (no upsert options)', async () => {
    const tenantId = `import-json-no-upsert-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    const result = await db.records.importJson({
      label: 'IJSimple',
      data: [
        { key: 'A', value: 1, tenantId },
        { key: 'B', value: 2, tenantId }
      ],
      options: { suggestTypes: true, returnResult: true }
    })

    expect(isArray(result.data)).toBe(true)
    expect(result.data.length).toBe(2)
    const keys = result.data.map((r) => (r as any).data.key).sort()
    expect(keys).toEqual(['A', 'B'])

    await db.records.delete({ labels: ['IJSimple'], where: { tenantId } })
  })

  it('importJson upsert append (mergeStrategy append)', async () => {
    const tenantId = `import-json-append-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    // Initial import
    await db.records.importJson({
      label: 'IJAppend',
      data: [
        { code: 'C1', name: 'First', tenantId },
        { code: 'C2', name: 'Second', tenantId }
      ],
      options: {
        suggestTypes: true,
        returnResult: true,
        mergeBy: ['code', 'tenantId'],
        mergeStrategy: 'append'
      }
    })

    // Append new fields to existing records
    const updateResult = await db.records.importJson({
      label: 'IJAppend',
      data: [
        { code: 'C1', extra: 'X1', tenantId },
        { code: 'C2', extra: 'X2', tenantId }
      ],
      options: {
        suggestTypes: true,
        returnResult: true,
        mergeBy: ['code', 'tenantId'],
        mergeStrategy: 'append'
      }
    })

    expect(updateResult.data.every((r) => (r as any).data.extra && (r as any).data.name)).toBe(true)

    await db.records.delete({ labels: ['IJAppend'], where: { tenantId } })
  })

  it('importJson upsert rewrite removes unspecified fields', async () => {
    const tenantId = `import-json-rewrite-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    // Initial import
    await db.records.importJson({
      label: 'IJRewrite',
      data: [
        { code: 'R1', name: 'First', price: 10, tenantId },
        { code: 'R2', name: 'Second', price: 20, tenantId }
      ],
      options: {
        suggestTypes: true,
        returnResult: true,
        mergeBy: ['code', 'tenantId'],
        mergeStrategy: 'append'
      }
    })

    // Rewrite with only code + price
    const rewriteResult = await db.records.importJson({
      label: 'IJRewrite',
      data: [
        { code: 'R1', price: 15, tenantId },
        { code: 'R2', price: 25, tenantId }
      ],
      options: {
        suggestTypes: true,
        returnResult: true,
        mergeBy: ['code', 'tenantId'],
        mergeStrategy: 'rewrite'
      }
    })

    rewriteResult.data.forEach((r) => {
      const rec = (r as any).data
      expect(rec.price).toBe(rec.code === 'R1' ? 15 : 25)
      expect(rec.name).toBeUndefined()
    })

    await db.records.delete({ labels: ['IJRewrite'], where: { tenantId } })
  })

  it('createMany simple create (no upsert options)', async () => {
    const tenantId = `create-many-no-upsert-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    const result = await db.records.createMany({
      label: 'CMPlain',
      data: [
        { sku: 'S1', qty: 1, tenantId },
        { sku: 'S2', qty: 2, tenantId }
      ],
      options: { suggestTypes: true, returnResult: true }
    })

    expect(result.data.length).toBe(2)
    await db.records.delete({ labels: ['CMPlain'], where: { tenantId } })
  })

  it('createMany upsert append', async () => {
    const tenantId = `create-many-append-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    await db.records.createMany({
      label: 'CMAppend',
      data: [
        { sku: 'S1', qty: 1, tenantId },
        { sku: 'S2', qty: 2, tenantId }
      ],
      options: {
        suggestTypes: true,
        returnResult: true,
        mergeBy: ['sku', 'tenantId'],
        mergeStrategy: 'append'
      }
    })

    const update = await db.records.createMany({
      label: 'CMAppend',
      data: [
        { sku: 'S1', extra: 'E1', tenantId },
        { sku: 'S2', extra: 'E2', tenantId }
      ],
      options: {
        suggestTypes: true,
        returnResult: true,
        mergeBy: ['sku', 'tenantId'],
        mergeStrategy: 'append'
      }
    })

    update.data.forEach((r) => {
      const rec = (r as any).data
      expect(rec.extra).toBe(rec.sku === 'S1' ? 'E1' : 'E2')
      expect(rec.qty).toBe(rec.sku === 'S1' ? 1 : 2)
    })

    await db.records.delete({ labels: ['CMAppend'], where: { tenantId } })
  })

  it('createMany upsert rewrite', async () => {
    const tenantId = `create-many-rewrite-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    await db.records.createMany({
      label: 'CMRewrite',
      data: [
        { sku: 'S1', qty: 1, tenantId },
        { sku: 'S2', qty: 2, tenantId }
      ],
      options: {
        suggestTypes: true,
        returnResult: true,
        mergeBy: ['sku', 'tenantId'],
        mergeStrategy: 'append'
      }
    })

    const rewrite = await db.records.createMany({
      label: 'CMRewrite',
      data: [
        { sku: 'S1', qty: 10, tenantId },
        { sku: 'S2', qty: 20, tenantId }
      ],
      options: {
        suggestTypes: true,
        returnResult: true,
        mergeBy: ['sku', 'tenantId'],
        mergeStrategy: 'rewrite'
      }
    })

    rewrite.data.forEach((r) => {
      const rec = (r as any).data
      expect(rec.qty).toBe(rec.sku === 'S1' ? 10 : 20)
    })

    await db.records.delete({ labels: ['CMRewrite'], where: { tenantId } })
  })
})
