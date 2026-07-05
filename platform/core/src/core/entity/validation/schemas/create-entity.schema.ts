import { z } from 'zod'

import {
  booleanValueSchema,
  datetimeValueSchema,
  nullValueSchema,
  numberValueSchema,
  propertySchema,
  stringValueSchema
} from '@/core/property/validation/schemas/property.schema'

const dataValueSchema = z.union([
  nullValueSchema,
  booleanValueSchema,
  datetimeValueSchema,
  numberValueSchema,
  stringValueSchema
])

const optionsSchema = z
  .object({
    suggestTypes: z.boolean().optional(),
    skipEmptyValues: z.boolean().optional(),
    mergeStrategy: z.enum(['append', 'rewrite']).optional(),
    mergeBy: z.array(z.string().min(1).max(100)).optional()
  })
  .passthrough()

// Each alternative requires its payload key (`properties` / `data`): the pipe keeps unknown keys
// (passthrough), so without this a body with a mis-named payload field (e.g.
// `{ label, payload: {...} }`) passed validation empty and died in Cypher as a raw 500.
export const createEntitySchema = z.union([
  z
    .object({
      label: z.string().min(1).optional(),
      properties: z.array(propertySchema),
      options: optionsSchema.optional()
    })
    .passthrough(),
  z
    .object({
      label: z.string().min(1).optional(),
      data: z.record(z.string().min(1).max(100), dataValueSchema),
      options: optionsSchema.optional()
    })
    .passthrough()
])
