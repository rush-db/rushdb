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

export const editEntitySchema = z.union([
  z
    .object({
      label: z.string().min(1).optional(),
      properties: z.array(propertySchema).optional()
    })
    .passthrough(),
  z
    .object({
      label: z.string().min(1).optional(),
      data: z.record(z.string().min(1).max(100), dataValueSchema).optional(),
      options: z
        .object({
          suggestTypes: z.boolean().optional(),
          skipEmptyValues: z.boolean().optional()
        })
        .passthrough()
        .optional()
    })
    .passthrough()
])
