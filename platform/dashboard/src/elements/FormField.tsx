import type { ReactNode } from 'react'

import { forwardRef } from 'react'

import { cn } from '~/lib/utils'

export type FormFieldProps = {
  caption?: ReactNode
  error?: ReactNode
  label?: ReactNode
}

export const Label: TPolymorphicComponent<FormFieldProps, 'label'> = ({
  as = 'label',
  children,
  className,
  ...props
}) => {
  const As = as
  return (
    <As className={cn('ml-1 text-start text-sm', className)} {...props}>
      {children}
    </As>
  )
}

// @ts-ignore
export const FormField: TPolymorphicComponent<FormFieldProps, 'label'> = forwardRef(
  ({ caption, children, className, error, label, as = 'label', ...forwardedProps }, ref) => {
    const Wrapper = as

    return (
      <fieldset className="flex-start flex flex-col gap-1">
        <Wrapper className={cn('flex-start flex flex-col gap-1', className)} ref={ref} {...forwardedProps}>
          {label ?
            <Label as="span">{label}</Label>
          : null}
          {children}
          {caption && !error ?
            <span className="text-content-secondary text-xs">{caption}</span>
          : null}
        </Wrapper>
        {error ?
          <span className="text-danger text-start text-xs">{error}</span>
        : null}
      </fieldset>
    )
  }
)

FormField.displayName = 'FormField'
