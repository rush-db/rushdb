import { isArray } from '@/common/utils/isArray'
import { isObject } from '@/common/utils/isObject'
import { toBoolean } from '@/common/utils/toBolean'
import { ROOT_RECORD_ALIAS } from '@/core/common/constants'
import { Where } from '@/core/common/types'
import { ParseContext } from '@/core/search/parser/types'
import { isCycleOperatorKey, splitCriteria, wrapInParentheses } from '@/core/search/parser/utils'
import { TSearchQueryBuilderOptions } from '@/core/search/search.types'

const parseCurrentLevel = (input: Where, options?: TSearchQueryBuilderOptions, ctx?: ParseContext) => {
  if (isArray(input)) {
    const condition = input
      .map((condition, index) => {
        // @TODO: Refactor this
        // @FYI: For some scenarios we need to check Nodes with `IS NOT NULL` clause along
        // with Nodes' property criteria that was combined with logical operators.
        //
        // Example query
        // $or: [
        //    {
        //        CAR: {
        //            color: 'red',
        //        },
        //    },
        //    {
        //        SPOUSE: {
        //            gender: 'male',
        //        },
        //    },
        //    {
        //        title: {
        //            $ne: 'Forest',
        //        },
        //    },
        // ],
        // This approach allows us to do smth like this:
        // `WHERE record IS NOT NULL AND (record1 IS NOT NULL OR record2 IS NOT NULL OR (any(value IN record.title WHERE value <> \"Forest\")))`
        // Instead of this:
        // `WHERE record IS NOT NULL AND (record1 IS NOT NULL OR record2 IS NOT NULL)`
        // SO THIS {record.title <> 'Forest'} clause will be checked lately along with existence of record1 OR record2

        const nodeAlias = ROOT_RECORD_ALIAS + index
        const nodeAliasAtCurrentLevel = ROOT_RECORD_ALIAS + ctx.level
        const withQueryQueuePart =
          ctx.withQueryQueue[nodeAlias] ?? ctx.withQueryQueue[nodeAliasAtCurrentLevel] ?? []

        if (ctx.withQueryQueue[nodeAlias]) {
          ctx.withQueryQueue[nodeAlias] = []
        }

        if (ctx.withQueryQueue[nodeAliasAtCurrentLevel]) {
          ctx.withQueryQueue[nodeAliasAtCurrentLevel] = []
        }

        return [...parseCurrentLevel(condition, options, ctx), ...withQueryQueuePart]
      })
      .flat()
      .filter(toBoolean)

    return condition.join(options.joinOperator ? ` ${options.joinOperator} ` : ' AND ')
  }
  //
  else if (isObject(input)) {
    const { currentLevel, subQueries } = splitCriteria(input)
    // SUB QUERY PROCESSING
    if (toBoolean(subQueries)) {
      return Object.entries(subQueries).map(([k, value]) => parseSubQuery(k, value, options, ctx))
    } else {
      return Object.entries(currentLevel).map(([k, v]) => {
        switch (k) {
          case '$or': {
            const condition = parseCurrentLevel(
              v as Where,
              {
                ...options,
                joinOperator: 'OR'
              },
              ctx
            )

            if (condition) {
              return wrapInParentheses(isArray(condition) ? condition.join(' OR ') : condition)
            }
            break
          }
          case '$xor': {
            const condition = parseCurrentLevel(
              v as Where,
              {
                ...options,
                joinOperator: 'XOR'
              },
              ctx
            )

            if (condition) {
              return wrapInParentheses(isArray(condition) ? condition.join(' XOR ') : condition)
            }
            break
          }
          case '$nor': {
            const condition = parseCurrentLevel(
              v as Where,
              {
                ...options,
                joinOperator: 'OR'
              },
              ctx
            )
            if (condition) {
              return wrapInParentheses(
                `NOT${wrapInParentheses(isArray(condition) ? condition.join(' OR ') : condition)}`
              )
            }
            break
          }
          case '$not': {
            const condition = parseCurrentLevel(v as Where, options, ctx)

            if (condition) {
              return wrapInParentheses(
                `NOT${wrapInParentheses(isArray(condition) ? condition.join(' AND ') : condition)}`
              )
            }
            break
          }
          case '$and': {
            const conditionRaw = parseCurrentLevel(
              v as Where,
              {
                ...options,
                joinOperator: 'AND'
              },
              ctx
            )

            // Array input (e.g. $and: [{...}, {...}]) makes parseCurrentLevel return a
            // pre-joined string — pass it through like the other operators do, instead
            // of dropping it via a filter() that only exists on arrays.
            const condition = isArray(conditionRaw) ? conditionRaw.filter(toBoolean) : conditionRaw

            if (isArray(condition) ? condition.length : toBoolean(condition)) {
              return wrapInParentheses(isArray(condition) ? condition.join(' AND ') : condition)
            }
            break
          }
          default: {
            break
          }
        }
      })
    }
  }
}

const parseSubQuery = (key: string, input: any, options?: TSearchQueryBuilderOptions, ctx?: ParseContext) => {
  ctx.level += 1

  if (isCycleOperatorKey(key)) {
    // $cycle binds no endpoint node — Pass 1 compiled it to an EXISTS subquery
    // predicate (short-circuits at the first cycle found per record) stored by level.
    // No recursion: the operator's value is a traversal spec, so level lockstep holds.
    return ctx.cycleExistsByLevel[ctx.level]
  }

  const { $relation, $alias, ...other } = input as any

  const nodeAlias = ROOT_RECORD_ALIAS + ctx.level

  const result = parseCurrentLevel(other, options, ctx)

  let subtree = [`${nodeAlias} IS NOT NULL`, ...result.filter(toBoolean)].join(' AND ')

  if (isArray(result) && result.filter(toBoolean).length) {
    subtree = wrapInParentheses(subtree)
  }

  return [subtree].filter(toBoolean).join(options.joinOperator ? ` ${options.joinOperator} ` : ' AND ')
}

export const buildWhereClause = (input: any, options?: TSearchQueryBuilderOptions, ctx?: ParseContext) => {
  ctx.level = 0
  const firstLevelClause = 'record IS NOT NULL'
  const relatedRecordsClause = parseCurrentLevel(input, options, ctx)

  const hasRelatedRecordClause =
    isArray(relatedRecordsClause) ?
      toBoolean(relatedRecordsClause.filter(toBoolean))
    : toBoolean(relatedRecordsClause)

  const relatedClause =
    hasRelatedRecordClause ?
      isArray(relatedRecordsClause) ? ' AND ' + relatedRecordsClause.filter(toBoolean).join(' AND ')
      : ' AND ' + relatedRecordsClause
    : ''

  return relatedClause ? firstLevelClause + relatedClause : ''
}
