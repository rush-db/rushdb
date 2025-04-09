import type { HttpClient } from '../network/HttpClient.js'
import type {
  DBRecord,
  DBRecordTarget,
  RelationDetachOptions,
  RelationOptions,
  RelationTarget,
  Relation,
  DBRecordInferred,
  DBRecordWriteOptions
} from '../sdk/record.js'
import type { SDKConfig } from '../sdk/types.js'
import type {
  SearchQuery,
  Schema,
  InferSchemaTypesWrite,
  MaybeArray,
  PropertyValuesOptions,
  PropertyDraft,
  Property,
  PropertyValuesData,
  AnyObject
} from '../types/index.js'
import type { ApiResponse } from './types.js'

import { isArray, isEmptyObject, isObject, isFlatObject, toBoolean } from '../common/utils.js'
import { createFetcher } from '../network/index.js'
import { EmptyTargetError, NonUniqueResultError } from '../sdk/errors.js'
import { DBRecordInstance, DBRecordsArrayInstance } from '../sdk/record.js'
import { Transaction } from '../sdk/transaction.js'
import {
  buildTransactionHeader,
  buildUrl,
  generateRandomId,
  isPropertyDraft,
  pickRecordId,
  pickTransactionId
} from './utils.js'

export class RestAPI {
  public fetcher: ReturnType<typeof createFetcher>
  public options: SDKConfig['options']
  public logger: SDKConfig['logger']

  constructor(token?: string, config?: SDKConfig & { httpClient: HttpClient }) {
    this.fetcher = null as unknown as ReturnType<typeof createFetcher>

    if (config?.httpClient) {
      const url = buildUrl(config)
      this.fetcher = createFetcher({
        httpClient: config.httpClient,
        token,
        url
      })
    }

    if (config?.options) {
      this.options = config?.options
    }

    if (config?.logger) {
      this.logger = config?.logger
    }
  }

  _extractTargetIds(target: RelationTarget, operation: string): Array<string> {
    // target is MaybeArray<DBRecordInstance>
    // if (target instanceof DBRecordInstance) {
    //   const id = pickRecordId(target)
    //   if (id) {
    //     return await this.api.records.attach(source, id, options, transaction)
    //   } else {
    //     throw new EmptyTargetError('Attach error: Target id is empty')
    //   }
    // } else if (isArray(target) && target.every((r) => r instanceof DBRecordInstance)) {
    //   const ids = target.map(pickRecordId).filter(toBoolean)
    //   if (ids.length) {
    //     return await this.api.records.attach(source, ids as Array<string>, options, transaction)
    //   } else {
    //     throw new EmptyTargetError('Attach error: Target ids are empty')
    //   }
    // }
    //
    // // target is DBRecordsArrayInstance
    // else if (target instanceof DBRecordsArrayInstance) {
    //   const ids = target.data?.map(pickRecordId).filter(Boolean)
    //   if (ids?.length) {
    //     return await this.api.records.attach(source, ids, options, transaction)
    //   } else {
    //     throw new EmptyTargetError('Attach error: Target ids are empty')
    //   }
    // }
    //
    // // target is MaybeArray<DBRecord>
    // else if (isObject(target) && '__id' in target) {
    //   return await this.api.records.attach(source, target.__id, options, transaction)
    // } else if (isArray(target) && target.every((r) => isObject(r) && '__id' in r)) {
    //   const ids = target?.map(pickRecordId).filter(Boolean)
    //   if (ids?.length) {
    //     return await this.api.records.attach(source, ids, options, transaction)
    //   } else {
    //     throw new EmptyTargetError('Attach error: Target ids are empty')
    //   }
    // }
    //
    // // target is MaybeArray<string>
    // else {
    //   return await this.api.records.attach(source, target as MaybeArray<string>, options, transaction)
    // }

    if (target instanceof DBRecordInstance) {
      const id = pickRecordId(target)
      if (!id) throw new EmptyTargetError(`${operation} error: Target id is empty`)
      return [id]
    }

    if (Array.isArray(target) && target.every((r) => r instanceof DBRecordInstance)) {
      const ids = target.map(pickRecordId).filter(toBoolean)
      if (!ids.length) throw new EmptyTargetError(`${operation} error: Target ids are empty`)
      return ids as Array<string>
    }

    if (target instanceof DBRecordsArrayInstance) {
      const ids = target.data?.map(pickRecordId).filter(toBoolean)
      if (!ids?.length) throw new EmptyTargetError(`${operation} error: Target ids are empty`)
      return ids as Array<string>
    }

    if (isObject(target) && '__id' in target) {
      return [target.__id]
    }

    if (Array.isArray(target) && target.every((r) => isObject(r) && '__id' in r)) {
      const ids = target.map(pickRecordId).filter(toBoolean)
      if (!ids.length) throw new EmptyTargetError(`${operation} error: Target ids are empty`)
      return ids as Array<string>
    }

    return target as Array<string>
  }

