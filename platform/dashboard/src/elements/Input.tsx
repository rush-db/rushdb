import type { VariantProps } from 'class-variance-authority'
import type { ReactNode } from 'react'

import { cva } from 'class-variance-authority'
import { Copy, CopyCheck, Search } from 'lucide-react'
import { forwardRef, useState } from 'react'

import { cn, copyToClipboard } from '~/lib/utils'

import type { FormFieldProps } from './FormField'

import { FormField } from './FormField'
import { useTimeout } from '~/hooks/useTimeout'

export const inputWrapper = cva(
  'flex w-full cursor-text items-center px-3  transition focus-within:outline-none [&_svg]:flex-shrink-0',
  {
    variants: {
      size: {
        medium: 'h-11 gap-3 rounded-md text-sm [&>svg]:w-4 [&>svg]:h-4',
        small: 'h-9 gap-3 rounded-md text-sm [&>svg]:w-4 [&>svg]:h-4'
      },
      variant: {
        primary:
          'bg-input focus-within:ring border border-transparent bg-secondary ring-interaction-ring focus-within:border-interaction-focus focus-within:ring [&:hover:not(:focus-within)]:bg-secondary-hover',
        ghost: 'bg-transparent border-b',
        disabled: 'bg-[#222] border text-content-secondary ',
        error:
          'bg-input focus-within:ring border border-danger bg-secondary ring-danger-ring focus-within:border-danger-focus focus-within:ring [&:hover:not(:focus-within)]:bg-secondary-hover'
      }
    },
    defaultVariants: {
      size: 'medium',
      variant: 'primary'
    }
  }
)

export const input = cva(
  'w-full h-full cursor-inherit appearance-none text-start file:border-0 file:font-inherit file:text-inherit file:bg-transparent  bg-transparent text-inherit outline-none read-only:cursor-pointer disabled:cursor-not-allowed disabled:opacity-50 text-ellipsis',
  {
    variants: {
      size: {
        medium: '',
        small: ''
      },
      variant: {
        primary: 'placeholder:text-content3',
        ghost: 'placeholder:text-content3',
        error: 'placeholder:text-content3',
        disabled: 'placeholder:text-content2'
      }
    },
    defaultVariants: {
      size: 'medium',
      variant: 'primary'
    }
  }
)

export type InputProps = Omit<
  TInheritableElementProps<
    'input',
    {
      prefix?: ReactNode
      suffix?: ReactNode
    }
  >,
  'size'
> &
  AsProp<'div' | 'label'> &
  VariantProps<typeof inputWrapper>

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ className, prefix, suffix, as = 'label', size, variant, readOnly, ...props }, ref) => {
    const Element = as

    return (
      <Element
        className={cn(
          inputWrapper({ size, variant: props.disabled ? 'disabled' : variant }),
          {
            'cursor-pointer': readOnly
          },
          className
        )}
      >
        {prefix}
        <input className={input({ size, variant })} {...props} ref={ref} readOnly={readOnly} />
        {suffix}
      </Element>
    )
  }
)

export const SearchInput = forwardRef<HTMLInputElement, InputProps>((props, ref) => {
  return <Input ref={ref} type="search" {...props} prefix={<Search size={16} />} />
})

// @ts-ignore
export const TextField: TPolymorphicComponent<InputProps & FormFieldProps, 'label'> = forwardRef(
  ({ caption, className, error, label, as, ...inputProps }, ref) => {
    return (
      <FormField as={as} caption={caption} className={className} error={error} label={label} ref={ref}>
        <Input
          aria-invalid={Boolean(error)}
          as="div"
          variant={error ? 'error' : inputProps.variant}
          {...inputProps}
        />
      </FormField>
    )
  }
)

export const CopyInput = forwardRef<HTMLInputElement, InputProps>(({ className, value, ...props }, ref) => {
  const [justCopied, setJustCopied] = useState(false)

  useTimeout(
    () => {
      setJustCopied(false)
    },
    justCopied ? 1000 : null
  )

  const copyToken = () => {
    if (value && !Array.isArray(value)) {
      copyToClipboard(value as string, {
        callback: () => setJustCopied(true)
      })
    }
  }

  return (
    <Input
      ref={ref}
      onClick={copyToken}
      suffix={justCopied ? <CopyCheck className="text-success h-5 w-5" /> : <Copy className="h-5 w-5" />}
      readOnly
      className={cn('!border-transparent !ring-0', { '!border-success': justCopied }, className)}
      value={value}
      {...props}
    />
  )
})

Input.displayName = 'Input'
TextField.displayName = 'TextField'
SearchInput.displayName = 'SearchInput'
CopyInput.displayName = 'CopyInput'
