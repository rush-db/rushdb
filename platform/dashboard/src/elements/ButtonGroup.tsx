import { cn } from '~/lib/utils'

//  tailwind idi nahui
import styles from './ButtonGroup.module.css'

export function ButtonGroup({
  className,
  ...props
}: TPolymorphicComponentProps<'div'>) {
  return (
    <div
      className={cn('flex', styles.wrapper, className)}
      role="group"
      {...props}
    />
  )
}
