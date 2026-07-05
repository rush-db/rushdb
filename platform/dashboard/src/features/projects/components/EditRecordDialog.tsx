import type { ReactNode } from 'react'
import { useState, useEffect, useCallback } from 'react'
import type { DBRecord, PropertyType, PropertySingleValue } from '@rushdb/javascript-sdk'
import { idToDate } from '@rushdb/javascript-sdk'
import { Plus, TrashIcon, X } from 'lucide-react'

import { Dialog, DialogTitle, DialogFooter } from '~/elements/Dialog'
import { Divider } from '~/elements/Divider'
import { Input, inputWrapper, input } from '~/elements/Input'
import { Switch } from '~/elements/Switch'
import { Calendar } from '~/elements/Calendar'
import { Popover, PopoverTrigger, PopoverContent } from '~/elements/Popover'
import { Select } from '~/elements/Select'
import { Button } from '~/elements/Button'
import { IconButton } from '~/elements/IconButton'
import { ChipsInput } from '~/elements/ChipsInput'
import { PropertyName } from '~/features/properties/components/PropertyName'
import { RecordTitle } from '~/features/records/components/RecordTitle'
import { collectPropertiesFromRecord } from '~/features/projects/utils'
import { useSetRecordMutation } from '~/features/records/hooks/useRecordMutations'
import { cn } from '~/lib/utils'

type EditableProp = {
  name: string
  type: PropertyType
  // RushDB has no distinct array type — array-ness is detected per value via Array.isArray on load.
  isArray: boolean
  rawValue: string // used when !isArray
  items: string[] // used when isArray
}

type NewProp = {
  name: string
  type: PropertyType
  rawValue: string
}

const PROPERTY_TYPES: PropertyType[] = ['string', 'number', 'boolean', 'datetime']

const TYPE_OPTIONS = PROPERTY_TYPES.map((t) => ({ value: t, label: t }))

function castValue(type: PropertyType, raw: string): PropertySingleValue {
  if (type === 'number') return Number(raw)
  if (type === 'boolean') return raw === 'true'
  return raw
}

function isPropValid(type: PropertyType, rawValue: string): boolean {
  if (type === 'boolean') return true
  if (type === 'number') return rawValue !== '' && !isNaN(Number(rawValue))
  return rawValue.trim() !== ''
}

function PropertyValueEditor({
  type,
  value,
  onChange
}: {
  type: PropertyType
  value: string
  onChange: (v: string) => void
}) {
  if (type === 'boolean') {
    return <Switch checked={value === 'true'} onCheckedChange={(checked) => onChange(String(checked))} />
  }

  if (type === 'datetime') {
    const date = value ? new Date(value) : undefined
    const displayValue = date && !isNaN(date.getTime()) ? date.toLocaleString() : value

    return (
      <Popover>
        <PopoverTrigger asChild>
          <div>
            <Input
              readOnly
              value={displayValue}
              placeholder="Pick a date…"
              className="cursor-pointer text-base"
            />
          </div>
        </PopoverTrigger>
        <PopoverContent align="start" className="p-3">
          <Calendar
            mode="single"
            selected={date && !isNaN(date.getTime()) ? date : undefined}
            defaultMonth={date && !isNaN(date.getTime()) ? date : undefined}
            onSelect={(d) => {
              if (d) onChange(new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate())).toISOString())
            }}
          />
        </PopoverContent>
      </Popover>
    )
  }

  if (type === 'number') {
    return (
      <Input
        type="number"
        step="any"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="text-base"
      />
    )
  }

  // string
  return (
    <label className={cn(inputWrapper(), 'h-auto')}>
      <textarea
        className={cn(input(), 'h-auto min-h-[68px] resize-none py-2 text-base')}
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
    </label>
  )
}

