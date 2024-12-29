import { VariantProps, cva } from 'class-variance-authority'

export const badgeVariants = cva('flex items-center gap-1', {
  defaultVariants: { size: 'medium', variant: 'primary' },
  variants: {
    size: {
      medium: 'h-5 px-2 text-2xs font-medium rounded-full'
    },
    variant: {
      primary: 'bg-content3/10 opacity-70'
    }
  }
})

type BadgeProps = {
  children: React.ReactNode
  className?: string
} & VariantProps<typeof badgeVariants>

export function Badge({ children, size, variant, className }: BadgeProps) {
  return (
    <div className={badgeVariants({ size, variant, className })}>
      {children}
    </div>
  )
}
