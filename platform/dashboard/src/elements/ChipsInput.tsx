import type { KeyboardEvent } from 'react'
import type { PropertyType } from '@rushdb/javascript-sdk'

import { useState } from 'react'
import { X } from 'lucide-react'

import { Badge } from '~/elements/Badge'
import { Input } from '~/elements/Input'

/**
 * Controlled multi-value ("chips") editor. RushDB has no distinct array property type — a value is just
 * a single value or an array of single values — so this is used whenever the edited value is already an
 * array, to preserve its structure instead of flattening it to a comma-joined string.
 */
export function ChipsInput({
  type,
  values,
  onChange
}: {
  type: PropertyType
  values: string[]
  onChange: (values: string[]) => void
}) {
  const [draft, setDraft] = useState('')

  const commit = () => {
    const trimmed = draft.trim()
    if (trimmed === '') return
    onChange([...values, trimmed])
    setDraft('')
  }

  const removeAt = (index: number) => {
    onChange(values.filter((_, i) => i !== index))
  }

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    // Enter (and comma for non-numbers) commits the draft; Backspace on an empty draft removes the last chip.
    if (event.key === 'Enter' || (type !== 'number' && event.key === ',')) {
      event.preventDefault()
      commit()
    } else if (event.key === 'Backspace' && draft === '' && values.length > 0) {
      removeAt(values.length - 1)
    }
  }

  return (
    <div className="flex flex-col gap-2">
      {values.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {values.map((value, i) => (
            <Badge key={`${value}-${i}`} className="h-auto py-1 pr-1.5 text-base">
              <span className="max-w-[220px] truncate">{value}</span>
              <button
                type="button"
                aria-label={`remove ${value}`}
                onClick={() => removeAt(i)}
                className="opacity-60 transition hover:opacity-100"
              >
                <X size={14} />
              </button>
            </Badge>
          ))}
        </div>
      )}
      <Input
        type={type === 'number' ? 'number' : 'text'}
        step={type === 'number' ? 'any' : undefined}
        size="small"
        className="text-base"
        value={draft}
        placeholder="Add value and press Enter"
        onChange={(event) => setDraft(event.target.value)}
        onKeyDown={handleKeyDown}
        onBlur={commit}
      />
    </div>
  )
}