  public records = {
    attach: async (
      {
        source,
        target,
        options
      }: {
        source: DBRecordTarget
        target: RelationTarget
        options?: RelationOptions
      },
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(source)!
      const path = `/records/${recordId}/relations`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: {
          targetIds: this._extractTargetIds(target, 'Attach'),
          ...(options?.type && { type: options.type }),
          ...(options?.direction && { direction: options.direction })
        }
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<{ message: string }>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },

    detach: async (
      {
        source,
        target,
        options
      }: {
        source: DBRecordTarget
        target: RelationTarget
        options?: RelationDetachOptions
      },
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(source)!
      const path = `/records/${recordId}/relations`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'PUT',
        requestData: {
          targetIds: this._extractTargetIds(target, 'Detach'),
          ...(options?.typeOrTypes && { typeOrTypes: options.typeOrTypes }),
          ...(options?.direction && { direction: options.direction })
        }
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<{ message: string }>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },

    create: async <S extends Schema = any>(
      {
        label,
        data,
        options
      }: {
        label: string
        data: InferSchemaTypesWrite<S> | Array<PropertyDraft>
        options?: DBRecordWriteOptions
      },
      transaction?: Transaction | string
    ): Promise<DBRecordInstance<S>> => {
      const txId = pickTransactionId(transaction)
      const path = `/records`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: {}
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''

      if (isArray(data) && data.every(isPropertyDraft)) {
        payload.requestData = { label, properties: data }
      } else if (isFlatObject(data)) {
        payload.requestData = { label, payload: data, options }
      } else if (isObject(data)) {
        throw Error('Provided data is not a flat object. Consider to use `createMany` method.')
      }

      this.logger?.({ requestId, path, ...payload })
      const response = await this.fetcher<ApiResponse<DBRecord<S> | undefined>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      if (response?.success && response?.data) {
        const result = new DBRecordInstance<S>(response.data)
        result.init(this)
        return result
      }

      return new DBRecordInstance<S>()
    },

    createMany: async <S extends Schema = any>(
      data: {
        label: string
        options?: DBRecordWriteOptions
        payload: MaybeArray<AnyObject>
      },
      transaction?: Transaction | string
    ): Promise<DBRecordsArrayInstance<S>> => {
      const txId = pickTransactionId(transaction)
      const path = `/records/import/json`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: data
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Array<DBRecord<S>>>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      if (response?.success && response?.data) {
        const result = new DBRecordsArrayInstance<S>(response.data, response.total)
        result.init(this)
        return result
      }

      return new DBRecordsArrayInstance<S>([])
    },

    delete: async <S extends Schema = any>(
      searchQuery: SearchQuery<S>,
      transaction?: Transaction | string
    ) => {
      if (isEmptyObject(searchQuery.where) && !this?.options?.allowForceDelete) {
        throw new EmptyTargetError(
          `You must specify criteria to delete records. Empty criteria are not allowed. If this was intentional, use the Dashboard instead.`
        )
      }

      const txId = pickTransactionId(transaction)
      const path = `/records/delete`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'PUT',
        requestData: searchQuery
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<{ message: string }>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },

    deleteById: async (idOrIds: MaybeArray<string>, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)
      const multipleTargets = isArray(idOrIds)
      const path = multipleTargets ? `/records/delete` : `/records/${idOrIds}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: multipleTargets ? 'PUT' : 'DELETE',
        requestData: multipleTargets ? { limit: 1000, where: { $id: { $in: idOrIds } } } : undefined
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<{ message: string }>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },

    export: async <S extends Schema = any>(
      searchQuery: SearchQuery<S>,
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)
      const path = `/records/export/csv`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: searchQuery
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<{ dateTime: string; fileContent: string }>>(
        path,
        payload
      )
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },

    find: async <S extends Schema = any, Q extends SearchQuery<S> = SearchQuery<S>>(
      searchQueryWithEntryPoint: Q & { id?: string },
      transaction?: Transaction | string
    ): Promise<DBRecordsArrayInstance<S, Q>> => {
      const { id, ...searchQuery } = searchQueryWithEntryPoint

      const txId = pickTransactionId(transaction)
      const path = id ? `/records/${id}/search` : `/records/search`

      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: searchQuery
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Array<DBRecordInferred<S, Q>>>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      const result = new DBRecordsArrayInstance<S, Q>(
        response.data,
        response.total,
        searchQueryWithEntryPoint
      )
      result.init(this)
      return result
    },

    findById: async <
      S extends Schema = Schema,
      Arg extends MaybeArray<string> = MaybeArray<string>,
      Result = Arg extends Array<string> ? DBRecordsArrayInstance<S> : DBRecordInstance<S>
    >(
      idOrIds: Arg,
      transaction?: Transaction | string
    ): Promise<Result> => {
      const txId = pickTransactionId(transaction)
      const path = isArray(idOrIds) ? `/records` : `/records/${idOrIds}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: isArray(idOrIds) ? 'POST' : 'GET',
        requestData: isArray(idOrIds) ? { ids: idOrIds } : undefined
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Array<DBRecord<S>> | DBRecord<S>>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      if (isArray(idOrIds)) {
        const result = new DBRecordsArrayInstance<S>(response.data as Array<DBRecord<S>>, response.total)
        result.init(this)
        return result as Result
      } else {
        const result = new DBRecordInstance<S>(response.data as DBRecord<S>)
        result.init(this)
        return result as Result
      }
    },

