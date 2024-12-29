import type { VariantProps } from 'class-variance-authority'

import { cva } from 'class-variance-authority'

import { SvgIcon } from '~/elements/SvgIcon'

import { SearchOperations } from '../constants'

const iconNames: Record<SearchOperations, string> = {
  // [SearchOperations.ExactMatch]: 'equals',
  [SearchOperations.Equals]: 'equals',
  [SearchOperations.NotEquals]: 'not_equals',
  // [SearchOperations.ExactExclude]: 'not_equals',
  // [SearchOperations.Range]: 'in_range',
  // [SearchOperations.ExcludeRange]: 'not_in_range',
  [SearchOperations.Less]: 'less',
  [SearchOperations.LessOrEqual]: 'less_or_equals',
  [SearchOperations.Greater]: 'greater',
  [SearchOperations.GreaterOrEqual]: 'greater_or_equals',

  [SearchOperations.Contains]: 'contains',
  [SearchOperations.StartsWith]: 'starts_with',
  [SearchOperations.EndsWith]: 'ends_with'
  // [SearchOperations.In]: '=~',
  // [SearchOperations.NotIn]: '!=~'
}

const operationIcon = cva('grid h-5 w-5 place-items-center rounded', {
  variants: {
    variant: {
      filled:
        'grid h-5 w-5 place-items-center rounded bg-gradient-to-br from-accent/10 to-accent-hover/30 text-accent h-5 w-5 [&>svg]:w-4 [&>svg]:h-4',
      normal: 'contents [&_svg]:w-4 [&>svg]:h-4'
    }
  },
  defaultVariants: {
    variant: 'normal'
  }
})

export function SearchOperationIcon({
  operation,
  className,
  variant,
  size
}: {
  className?: string
  operation: SearchOperations
  size?: number
} & VariantProps<typeof operationIcon>) {
  if (!operation) {
    return null
  }

  return (
    <div className={operationIcon({ className, variant })}>
      <SvgIcon height={size} name={iconNames[operation]} width={size} />
    </div>
  )
}
