import { z } from 'zod'

export const cypherSchema = z
  .object({
    query: z.string().min(1),
    params: z.record(z.unknown()).optional()
  })
  .passthrough()
