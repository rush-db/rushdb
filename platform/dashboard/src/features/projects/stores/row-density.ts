import { persistentAtom } from '@nanostores/persistent'

export type RowDensity = 'normal' | 'compact'

export const $rowDensity = persistentAtom<RowDensity>('records:row-density', 'normal')
