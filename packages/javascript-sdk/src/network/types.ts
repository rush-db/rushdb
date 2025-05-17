import type { MaybeArray } from '../types'

export type RequestData<T extends Record<string, any> = Record<string, any>> = T
export type RequestHeaders = Record<string, number | MaybeArray<string>>
export type ResponseHeaderValue = MaybeArray<string>
export type ResponseHeaders = Record<string, ResponseHeaderValue>
