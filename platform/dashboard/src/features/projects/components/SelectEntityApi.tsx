import type { ComponentPropsWithoutRef, ReactNode } from 'react'

import { useStore } from '@nanostores/react'
import { Code2, TableIcon } from 'lucide-react'

import { RadioGroup } from '~/elements/RadioGroup'
import { Tooltip } from '~/elements/Tooltip'

import { $recordRawApiEntity, $recordView } from '../stores/current-project'
import { RawApiEntityType, RecordViewType } from '~/features/projects/types.ts'
import { GraphIcon } from '~/elements/GraphIcon.tsx'

const options = [
  {
    icon: (
      <div className="text-content m-auto flex w-[90px] items-center justify-center text-sm hover:no-underline">
        Records
      </div>
    ),
    value: 'records'
  },
  {
    icon: (
      <div className="text-content m-auto flex w-[90px] items-center justify-center text-sm hover:no-underline">
        Properties
      </div>
    ),
    value: 'properties'
  },
  {
    icon: (
      <div className="text-content m-auto flex w-[90px] items-center justify-center text-sm hover:no-underline">
        Labels
      </div>
    ),
    value: 'labels'
  }
] as Array<{
  icon: ReactNode
  value: RawApiEntityType
}>

export function SelectEntityApi({
  size = 'small',
  ...props
}: Partial<ComponentPropsWithoutRef<typeof RadioGroup>>) {
  const entity = useStore($recordRawApiEntity)

  return (
    <RadioGroup
      {...props}
      onChange={$recordRawApiEntity.set}
      options={options}
      size={size}
      value={entity}
      useDefaultButton={true}
    />
  )
}
