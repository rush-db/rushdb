'use client'

import * as SwitchPrimitives from '@radix-ui/react-switch'
import * as React from 'react'

import { cn } from '~/lib/utils'

import type { FormFieldProps } from './FormField'

import { FormField } from './FormField'

export const Switch = React.forwardRef<
  React.ElementRef<typeof SwitchPrimitives.Root>,
  React.ComponentPropsWithoutRef<typeof SwitchPrimitives.Root>
>(({ className, ...props }, ref) => (
  <SwitchPrimitives.Root
    className={cn(
      'peer inline-flex h-[24px] w-[44px] shrink-0 cursor-pointer items-center rounded-full border border-transparent ring-interaction-ring transition-colors focus-visible:border-interaction-focus  focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:bg-accent-hover data-[state=unchecked]:bg-secondary data-[state=checked]:hover:bg-accent data-[state=unchecked]:hover:bg-secondary-hover',
      className
    )}
    {...props}
    ref={ref}
  >
    <SwitchPrimitives.Thumb
      className={cn(
        'pointer-events-none block h-5 w-5 rounded-full bg-content shadow-lg ring-0 transition-transform data-[state=checked]:translate-x-5 data-[state=unchecked]:translate-x-0'
      )}
    />
  </SwitchPrimitives.Root>
))
Switch.displayName = SwitchPrimitives.Root.displayName

export const SwitchField: TPolymorphicComponent<
  React.ComponentPropsWithoutRef<typeof Switch> & FormFieldProps,
  'label'
> = React.forwardRef(
  ({ caption, className, error, label, as, ...inputProps }, ref) => {
    return (
      <FormField
        as={as}
        caption={caption}
        className={cn('justify-start', className)}
        error={error}
        label={label}
        ref={ref}
      >
        <Switch aria-invalid={Boolean(error)} {...inputProps} />
      </FormField>
    )
  }
)
SwitchField.displayName = 'SwitchField'
