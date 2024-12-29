import { AliasesMap } from '@/core/common/types'

export type ParseContext = {
  nodeAliases: string[]
  level: number
  result: Record<string, string>
  aliasesMap: AliasesMap
  withQueryQueue?: Record<string, any>
}
