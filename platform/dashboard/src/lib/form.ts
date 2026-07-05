import type { FieldValues, Resolver, UseFormProps } from 'react-hook-form'
import type { ZodType, ZodTypeDef } from 'zod'

import { zodResolver } from '@hookform/resolvers/zod'
import { useForm as useFormHook } from 'react-hook-form'

export { z } from 'zod'
export type { TypeOf as InferType } from 'zod'

export const useForm = <TFieldValues extends FieldValues = FieldValues, TContext = any>({
  schema,
  ...props
}: Omit<UseFormProps<TFieldValues, TContext>, 'resolver'> & {
  schema: ZodType<any, ZodTypeDef, any>
}) =>
  useFormHook<TFieldValues, TContext>({
    resolver: zodResolver(schema) as Resolver<TFieldValues, TContext>,
    ...props
  })
