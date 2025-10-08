import { AliasesMap } from '@/core/common/types'

export type ParseContext = {
  nodeAliases: string[]
  level: number
  result: Record<string, string>
  aliasesMap: AliasesMap
  withQueryQueue?: Record<string, any>
}

export type AggregateContext = {
  fieldsInCollect: string[]
  withAggregations: string[]
  orderClauses: string[]
  alias?: string
  returnAlias?: string
  groupBy?: string[]
}
