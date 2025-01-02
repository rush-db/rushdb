import type { FieldValues, UseFormProps } from 'react-hook-form'
import type { AnyObjectSchema } from 'yup'

import { yupResolver } from '@hookform/resolvers/yup'
import { useForm as useFormHook } from 'react-hook-form'

export * from 'yup'

export const useForm = <TFieldValues extends FieldValues = FieldValues, TContext = any>({
  schema,
  ...props
}: Omit<UseFormProps<TFieldValues, TContext>, 'schema'> & {
  schema: AnyObjectSchema
}) =>
  useFormHook<TFieldValues, TContext>({
    resolver: yupResolver(schema),
    ...props
  })
