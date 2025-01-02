export type DateObject = {
  day: number
  hour: number
  minute: number
  month: number
  nanosecond: number
  second: number
  year: number
}

/**
 * Make a type assembled from several types/utilities more readable.
 * (e.g. the type will be shown as the final resulting type instead of as a bunch of type utils wrapping the initial type).
 * https://stackoverflow.com/questions/51599481/replacing-property-of-a-typescript-type
 */
type FinalType<T> = T extends infer U ? { [K in keyof U]: U[K] } : never

/**
 * Merge keys of U into T, overriding value types with those in U.
 * https://stackoverflow.com/questions/51599481/replacing-property-of-a-typescript-type
 */
export type Override<T, U extends Partial<Record<keyof T, unknown>>> = FinalType<Omit<T, keyof U> & U>

export type ISO8601 = `${number}-${number}-${number}T${number}:${number}:${number}` | ({} & string)

export type GenericApiResponse<Data> = {
  data: Data
  success: boolean
  total?: number
}

export type AnyObject = Record<string, any>

export type AnyFunction = (...args: any[]) => any

export type DeepPartial<T> =
  T extends IObject ?
    {
      [P in keyof T]?: DeepPartial<T[P]>
    }
  : T

export type DeepRequired<T> =
  T extends IObject ?
    {
      [P in keyof T]-?: DeepRequired<T[P]>
    }
  : T

export type DeepNonNullable<T> =
  T extends IObject ?
    {
      [P in keyof T]-?: DeepNonNullable<T[P]>
    }
  : NonNullable<T>

export const isAnyObject = (input: unknown): input is AnyObject =>
  typeof input === 'object' && input !== null && !Array.isArray(input)

export type BatchActionSelection = Array<string>

export type SortDirection = 'asc' | 'desc'
export type SortMap = Partial<Record<string, SortDirection>>
export type Sort = SortDirection | SortMap | undefined

export type FiltersCombineMode = 'and' | 'or'

export type WithoutId<T extends AnyObject> = Omit<T, 'id'> & {
  id?: never
}
