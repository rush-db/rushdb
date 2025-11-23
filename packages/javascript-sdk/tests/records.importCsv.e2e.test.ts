import path from 'path'
import dotenv from 'dotenv'

dotenv.config({ path: path.resolve(__dirname, '../.env') })

import RushDB from '../src/index.node'
import { DBRecordInstance } from '../src/sdk/record.js'

jest.setTimeout(60_000)

describe('records.importCsv (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY
  const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'

  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => {
      expect(true).toBe(true)
    })
    return
  }

  const db = new RushDB(apiKey, { url: apiUrl })

  function buildCsv(rows: Array<Record<string, any>>): string {
    const headers = Object.keys(rows[0])
    const lines = [headers.join(',')]
    for (const r of rows) {
      lines.push(headers.map((h) => r[h]).join(','))
    }
    return lines.join('\n')
  }

  it('importCsv simple create (no upsert)', async () => {
    const tenantId = `import-csv-no-upsert-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
    const csv = buildCsv([
      { code: 'CSV1', value: 100, tenantId },
      { code: 'CSV2', value: 200, tenantId }
    ])

    const result = await db.records.importCsv({
      label: 'CSVPlain',
      data: csv,
      options: { returnResult: true, suggestTypes: true }
    })
    expect(result.data.length).toBe(2)
    const codes = result.data.map((r) => (r as DBRecordInstance).data.code).sort()
    expect(codes).toEqual(['CSV1', 'CSV2'])

    await db.records.delete({ labels: ['CSVPlain'], where: { tenantId } })
  })

  it('importCsv upsert append adds new props preserving existing', async () => {
    const tenantId = `import-csv-append-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
    const initialCsv = buildCsv([
      { code: 'CSVAPP1', qty: 5, tenantId },
      { code: 'CSVAPP2', qty: 6, tenantId }
    ])
    await db.records.importCsv({
      label: 'CSVAppend',
      data: initialCsv,
      options: {
        returnResult: true,
        suggestTypes: true,
        mergeBy: ['code', 'tenantId'],
        mergeStrategy: 'append'
      }
    })

    const updateCsv = buildCsv([
      { code: 'CSVAPP1', note: 'N1', tenantId },
      { code: 'CSVAPP2', note: 'N2', tenantId }
    ])
    const updateResult = await db.records.importCsv({
      label: 'CSVAppend',
      data: updateCsv,
      options: {
        returnResult: true,
        suggestTypes: true,
        mergeBy: ['code', 'tenantId'],
        mergeStrategy: 'append'
      }
    })

    updateResult.data.forEach((r) => {
      const rec = (r as DBRecordInstance).data
      expect(rec.note).toBe(rec.code === 'CSVAPP1' ? 'N1' : 'N2')
      expect(rec.qty).toBe(rec.code === 'CSVAPP1' ? 5 : 6)
    })

    await db.records.delete({ labels: ['CSVAppend'], where: { tenantId } })
  })

  it('importCsv upsert rewrite removes unspecified props', async () => {
    const tenantId = `import-csv-rewrite-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
    const initialCsv = buildCsv([
      { code: 'CSVREW1', qty: 5, desc: 'A', tenantId },
      { code: 'CSVREW2', qty: 6, desc: 'B', tenantId }
    ])
    await db.records.importCsv({
      label: 'CSVRewrite',
      data: initialCsv,
      options: {
        returnResult: true,
        suggestTypes: true,
        mergeBy: ['code', 'tenantId'],
        mergeStrategy: 'append'
      }
    })

    const rewriteCsv = buildCsv([
      { code: 'CSVREW1', qty: 10, tenantId },
      { code: 'CSVREW2', qty: 12, tenantId }
    ])
    const rewriteResult = await db.records.importCsv({
      label: 'CSVRewrite',
      data: rewriteCsv,
      options: {
        returnResult: true,
        suggestTypes: true,
        mergeBy: ['code', 'tenantId'],
        mergeStrategy: 'rewrite'
      }
    })

    rewriteResult.data.forEach((r) => {
      const rec = (r as DBRecordInstance).data
      expect(rec.qty).toBe(rec.code === 'CSVREW1' ? 10 : 12)
      expect(rec.desc).toBeUndefined()
    })

    await db.records.delete({ labels: ['CSVRewrite'], where: { tenantId } })
  })
})
