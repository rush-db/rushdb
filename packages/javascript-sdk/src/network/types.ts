export type RequestData<T extends Record<string, any> = Record<string, any>> = T
export type RequestHeaders = Record<string, number | string | string[]>
export type ResponseHeaderValue = string | string[]
export type ResponseHeaders = Record<string, ResponseHeaderValue>
