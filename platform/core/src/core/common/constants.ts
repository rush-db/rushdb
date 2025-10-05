// Internal property keys
export const RUSHDB_KEY_ID = '__RUSHDB__KEY__ID__' as const
export const RUSHDB_KEY_PROJECT_ID = '__RUSHDB__KEY__PROJECT__ID__' as const
export const RUSHDB_KEY_LABEL = '__RUSHDB__KEY__LABEL__' as const
// Dynamically calculated property
export const RUSHDB_KEY_SIMILARITY = '__RUSHDB__KEY__RUSHDB_KEY_SIMILARITY__' as const

export const RUSHDB_KEY_PROPERTIES_META = '__RUSHDB__KEY__PROPERTIES__META__' as const

// Internal property values
export const RUSHDB_VALUE_NULL = '__RUSHDB__VALUE__NULL__' as const
export const RUSHDB_VALUE_EMPTY_ARRAY = '__RUSHDB__VALUE__EMPTY__ARRAY__' as const

// Built-in relation labels
export const RUSHDB_RELATION_DEFAULT = '__RUSHDB__RELATION__DEFAULT__' as const
export const RUSHDB_RELATION_VALUE = '__RUSHDB__RELATION__VALUE__' as const

// RushDB core labels
export const RUSHDB_LABEL_RECORD = '__RUSHDB__LABEL__RECORD__' as const
export const RUSHDB_LABEL_PROPERTY = '__RUSHDB__LABEL__PROPERTY__' as const

// Property Aliases
export const RUSHDB_KEY_ID_ALIAS = '__id' as const
export const RUSHDB_KEY_PROJECT_ID_ALIAS = '__projectId' as const
export const RUSHDB_KEY_PROPERTIES_META_ALIAS = '__proptypes' as const
export const RUSHDB_KEY_LABEL_ALIAS = '__label' as const

export const RUSHDB_INTERNALS_ALIASES = {
  [RUSHDB_KEY_ID]: RUSHDB_KEY_ID_ALIAS,
  [RUSHDB_KEY_PROJECT_ID]: RUSHDB_KEY_PROJECT_ID_ALIAS,
  [RUSHDB_KEY_PROPERTIES_META]: RUSHDB_KEY_PROPERTIES_META_ALIAS,
  [RUSHDB_KEY_LABEL]: RUSHDB_KEY_LABEL_ALIAS,
  [RUSHDB_RELATION_DEFAULT]: 'RUSHDB_DEFAULT_RELATION',
  [RUSHDB_VALUE_NULL]: null,
  [RUSHDB_LABEL_RECORD]: null,
  [RUSHDB_VALUE_EMPTY_ARRAY]: []
} as const

export const ISO_8601_REGEX =
  /^(?:\d{4}-(?:(?:0[1-9]|1[0-2])-(?:0[1-9]|1\d|2[0-8])|(?:0[13-9]|1[0-2])-(?:29|30)|(?:0[13578]|1[02])-31)|(?:\d{2}(?:[02468][048]|[13579][26])-02-29))T(0[0-9]|1[0-9]|2[0-3]):(0[0-9]|[1-5][0-9]):(0[0-9]|[1-5][0-9])?(?:\.([0-9]{1,9}))?([zZ]?|([\+-])(((([0][0-9])|([1][0-3])):?(([03][0])|([14][5])))|14:00)?)$/
export const NUMERIC_REGEX = /^\d+\.\d+$|^\d+$/

export const ROOT_RECORD_ALIAS = 'record'
