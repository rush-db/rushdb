import type {
  PROPERTY_TYPE_BOOLEAN,
  PROPERTY_TYPE_DATETIME,
  PROPERTY_TYPE_NULL,
  PROPERTY_TYPE_NUMBER,
  PROPERTY_TYPE_STRING,
  PROPERTY_TYPE_VECTOR
} from '../common/constants.js'
import type { MaybeArray } from './utils.js'
import type { OrderDirection } from './query.js'

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
  | typeof PROPERTY_TYPE_VECTOR

type WithId<T> = T & { id: string }
type WithValue<T> = T & { value: PropertyValue }

export type Property = WithId<{
  metadata?: string
  name: string
  type: PropertyType
}>

export type PropertyWithValue = WithValue<Property>

export type PropertyValuesData = {
  max?: number
  min?: number
  values: Array<PropertyValue>
}

export type PropertyDraft = Omit<PropertyWithValue, 'id'> & {
  valueSeparator?: string
}

export type PropertyValuesOptions = { sort?: OrderDirection; skip?: number; limit?: number; query?: string }

export type PropertySingleValue<TType extends PropertyType = PropertyType> =
  TType extends typeof PROPERTY_TYPE_DATETIME ? DatetimeValue
  : TType extends typeof PROPERTY_TYPE_NUMBER ? NumberValue
  : TType extends typeof PROPERTY_TYPE_STRING ? StringValue
  : TType extends typeof PROPERTY_TYPE_NULL ? NullValue
  : TType extends typeof PROPERTY_TYPE_BOOLEAN ? BooleanValue
  : TType extends typeof PROPERTY_TYPE_VECTOR ? Array<number>
  : StringValue

export type PropertyValue<TType extends PropertyType = PropertyType> =
  TType extends typeof PROPERTY_TYPE_VECTOR ? Array<number> : MaybeArray<PropertySingleValue<TType>>
