import { useStore } from '@nanostores/react'
import { $currentPeriod } from '~/features/billing/stores/plans.ts'
import { Switch } from '~/elements/Switch.tsx'
import { cx } from 'class-variance-authority'
import { Label } from '~/elements/Label.tsx'

export function SelectPeriod({ className }: { className?: string }) {
  const currentPeriod = useStore($currentPeriod)

  return (
    <>
      <div className={cx('flex items-center justify-center gap-4', className)}>
        <p className="typography-base font-medium text-content2">Monthly</p>
        <Switch
          checked={currentPeriod === 'annual'}
          onCheckedChange={(checked) => $currentPeriod.set(checked ? 'annual' : 'month')}
        />
        <p className="typography-base font-medium text-content2">Annual</p>
      </div>
    </>
  )
}
