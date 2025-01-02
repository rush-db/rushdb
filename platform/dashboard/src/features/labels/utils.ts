import type { VariantProps } from 'class-variance-authority'

import { persistentMap } from '@nanostores/persistent'

import type { filterLabel } from '~/features/labels/components/FilterLabel'

import { variants } from '~/features/labels/components/FilterLabel'
import { getFromIndex } from '~/lib/utils'

import { UNLABELED } from './constants'

type LabelColor = NonNullable<VariantProps<typeof filterLabel>['variant']>

const $assignedColors = persistentMap<Record<string, LabelColor>>('label:colors', {
  [UNLABELED]: 'blank'
})

export const getLabelColor = (label: string, idx: number): LabelColor => {
  if (label === UNLABELED || !label) {
    return 'blank'
  }

  const assignedColors = $assignedColors.get()

  if (label in assignedColors) {
    return assignedColors[label] as LabelColor
  }

  // assign new color
  const array = Object.keys(variants).filter((v) => v !== 'blank') as Array<LabelColor>
  const color = getFromIndex(array, idx, 0)
  $assignedColors.setKey(label, color)
  return color
}
