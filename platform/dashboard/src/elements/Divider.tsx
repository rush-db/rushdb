import { cn } from '~/lib/utils'

export function Divider({
  className,
  vertical,
  ...props
}: TPolymorphicComponentProps<'div', { vertical?: boolean }>) {
  return (
    <div
      {...props}
      className={cn(
        'bg-stroke',
        {
          'h-px w-[stretch]': !vertical,
          'w-px self-stretch': vertical
        },
        className
      )}
      aria-hidden
    />
  )
}

export function AngledSeparator({ size = 24 }: { size?: number | string }) {
  return (
    <span aria-hidden className="text-content/30">
      <svg fill="none" height={size} viewBox="0 0 24 24" width={size}>
        <path
          d="M16.8801 3.54883L7.12012 20.4508"
          stroke="currentColor"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    </span>
  )
}
