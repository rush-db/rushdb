import type {
  PROPERTY_TYPE_BOOLEAN,
  PROPERTY_TYPE_DATETIME,
  PROPERTY_TYPE_NULL,
  PROPERTY_TYPE_NUMBER,
  PROPERTY_TYPE_STRING
} from '../common/constants.js'
import type { MaybeArray } from './utils.js'

// DATETIME
export type DatetimeObject = {
  $day?: number
  $hour?: number
  $microsecond?: number
  $millisecond?: number
  $minute?: number
  $month?: number
  $nanosecond?: number
  $second?: number
  $year: number
}
export type DatetimeValue = DatetimeObject | string

// BOOLEAN
export type BooleanValue = boolean

// NULL
export type NullValue = null

// NUMBER
export type NumberValue = number

// STRING
export type StringValue = string

export type PropertyType =
  | typeof PROPERTY_TYPE_BOOLEAN
  | typeof PROPERTY_TYPE_DATETIME
  | typeof PROPERTY_TYPE_NULL
  | typeof PROPERTY_TYPE_NUMBER
  | typeof PROPERTY_TYPE_STRING

export type Property = {
  id: string
  metadata?: string
  name: string
  type: PropertyType
}

export type PropertyWithValue = Property & {
  value: PropertyValue
}

export type PropertyValuesData = {
  max?: number
  min?: number
  values: PropertyValue[]
}

export type PropertySingleValue<TType extends PropertyType = PropertyType> =
  TType extends typeof PROPERTY_TYPE_DATETIME ? DatetimeValue
  : TType extends typeof PROPERTY_TYPE_NUMBER ? NumberValue
  : TType extends typeof PROPERTY_TYPE_STRING ? StringValue
  : TType extends typeof PROPERTY_TYPE_NULL ? NullValue
  : TType extends typeof PROPERTY_TYPE_BOOLEAN ? BooleanValue
  : StringValue

export type PropertyValue<TType extends PropertyType = PropertyType> = MaybeArray<PropertySingleValue<TType>>
