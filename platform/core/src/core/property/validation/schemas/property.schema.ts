import { z } from 'zod'

import { DATE_ONLY_REGEX, ISO_8601_REGEX, NUMERIC_REGEX } from '@/core/common/constants'
import {
  PROPERTY_TYPE_BOOLEAN,
  PROPERTY_TYPE_DATETIME,
  PROPERTY_TYPE_NUMBER,
  PROPERTY_TYPE_STRING
} from '@/core/property/property.constants'

const nonEmptyStringSchema = z.string().min(1)

const dateOnlySchema = z.string().regex(DATE_ONLY_REGEX, 'fails to match YYYY-MM-DD Date')
const datetimeSchema = z.string().regex(ISO_8601_REGEX, 'fails to match ISO8601 DateTime')
const dateOrDatetimeSchema = z.union([datetimeSchema, dateOnlySchema])

export const numericStringSchema = z.string().regex(NUMERIC_REGEX, 'is not a numeric value')

export const fieldValueSchema = z.union([numericStringSchema, dateOrDatetimeSchema])

export const stringArraySchema = z.array(nonEmptyStringSchema)
export const datetimeArraySchema = z.array(dateOrDatetimeSchema)
export const numberArraySchema = z.array(z.number())
export const nullArraySchema = z.array(z.null())
export const booleanArraySchema = z.array(z.boolean())
export const emptyArraySchema = z.array(z.any()).max(0)

// Accepts null / arrays-of-null / [] as INPUT. `null` is no longer a stored type — such values are
// validated here so the request is accepted, then dropped during normalization (null === field unset).
export const nullValueSchema = z.union([z.null(), nullArraySchema, emptyArraySchema])
export const booleanValueSchema = z.union([z.boolean(), booleanArraySchema, emptyArraySchema])
export const datetimeValueSchema = z.union([dateOrDatetimeSchema, datetimeArraySchema, emptyArraySchema])
export const numberValueSchema = z.union([z.number(), numberArraySchema, emptyArraySchema])
export const stringValueSchema = z.union([z.string(), stringArraySchema, emptyArraySchema])

const valueSchemaFor = (property: { type?: string; valueSeparator?: string }): z.ZodTypeAny => {
  const hasSeparator = typeof property.valueSeparator === 'string'

  switch (property.type) {
    case PROPERTY_TYPE_BOOLEAN:
      return booleanValueSchema
    case PROPERTY_TYPE_DATETIME:
      return hasSeparator ? dateOrDatetimeSchema : datetimeValueSchema
    case PROPERTY_TYPE_STRING:
      return hasSeparator ? nonEmptyStringSchema : stringValueSchema
    case PROPERTY_TYPE_NUMBER:
      return hasSeparator ? nonEmptyStringSchema : numberValueSchema
    default:
      return z.never()
  }
}

export const propertySchema = z
  .object({
    type: z.enum([PROPERTY_TYPE_STRING, PROPERTY_TYPE_DATETIME, PROPERTY_TYPE_BOOLEAN, PROPERTY_TYPE_NUMBER]),
    name: nonEmptyStringSchema,
    value: z.unknown(),
    valueSeparator: nonEmptyStringSchema.optional(),
    metadata: z.record(z.unknown()).optional()
  })
  .passthrough()
  .superRefine((property, ctx) => {
    // `value` is typed as unknown so its presence must be enforced manually; the accepted shape
    // depends on `type` and on whether the value arrives pre-joined via `valueSeparator`.
    if (!('value' in property) || property.value === undefined) {
      ctx.addIssue({ code: z.ZodIssueCode.custom, path: ['value'], message: 'Required' })
      return
    }

    const result = valueSchemaFor(property).safeParse(property.value)
    if (!result.success) {
      for (const issue of result.error.issues) {
        ctx.addIssue({ ...issue, path: ['value', ...issue.path] })
      }
    }
  })
