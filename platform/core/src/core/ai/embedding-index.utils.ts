export const EMBEDDING_INDEX_SOURCE_TYPES = ['managed', 'external'] as const
export const EMBEDDING_INDEX_SIMILARITY_FUNCTIONS = ['cosine', 'euclidean'] as const

export type EmbeddingIndexSourceType = (typeof EMBEDDING_INDEX_SOURCE_TYPES)[number]
export type EmbeddingIndexSimilarityFunction = (typeof EMBEDDING_INDEX_SIMILARITY_FUNCTIONS)[number]

export function buildVectorPropertyName({
  sourceType,
  similarityFunction,
  dimensions
}: {
  sourceType: EmbeddingIndexSourceType
  similarityFunction: EmbeddingIndexSimilarityFunction
  dimensions: number
}): string {
  return `_emb_${sourceType}_${similarityFunction}_${dimensions}`
}

export function buildVectorIndexName({
  sourceType,
  similarityFunction,
  dimensions
}: {
  sourceType: EmbeddingIndexSourceType
  similarityFunction: EmbeddingIndexSimilarityFunction
  dimensions: number
}): string {
  return `rushdb_emb_rel_${sourceType}_${similarityFunction}_${dimensions}`
}

export function isValidEmbeddingDimensions(value: number): boolean {
  return Number.isInteger(value) && value >= 1 && value <= 4096
}
