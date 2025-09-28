import type { StandardSchemaV1 } from '@standard-schema/spec'
import { z, ZodTypeAny } from 'zod'

export function zodToStandard<S extends ZodTypeAny>(schema: S): StandardSchemaV1<z.input<S>, z.output<S>> {
  return {
    '~standard': {
      version: 1,
      vendor: 'zod',
      validate: (value: unknown) => {
        const r = schema.safeParse(value)
        return r.success ? { value: r.data } : { issues: r.error.issues }
      },
      types: {
        input: undefined as unknown as z.input<S>,
        output: undefined as unknown as z.output<S>
      }
    }
  } as any
}