    findOne: async <S extends Schema = any, Q extends SearchQuery<S> = SearchQuery<S>>(
      searchQuery: Q,
      transaction?: Transaction | string
    ): Promise<DBRecordInstance<S>> => {
      const txId = pickTransactionId(transaction)
      const path = `/records/search`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: { ...searchQuery, limit: 1, skip: 0 }
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Array<DBRecordInferred<S, Q>>>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })
      const [record] = response.data

      const result = new DBRecordInstance<S, Q>(record)
      result.init(this)
      return result
    },

    findUniq: async <S extends Schema = any, Q extends SearchQuery<S> = SearchQuery<S>>(
      searchQuery: Q,
      transaction?: Transaction | string
    ): Promise<DBRecordInstance<S, Q>> => {
      const txId = pickTransactionId(transaction)
      const path = `/records/search`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: { ...searchQuery, limit: 1, skip: 0 }
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Array<DBRecordInferred<S, Q>>>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      if (typeof response.total !== 'undefined' && response.total > 1) {
        throw new NonUniqueResultError(response.total, searchQuery)
      }

      const [record] = response.data

      const result = new DBRecordInstance<S, Q>(record)
      result.init(this)
      return result
    },

    properties: async (target: DBRecordTarget, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(target)!
      const path = `/records/${recordId}/properties`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'GET'
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Array<Property>>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },

    relations: async (target: DBRecordTarget, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(target)!
      const path = `/records/${recordId}/relations`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'GET'
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Array<Relation>>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },

    set: async <S extends Schema = any>(
      {
        target,
        label,
        data,
        options
      }: {
        target: DBRecordTarget
        label: string
        data: InferSchemaTypesWrite<S> | Array<PropertyDraft>
        options?: DBRecordWriteOptions
      },
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(target)!
      const path = `/records/${recordId}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'PUT',
        requestData: {}
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''

      if (isArray(data) && data.every(isPropertyDraft)) {
        payload.requestData = { label, properties: data }
      } else if (isFlatObject(data)) {
        payload.requestData = { label, payload: data, options }
      } else if (isObject(data)) {
        throw Error('Provided data is not a flat object. Consider to use `createMany` method.')
      }

      this.logger?.({ requestId, path, ...payload })
      const response = await this.fetcher<ApiResponse<DBRecord<S> | undefined>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      if (response?.success && response?.data) {
        const result = new DBRecordInstance<S>(response.data)
        result.init(this)
        return result
      }

      return new DBRecordInstance<S>()
    },

    update: async <S extends Schema = any>(
      {
        target,
        label,
        data,
        options
      }: {
        target: DBRecordTarget
        label: string
        data: Partial<InferSchemaTypesWrite<S>> | Array<PropertyDraft>
        options?: DBRecordWriteOptions
      },
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)
      const recordId = pickRecordId(target)!
      const path = `/records/${recordId}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'PATCH',
        requestData: {}
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''

      if (isArray(data) && data.every(isPropertyDraft)) {
        payload.requestData = { label, properties: data }
      } else if (isFlatObject(data)) {
        payload.requestData = { label, payload: data, options }
      } else if (isObject(data)) {
        throw Error('Provided data is not a flat object. Consider to use `createMany` method.')
      }

      this.logger?.({ requestId, path, ...payload })
      const response = await this.fetcher<ApiResponse<DBRecord<S> | undefined>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      if (response?.success && response?.data) {
        const result = new DBRecordInstance<S>(response.data)
        result.init(this)
        return result
      }

      return new DBRecordInstance<S>()
    },

    upsert: async <S extends Schema = any>(
      {
        label,
        data,
        matchBy,
        options
      }: {
        label: string
        data: InferSchemaTypesWrite<S> | Array<PropertyDraft>
        matchBy?: Array<string>
        options?: DBRecordWriteOptions
      },
      transaction?: Transaction | string
    ): Promise<DBRecordInstance<S>> => {
      const txId = pickTransactionId(transaction)
      const path = `/records`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'PUT',
        requestData: {}
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''

      if (isArray(data) && data.every(isPropertyDraft)) {
        payload.requestData = { matchBy, properties: data, label }
      } else if (isFlatObject(data)) {
        payload.requestData = { matchBy, payload: data, label, options }
      } else if (isObject(data)) {
        throw Error('Provided data is not a flat object. Consider to use `createMany` method.')
      }

      this.logger?.({ requestId, path, ...payload })
      const response = await this.fetcher<ApiResponse<DBRecord<S> | undefined>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      if (response?.success && response?.data) {
        const result = new DBRecordInstance<S>(response.data)
        result.init(this)
        return result
      }

      return new DBRecordInstance<S>()
    }
  }
  public relations = {
    find: async <S extends Schema = any>(searchQuery: SearchQuery<S>, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)
      const queryParams = new URLSearchParams()

      if (searchQuery?.limit !== undefined) {
        queryParams.append('limit', searchQuery.limit.toString())
      }
      if (searchQuery?.skip !== undefined) {
        queryParams.append('skip', searchQuery.skip.toString())
      }

      const queryString = queryParams.toString() ? '?' + queryParams.toString() : ''
      const path = `/records/relations/search${queryString}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: searchQuery
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Array<Relation>>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    }
  }
  public properties = {
    delete: async (id: string, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)
      const path = `/properties/${id}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'DELETE'
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Property>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    find: async <S extends Schema = any>(searchQuery: SearchQuery<S>, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)
      const path = `/properties`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: searchQuery
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Array<Property>>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    findById: async (id: string, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)
      const path = `/properties/${id}`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'GET'
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Property>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
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
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Property & PropertyValuesData>>(fullPath, payload)
      this.logger?.({ requestId, path: fullPath, ...payload, responseData: response.data })

      return response
    }
    // update: () => {
    //   // @TODO
    // },
    // updateValues: (id: string, transaction?: Transaction | string) => {
    //   // @TODO
    // },
  }
  public labels = {
    find: async <S extends Schema = any>(searchQuery: SearchQuery<S>, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)

      const path = '/labels'
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: searchQuery
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Record<string, number>>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    }
  }
  public tx = {
    begin: async (config?: Partial<{ ttl: number }>) => {
      const path = `/tx`
      const payload = {
        method: 'POST',
        requestData: config
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const transaction = await this.fetcher<ApiResponse<{ id: string }>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: transaction.data })

      const result = new Transaction(transaction.data.id)
      result.init(this)
      return result
    },
    commit: async (id: string) => {
      const path = `/tx/${id}/commit`
      const payload = {
        method: 'POST',
        requestData: {}
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<{ message: string }>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    get: async (id: string) => {
      const path = `/tx/${id}`
      const payload = {
        method: 'GET'
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const transaction = await this.fetcher<ApiResponse<{ id: string }>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: transaction.data })

      const result = new Transaction(transaction.data.id)
      result.init(this)
      return result
    },
    rollback: async (id: string) => {
      const path = `/tx/${id}/rollback`
      const payload = {
        method: 'POST',
        requestData: {}
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<{ message: string }>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    }
  }
}
