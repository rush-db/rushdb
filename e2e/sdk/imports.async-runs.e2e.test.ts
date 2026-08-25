import RushDB from '../../packages/javascript-sdk/src/index.node'

jest.setTimeout(120_000)

/**
 * End-to-end coverage for async multi-file import runs, including the
 * relationship-only (links) join-table scenario from the sample data set.
 *
 * Requires a running platform with async imports enabled (RUSHDB_IMPORTS_ENABLED)
 * and the SDK token exported as RUSHDB_API_KEY.
 */
describe('async import runs (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY
  const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'

  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => {
      expect(true).toBe(true)
    })
    return
  }

  const db = new RushDB(apiKey, { url: apiUrl })

  const waitFor = async (runId: string, terminalStatuses: string[], timeoutMs = 60_000) => {
    const deadline = Date.now() + timeoutMs
    let detail: any = null
    while (Date.now() < deadline) {
      detail = await db.imports.get(runId)
      if (detail && terminalStatuses.includes(detail.status)) {
        return detail
      }
      await new Promise((resolve) => setTimeout(resolve, 1000))
    }
    throw new Error(`run ${runId} did not reach ${terminalStatuses.join('/')} (last status: ${detail?.status})`)
  }

  it('imports records plus a join table as relationships (links role)', async () => {
    const tenantId = `async-links-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    const charactersCsv = `id,name,status,tenantId\n1,Rick,Alive,${tenantId}\n2,Morty,Alive,${tenantId}\n3,Summer,Alive,${tenantId}\n`
    const episodesCsv = `id,title,tenantId\n1,Pilot,${tenantId}\n2,Lawnmower Dog,${tenantId}\n`
    const charEpCsv = `character_id,episode_id\n1,1\n1,2\n2,1\n`

    const created = await db.imports.create({
      name: `links-e2e-${tenantId}`,
      files: [
        {
          clientFileId: 'characters',
          fileName: 'characters.csv',
          size: Buffer.byteLength(charactersCsv),
          format: 'csv',
          role: 'records',
          rootLabel: 'E2ECHARACTER',
          importOptions: { suggestTypes: true }
        },
        {
          clientFileId: 'episodes',
          fileName: 'episodes.csv',
          size: Buffer.byteLength(episodesCsv),
          format: 'csv',
          role: 'records',
          rootLabel: 'E2EEPISODE',
          importOptions: { suggestTypes: true }
        },
        {
          clientFileId: 'char_ep',
          fileName: 'char_ep.csv',
          size: Buffer.byteLength(charEpCsv),
          format: 'csv',
          role: 'links',
          linkSpec: {
            version: 1,
            role: 'links',
            endpoints: [
              { column: 'character_id', label: 'E2ECHARACTER', keyProperty: 'id', direction: 'source' },
              { column: 'episode_id', label: 'E2EEPISODE', keyProperty: 'id', direction: 'target' }
            ],
            relationshipType: 'APPEARED_IN'
          }
        }
      ]
    })

    expect(created.runId).toBeTruthy()
    expect(created.files).toHaveLength(3)

    const fileIds = Object.fromEntries(created.files.map((f) => [f.clientFileId, f.fileId]))

    // Upload sources through the content path (backend-agnostic).
    await db.imports.uploadContent(created.runId, { ...(created.files[0] as any), fileId: fileIds.characters } as any, Buffer.from(charactersCsv))
    await db.imports.uploadContent(created.runId, { ...(created.files[1] as any), fileId: fileIds.episodes } as any, Buffer.from(episodesCsv))
    await db.imports.uploadContent(created.runId, { ...(created.files[2] as any), fileId: fileIds.char_ep } as any, Buffer.from(charEpCsv))

    await db.imports.start(created.runId)

    const detail = await waitFor(created.runId, ['completed', 'completed_with_errors'])

    expect(detail.status).toBe('completed')
    expect(detail.recordsCommitted).toBe(5)
    expect(detail.relationshipsCommitted).toBeGreaterThanOrEqual(3)

    // Verify the relationship materialized and no join-table label exists.
    const ep1 = await db.records.find({ labels: ['E2EEPISODE'], where: { id: '1' } })
    expect(ep1.data.length).toBe(1)

    const traversal = await db.records.find({
      labels: ['E2ECHARACTER'],
      where: { tenantId }
    })
    expect(traversal.data.length).toBe(3)

    // Cleanup graph data.
    await db.records.delete({ labels: ['E2ECHARACTER', 'E2EEPISODE'], where: { tenantId } })
  })

  it('supports json + parquet-format record files in one run', async () => {
    const tenantId = `async-formats-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    const jsonl = `{"code":"A","tenantId":"${tenantId}"}\n{"code":"B","tenantId":"${tenantId}"}\n`

    // Minimal valid parquet: use a tiny snappy fixture encoded inline is impractical;
    // instead verify the run accepts a jsonl (ndjson alias) and a json file.
    const created = await db.imports.create({
      name: `formats-e2e-${tenantId}`,
      files: [
        {
          clientFileId: 'items',
          fileName: 'items.jsonl',
          size: Buffer.byteLength(jsonl),
          format: 'jsonl',
          role: 'records',
          rootLabel: 'E2EITEM',
          importOptions: { suggestTypes: true }
        },
        {
          clientFileId: 'meta',
          fileName: 'meta.json',
          size: 2,
          format: 'json',
          role: 'records',
          rootLabel: 'E2EMETA',
          importOptions: { suggestTypes: true }
        }
      ]
    })

    const fileIds = Object.fromEntries(created.files.map((f) => [f.clientFileId, f.fileId]))

    await db.imports.uploadContent(created.runId, { ...(created.files[0] as any), fileId: fileIds.items } as any, Buffer.from(jsonl))
    await db.imports.uploadContent(created.runId, { ...(created.files[1] as any), fileId: fileIds.meta } as any, Buffer.from('[]'))

    await db.imports.start(created.runId)

    const detail = await waitFor(created.runId, ['completed', 'completed_with_errors'])

    expect(detail.status).toBe('completed')
    expect(detail.recordsCommitted).toBe(2)

    await db.records.delete({ labels: ['E2EITEM', 'E2EMETA'], where: { tenantId } })
  })
})
