export const PROPERTY_TYPE_STRING = 'string' as const
export const PROPERTY_TYPE_DATETIME = 'datetime' as const
export const PROPERTY_TYPE_BOOLEAN = 'boolean' as const
export const PROPERTY_TYPE_NUMBER = 'number' as const
export const PROPERTY_TYPE_NULL = 'null' as const

// Special type for embeddings and vector search. Strictly for holding number[] values.
export const PROPERTY_TYPE_VECTOR = 'vector' as const

const embedding = [1, 2, 3, 4, 5]

const db = {
  records: {
    find: (any: any) => {},
    update: (...any: any) => {}
  }
}

db.records.find({
  labels: ['CONTENT'],
  where: {
    name: 'Neo4j Documentation',
    emb_prop: {
      $vector: {
        fn: 'cosine',
        value: embedding,
        query: {
          $gte: 0.5,
          $lte: 0.8
        }
      }
    }
  }
})

db.records.update('record-id', {
  name: 'emb_prop',
  type: 'vector',
  value: embedding
})
