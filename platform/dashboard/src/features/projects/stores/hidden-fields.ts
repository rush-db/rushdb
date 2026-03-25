import type { Property } from '@rushdb/javascript-sdk'

import { persistentAtom } from '@nanostores/persistent'
import { action, computed } from 'nanostores'

import { toast } from '~/elements/Toast'
import { addOrRemove } from '~/lib/utils'

export const isFieldHidden = (columns: Array<Property['id']>, fieldId: Property['id']) =>
  columns?.includes(fieldId) ?? false

export const $hiddenFields = persistentAtom<Array<Property['id']>>('records:hidden-fields', [], {
  decode(value) {
    try {
      return JSON.parse(value)
    } catch {
      return []
    }
  },
  encode(value) {
    return JSON.stringify(value)
  }
})

export const $hasHiddenFields = computed($hiddenFields, (store) => store && store.length > 0)

export const $toggleHiddenField = action(
  $hiddenFields,
  '$toggleHiddenField',
  (store, fieldId: Property['id']) => {
    const hiddenFields = store.get()

    store.set(addOrRemove(hiddenFields, fieldId))
  }
)

export const $resetHiddenFields = action($hiddenFields, '$resetHiddenFields', (store) => {
  store.set([])
})
