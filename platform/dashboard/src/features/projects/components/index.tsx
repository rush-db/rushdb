import type { ComponentPropsWithoutRef, ReactNode } from 'react'

export function Counter({ children }: ComponentPropsWithoutRef<'div'>) {
  return (
    <div className="bg-background-secondary text-content-secondary inline-flex flex-shrink-0 items-center rounded-full px-2 text-sm font-medium leading-snug">
      {children}
    </div>
  )
}

export function Metric({
  label,
  value
}: {
  label: ReactNode
  value: ReactNode
}) {
  return (
    <div className="flex gap-1 sm:gap-3">
      <div className="flex flex-shrink-0 flex-col truncate text-start text-sm font-medium leading-snug text-content2">
        {label}
        <span className="font-mono text-2xl font-bold text-content">
          {value}
        </span>
      </div>
    </div>
  )
}
