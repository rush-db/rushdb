// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-nocheck
import { EmbeddingModelMigrationService } from '@/core/ai/embedding-model-migration.service'

const TARGET_MODEL = 'openai/text-embedding-3-small'
const OLD_MODEL = 'qwen/qwen3-embedding-8b'

const makeRow = (overrides = {}) => ({
  id: 'idx-1',
  projectId: 'proj-1',
  label: 'Article',
  propertyName: 'description',
  modelKey: OLD_MODEL,
  sourceType: 'managed',
  similarityFunction: 'cosine',
  dimensions: 1024,
  vectorPropertyName: '_emb_managed_cosine_1024',
  enabled: true,
  status: 'ready',
  ...overrides
})

function makeService({
  config = { RUSHDB_EMBEDDING_MODEL: TARGET_MODEL, RUSHDB_EMBEDDING_DIMENSIONS: '1024' },
  staleRows = [],
  probeDimensions = 1024
} = {}) {
  const configService = { get: (key) => config[key] }
  const repository = {
    findManagedWithStaleModelKey: jest.fn().mockResolvedValue(staleRows),
    updateModelKeyAndStatus: jest.fn().mockResolvedValue(undefined)
  }
  const provider = {
    embed: jest.fn().mockResolvedValue(new Array(probeDimensions).fill(0))
  }
  const session = { run: jest.fn().mockResolvedValue({}), close: jest.fn().mockResolvedValue(undefined) }
  const neogmaService = { createSession: jest.fn().mockReturnValue(session) }
  const aiQueryService = { getStripEmbeddingsQuery: jest.fn().mockReturnValue('STRIP QUERY') }

  const service = new EmbeddingModelMigrationService(
    configService,
    repository,
    provider,
    neogmaService,
    aiQueryService
  )
  return { service, repository, provider, session, neogmaService, aiQueryService }
}

describe('EmbeddingModelMigrationService.migrateStaleIndexes', () => {
  it('throws when the embedding provider is not configured', async () => {
    const { service } = makeService({ config: {} })
    await expect(service.migrateStaleIndexes()).rejects.toThrow('RUSHDB_EMBEDDING_MODEL')
  })

  it('is a cheap no-op when every managed index already matches the configured model', async () => {
    const { service, provider, session } = makeService({ staleRows: [] })
    const summary = await service.migrateStaleIndexes()
    expect(summary).toEqual({ targetModel: TARGET_MODEL, migrated: 0, skipped: 0 })
    // No provider probe and no graph work on the happy no-op path (runs every boot).
    expect(provider.embed).not.toHaveBeenCalled()
    expect(session.run).not.toHaveBeenCalled()
  })

  it('aborts before touching data when the provider probe returns wrong dimensions', async () => {
    const { service, repository, session } = makeService({
      staleRows: [makeRow()],
      probeDimensions: 1536
    })
    await expect(service.migrateStaleIndexes()).rejects.toThrow('expected 1024')
    expect(session.run).not.toHaveBeenCalled()
    expect(repository.updateModelKeyAndStatus).not.toHaveBeenCalled()
  })

  it('strips old vectors then re-keys the row to the configured model as pending', async () => {
    const { service, repository, session } = makeService({ staleRows: [makeRow()] })
    const summary = await service.migrateStaleIndexes()

    expect(summary).toEqual({ targetModel: TARGET_MODEL, migrated: 1, skipped: 0 })
    expect(session.run).toHaveBeenCalledWith('STRIP QUERY', {
      projectId: 'proj-1',
      propertyName: 'description',
      propKey: 'Article:description'
    })
    expect(session.close).toHaveBeenCalled()
    expect(repository.updateModelKeyAndStatus).toHaveBeenCalledWith('idx-1', TARGET_MODEL, 'pending')
  })

  it('skips rows whose dimensions differ from the configured dimensions', async () => {
    const { service, repository, session } = makeService({
      staleRows: [makeRow({ id: 'idx-512', dimensions: 512 }), makeRow({ id: 'idx-1024' })]
    })
    const summary = await service.migrateStaleIndexes()

    expect(summary).toEqual({ targetModel: TARGET_MODEL, migrated: 1, skipped: 1 })
    expect(session.run).toHaveBeenCalledTimes(1)
    expect(repository.updateModelKeyAndStatus).toHaveBeenCalledTimes(1)
    expect(repository.updateModelKeyAndStatus).toHaveBeenCalledWith('idx-1024', TARGET_MODEL, 'pending')
  })

  it('dry run reports without stripping or re-keying', async () => {
    const { service, repository, session } = makeService({ staleRows: [makeRow()] })
    const summary = await service.migrateStaleIndexes({ dryRun: true })

    expect(summary).toEqual({ targetModel: TARGET_MODEL, migrated: 1, skipped: 0 })
    expect(session.run).not.toHaveBeenCalled()
    expect(repository.updateModelKeyAndStatus).not.toHaveBeenCalled()
  })
})

describe('EmbeddingModelMigrationService.onApplicationBootstrap', () => {
  it('does nothing when embeddings are not configured', () => {
    const { service, repository } = makeService({ config: {} })
    service.onApplicationBootstrap()
    expect(repository.findManagedWithStaleModelKey).not.toHaveBeenCalled()
  })

  it('does nothing when the manual script suppressed the startup run', () => {
    const { service, repository } = makeService({ staleRows: [makeRow()] })
    EmbeddingModelMigrationService.suppressStartupRun = true
    try {
      service.onApplicationBootstrap()
    } finally {
      EmbeddingModelMigrationService.suppressStartupRun = false
    }
    expect(repository.findManagedWithStaleModelKey).not.toHaveBeenCalled()
  })

  it('kicks off the migration when configured, without letting failures escape', async () => {
    const { service, repository } = makeService({ staleRows: [makeRow()] })
    repository.findManagedWithStaleModelKey.mockRejectedValue(new Error('sql down'))
    expect(() => service.onApplicationBootstrap()).not.toThrow()
    // Let the fire-and-forget promise settle; the rejection must be swallowed and logged.
    await new Promise((resolve) => setImmediate(resolve))
    expect(repository.findManagedWithStaleModelKey).toHaveBeenCalled()
  })
})
