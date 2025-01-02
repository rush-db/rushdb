'use client'

// @ts-nocheck
import * as SliderPrimitive from '@radix-ui/react-slider'
import * as React from 'react'

import { cn, range } from '~/lib/utils'

import { FormField, type FormFieldProps } from './FormField'

export const Slider = React.forwardRef<
  React.ElementRef<typeof SliderPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof SliderPrimitive.Root> & {
    thumbsCount: number
  }
>(({ className, thumbsCount = 1, ...props }, ref) => (
  <SliderPrimitive.Root
    className={cn('relative flex w-full touch-none select-none items-center', className)}
    ref={ref}
    {...props}
  >
    <SliderPrimitive.Track className="bg-secondary relative h-2 w-full grow overflow-hidden rounded-full">
      <SliderPrimitive.Range className="bg-primary absolute h-full" />
    </SliderPrimitive.Track>
    {range(thumbsCount).map((idx) => (
      <SliderPrimitive.Thumb
        className="ring-offset-background focus-visible:ring-ring border-primary bg-fill block h-5 w-5 rounded-full border-2 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50"
        key={idx}
      />
    ))}
  </SliderPrimitive.Root>
))
Slider.displayName = SliderPrimitive.Root.displayName

// @ts-ignore
export const SliderField: TPolymorphicComponent<
  React.ComponentPropsWithoutRef<typeof Slider> & FormFieldProps,
  'label'
> = React.forwardRef(({ caption, className, error, label, as, ...inputProps }, ref) => {
  // @ts-ignore
  return (
    <FormField
      as={as}
      caption={caption}
      className={cn('justify-start', className)}
      error={error}
      label={label}
      ref={ref}
    >
      {/*// @ts-ignore */}
      <Slider aria-invalid={Boolean(error)} {...inputProps} />
    </FormField>
  )
})
SliderField.displayName = 'SliderField'
