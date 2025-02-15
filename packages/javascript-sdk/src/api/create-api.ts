import type { createFetcher } from '../network/index.js'
import type {
  DBRecord,
  DBRecordInferred,
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
  MaybeArray,
  PropertyValuesOptions
} from '../types/index.js'
import type { ApiResponse } from './types.js'

import { isArray } from '../common/utils.js'
import { NonUniqueResultError } from '../sdk/errors.js'
import { DBRecordsBatchDraft, DBRecordDraft } from '../sdk/record.js'
import { buildTransactionHeader, generateRandomId, pickRecordId, pickTransactionId } from './utils.js'
import type { Logger } from '../sdk/types.js'

export const createApi = (fetcher: ReturnType<typeof createFetcher>, logger?: Logger) => ({
  labels: {
    async find<S extends Schema = any>(searchParams: SearchQuery<S>, transaction?: Transaction | string) {
      const txId = pickTransactionId(transaction)

      const path = '/labels'
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: searchParams
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<Record<string, number>>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    }
  },
  properties: {
    delete: async (id: string, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)
      const path = `/properties/${id}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'DELETE'
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<Property>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    find: async <S extends Schema = any>(
      searchParams: SearchQuery<S>,
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)
      const path = `/properties`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: searchParams
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<Property[]>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    findById: async (id: string, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)
      const path = `/properties/${id}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'GET'
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<Property>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    // update: () => {
    //   // @TODO
    // },
    // updateValues: (id: string, transaction?: Transaction | string) => {
    //   // @TODO
    // },
    values: async (id: string, options?: PropertyValuesOptions, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)
      const path = `/properties/${id}/values`

      const { sort, skip, limit, query } = options ?? {}

      const queryParams = new URLSearchParams()
      if (sort !== undefined) queryParams.append('sort', sort)
      if (skip !== undefined) queryParams.append('skip', skip.toString())
      if (limit !== undefined) queryParams.append('limit', limit.toString())
      if (query !== undefined) queryParams.append('query', query)

      const queryString = queryParams.toString()
      const fullPath = queryString ? `${path}?${queryString}` : path

      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'GET'
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<Property & PropertyValuesData>>(fullPath, payload)
      logger?.({ requestId, path: fullPath, ...payload, responseData: response.data })

      return response
    }
  },
  records: {
    async attach(
      source: DBRecordTarget,
      idOrIds: MaybeArray<string>,
      options?: RelationOptions,
      transaction?: Transaction | string
    ) {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(source)!
      const path = `/records/${recordId}/relations`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: {
          targetIds: idOrIds,
          ...(options?.type && { type: options.type }),
          ...(options?.direction && { direction: options.direction })
        }
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<{ message: string }>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },

    async create<S extends Schema = any>(
      data: DBRecordDraft | S,
      transaction?: Transaction | string
    ): Promise<ApiResponse<DBRecord<S> | undefined>> {
      const txId = pickTransactionId(transaction)
      const path = `/records`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: data instanceof DBRecordDraft ? data.toJson() : data
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<DBRecord<S> | undefined>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },

    async createMany<S extends Schema = any>(
      data: DBRecordsBatchDraft | S[],
      transaction?: Transaction | string
    ): Promise<ApiResponse<DBRecord<S>[]>> {
      const txId = pickTransactionId(transaction)
      const path = `/records/import/json`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: data instanceof DBRecordsBatchDraft ? data.toJson() : data
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<DBRecord<S>[]>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },

    async delete<S extends Schema = any>(searchParams: SearchQuery<S>, transaction?: Transaction | string) {
      const txId = pickTransactionId(transaction)
      const path = `/records/delete`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'PUT',
        requestData: searchParams
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<{ message: string }>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },

    async deleteById(idOrIds: MaybeArray<string>, transaction?: Transaction | string) {
      const txId = pickTransactionId(transaction)
      const multipleTargets = isArray(idOrIds)
      const path = multipleTargets ? `/records/delete` : `/records/${idOrIds}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: multipleTargets ? 'PUT' : 'DELETE',
        requestData: multipleTargets ? { limit: 1000, where: { $id: { $in: idOrIds } } } : undefined
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<{ message: string }>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    async detach(
      source: DBRecordTarget,
      idOrIds: MaybeArray<string>,
      options?: RelationDetachOptions,
      transaction?: Transaction | string
    ) {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(source)!
      const path = `/records/${recordId}/relations`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'PUT',
        requestData: {
          targetIds: idOrIds,
          ...(options?.typeOrTypes && { typeOrTypes: options.typeOrTypes }),
          ...(options?.direction && { direction: options.direction })
        }
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<{ message: string }>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    async export<S extends Schema = any>(searchParams: SearchQuery<S>, transaction?: Transaction | string) {
      const txId = pickTransactionId(transaction)
      const path = `/records/export/csv`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: searchParams
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<{ dateTime: string; fileContent: string }>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    async find<S extends Schema = any, Q extends SearchQuery<S> = SearchQuery<S>>(
      params?: { id?: string; searchParams: Q },
      transaction?: Transaction | string
    ): Promise<ApiResponse<DBRecordInferred<S, Q>[]>> {
      const txId = pickTransactionId(transaction)
      const path = params?.id ? `/records/${params.id}/search` : `/records/search`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: params?.searchParams
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<DBRecordInferred<S, Q>[]>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    async findById<S extends Schema = any>(idOrIds: MaybeArray<string>, transaction?: Transaction | string) {
      const txId = pickTransactionId(transaction)
      const path = isArray(idOrIds) ? `/records` : `/records/${idOrIds}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: isArray(idOrIds) ? 'POST' : 'GET',
        requestData: isArray(idOrIds) ? { ids: idOrIds } : undefined
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<DBRecord<S>[] | DBRecord<S>>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    async findOne<S extends Schema = any, Q extends SearchQuery<S> = SearchQuery<S>>(
      searchParams: Q,
      transaction?: Transaction | string
    ) {
      const txId = pickTransactionId(transaction)
      const path = `/records/search`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: { ...searchParams, limit: 1, skip: 0 }
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<DBRecordInferred<S, Q>[]>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })
      const [record] = response.data
      return { ...response, data: record } as ApiResponse<DBRecordInferred<S, Q>>
    },
    async findUniq<S extends Schema = any, Q extends SearchQuery<S> = SearchQuery<S>>(
      searchParams: Q,
      transaction?: Transaction | string
    ) {
      const txId = pickTransactionId(transaction)
      const path = `/records/search`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: { ...searchParams, limit: 1, skip: 0 }
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<DBRecordInferred<S, Q>[]>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      if (typeof response.total !== 'undefined' && response.total > 1) {
        throw new NonUniqueResultError(response.total, searchParams)
      }

      const [record] = response.data
      return { ...response, data: record } as ApiResponse<DBRecordInferred<S, Q>>
    },
    async properties(target: string, transaction?: Transaction | string) {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(target)!
      const path = `/records/${recordId}/properties`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'GET'
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<Property[]>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    async relations(target: DBRecordTarget, transaction?: Transaction | string) {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(target)!
      const path = `/records/${recordId}/relations`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'GET'
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<Array<Relation>>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    // upsert() {
    //   // @TODO
    // }

    async set<S extends Schema = any>(
      target: DBRecordTarget,
      data: DBRecordDraft | S,
      transaction?: Transaction | string
    ) {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(target)!
      const path = `/records/${recordId}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'PUT',
        requestData: data instanceof DBRecordDraft ? data.toJson() : data
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<DBRecord<S>>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    async update<S extends Schema = any>(
      target: DBRecordTarget,
      data: DBRecordDraft | S,
      transaction?: Transaction | string
    ) {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(target)!
      const path = `/records/${recordId}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'PATCH',
        requestData: data instanceof DBRecordDraft ? data.toJson() : data
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<DBRecord<S>>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    }
  },
  relations: {
    async find<S extends Schema = any>(
      searchParams: SearchQuery<S>,
      pagination?: Pick<SearchQuery, 'limit' | 'skip'>,
      transaction?: Transaction | string
    ) {
      const txId = pickTransactionId(transaction)
      const queryParams = new URLSearchParams()

      if (pagination?.limit !== undefined) {
        queryParams.append('limit', pagination.limit.toString())
      }
      if (pagination?.skip !== undefined) {
        queryParams.append('skip', pagination.skip.toString())
      }

      const queryString = queryParams.toString() ? '?' + queryParams.toString() : ''
      const path = `/records/relations/search${queryString}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: searchParams
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<Array<Relation>>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    }
  },
  tx: {
    async begin(config: Partial<{ ttl: number }> = {}) {
      const path = `/tx`
      const payload = {
        method: 'POST',
        requestData: config
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<{ id: string }>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    async commit(id: string) {
      const path = `/tx/${id}/commit`
      const payload = {
        method: 'POST',
        requestData: {}
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<{ message: string }>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    async get(id: string) {
      const path = `/tx/${id}`
      const payload = {
        method: 'GET'
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<{ id: string }>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    async rollback(id: string) {
      const path = `/tx/${id}/rollback`
      const payload = {
        method: 'POST',
        requestData: {}
      }
      const requestId = typeof logger === 'function' ? generateRandomId() : ''
      logger?.({ requestId, path, ...payload })

      const response = await fetcher<ApiResponse<{ message: string }>>(path, payload)
      logger?.({ requestId, path, ...payload, responseData: response.data })
    }
  }
})
