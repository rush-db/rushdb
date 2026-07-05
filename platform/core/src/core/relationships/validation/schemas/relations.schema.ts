import { z } from 'zod'

const nonEmptyStringSchema = z.string().min(1)
const anyObjectSchema = z.record(z.unknown())
const directionSchema = z.enum(['in', 'out', 'IN', 'OUT'])

export const targetIdsSchema = z.union([nonEmptyStringSchema, z.array(nonEmptyStringSchema)])

export const createRelationSchema = z
  .object({
    targetIds: targetIdsSchema,
    type: nonEmptyStringSchema.optional(),
    direction: directionSchema.optional(),
    properties: anyObjectSchema.optional()
  })
  .passthrough()

export const deleteRelationsSchema = z
  .object({
    targetIds: targetIdsSchema,
    typeOrTypes: z.union([nonEmptyStringSchema, z.array(nonEmptyStringSchema)]).optional(),
    direction: directionSchema.optional()
  })
  .passthrough()

// Two allowed shapes per endpoint side:
// 1) Key-based join: both source.key and target.key provided
// 2) Many-to-many: keys omitted but manyToMany=true and both sides must provide non-empty `where` filters
const relationEndpointSchema = z.union([
  z
    .object({
      label: nonEmptyStringSchema,
      key: nonEmptyStringSchema,
      where: anyObjectSchema.optional()
    })
    .passthrough(),
  z
    .object({
      label: nonEmptyStringSchema,
      key: z.string().nullable().optional(),
      where: anyObjectSchema.refine((value) => Object.keys(value).length > 0, {
        message: 'must have at least 1 key'
      })
    })
    .passthrough()
])

export const createRelationsByKeysSchema = z
  .object({
    source: relationEndpointSchema,
    target: relationEndpointSchema,
    type: nonEmptyStringSchema.optional(),
    direction: directionSchema.optional(),
    properties: anyObjectSchema.optional(),
    manyToMany: z.boolean().optional()
  })
  .passthrough()