export function EditRecordDialog({ record, trigger }: { record: DBRecord; trigger: ReactNode }) {
  const [open, setOpen] = useState(false)
  const [properties, setProperties] = useState<EditableProp[]>([])
  const [newProps, setNewProps] = useState<NewProp[]>([])
  const [label, setLabel] = useState(record.__label)
  const { mutate, isPending } = useSetRecordMutation()

  useEffect(() => {
    if (!open) return
    setProperties(
      collectPropertiesFromRecord(record)
        // null/undefined means the field is unset — never editable (legacy null-typed props are skipped).
        .filter((p) => p.value !== null && p.value !== undefined)
        .map((p) => {
          const isArray = Array.isArray(p.value)
          return {
            name: p.name,
            type: p.type,
            isArray,
            rawValue:
              isArray ? ''
              : p.type === 'boolean' ? String(p.value)
              : String(p.value ?? ''),
            items: isArray ? (p.value as PropertySingleValue[]).map((v) => String(v)) : []
          }
        })
    )
    setNewProps([])
    setLabel(record.__label)
  }, [open, record])

  const updatePropValue = useCallback((index: number, rawValue: string) => {
    setProperties((prev) => prev.map((p, i) => (i === index ? { ...p, rawValue } : p)))
  }, [])

  const updatePropItems = useCallback((index: number, items: string[]) => {
    setProperties((prev) => prev.map((p, i) => (i === index ? { ...p, items } : p)))
  }, [])

  const removeProp = useCallback((index: number) => {
    setProperties((prev) => prev.filter((_, i) => i !== index))
  }, [])

  const isValid =
    label.trim() !== '' &&
    properties.every((p) =>
      p.isArray ? p.items.every((item) => isPropValid(p.type, item)) : isPropValid(p.type, p.rawValue)
    ) &&
    newProps.every((p) => p.name.trim() !== '' && isPropValid(p.type, p.rawValue))

  const handleSave = () => {
    const existing = properties.map((p) => ({
      name: p.name,
      type: p.type,
      value: p.isArray ? p.items.map((item) => castValue(p.type, item)) : castValue(p.type, p.rawValue)
    }))

    const additions = newProps.map((p) => ({
      name: p.name.trim(),
      type: p.type,
      value: castValue(p.type, p.rawValue)
    }))

    mutate(
      { id: record.__id, label: label.trim(), data: [...existing, ...additions] },
      { onSuccess: () => setOpen(false) }
    )
  }

  return (
    <Dialog open={open} onOpenChange={setOpen} trigger={trigger} loading={isPending}>
      <DialogTitle>Edit Record</DialogTitle>

      <div className="border-stroke-tertiary border-b pt-1 pb-3">
        <RecordTitle id={record.__id} label={record.__label} createdAt={idToDate(record.__id)} />
      </div>

      <div className="flex max-h-[60vh] flex-col gap-4 overflow-y-auto py-2">
        <div className="flex flex-col gap-1.5">
          <span className="text-sm font-medium">Label</span>
          <Input value={label} onChange={(e) => setLabel(e.target.value)} placeholder="Record label" />
        </div>

        <Divider />

        {properties.map((prop, i) => (
          <div key={prop.name} className="flex flex-col gap-1.5">
            <div className="flex items-center justify-between gap-1 text-sm">
              <PropertyName
                className="font-mono text-xs font-medium text-content2"
                name={prop.name}
                type={prop.type}
              />
              <IconButton
                aria-label={`delete ${prop.name}`}
                variant="ghost"
                size="small"
                onClick={() => removeProp(i)}
              >
                <TrashIcon size={14} />
              </IconButton>
            </div>
            {prop.isArray ?
              <ChipsInput
                type={prop.type}
                values={prop.items}
                onChange={(items) => updatePropItems(i, items)}
              />
            : <PropertyValueEditor
                type={prop.type}
                value={prop.rawValue}
                onChange={(v) => updatePropValue(i, v)}
              />
            }
          </div>
        ))}

        {newProps.map((prop, i) => (
          <div key={i} className="flex flex-col gap-3 rounded-md border p-3">
            <div className="flex items-center gap-2">
              <Input
                size="small"
                placeholder="Property name"
                value={prop.name}
                onChange={(e) =>
                  setNewProps((prev) => prev.map((p, j) => (j === i ? { ...p, name: e.target.value } : p)))
                }
                className="flex-1"
              />
              <Select
                size="small"
                options={TYPE_OPTIONS}
                value={prop.type}
                onChange={(e) =>
                  setNewProps((prev) =>
                    prev.map((p, j) =>
                      j === i ?
                        {
                          ...p,
                          type: e.target.value as PropertyType,
                          rawValue: e.target.value === 'boolean' ? 'false' : ''
                        }
                      : p
                    )
                  )
                }
                className="w-[130px]"
              />
              <IconButton
                aria-label="cancel add property"
                variant="ghost"
                size="small"
                onClick={() => setNewProps((prev) => prev.filter((_, j) => j !== i))}
              >
                <X />
              </IconButton>
            </div>
            <PropertyValueEditor
              type={prop.type}
              value={prop.rawValue}
              onChange={(v) =>
                setNewProps((prev) => prev.map((p, j) => (j === i ? { ...p, rawValue: v } : p)))
              }
            />
          </div>
        ))}

        <Button
          variant="ghost"
          size="small"
          className="self-start"
          onClick={() => setNewProps((prev) => [...prev, { type: 'string', name: '', rawValue: '' }])}
        >
          <Plus />
          Add Property
        </Button>
      </div>

      <DialogFooter className="mt-4 flex gap-4">
        <Button variant="outline" onClick={() => setOpen(false)}>
          Cancel
        </Button>
        <Button variant="primary" disabled={!isValid || isPending} onClick={handleSave}>
          Save
        </Button>
      </DialogFooter>
    </Dialog>
  )
}
