import { AliasesMap } from '@/core/common/types'

export type ParseContext = {
  nodeAliases: string[]
  level: number
  result: Record<string, string>
  aliasesMap: AliasesMap
  withQueryQueue?: Record<string, any>
  /** EXISTS predicates compiled from $cycle blocks in Pass 1, keyed by traversal level
   *  so Pass 2 (buildWhereClause), which walks levels in lockstep, can inline them. */
  cycleExistsByLevel: Record<number, string>
}

export type AggregateContext = {
  fieldsInCollect: string[]
  withAggregations: string[]
  orderClauses: string[]
  alias?: string
  returnAlias?: string
  groupBy?: string[]
}
