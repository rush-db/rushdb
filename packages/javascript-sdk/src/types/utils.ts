import type { PropertyValue } from './value'

export type RequireAtLeastOne<T> = {
  [K in keyof T]-?: Required<Pick<T, K>> & Partial<Pick<T, Exclude<keyof T, K>>>
}[keyof T]

export type MaybeArray<T> = Array<T> | T

export type FlattenTypes<T> = T extends object ? { [K in keyof T]: FlattenTypes<T[K]> } : T

export type AnyObject = Record<string, any>

export type FlatObject = Record<PropertyKey, PropertyValue>
