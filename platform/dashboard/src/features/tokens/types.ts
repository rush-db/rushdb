import type { ISO8601 } from '~/types'

export type ProjectToken = {
  created: ISO8601
  description?: string
  expiration: number | string
  expired?: boolean // exists on list
  id: string
  name: string
  noExpire?: boolean
  value?: string // exists when created
}
