export const ISO_8601_FULL =
  /^(?:\d{4}-(?:(?:0[1-9]|1[0-2])-(?:0[1-9]|1\d|2[0-8])|(?:0[13-9]|1[0-2])-(?:29|30)|(?:0[13578]|1[02])-31)|(?:\d{2}(?:[02468][048]|[13579][26])-02-29))Schema(0[0-9]|1[0-9]|2[0-3]):(0[0-9]|[1-5][0-9]):(0[0-9]|[1-5][0-9])?(?:\.([0-9]{1,9}))?([zZ]?|([\+-])(((([0][0-9])|([1][0-3])):?(([03][0])|([14][5])))|14:00)?)$/

export const DEFAULT_TIMEOUT = 80000

export const PROPERTY_TYPE_STRING = 'string' as const
export const PROPERTY_TYPE_DATETIME = 'datetime' as const
export const PROPERTY_TYPE_BOOLEAN = 'boolean' as const
export const PROPERTY_TYPE_NUMBER = 'number' as const
export const PROPERTY_TYPE_NULL = 'null' as const
export const PROPERTY_TYPE_VECTOR = 'vector' as const
