import { z } from 'zod'

const endpointSchema = z.object({
  column: z.string().min(1).max(255),
  label: z.string().regex(/^[A-Za-z][A-Za-z0-9_]{0,99}$/),
  keyProperty: z.string().min(1).max(255),
  direction: z.enum(['source', 'target'])
})

export const linkSpecSchema = z.object({
  version: z.literal(1),
  role: z.literal('links'),
  endpoints: z.tuple([endpointSchema, endpointSchema]),
  relationshipType: z.string().regex(/^[A-Za-z][A-Za-z0-9_]{0,99}$/),
  propertyColumns: z.record(z.string().min(1).max(255)).optional()
})

export const importFileManifestSchema = z
  .object({
    clientFileId: z.string().min(1).max(100),
    fileName: z.string().min(1).max(255),
    size: z.number().int().nonnegative(),
    format: z.enum(['csv', 'jsonl', 'ndjson', 'json', 'parquet']),
    role: z.enum(['records', 'links']).default('records'),
    rootLabel: z
      .string()
      .regex(/^[A-Za-z][A-Za-z0-9_]{0,99}$/)
      .optional(),
    linkSpec: linkSpecSchema.optional(),
    parseOptions: z.record(z.unknown()).optional(),
    importOptions: z.record(z.unknown()).optional()
  })
  .superRefine((item, ctx) => {
    if (item.role === 'links') {
      if (!item.linkSpec) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          message: 'linkSpec is required for links files',
          path: ['linkSpec']
        })
      }
      if (item.rootLabel) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          message: 'rootLabel must not be set for links files',
          path: ['rootLabel']
        })
      }
    } else if (!item.rootLabel) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        message: 'rootLabel is required for records files',
        path: ['rootLabel']
      })
    }
  })

export const createImportRunSchema = z.object({
  name: z.string().min(1).max(255).optional(),
  failurePolicy: z.enum(['continue', 'stop_new_files']).default('continue'),
  files: z.array(importFileManifestSchema).min(1).max(50)
})

export type CreateImportRunDto = z.infer<typeof createImportRunSchema>
export type ImportFileManifestDto = z.infer<typeof importFileManifestSchema>
