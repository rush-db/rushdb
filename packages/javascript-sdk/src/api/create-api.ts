import type { createFetcher } from '../network/index.js'
import type {
  DBRecord,
  DBRecordTarget,
  Relation,
  RelationDetachOptions,
  RelationOptions
} from '../sdk/record.js'
import type { Transaction } from '../sdk/transaction.js'
import type {
  Property,
  PropertyValuesData,
  SearchQuery,
  Schema,
  MaybeArray
} from '../types/index.js'
import type { ApiResponse } from './types.js'

import { isArray } from '../common/utils.js'
import { NonUniqueResultError } from '../sdk/errors.js'
import { DBRecordsBatchDraft, DBRecordDraft } from '../sdk/record.js'
import { buildTransactionHeader, pickRecordId, pickTransactionId } from './utils.js'

// POST /api/v1/records/:id @TODO

export const createApi = (fetcher: ReturnType<typeof createFetcher>) => ({
  labels: {
    async find<S extends Schema = any>(
      searchParams: SearchQuery<S>,
      transaction?: Transaction | string
    ) {
      const txId = pickTransactionId(transaction)

      return fetcher<ApiResponse<Record<string, number>>>(`/labels`, {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: searchParams
      })
    }
  },
  properties: {
    delete: (id: string, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)

      return fetcher<ApiResponse<Property>>(`/properties/${id}`, {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'DELETE'
      })
    },
    find: <S extends Schema = any>(
      searchParams: SearchQuery<S>,
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)

      return fetcher<ApiResponse<Property[]>>(`/properties`, {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: searchParams
      })
    },
    findById: (id: string, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)

      return fetcher<ApiResponse<Property>>(`/properties/${id}`, {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'GET'
      })
    },
    // update: () => {
    //   // @TODO
    // },
    // updateValues: (id: string, transaction?: Transaction | string) => {
    //   // @TODO
    // },
    values: (id: string, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)

      return fetcher<ApiResponse<Property & PropertyValuesData>>(
        `/properties/${id}/values`,
        {
          headers: Object.assign({}, buildTransactionHeader(txId)),
          method: 'GET'
        }
      )
    }
  },
  records: {
    attach: async (
      source: DBRecordTarget,
      idOrIds: MaybeArray<string>,
      options?: RelationOptions,
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(source)!

      return fetcher<ApiResponse<{ message: string }>>(`/records/${recordId}/relations`, {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: {
          targetIds: idOrIds,
          ...(options?.type && { type: options?.type }),
          ...(options?.direction && { direction: options?.direction })
        }
      })
    },
    async create<S extends Schema = any>(
      data: DBRecordDraft | S,
      transaction?: Transaction | string
    ): Promise<ApiResponse<DBRecord<S> | undefined>> {
      const txId = pickTransactionId(transaction)

      if (data instanceof DBRecordDraft) {
        return fetcher<ApiResponse<DBRecord<S>>>(`/records`, {
          headers: Object.assign({}, buildTransactionHeader(txId)),
          method: 'POST',
          requestData: data.toJson()
        })
      }

      return { data: undefined, success: false }
    },

    async createMany<S extends Schema = any>(
      data: DBRecordsBatchDraft | S[],
      transaction?: Transaction | string
    ): Promise<ApiResponse<DBRecord<S>[]>> {
      const txId = pickTransactionId(transaction)

      if (data instanceof DBRecordsBatchDraft) {
        return fetcher<ApiResponse<DBRecord<S>[]>>(`/records/import/json`, {
          headers: Object.assign({}, buildTransactionHeader(txId)),
          method: 'POST',
          requestData: data.toJson()
        })
      }

      return { data: [], success: false }
    },

    delete<S extends Schema = any>(
      searchParams: SearchQuery<S>,
      transaction?: Transaction | string
    ) {
      const txId = pickTransactionId(transaction)

      return fetcher<ApiResponse<{ message: string }>>(`/records/delete`, {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'PUT',
        requestData: searchParams
      })
    },

    deleteById(idOrIds: MaybeArray<string>, transaction?: Transaction | string) {
      const txId = pickTransactionId(transaction)

      if (isArray(idOrIds)) {
        return fetcher<ApiResponse<{ message: string }>>(`/records/delete`, {
          headers: Object.assign({}, buildTransactionHeader(txId)),
          method: 'PUT',
          requestData: { limit: 1000, where: { $id: { $in: idOrIds } } }
        })
      } else {
        return fetcher<ApiResponse<{ message: string }>>(`/records/${idOrIds}`, {
          headers: Object.assign({}, buildTransactionHeader(txId)),
          method: 'DELETE'
        })
      }
    },

    detach: async (
      source: DBRecordTarget,
      idOrIds: MaybeArray<string>,
      options?: RelationDetachOptions,
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(source)!

      return fetcher<ApiResponse<{ message: string }>>(`/records/${recordId}/relations`, {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'PUT',
        requestData: {
          targetIds: idOrIds,
          ...(options?.typeOrTypes && { typeOrTypes: options?.typeOrTypes }),
          ...(options?.direction && { direction: options?.direction })
        }
      })
    },

    export<S extends Schema = any>(
      searchParams: SearchQuery<S>,
      transaction?: Transaction | string
    ) {
      const txId = pickTransactionId(transaction)

      return fetcher<ApiResponse<{ dateTime: string; fileContent: string }>>(
        `/records/export/csv`,
        {
          headers: Object.assign({}, buildTransactionHeader(txId)),
          method: 'POST',
          requestData: searchParams
        }
      )
    },

    find<S extends Schema = any>(
      params?: { id?: string; searchParams: SearchQuery<S> },
      transaction?: Transaction | string
    ) {
      const txId = pickTransactionId(transaction)
      const url = params?.id ? `/records/${params?.id}/search` : `/records/search`

      return fetcher<ApiResponse<DBRecord<S>[]>>(url, {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: params?.searchParams
      })
    },

    findById<S extends Schema = any>(
      idOrIds: MaybeArray<string>,
      transaction?: Transaction | string
    ) {
      const txId = pickTransactionId(transaction)
      if (isArray(idOrIds)) {
        return fetcher<ApiResponse<DBRecord<S>[]>>(`/records`, {
          headers: Object.assign({}, buildTransactionHeader(txId)),
          method: 'POST',
          requestData: { ids: idOrIds }
        })
      }
      return fetcher<ApiResponse<DBRecord<S>>>(`/records/${idOrIds}`, {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'GET'
      })
    },

    async findOne<S extends Schema = any>(
      searchParams: SearchQuery<S>,
      transaction?: Transaction | string
    ) {
      const txId = pickTransactionId(transaction)

      const response = await fetcher<ApiResponse<DBRecord<S>[]>>(
        `/records/search`,
        {
          headers: Object.assign({}, buildTransactionHeader(txId)),
          method: 'POST',
          requestData: { ...searchParams, limit: 1, skip: 0 } as SearchQuery<S>
        }
      )
      const [record] = response.data
      return { ...response, data: record } as ApiResponse<DBRecord<S>>
    },

    async findUniq<S extends Schema = any>(
      searchParams: SearchQuery<S>,
      transaction?: Transaction | string
    ) {
      const txId = pickTransactionId(transaction)

      const response = await fetcher<ApiResponse<DBRecord<S>[]>>(
        `/records/search`,
        {
          headers: Object.assign({}, buildTransactionHeader(txId)),
          method: 'POST',
          requestData: { ...searchParams, limit: 1, skip: 0 } as SearchQuery<S>
        }
      )

      if (typeof response.total !== 'undefined' && response.total > 1) {
        throw new NonUniqueResultError(response.total, searchParams)
      }

      const [record] = response.data
      return { ...response, data: record } as ApiResponse<DBRecord<S>>
    },

    properties(target: string, transaction?: Transaction | string) {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(target)!

      return fetcher<ApiResponse<Property[]>>(`/records/${recordId}/properties`, {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'GET'
      })
    },

    relations: async (target: DBRecordTarget, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(target)!

      return fetcher<ApiResponse<Array<Relation>>>(`/records/${recordId}/relations`, {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'GET'
      })
    },
    // upsert() {
    //   // @TODO
    // }

    set<S extends Schema = any>(
      target: DBRecordTarget,
      data: DBRecordDraft | S,
      transaction?: Transaction | string
    ) {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(target)!

      return fetcher<ApiResponse<DBRecord<S>>>(`/records/${recordId}`, {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'PUT',
        requestData: data instanceof DBRecordDraft ? data.toJson() : data
      })
    },
    update<S extends Schema = any>(
      target: DBRecordTarget,
      data: DBRecordDraft | S,
      transaction?: Transaction | string
    ) {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(target)!

      return fetcher<ApiResponse<DBRecord<S>>>(`/records/${recordId}`, {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'PATCH',
        requestData: data instanceof DBRecordDraft ? data.toJson() : data
      })
    }
  },
  relations: {
    find: async <S extends Schema = any>(
      searchParams: SearchQuery<S>,
      pagination?: Pick<SearchQuery, 'limit' | 'skip'>,
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)

      const queryParams = new URLSearchParams()

      if (typeof pagination?.limit !== 'undefined') {
        queryParams.append('limit', pagination.limit.toString())
      }
      if (typeof pagination?.skip !== 'undefined') {
        queryParams.append('skip', pagination?.skip.toString())
      }

      const queryString = queryParams.toString() ? '?' + queryParams.toString() : ''

      return fetcher<ApiResponse<Array<Relation>>>(
        `/records/relations/search${queryString}`,
        {
          headers: Object.assign({}, buildTransactionHeader(txId)),
          method: 'POST',
          requestData: searchParams
        }
      )
    }
  },
  tx: {
    begin(config: Partial<{ ttl: number }> = {}) {
      return fetcher<ApiResponse<{ id: string }>>(`/tx`, {
        method: 'POST',
        requestData: config
      })
    },
    commit(id: string) {
      return fetcher<ApiResponse<{ message: string }>>(`/tx/${id}/commit`, {
        method: 'POST',
        requestData: {}
      })
    },
    get(id: string) {
      return fetcher<ApiResponse<{ id: string }>>(`/tx/${id}`, {
        method: 'GET'
      })
    },
    rollback(id: string) {
      return fetcher<ApiResponse<{ message: string }>>(`/tx/${id}/rollback`, {
        method: 'POST',
        requestData: {}
      })
    }
  }
})
