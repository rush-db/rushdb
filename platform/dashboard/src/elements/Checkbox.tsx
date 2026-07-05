'use client'

import * as CheckboxPrimitive from '@radix-ui/react-checkbox'
import { Check, Minus } from 'lucide-react'
import * as React from 'react'

import { cn } from '~/lib/utils'
import type { FormFieldProps } from '~/elements/FormField.tsx'
import { FormField } from '~/elements/FormField.tsx'
import type { Switch } from '~/elements/Switch.tsx'

export const Checkbox = React.forwardRef<
  React.ElementRef<typeof CheckboxPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof CheckboxPrimitive.Root> & {
    mixed?: boolean
  }
>(({ className, mixed, ...props }, ref) => (
  <CheckboxPrimitive.Root
    className={cn(
      'ring-offset-background peer h-4 w-4 shrink-0 rounded-sm border border-content3 focus-visible:ring-2 focus-visible:ring-primary-ring focus-visible:ring-offset-2 focus-visible:outline-hidden disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:border-primary data-[state=checked]:bg-primary data-[state=checked]:text-primary-contrast',
      className
    )}
    ref={ref}
    {...props}
  >
    <CheckboxPrimitive.Indicator className={cn('-mt-[1px] flex items-center justify-center text-current')}>
      {mixed ?
        <Minus className="h-4 w-4" />
      : <Check className="h-4 w-4" />}
    </CheckboxPrimitive.Indicator>
  </CheckboxPrimitive.Root>
))
Checkbox.displayName = CheckboxPrimitive.Root.displayName

// @ts-ignore
export const CheckboxField: TPolymorphicComponent<
  React.ComponentPropsWithoutRef<typeof Switch> & FormFieldProps,
  'label'
> = React.forwardRef(({ caption, className, error, label, as, ...inputProps }, ref) => {
  return (
    <FormField
      as={as}
      caption={caption}
      className={cn('flex-row-reverse items-center justify-end gap-2', className)}
      error={error}
      label={label}
      ref={ref}
    >
      <Checkbox aria-invalid={Boolean(error)} {...inputProps} />
    </FormField>
  )
})
CheckboxField.displayName = 'CheckboxField'
