import {
  PROPERTY_TYPE_BOOLEAN,
  PROPERTY_TYPE_DATETIME,
  PROPERTY_TYPE_NULL,
  PROPERTY_TYPE_NUMBER,
  PROPERTY_TYPE_STRING
} from '@/core/property/property.constants'

export type TPropertyType =
  | typeof PROPERTY_TYPE_STRING
  | typeof PROPERTY_TYPE_DATETIME
  | typeof PROPERTY_TYPE_BOOLEAN
  | typeof PROPERTY_TYPE_NUMBER
  | typeof PROPERTY_TYPE_NULL

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

export type TPropertyPrimitiveValue = string | number | boolean | null
export type TPropertyDatetimeValue = DatetimeObject | string

export type TPropertySingleValue<TType extends TPropertyType = TPropertyType> =
  TType extends typeof PROPERTY_TYPE_DATETIME ? TPropertyDatetimeValue : TPropertyPrimitiveValue

export type TPropertyMultipleValue<TType extends TPropertyType = TPropertyType> = Array<
  TPropertySingleValue<TType>
>

export type TPropertyValue<TType extends TPropertyType = TPropertyType> =
  | TPropertySingleValue<TType>
  | TPropertyMultipleValue<TType>

export type TPropertyPropertiesWithValue = TPropertyProperties & {
  value: TPropertyValue
}

export type TPropertyProperties = {
  id: string
  name: string
  type: TPropertyType
  projectId: string
  metadata?: string
}
