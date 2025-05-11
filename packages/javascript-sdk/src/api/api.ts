import type { HttpClient } from '../network/HttpClient.js'
import type {
  DBRecord,
  DBRecordTarget,
  RelationDetachOptions,
  RelationOptions,
  RelationTarget,
  Relation,
  DBRecordInferred,
  DBRecordCreationOptions
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

import { isArray, isEmptyObject, isObject, isFlatObject, toBoolean, isString } from '../common/utils.js'
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
    // target is DBRecordInstance
    if (target instanceof DBRecordInstance) {
      const id = pickRecordId(target)
      if (!id) throw new EmptyTargetError(`${operation} error: Target id is empty`)
      return [id]
    }

    // target is Array<DBRecordInstance>
    if (isArray(target) && target.every((r) => r instanceof DBRecordInstance)) {
      const ids = target.map(pickRecordId).filter(toBoolean)
      if (!ids.length) throw new EmptyTargetError(`${operation} error: Target ids are empty`)
      return ids as Array<string>
    }

    // target is DBRecordsArrayInstance
    if (target instanceof DBRecordsArrayInstance) {
      const ids = target.data?.map(pickRecordId).filter(toBoolean)
      if (!ids?.length) throw new EmptyTargetError(`${operation} error: Target ids are empty`)
      return ids as Array<string>
    }

    // target is DBRecord
    if (isObject(target) && '__id' in target) {
      return [target.__id]
    }

    // target is Array<DBRecord>
    if (isArray(target) && target.every((r) => isObject(r) && '__id' in r)) {
      const ids = target.map(pickRecordId).filter(toBoolean)
      if (!ids.length) throw new EmptyTargetError(`${operation} error: Target ids are empty`)
      return ids as Array<string>
    }

    // target is MaybeArray<string>
    return (
      isArray(target) ? target
      : isString(target) ? [target]
      : []
    )
  }

  /**
   * API methods for managing database records
   */
  public records = {
    /**
     * Attaches a relation between records
     * @param source - The source record to create relation from
     * @param target - The target record(s) to create relation to
     * @param options - Optional relation configuration
     * @param options.type - The type of relation to create
     * @param options.direction - The direction of the relation
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise with the API response
     */
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
      const path = `/relationships/${recordId}`
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

    /**
     * Detaches (removes) a relation between records
     * @param source - The source record to remove relation from
     * @param target - The target record(s) to remove relation to
     * @param options - Optional detach configuration
     * @param options.typeOrTypes - The type(s) of relations to remove
     * @param options.direction - The direction of relations to remove
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise with the API response
     */
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
      const path = `/relationships/${recordId}`
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

    /**
     * Creates a new record in the database
     * @param label - The label/type of the record
     * @param data - The record data, either as a flat object or array of property drafts
     * @param options - Optional write configuration
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise resolving to the created DBRecordInstance
     * @throws Error if data is not a flat object and createMany should be used instead
     */
    create: async <S extends Schema = any>(
      {
        label,
        data,
        options
      }: {
        label: string
        data: InferSchemaTypesWrite<S> | Array<PropertyDraft>
        options?: DBRecordCreationOptions
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

    /**
     * Creates multiple records in a single operation
     * @param data - Object containing label, options and payload array for multiple records
     * @param data.label - The label/type for all records
     * @param data.options - Optional write configuration
     * @param data.payload - Array of record data to create
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise resolving to DBRecordsArrayInstance containing created records
     */
    createMany: async <S extends Schema = any>(
      data: {
        label: string
        options?: DBRecordCreationOptions
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
        const dbRecordInstances = (response.data as Array<DBRecord<S>>).map((r) => {
          const instance = new DBRecordInstance<S>(r)
          instance.init(this)
          return instance
        })

        const result = new DBRecordsArrayInstance<S>(dbRecordInstances, response.total)
        result.init(this)
        return result
      }

      return new DBRecordsArrayInstance<S>([])
    },

    /**
     * Deletes records matching the search query
     * @param searchQuery - Query to identify records to delete
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise with the API response
     * @throws EmptyTargetError if query is empty and force delete is not allowed
     */
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

    /**
     * Deletes record(s) by ID
     * @param idOrIds - Single ID or array of IDs to delete
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise with the API response
     */
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

    /**
     * Exports records matching the search query to CSV format
     * @param searchQuery - Query to identify records to export
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise with the API response containing CSV data
     */
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

    /**
     * Searches for records matching the query criteria
     * @param searchQueryWithEntryPoint - Search query with optional entry point ID
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise resolving to DBRecordsArrayInstance containing matched records
     */
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

      const dbRecordInstances = (response.data as Array<DBRecord<S>>).map((r) => {
        const instance = new DBRecordInstance<S>(r)
        instance.init(this)
        return instance
      })

      const result = new DBRecordsArrayInstance<S, Q>(
        dbRecordInstances,
        response.total,
        searchQueryWithEntryPoint
      )
      result.init(this)
      return result
    },

    /**
     * Retrieves record(s) by ID
     * @param idOrIds - Single ID or array of IDs to retrieve
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise resolving to DBRecordInstance or DBRecordsArrayInstance depending on input
     */
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
        const dbRecordInstances = (response.data as Array<DBRecord<S>>).map((r) => {
          const instance = new DBRecordInstance<S>(r)
          instance.init(this)
          return instance
        })

        const result = new DBRecordsArrayInstance<S>(dbRecordInstances, response.total)
        result.init(this)
        return result as Result
      } else {
        const result = new DBRecordInstance<S>(response.data as DBRecord<S>)
        result.init(this)
        return result as Result
      }
    },

    /**
     * Finds a single record matching the search query
     * @param searchQuery - Query to identify the record
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise resolving to DBRecordInstance
     */
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

    /**
     * Finds a unique record matching the search query
     * @param searchQuery - Query to identify the record
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise resolving to DBRecordInstance
     * @throws NonUniqueResultError if multiple records match the query
     */
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

    /**
     * Sets (overwrites) record data
     * @param target - The record to update
     * @param label - The label/type of the record
     * @param data - The new record data
     * @param options - Optional write configuration
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise resolving to updated DBRecordInstance
     * @throws Error if data is not a flat object
     */
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
        options?: DBRecordCreationOptions
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

    /**
     * Updates record data (partial update)
     * @param target - The record to update
     * @param label - The label/type of the record
     * @param data - The partial record data to update
     * @param options - Optional write configuration
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise resolving to updated DBRecordInstance
     * @throws Error if data is not a flat object
     */
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
        options?: DBRecordCreationOptions
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

    /**
     * Creates or updates a record based on matching criteria
     * @param label - The label/type of the record
     * @param data - The record data to upsert
     * @param matchBy - Optional array of property names to match existing records
     * @param options - Optional write configuration
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise resolving to DBRecordInstance
     * @throws Error if data is not a flat object
     */
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
        options?: DBRecordCreationOptions
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

  /**
   * API methods for managing relations between records
   */
  public relationships = {
    /**
     * Searches for relations matching the query criteria
     * @param searchQuery - Query to identify relations
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise with the API response containing matched relations
     */
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
      const path = `/relationships/search${queryString}`
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

  /**
   * API methods for managing properties in the database
   */
  public properties = {
    /**
     * Deletes a property by its ID
     * @param id - The unique identifier of the property to delete
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise with the API response containing the deleted property
     */
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

    /**
     * Searches for properties based on the provided query
     * @param searchQuery - Query parameters to filter properties
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise with the API response containing an array of matching properties
     */
    find: async <S extends Schema = any>(searchQuery: SearchQuery<S>, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)
      const path = `/properties/search`
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

    /**
     * Retrieves a specific property by its ID
     * @param id - The unique identifier of the property to retrieve
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise with the API response containing the requested property
     */
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

    /**
     * Retrieves values for a specific property
     * @param id - The unique identifier of the property
     * @param options - Optional parameters for pagination, sorting, and filtering
     * @param options.sort - Sort direction for values ('asc' or 'desc')
     * @param options.skip - Number of values to skip
     * @param options.limit - Maximum number of values to return
     * @param options.query - Filter query for values
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise with the API response containing the property and its values
     */
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

  /**
   * API methods for managing labels in the database
   */
  public labels = {
    /**
     * Searches for labels based on the provided query
     * @param searchQuery - Query parameters to filter labels
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise with the API response containing a record of label names and their counts
     */
    find: async <S extends Schema = any>(searchQuery: SearchQuery<S>, transaction?: Transaction | string) => {
      const txId = pickTransactionId(transaction)

      const path = '/labels/search'
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

  /**
   * API methods for managing database transactions
   */
  public tx = {
    /**
     * Begins a new database transaction
     * @param config - Optional configuration object for the transaction
     * @param config.ttl - Time-to-live in milliseconds for the transaction
     * @returns A new Transaction instance that can be used for subsequent operations
     * @throws If the transaction cannot be started
     */
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

    /**
     * Commits a transaction, making its changes permanent
     * @param tx - The ID of the transaction or Transaction instance to commit
     * @returns An ApiResponse indicating success or failure of the commit
     * @throws If the transaction cannot be committed or doesn't exist
     */
    commit: async (tx: string | Transaction) => {
      const id = pickTransactionId(tx)!
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

    /**
     * Retrieves an existing transaction by its ID
     * @param tx - The ID of the transaction or Transaction instance to retrieve
     * @returns A Transaction instance representing the retrieved transaction
     * @throws If the transaction doesn't exist or cannot be retrieved
     */
    get: async (tx: string | Transaction) => {
      const id = pickTransactionId(tx)!
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

    /**
     * Rolls back a transaction, undoing all changes made within it
     * @param tx - The ID of the transaction or Transaction instance to roll back
     * @returns An ApiResponse indicating success or failure of the rollback
     * @throws If the transaction cannot be rolled back or doesn't exist
     */
    rollback: async (tx: string | Transaction) => {
      const id = pickTransactionId(tx)!
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
