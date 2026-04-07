const DEFAULT_LIMIT = (() => {
  const v = parseInt(process.env.RUSHDB_PAGINATION_DEFAULT_LIMIT ?? '', 10)
  return Number.isFinite(v) && v > 0 ? v : 100
})()

const MAX_LIMIT = (() => {
  const v = parseInt(process.env.RUSHDB_PAGINATION_MAX_LIMIT ?? '', 10)
  return Number.isFinite(v) && v > 0 ? v : 1000
})()

export const pagination = (skipRaw = 0, limitRaw?: number) => {
  const limit = limitRaw !== undefined && limitRaw > 0 && limitRaw <= MAX_LIMIT ? limitRaw : DEFAULT_LIMIT
  const skip = skipRaw > 0 ? skipRaw : 0

  return { limit, skip }
}
