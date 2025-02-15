import { LabelColorIcon } from './LabelColorIcon'

export function LabelName({ label, idx }: { idx: number; label: string }) {
  return (
    <div className="flex items-center gap-2">
      <LabelColorIcon idx={idx} label={label} />
      {label}
    </div>
  )
}
