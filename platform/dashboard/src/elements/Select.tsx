import type { VariantProps } from 'class-variance-authority'
import type { ReactNode } from 'react'

import { forwardRef } from 'react'

import { cn } from '~/lib/utils'

import type { FormFieldProps } from './FormField'

import { FormField } from './FormField'
import { inputWrapper } from './Input'
import { MenuIcon } from './Menu'

type TSelectOption = {
  label?: ReactNode
  value: number | string
}

type TSelectProps = TPolymorphicComponentProps<
  'select',
  {
    options?: TSelectOption[]
  } & VariantProps<typeof inputWrapper>
>

export const Select = forwardRef<HTMLSelectElement, TSelectProps>(
  ({ className, options, size, variant, ...props }, ref) => {
    return (
      <div className="relative flex items-center">
        <select
          className={cn(
            inputWrapper({ className, size, variant }),
            'cursor-pointer appearance-none'
          )}
          {...props}
          ref={ref}
        >
          {options?.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label ?? o.value}
            </option>
          ))}
        </select>
        <MenuIcon className="pointer-events-none absolute end-2" size={20} />
      </div>
    )
  }
)

Select.displayName = 'Select'

export const SelectField = forwardRef<
  HTMLSelectElement,
  FormFieldProps & TSelectProps
>(({ caption, className, error, label, ...inputProps }, ref) => {
  return (
    <FormField
      caption={caption}
      className={className}
      error={error}
      label={label}
    >
      <Select aria-invalid={Boolean(error)} ref={ref} {...inputProps} />
    </FormField>
  )
})

SelectField.displayName = 'SelectField'
