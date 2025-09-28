import type { StandardSchemaV1 } from '@standard-schema/spec'
import Joi, { Schema } from 'joi'

/**
 * Note: Joi lacks native TS inference; the adapter is generic in TOut.
 * Callers can specify <TOut> to get strong types at compile time.
 */
export function joiToStandard<TOut>(schema: Schema): StandardSchemaV1<unknown, TOut> {
  return {
    '~standard': {
      version: 1,
      vendor: 'joi',
      validate: (value: unknown) => {
        const { error, value: v } = schema.validate(value, { abortEarly: false })
        return error ? { issues: error.details } : { value: v as TOut }
      },
      // No way to expose input type; leave as unknown
      types: { input: undefined as unknown, output: undefined as unknown as TOut }
    }
  } as any
}
