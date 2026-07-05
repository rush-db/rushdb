import { z } from 'zod'

const anyObjectSchema = z.record(z.unknown())

const importOptionsSchema = z
  .object({
    suggestTypes: z.boolean().optional(),
    convertNumericValuesToNumbers: z.boolean().optional(),
    capitalizeLabels: z.boolean().optional(),
    skipEmptyValues: z.boolean().optional(),
    relationshipType: z.string().min(1).optional(),
    returnResult: z.boolean().optional(),
    mergeStrategy: z.enum(['append', 'rewrite']).optional(),
    mergeBy: z.array(z.string().min(1)).optional()
  })
  .passthrough()

export const importJsonSchema = z
  .object({
    label: z.string().min(1).optional(),
    data: z.union([anyObjectSchema, z.array(anyObjectSchema), z.string().min(1)]).optional(),
    format: z.enum(['json', 'jsonl', 'ndjson']).optional(),
    options: importOptionsSchema.optional()
  })
  .passthrough()

export const importCsvSchema = z
  .object({
    label: z.string().min(1).optional(),
    data: z.string().min(1).optional(),
    options: importOptionsSchema.optional(),
    parseConfig: z
      .object({
        delimiter: z.string().min(1).max(5).optional(),
        header: z.boolean().optional(),
        skipEmptyLines: z.union([z.boolean(), z.literal('greedy')]).optional(),
        dynamicTyping: z.boolean().optional(),
        quoteChar: z.string().min(1).max(3).optional(),
        escapeChar: z.string().min(1).max(3).optional(),
        newline: z.string().min(1).max(4).optional()
      })
      .passthrough()
      .optional(),
    vectors: z
      .array(
        z.array(
          z
            .object({
              propertyName: z.string().min(1),
              vector: z.array(z.number()),
              similarityFunction: z.enum(['cosine', 'euclidean']).optional()
            })
            .passthrough()
        )
      )
      .optional()
  })
  .passthrough()
