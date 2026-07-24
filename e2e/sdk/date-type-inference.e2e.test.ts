import RushDB from '../../packages/javascript-sdk/src/index.node'
import { DBRecordInstance } from '../../packages/javascript-sdk/src/sdk/record.js'

jest.setTimeout(60_000)

describe('Date-only YYYY-MM-DD type inference (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY
  const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'

  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => {
      expect(true).toBe(true)
    })
    return
  }

  const db = new RushDB(apiKey, { url: apiUrl })
  const tenantId = `dateinf-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
  const LABELS = ['DateInfJson', 'DateInfCsv', 'DateInfCreate', 'DateInfCreateMany']

  // Raw client for __proptypes verification (the SDK strips system keys).
  const raw = async (path: string, body: unknown, method = 'POST') => {
    const response = await fetch(`${apiUrl}/api/v1${path}`, {
      method,
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${apiKey}`
      },
      body: JSON.stringify(body)
    })
    const json: any = await response.json().catch(() => ({}))
    return { status: response.status, json }
  }

  afterAll(async () => {
    await db.records.delete({ labels: LABELS, where: { tenantId } }).catch(() => undefined)
  })

  // ── JSON import ──────────────────────────────────────────────────────────

  it('importJson with YYYY-MM-DD infers datetime type', async () => {
    const result = await db.records.importJson({
      label: 'DateInfJson',
      data: [
        { event: 'start', date: '2026-07-23', tenantId },
        { event: 'end', date: '2026-12-31', tenantId }
      ],
      options: { suggestTypes: true, returnResult: true }
    })

    expect(result.data.length).toBe(2)

    // Query with datetime comparison — only works if stored as datetime, not string
    const found = await db.records.find({
      labels: ['DateInfJson'],
      where: {
        tenantId,
        date: { $gte: '2026-07-01', $lte: '2026-08-01' }
      }
    })
    expect(found.data.length).toBe(1)
    expect((found.data[0] as any).data.event).toBe('start')

    // $type: "datetime" filter must match
    const typed = await db.records.find({
      labels: ['DateInfJson'],
      where: { tenantId, date: { $type: 'datetime' } }
    })
    expect(typed.data.length).toBe(2)
  })

  it('importJson with YYYY-MM-DD stores datetime in __proptypes', async () => {
    // Re-fetch via raw search to inspect __proptypes
    const { json } = await raw('/records/search', {
      labels: ['DateInfJson'],
      where: { tenantId },
      limit: 1
    })
    const records = json.data ?? json.records ?? []
    expect(records.length).toBeGreaterThan(0)
    const record = records[0]
    expect(record.__proptypes?.date).toBe('datetime')
  })

  // ── CSV import ───────────────────────────────────────────────────────────

  function buildCsv(rows: Array<Record<string, any>>): string {
    const headers = Object.keys(rows[0])
    const lines = [headers.join(',')]
    for (const r of rows) {
      lines.push(headers.map((h) => String(r[h])).join(','))
    }
    return lines.join('\n')
  }

  it('importCsv with YYYY-MM-DD infers datetime type', async () => {
    const csv = buildCsv([
      { event: 'csv-start', date: '2025-01-15', tenantId },
      { event: 'csv-end', date: '2025-06-30', tenantId }
    ])

    await db.records.importCsv({
      label: 'DateInfCsv',
      data: csv,
      options: { suggestTypes: true, returnResult: true }
    })

    // Datetime comparison query — would fail if stored as string
    const found = await db.records.find({
      labels: ['DateInfCsv'],
      where: {
        tenantId,
        date: { $gte: '2025-01-01', $lte: '2025-03-01' }
      }
    })
    expect(found.data.length).toBe(1)
    expect((found.data[0] as DBRecordInstance).data.event).toBe('csv-start')

    // $type: "datetime" filter must match
    const typed = await db.records.find({
      labels: ['DateInfCsv'],
      where: { tenantId, date: { $type: 'datetime' } }
    })
    expect(typed.data.length).toBe(2)
  })

  it('importCsv with YYYY-MM-DD stores datetime in __proptypes', async () => {
    const { json } = await raw('/records/search', {
      labels: ['DateInfCsv'],
      where: { tenantId },
      limit: 1
    })
    const records = json.data ?? json.records ?? []
    expect(records.length).toBeGreaterThan(0)
    const record = records[0]
    expect(record.__proptypes?.date).toBe('datetime')
  })

  // ── create + createMany ──────────────────────────────────────────────────

  it('create with YYYY-MM-DD infers datetime type', async () => {
    const result = await db.records.create({
      label: 'DateInfCreate',
      data: { event: 'single', date: '2024-03-15', tenantId },
      options: { suggestTypes: true }
    })
    expect(result.data.event).toBe('single')

    // $type: "datetime" filter must match
    const typed = await db.records.find({
      labels: ['DateInfCreate'],
      where: { tenantId, date: { $type: 'datetime' } }
    })
    expect(typed.data.length).toBe(1)
  })

  it('createMany with YYYY-MM-DD infers datetime type', async () => {
    const result = await db.records.createMany({
      label: 'DateInfCreateMany',
      data: [
        { event: 'many-a', date: '2023-01-01', tenantId },
        { event: 'many-b', date: '2023-12-31', tenantId }
      ],
      options: { suggestTypes: true, returnResult: true }
    })
    expect(result.data.length).toBe(2)

    const typed = await db.records.find({
      labels: ['DateInfCreateMany'],
      where: { tenantId, date: { $type: 'datetime' } }
    })
    expect(typed.data.length).toBe(2)
  })

  // ── Query date-only values as criteria ───────────────────────────────────

  it('filters by exact YYYY-MM-DD value via datetime()', async () => {
    const found = await db.records.find({
      labels: ['DateInfJson'],
      where: { tenantId, date: { $eq: '2026-07-23' } }
    })
    expect(found.data.length).toBe(1)
    expect((found.data[0] as any).data.event).toBe('start')
  })

  it('sorts by YYYY-MM-DD property', async () => {
    const sorted = await db.records.find({
      labels: ['DateInfJson'],
      where: { tenantId },
      orderBy: { date: 'desc' }
    })
    expect(sorted.data.length).toBe(2)
    expect((sorted.data[0] as any).data.date).toBe('2026-12-31')
    expect((sorted.data[1] as any).data.date).toBe('2026-07-23')
  })


})
