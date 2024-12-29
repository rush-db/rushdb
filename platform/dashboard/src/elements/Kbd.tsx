import { ArrowDownUp, Command, Delete } from 'lucide-react'

import { cn } from '~/lib/utils'

const specialCodes = {
  Backspace: <Delete />,
  Meta: <Command />,
  ArrowDownUp: <ArrowDownUp />
}

export function Kbd({
  code: codeProp,
  pressed,
  children,
  className,
  ...props
}: TPolymorphicComponentProps<
  'kbd',
  {
    code: string | string[]
    pressed?: boolean
  }
>) {
  const codes = Array.isArray(codeProp) ? codeProp : [codeProp]

  return (
    <div
      className="inline-flex shrink-0 items-center gap-2 text-content"
      {...props}
    >
      {codes.map((code) => {
        const content =
          code in specialCodes
            ? specialCodes[code as keyof typeof specialCodes]
            : code

        return (
          <kbd
            className={cn(
              'grid h-4 min-w-[1rem]  place-items-center rounded-[4px] border border-content3 bg-content2 px-0.5 text-[0.5rem] font-black uppercase leading-none text-fill2 [&>svg]:h-3 [&>svg]:w-3',
              {
                'border-b-[2px]': !pressed,
                '': pressed
              },
              className
            )}
            key={code}
          >
            {content}
          </kbd>
        )
      })}

      {children && <span className="text-xs text-content3">{children}</span>}
    </div>
  )
}
