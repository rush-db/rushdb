import { z } from 'zod'

export const createEmbeddingIndexSchema = z
  .object({
    label: z
      .string({ required_error: 'label is required and must be a non-empty string' })
      .min(1, 'label is required and must be a non-empty string'),
    propertyName: z
      .string({ required_error: 'propertyName is required and must be a non-empty string' })
      .min(1, 'propertyName is required and must be a non-empty string'),
    sourceType: z.enum(['managed', 'external']).optional(),
    external: z.boolean().optional(),
    similarityFunction: z.enum(['cosine', 'euclidean']).optional(),
    dimensions: z.number().int().min(1).max(4096).optional()
  })
  .passthrough()
