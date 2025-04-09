export type ApiResponse<T, E = Record<string, any>> = {
  data: T
  success: boolean
  total?: number
} & E
