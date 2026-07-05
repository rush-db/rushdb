import { z } from 'zod'

import { SORT_DESC, SORT_ASC } from '@/core/search/search.constants'

const anyObjectSchema = z.record(z.unknown())

// Recursive expression schema for the select clause.
// Uses z.lazy to handle the recursive Expr type.
const exprSchema: z.ZodType<unknown> = z.lazy(() =>
  z.union([
    z.string(), // field ref "$record.field" or literal string
    z.number(), // literal number
    z.boolean(), // literal boolean
    z.object({ $ref: z.string() }).passthrough(),
    z.object({ $sum: exprSchema }).passthrough(),
    z
      .object({
        $avg: exprSchema,
        $precision: z.number().int().min(0).optional()
      })
      .passthrough(),
    z
      .object({
        $count: z.union([z.literal('*'), exprSchema])
      })
      .passthrough(),
    z.object({ $min: exprSchema }).passthrough(),
    z.object({ $max: exprSchema }).passthrough(),
    z.object({ $divide: z.array(exprSchema).length(2) }).passthrough(),
    z.object({ $multiply: z.array(exprSchema).length(2) }).passthrough(),
    z.object({ $add: z.array(exprSchema).length(2) }).passthrough(),
    z.object({ $subtract: z.array(exprSchema).length(2) }).passthrough(),
    z.object({ $collect: anyObjectSchema }).passthrough(),
    z.object({ $timeBucket: anyObjectSchema }).passthrough()
  ])
)

const selectSchema = z.record(exprSchema)

const sortDirectionSchema = z.enum([SORT_ASC, SORT_DESC])

const searchDtoSchema = z
  .object({
    limit: z.number().min(1).max(1000).optional(),
    skip: z.number().min(0).optional(),
    orderBy: z.union([sortDirectionSchema, z.record(sortDirectionSchema)]).optional(),
    labels: z.array(z.string().min(1).nullable()).optional(),
    where: anyObjectSchema.optional(),
    select: selectSchema.optional(),
    aggregate: anyObjectSchema.optional(), // legacy — loose validation preserved
    groupBy: z.array(z.string().min(1)).optional()
  })
  .passthrough()

export const searchSchema = searchDtoSchema
