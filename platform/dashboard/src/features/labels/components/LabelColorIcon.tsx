import { UNLABELED } from '../constants'
import { getLabelColor } from '../utils'
import { filterLabel } from './FilterLabel'

export function LabelColorIcon({ label = UNLABELED, idx = 0 }: { idx?: number; label: string }) {
  return (
    <div
      className={filterLabel({
        variant: getLabelColor(label, idx),
        size: 'circle'
      })}
      aria-hidden="true"
    />
  )
}
