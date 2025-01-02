export type TGetFirstArgument<T> =
  T extends (first: infer FirstArgument, ...args: any[]) => any ? FirstArgument : never

export type TAnyObject = Record<string, any>

export type TFlattenTypes<T> = T extends object ? { [K in keyof T]: TFlattenTypes<T[K]> } : T

export type TUnionToIntersection<U> =
  (U extends unknown ? (k: U) => void : never) extends (k: infer I) => void ? I : never

export type TIsUnion<T> = [T] extends [TUnionToIntersection<T>] ? false : true

export type TReplace<T, K extends keyof T, R> =
  TIsUnion<K> extends false ? TFlattenTypes<Omit<T, K> & { [key in T as K]: R }> : never
