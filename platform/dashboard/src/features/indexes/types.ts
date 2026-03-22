export type EmbeddingIndex = {
  id: string
  projectId: string
  propertyName: string
  modelKey: string
  dimensions: number
  enabled: boolean
  /** 'pending' | 'indexing' | 'ready' | 'error' */
  status: string
  createdAt: string
  updatedAt: string
}

export type CreateEmbeddingIndexParams = {
  propertyName: string
}

export type EmbeddingIndexStats = {
  totalRecords: number
  indexedRecords: number
}
