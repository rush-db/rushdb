export type EmbeddingIndex = {
  id: string
  projectId: string
  label: string
  propertyName: string
  modelKey: string
  sourceType?: 'managed' | 'external'
  similarityFunction?: 'cosine' | 'euclidean'
  dimensions: number
  enabled: boolean
  /** 'pending' | 'indexing' | 'ready' | 'error' */
  status: string
  createdAt: string
  updatedAt: string
}

export type CreateEmbeddingIndexParams = {
  label: string
  propertyName: string
}

export type EmbeddingIndexStats = {
  totalRecords: number
  indexedRecords: number
}
