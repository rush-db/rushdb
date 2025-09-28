import type { StandardSchemaV1 } from '@standard-schema/spec'
import * as yup from 'yup'

/**
 * Yup adapter. No static inference available from yup; callers should specify TOut.
 */
export function yupToStandard<TOut>(schema: yup.Schema<TOut>): StandardSchemaV1<unknown, TOut> {
  return {
    '~standard': {
      version: 1,
      vendor: 'yup',
      validate: (value: unknown) => {
        try {
          const v = schema.validateSync(value, { abortEarly: false, strict: false })
          return { value: v }
        } catch (err: any) {
          const issues = err?.inner?.length ? err.inner : [err]
          return { issues }
        }
      },
      types: { input: undefined as unknown, output: undefined as unknown as TOut }
    }
  } as any
}
