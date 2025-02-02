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
  OrderDirection
} from '../types/index.js'
import type { ApiResponse } from './types.js'

import { isArray } from '../common/utils.js'
import { NonUniqueResultError } from '../sdk/errors.js'
import { DBRecordsBatchDraft, DBRecordDraft } from '../sdk/record.js'
import { buildTransactionHeader, pickRecordId, pickTransactionId } from './utils.js'
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

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<Record<string, number>>>(path, payload)
    }
  },
  properties: {
    delete: (id: string, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)
      const path = `/properties/${id}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'DELETE'
      }

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<Property>>(path, payload)
    },
    find: <S extends Schema = any>(searchParams: SearchQuery<S>, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)
      const path = `/properties`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: searchParams
      }

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<Property[]>>(path, payload)
    },
    findById: (id: string, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)
      const path = `/properties/${id}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'GET'
      }

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<Property>>(path, payload)
    },
    // update: () => {
    //   // @TODO
    // },
    // updateValues: (id: string, transaction?: Transaction | string) => {
    //   // @TODO
    // },
    values: (
      id: string,
      options?: { sort?: OrderDirection; skip?: number; limit?: number },
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)
      const path = `/properties/${id}/values`

      const { sort, skip, limit } = options ?? {}

      const queryParams = new URLSearchParams()
      if (sort) queryParams.append('sort', sort)
      if (skip !== undefined) queryParams.append('skip', skip.toString())
      if (limit !== undefined) queryParams.append('limit', limit.toString())

      const queryString = queryParams.toString()
      const fullPath = queryString ? `${path}?${queryString}` : path

      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'GET'
      }

      logger?.({ path: fullPath, ...payload })
      return fetcher<ApiResponse<Property & PropertyValuesData>>(fullPath, payload)
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

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<{ message: string }>>(path, payload)
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

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<DBRecord<S> | undefined>>(path, payload)
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

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<DBRecord<S>[]>>(path, payload)
    },

    delete<S extends Schema = any>(searchParams: SearchQuery<S>, transaction?: Transaction | string) {
      const txId = pickTransactionId(transaction)
      const path = `/records/delete`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'PUT',
        requestData: searchParams
      }

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<{ message: string }>>(path, payload)
    },

    deleteById(idOrIds: MaybeArray<string>, transaction?: Transaction | string) {
      const txId = pickTransactionId(transaction)
      const multipleTargets = isArray(idOrIds)
      const path = multipleTargets ? `/records/delete` : `/records/${idOrIds}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: multipleTargets ? 'PUT' : 'DELETE',
        requestData: multipleTargets ? { limit: 1000, where: { $id: { $in: idOrIds } } } : undefined
      }

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<{ message: string }>>(path, payload)
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

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<{ message: string }>>(path, payload)
    },
    export<S extends Schema = any>(searchParams: SearchQuery<S>, transaction?: Transaction | string) {
      const txId = pickTransactionId(transaction)
      const path = `/records/export/csv`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: searchParams
      }

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<{ dateTime: string; fileContent: string }>>(path, payload)
    },
    find<S extends Schema = any, Q extends SearchQuery<S> = SearchQuery<S>>(
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

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<DBRecordInferred<S, Q>[]>>(path, payload)
    },
    findById<S extends Schema = any>(idOrIds: MaybeArray<string>, transaction?: Transaction | string) {
      const txId = pickTransactionId(transaction)
      const path = isArray(idOrIds) ? `/records` : `/records/${idOrIds}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: isArray(idOrIds) ? 'POST' : 'GET',
        requestData: isArray(idOrIds) ? { ids: idOrIds } : undefined
      }

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<DBRecord<S>[] | DBRecord<S>>>(path, payload)
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

      logger?.({ path, ...payload })
      const response = await fetcher<ApiResponse<DBRecordInferred<S, Q>[]>>(path, payload)
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

      logger?.({ path, ...payload })
      const response = await fetcher<ApiResponse<DBRecordInferred<S, Q>[]>>(path, payload)

      if (typeof response.total !== 'undefined' && response.total > 1) {
        throw new NonUniqueResultError(response.total, searchParams)
      }

      const [record] = response.data
      return { ...response, data: record } as ApiResponse<DBRecordInferred<S, Q>>
    },
    properties(target: string, transaction?: Transaction | string) {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(target)!
      const path = `/records/${recordId}/properties`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'GET'
      }

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<Property[]>>(path, payload)
    },
    async relations(target: DBRecordTarget, transaction?: Transaction | string) {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(target)!
      const path = `/records/${recordId}/relations`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'GET'
      }

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<Array<Relation>>>(path, payload)
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
      const path = `/records/${recordId}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'PUT',
        requestData: data instanceof DBRecordDraft ? data.toJson() : data
      }

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<DBRecord<S>>>(path, payload)
    },
    update<S extends Schema = any>(
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

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<DBRecord<S>>>(path, payload)
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

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<Array<Relation>>>(path, payload)
    }
  },
  tx: {
    begin(config: Partial<{ ttl: number }> = {}) {
      const path = `/tx`
      const payload = {
        method: 'POST',
        requestData: config
      }

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<{ id: string }>>(path, payload)
    },
    commit(id: string) {
      const path = `/tx/${id}/commit`
      const payload = {
        method: 'POST',
        requestData: {}
      }

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<{ message: string }>>(path, payload)
    },
    get(id: string) {
      const path = `/tx/${id}`
      const payload = {
        method: 'GET'
      }

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<{ id: string }>>(path, payload)
    },
    rollback(id: string) {
      const path = `/tx/${id}/rollback`
      const payload = {
        method: 'POST',
        requestData: {}
      }

      logger?.({ path, ...payload })
      return fetcher<ApiResponse<{ message: string }>>(path, payload)
    }
  }
})
