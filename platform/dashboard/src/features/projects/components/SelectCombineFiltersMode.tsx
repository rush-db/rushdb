import type { ComponentPropsWithoutRef, ReactNode } from 'react'

import { useStore } from '@nanostores/react'
import { Ampersand, Equal } from 'lucide-react'

import type { FiltersCombineMode } from '~/types'

import { RadioGroup } from '~/elements/RadioGroup'
import { Tooltip } from '~/elements/Tooltip'

import { $combineFilters } from '../stores/current-project'

const options = [
  {
    icon: (
      <Tooltip
        alignOffset={20}
        className="text-content2"
        sideOffset={10}
        trigger={<Ampersand />}
      >
        <div>
          <div className="flex items-center gap-1 text-2xs uppercase text-content">
            Search mode
            <strong className="block text-accent">AND</strong>
          </div>
          Show records that match <strong className="text-content">all</strong>{' '}
          active filters
        </div>
      </Tooltip>
    ),
    value: 'and'
  },
  {
    icon: (
      <Tooltip
        alignOffset={20}
        className="text-content2"
        sideOffset={10}
        trigger={<Equal className="rotate-90" />}
      >
        <div>
          <div className="flex items-center gap-1 text-2xs uppercase text-content">
            Search mode
            <strong className="block text-accent">OR</strong>
          </div>
          Show records that match <strong className="text-content">one</strong>{' '}
          of the active filters
        </div>
      </Tooltip>
    ),
    value: 'or'
  }
] as Array<{
  icon: ReactNode
  value: FiltersCombineMode
}>

export function SelectCombineFiltersMode({
  size = 'small',
  ...props
}: Partial<ComponentPropsWithoutRef<typeof RadioGroup>>) {
  const currentView = useStore($combineFilters)

  return (
    <RadioGroup
      {...props}
      onChange={$combineFilters.set}
      options={options}
      size={size}
      value={currentView}
    />
  )
}
