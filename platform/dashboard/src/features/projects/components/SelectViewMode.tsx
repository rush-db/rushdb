import type { ComponentPropsWithoutRef, ReactNode } from 'react'

import { useStore } from '@nanostores/react'
import { Code2, TableIcon } from 'lucide-react'

import { RadioGroup } from '~/elements/RadioGroup'
import { Tooltip } from '~/elements/Tooltip'

import { $recordView } from '../stores/current-project'
import type { RecordViewType } from '~/features/projects/types.ts'
import { GraphIcon } from '~/elements/GraphIcon.tsx'
import { $tourStep, setTourStep } from '~/features/tour/stores/tour.ts'
import { $editorData, onboardingAgentRunSelectQuery } from '~/features/projects/stores/raw-api.ts'

const options = [
  {
    icon: (
      <Tooltip alignOffset={20} className="text-content2" sideOffset={10} trigger={<TableIcon />}>
        <div>
          <div className="text-2xs text-content flex items-center gap-1 uppercase">Table mode</div>
        </div>
      </Tooltip>
    ),
    value: 'table'
  },
  {
    icon: (
      <Tooltip alignOffset={20} className="text-content2" sideOffset={10} trigger={<GraphIcon />}>
        <div>
          <div className="text-2xs text-content flex items-center gap-1 uppercase">Graph mode</div>
        </div>
      </Tooltip>
    ),
    value: 'graph'
  },
  {
    icon: (
      <Tooltip alignOffset={20} className="text-content2" sideOffset={10} trigger={<Code2 />}>
        <div>
          <div className="text-2xs text-content flex items-center gap-1 uppercase">Raw API mode</div>
        </div>
      </Tooltip>
    ),
    value: 'raw-api'
  }
] as Array<{
  icon: ReactNode
  value: RecordViewType
}>

export function SelectViewMode({
  size = 'small',
  ...props
}: Partial<ComponentPropsWithoutRef<typeof RadioGroup>>) {
  const view = useStore($recordView)
  const tourStep = useStore($tourStep)

  const handleChange = (nextView: RecordViewType) => {
    $recordView.set(nextView)
    if (tourStep === 'recordRawApiMode' && nextView === 'raw-api') {
      $editorData.set(onboardingAgentRunSelectQuery)
      setTourStep('rawApiSelectQuery', true)
    }
  }

  return (
    <RadioGroup
      data-tour="records-table-view-mode"
      {...props}
      onChange={handleChange}
      options={options}
      size={size}
      value={view}
      className="w-fit"
    />
  )
}
