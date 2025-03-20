import type { VariantProps } from 'class-variance-authority'
import type { ReactNode } from 'react'

import { cva } from 'class-variance-authority'
import { Fragment } from 'react'

import { cn } from '~/lib/utils'

import { IconButton } from './IconButton'
import { Button } from '~/elements/Button.tsx'

const group = cva('border-input-stroke bg-input relative flex border items-center shrink-0', {
  variants: {
    size: {
      medium: 'h-11 px-1 gap-1 rounded',
      small: 'h-9 px-1 gap-1 rounded'
    }
  },
  defaultVariants: {
    size: 'medium'
  }
})

type RadioGroupProps<
  Value,
  Option = {
    icon: ReactNode
    value: Value
  }
> = TPolymorphicComponentProps<
  'ul',
  {
    divide?: boolean
    onChange: (newValue: Value) => void
    options: Array<Option>
    value: Value
    useDefaultButton?: boolean
  } & VariantProps<typeof group>
>

export function RadioGroup<Value>({
  className,
  value,
  onChange,
  options,
  size,
  divide,
  useDefaultButton,
  ...props
}: RadioGroupProps<Value>) {
  const activeIdx = options?.findIndex((o) => o.value === value)

  const buttonSize =
    useDefaultButton ? 106
    : size === 'small' ? 28
    : 36

  return (
    <ul className={group({ className, size })} role="radiogroup" {...props}>
      <div
        style={{
          height: useDefaultButton ? 28 : buttonSize,
          width: buttonSize,
          transform: `translate3d(calc((${activeIdx} * ${buttonSize}px) + (4px * ${activeIdx})),-50%,0)`
        }}
        aria-hidden
        className="bg-secondary absolute start-1 top-1/2 h-[28px] w-[28px] shrink-0 rounded-sm transition-transform"
        // layoutId={id}
      />
      {options.map((o, idx) => {
        const active = value === o.value
        return (
          <Fragment key={o.value as string}>
            <li className="relative shrink-0">
              {/* {active ? (
                <motion.div
                  className="absolute start-0 top-0 h-full w-full rounded-sm bg-secondary"
                  layoutId={id}
                />
              ) : null} */}
              {useDefaultButton ?
                <Button
                  className={cn('relative z-10 text-center !no-underline', {
                    // 'text-content': active,
                    'text-content2': !active
                  })}
                  aria-label={String(o.value)}
                  key={o.value as string}
                  onClick={() => onChange(o.value)}
                  role="radio"
                  size={size === 'small' ? 'xsmall' : 'small'}
                  variant="link"
                >
                  {o.icon}
                </Button>
              : <IconButton
                  className={cn('relative z-10', {
                    // 'text-content': active,
                    'text-content2': !active
                  })}
                  aria-label={String(o.value)}
                  key={o.value as string}
                  onClick={() => onChange(o.value)}
                  role="radio"
                  size={size === 'small' ? 'xsmall' : 'small'}
                  variant="link"
                >
                  {o.icon}
                </IconButton>
              }
            </li>
          </Fragment>
        )
      })}
    </ul>
  )
}
