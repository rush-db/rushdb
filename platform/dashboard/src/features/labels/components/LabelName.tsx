import { UNLABELED } from '../constants'
import { LabelColorIcon } from './LabelColorIcon'

export function LabelName({ label, idx }: { idx: number; label: string }) {
  const text = label === UNLABELED ? 'Unlabeled (NULL)' : label

  return (
    <div className="flex items-center gap-2">
      <LabelColorIcon idx={idx} label={label} />
      {text}
    </div>
  )
}
