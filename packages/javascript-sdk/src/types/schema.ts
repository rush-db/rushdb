import type { FlattenTypes } from './utils.js'
import type { DatetimeObject, PropertyType, PropertyValue } from './value.js'

export type SchemaDefaultValue<T extends PropertyType = PropertyType> =
  | PropertyValue<T>
  | (() => PropertyValue<T>)
  | (() => Promise<PropertyValue<T>>)

export type SchemaField<T extends PropertyType = PropertyType> = {
  default?: SchemaDefaultValue<T>
  multiple?: boolean
  required?: boolean
  type: T
  uniq?: boolean
}

export type Schema = Record<string, SchemaField>

// Typings for writing ops (create/update)
type TypeMappingWrite = {
  boolean: boolean
  datetime: DatetimeObject | string
  null: null
  number: number
  string: string
  vector: Array<number>
}

export type OptionalKeysWrite<S extends Schema = Schema> = {
  [Key in keyof S]: S[Key]['required'] extends false ? Key
  : S[Key]['default'] extends SchemaDefaultValue ? Key
  : never
}[keyof S]

export type RequiredKeysWrite<S extends Schema = Schema> = {
  [Key in keyof S]: S[Key]['required'] extends false ? never
  : S[Key]['default'] extends SchemaDefaultValue ? never
  : Key
}[keyof S]

export type InferSchemaTypesWrite<S extends Schema = Schema> = FlattenTypes<
  {
    [Key in RequiredKeysWrite<S>]: S[Key]['multiple'] extends true ? Array<TypeMappingWrite[S[Key]['type']]>
    : TypeMappingWrite[S[Key]['type']]
  } & {
    [Key in OptionalKeysWrite<S>]?: S[Key]['multiple'] extends true ? Array<TypeMappingWrite[S[Key]['type']]>
    : TypeMappingWrite[S[Key]['type']]
  }
>

// Typings for read ops (find/findById/findOne)
type TypeMappingRead = {
  boolean: boolean
  datetime: string
  null: null
  number: number
  string: string
  vector: Array<number>
}

export type OptionalKeysRead<S extends Schema = Schema> = {
  [Key in keyof S]: S[Key]['required'] extends false ? Key : never
}[keyof S]

export type RequiredKeysRead<S extends Schema = Schema> = {
  [Key in keyof S]: S[Key]['required'] extends false ? never : Key
}[keyof S]

export type InferSchemaTypesRead<S extends Schema = Schema> = FlattenTypes<
  {
    [Key in RequiredKeysRead<S>]: S[Key]['multiple'] extends true ? Array<TypeMappingRead[S[Key]['type']]>
    : TypeMappingRead[S[Key]['type']]
  } & {
    [Key in OptionalKeysRead<S>]?: S[Key]['multiple'] extends true ? Array<TypeMappingRead[S[Key]['type']]>
    : TypeMappingRead[S[Key]['type']]
  }
>
