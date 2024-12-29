import { Check, Plus, SlidersHorizontal, X } from 'lucide-react'

import { Button } from '~/elements/Button'
import { ButtonGroup } from '~/elements/ButtonGroup'
import { CommandShortcut } from '~/elements/Command'
import { IconButton } from '~/elements/IconButton'
import { MenuIcon } from '~/elements/Menu'
import { SearchSelect, SelectItem } from '~/elements/SearchSelect'
import { Skeleton } from '~/elements/Skeleton'
import { numberCompact } from '~/lib/formatters'
import { addOrRemove, cn } from '~/lib/utils'

import type { LabelsResponse } from '../types'

import { FilterLabel } from './FilterLabel'
import { LabelColorIcon } from './LabelColorIcon'
import { LabelName } from './LabelName'

export function SelectLabels({
  loading,
  labels,
  activeLabels,
  onSelect
}: {
  activeLabels: Array<string>
  labels: LabelsResponse | undefined
  loading?: boolean
  onSelect: (value: Array<string>) => void
}) {
  const empty = activeLabels.length <= 0

  return (
    <ButtonGroup>
      <SearchSelect
        trigger={
          loading && !labels ? (
            <Skeleton enabled>
              <FilterLabel>Loading</FilterLabel>
            </Skeleton>
          ) : (
            <Button
              className={cn({
                'text-content3': empty
              })}
              size="small"
              variant="outline"
            >
              {empty && (
                <>
                  <SlidersHorizontal /> Labels
                </>
              )}

              {!empty && (
                <div className="flex gap-4">
                  {activeLabels?.length > 1 ? (
                    <div className="flex items-center gap-2">
                      <div className="flex items-center">
                        {activeLabels.map((label, idx) => (
                          <LabelColorIcon idx={idx} key={label} label={label} />
                        ))}
                      </div>
                      Labels
                      <CommandShortcut>{activeLabels.length}</CommandShortcut>
                    </div>
                  ) : (
                    activeLabels?.map((label, idx) => {
                      if (!label) return null

                      return <LabelName idx={idx} key={label} label={label} />
                    })
                  )}
                </div>
              )}
              {empty && <MenuIcon />}
            </Button>
          )
        }
      >
        {Object.entries(labels ?? {})?.map(([label, quantity], idx) => {
          const active = activeLabels?.includes(label)
          return (
            <SelectItem
              closeOnSelect={false}
              key={label}
              onSelect={() => onSelect(addOrRemove(activeLabels, label))}
            >
              <Check
                className={cn(
                  'text-content2',
                  active ? 'opacity-100' : 'opacity-0'
                )}
              />
              <LabelName idx={idx} label={label} />
              <CommandShortcut>
                {numberCompact.format(quantity)}
              </CommandShortcut>
            </SelectItem>
          )
        })}
      </SearchSelect>

      {!empty && (
        <IconButton
          aria-label="Reset labels"
          onClick={() => onSelect([])}
          size="small"
          variant="outline"
        >
          <X />
        </IconButton>
      )}
    </ButtonGroup>
  )
}
