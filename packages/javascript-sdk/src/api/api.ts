import type { HttpClient } from '../network/HttpClient.js'
import type {
  DBRecord,
  DBRecordCreationOptions,
  DBRecordInferred,
  DBRecordTarget,
  Relation,
  RelationDetachOptions,
  RelationDirection,
  RelationOptions,
  RelationTarget
} from '../sdk/record.js'
import { DBRecordInstance, DBRecordsArrayInstance } from '../sdk/record.js'
import type { SDKConfig, State } from '../sdk/types.js'
import type {
  AnyObject,
  FlatObject,
  InferSchemaTypesWrite,
  MaybeArray,
  OrderDirection,
  Property,
  PropertyDraft,
  PropertyValuesData,
  Schema,
  RelationshipSearchQuery,
  SearchQuery,
  Where
} from '../types/index.js'
import type { ApiResponse } from './types.js'
import type {
  CreateEmbeddingIndexParams,
  DeleteRelationshipPatternOptions,
  EmbeddingIndex,
  EmbeddingIndexStats,
  RelationshipPattern,
  RelationshipPatternListResponse,
  SmartSearchOptions,
  SmartSearchQueryResponse,
  UpsertEmbeddingVectorsParams,
  UpsertEmbeddingVectorsResult,
  VectorEntry,
  VectorSearchParams,
  VectorSearchResult
} from './types.js'

import {
  getOwnProperties,
  isArray,
  isEmptyObject,
  isFlatObject,
  jsonImportRequiresLabel,
  isObject,
  isString,
  removeUndefinedDeep,
  toBoolean
} from '../common/utils.js'
import { createFetcher } from '../network/index.js'
import { EmptyTargetError, NonUniqueResultError } from '../sdk/errors.js'
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
          ...(options?.direction && { direction: options.direction }),
          ...(options?.properties && { properties: options.properties })
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
        data: rawData,
        options,
        vectors
      }: {
        label: string
        data: InferSchemaTypesWrite<S> | Array<PropertyDraft>
        options?: Omit<DBRecordCreationOptions, 'returnResult' | 'capitalizeLabels' | 'relationshipType'>
        vectors?: VectorEntry[]
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

      const data = getOwnProperties(removeUndefinedDeep(rawData))

      if (isArray(data) && data.every(isPropertyDraft)) {
        payload.requestData = { label, properties: data, options, ...(vectors?.length && { vectors }) }
      } else if (isFlatObject(data)) {
        payload.requestData = { label, data, options, ...(vectors?.length && { vectors }) }
      } else if (isObject(data)) {
        throw new Error(
          'Provided data is not a flat object. Consider using the `importJson` method for nested objects or arrays of nested objects, or use `createMany` for arrays of flat objects.'
        )
      } else {
        throw new Error('Provided data is not valid.')
      }

      this.logger?.({ requestId, path, ...payload })
      const response = await this.fetcher<ApiResponse<DBRecord<S> | undefined>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      if (response?.success && response?.data) {
        return new DBRecordInstance<S>(response.data)
      }

      return new DBRecordInstance<S>()
    },

    /**
     * Creates multiple flat records in a single operation.
     * Use this only for CSV-like flat rows (no nested objects/arrays).
     * For nested/complex JSON, use `records.importJson`.
     *
     * @param data - Object containing label, options and data array (of flat objects)
     * @param data.label - The label/type for all records
     * @param data.options - Optional write configuration
     * @param data.data - Array of flat record data to create
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise resolving to DBRecordsArrayInstance containing created records
     * @throws Error if any record is not flat. Use `records.importJson` for nested JSON.
     */
    createMany: async <S extends Schema = any>(
      data: {
        label: string
        data: Array<InferSchemaTypesWrite<S>>
        options?: DBRecordCreationOptions
        /**
         * Per-row inline vectors for external embedding indexes.
         * Accepts either:
         * - `VectorEntry[][]` — outer array indexed per record, inner array per vector property on that record.
         *   `vectors[i]` is applied to `data[i]`. Its length must not exceed `data.length`.
         * - `VectorEntry[]` — a flat list applied to the first record (single-record batches).
         *   Auto-wrapped into `[[entry1, entry2, …]]` so callers don't need the double nesting.
         */
        vectors?: VectorEntry[][] | VectorEntry[]
      },
      transaction?: Transaction | string
    ): Promise<DBRecordsArrayInstance<S>> => {
      // Normalize to array and validate flatness
      const items = isArray(data.data) ? data.data : [data.data]
      const allFlat = items.every((r) => isFlatObject(r))
      if (!allFlat) {
        throw new Error(
          'records.createMany supports only flat records (no nested objects/arrays). Use records.importJson for nested JSON.'
        )
      }

      // Normalize flat VectorEntry[] to per-record VectorEntry[][]
      const vectors =
        data.vectors ?
          isArray(data.vectors[0]) ? (data.vectors as VectorEntry[][])
          : [data.vectors as VectorEntry[]]
        : undefined

      if (vectors && vectors.length > items.length) {
        throw new Error(
          `records.createMany: vectors length (${vectors.length}) exceeds the number of data rows (${items.length}).`
        )
      }

      // Inject per-row vectors as $vectors on each item so the backend BFS handles them
      const itemsWithVectors =
        vectors?.length ?
          items.map((item, i) => (vectors[i]?.length ? { ...item, $vectors: vectors[i] } : item))
        : items

      const txId = pickTransactionId(transaction)
      const path = `/records/import/json`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: { label: data.label, data: itemsWithVectors, options: data.options }
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Array<DBRecord<S>>>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      if (response?.success && response?.data) {
        // If `returnResult` was not explicitly set to `true`, `response.data` may be just `true`.
        // In that case, fallback to an empty array. Otherwise, map records to typed instances.
        const dbRecordInstances =
          isArray(response.data) ?
            (<Array<DBRecord<S>>>response.data)?.map((r) => {
              return new DBRecordInstance<S>(r)
            })
          : []
        return new DBRecordsArrayInstance<S>(dbRecordInstances, response.total)
      }

      return new DBRecordsArrayInstance<S>([])
    },

    /**
     * Import nested or complex JSON payloads.
     * Works in two modes:
     *  - With `label` provided
     *  - Without `label`: expects a container object whose top-level values are objects or arrays.
     *
     * Throws if `label` is missing and the input is a top-level array or object record.
     *
     * @example
     * // with label
     * await db.records.importJson({ label: 'ITEM', data: [{...}, {...}] })
     * // without label
     * await db.records.importJson({ data: { ITEM: [{...}, {...}], USER: [{...}] } })
     */
    importJson: async <S extends Schema = any>(
      params: {
        data: any
        label?: string
        options?: DBRecordCreationOptions
      },
      transaction?: Transaction | string
    ): Promise<DBRecordsArrayInstance<S>> => {
      const { data: rawData, options } = params
      let { label } = params

      const payloadData: any = rawData

      if (!label) {
        const own = isObject(rawData) && !isArray(rawData) ? getOwnProperties(rawData as AnyObject) : rawData
        if (jsonImportRequiresLabel(own)) {
          throw new Error(
            'records.importJson: Missing `label`. Provide `label` for top-level arrays or objects with primitive top-level properties, or pass a container object whose top-level values are objects or arrays, e.g. { ITEM: [...], USER: [...] }.'
          )
        }
      }

      const txId = pickTransactionId(transaction)
      const path = `/records/import/json`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: {
          label,
          data: payloadData,
          options
        }
      }

      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Array<DBRecord<S>>>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      if (response?.success && response?.data) {
        const dbRecordInstances =
          isArray(response.data) ?
            (<Array<DBRecord<S>>>response.data)?.map((r) => new DBRecordInstance<S>(r))
          : []
        return new DBRecordsArrayInstance<S>(dbRecordInstances, response.total)
      }

      return new DBRecordsArrayInstance<S>([])
    },

    /**
     * Imports records from CSV data.
     * @param params - CSV import configuration
     * @param params.label - Label applied to imported records
     * @param params.data - Raw CSV string
     * @param params.options - Import options (type inference etc.)
     * @param params.parseConfig - CSV parsing configuration (subset allowed by server)
     * @param params.parentId - Optional parent record id for hierarchical imports
     */
    importCsv: async <S extends Schema = any>(
      params: {
        label: string
        data: string
        options?: DBRecordCreationOptions
        parseConfig?: {
          delimiter?: string
          header?: boolean
          skipEmptyLines?: boolean | 'greedy'
          dynamicTyping?: boolean
          quoteChar?: string
          escapeChar?: string
          newline?: string
        }
        parentId?: string
        /**
         * Per-row inline vectors for external embedding indexes.
         * `vectors[i]` is applied to CSV row `i` (0-based, after header).
         * Its length must not exceed the number of data rows — validated server-side.
         */
        vectors?: VectorEntry[][]
      },
      transaction?: Transaction | string
    ): Promise<DBRecordsArrayInstance<S>> => {
      const txId = pickTransactionId(transaction)
      const path = `/records/import/csv`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: params
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Array<DBRecord<S>>>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      if (response?.success && response?.data) {
        const dbRecordInstances =
          isArray(response.data) ? response.data.map((r) => new DBRecordInstance<S>(r)) : []
        return new DBRecordsArrayInstance<S>(dbRecordInstances, response.total)
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
        method: 'POST',
        requestData: searchQuery ?? {}
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
        method: multipleTargets ? 'POST' : 'DELETE',
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
        requestData: searchQuery ?? {}
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
        requestData: searchQuery ?? {}
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Array<DBRecordInferred<S, Q>>>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      const dbRecordInstances = (response.data as Array<DBRecord<S>>).map((r) => {
        return new DBRecordInstance<S>(r)
      })

      return new DBRecordsArrayInstance<S, Q>(dbRecordInstances, response.total, searchQueryWithEntryPoint)
    },

    /**
     * Performs vector similarity search over records whose `propertyName` has an embedding index.
     *
     * RushDB narrows candidates by `labels` and optional `where` filters first, then ranks them by
     * similarity. Pass either `query` for managed indexes or `queryVector` for external/custom vectors.
     *
     * @param params - Vector search parameters including the indexed property, labels, and query input
     */
    vectorSearch: async <S extends Schema = any>(
      params: VectorSearchParams,
      transaction?: Transaction | string
    ): Promise<DBRecordsArrayInstance<S>> => {
      const txId = pickTransactionId(transaction)
      const path = `/ai/search`
      const payload = {
        method: 'POST',
        headers: Object.assign({}, buildTransactionHeader(txId)),
        requestData: params
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<VectorSearchResult<S>[]>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      const dbRecordInstances = (response.data ?? []).map((r) => new DBRecordInstance<S>(r as DBRecord<S>))
      return new DBRecordsArrayInstance<S>(dbRecordInstances, response.total ?? 0)
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
          return new DBRecordInstance<S>(r)
        })

        return new DBRecordsArrayInstance<S>(dbRecordInstances, response.total) as Result
      } else {
        return new DBRecordInstance<S>(response.data as DBRecord<S>) as Result
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

      return new DBRecordInstance<S, Q>(record)
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

      return new DBRecordInstance<S, Q>(record)
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
        data: rawData,
        options,
        vectors
      }: {
        target: DBRecordTarget
        label: string
        data: InferSchemaTypesWrite<S> | Array<PropertyDraft>
        options?: Omit<DBRecordCreationOptions, 'returnResult' | 'capitalizeLabels' | 'relationshipType'>
        vectors?: VectorEntry[]
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

      const data = getOwnProperties(removeUndefinedDeep(rawData))

      if (isArray(data) && data.every(isPropertyDraft)) {
        payload.requestData = { label, properties: data, ...(vectors?.length && { vectors }) }
      } else if (isFlatObject(data)) {
        payload.requestData = { label, data, options, ...(vectors?.length && { vectors }) }
      } else if (isObject(data)) {
        throw new Error('Provided data is not a flat object. Consider to use `importJson` method.')
      } else {
        throw new Error('Provided data is not valid.')
      }

      this.logger?.({ requestId, path, ...payload })
      const response = await this.fetcher<ApiResponse<DBRecord<S> | undefined>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      if (response?.success && response?.data) {
        return new DBRecordInstance<S>(response.data)
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
        data: rawData,
        options
      }: {
        target: DBRecordTarget
        label: string
        data: Partial<InferSchemaTypesWrite<S>> | Array<PropertyDraft>
        options?: Omit<DBRecordCreationOptions, 'returnResult' | 'capitalizeLabels' | 'relationshipType'>
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

      const data = getOwnProperties(removeUndefinedDeep(rawData))

      if (isArray(data) && data.every(isPropertyDraft)) {
        payload.requestData = { label, properties: data }
      } else if (isFlatObject(data)) {
        payload.requestData = { label, data, options }
      } else if (isObject(data)) {
        throw new Error('Provided data is not a flat object. Consider to use `importJson` method.')
      } else {
        throw new Error('Provided data is not valid.')
      }

      this.logger?.({ requestId, path, ...payload })
      const response = await this.fetcher<ApiResponse<DBRecord<S> | undefined>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      if (response?.success && response?.data) {
        return new DBRecordInstance<S>(response.data)
      }

      return new DBRecordInstance<S>()
    },
    /**
     * Upserts a record: attempts to find an existing record matching mergeBy property values (and optional label)
     * If found: mergeStrategy determines behavior.
     *  - 'rewrite': replaces all existing own properties with incoming (like set)
     *  - 'append': updates/adds provided properties, keeps others
     * If not found: creates a new record.
     * @param label - The label/type of the record
     * @param data - Flat object or array of property drafts
     * @param options.mergeBy - Property names to match on; If `[]`, all incoming keys are used for matching.
     * @param options.mergeStrategy - 'rewrite' | 'append'
     * @param transaction - Optional transaction for atomic operations
     */
    upsert: async <S extends Schema = any>(
      {
        label,
        data: rawData,
        options,
        vectors
      }: {
        label?: string
        data: InferSchemaTypesWrite<S> | Array<PropertyDraft>
        options?: Omit<DBRecordCreationOptions, 'returnResult' | 'capitalizeLabels' | 'relationshipType'> & {
          mergeBy?: string[]
          mergeStrategy?: 'rewrite' | 'append'
        }
        vectors?: VectorEntry[]
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

      const data = getOwnProperties(removeUndefinedDeep(rawData))

      const defaultOptions = {
        ...options,
        mergeBy: options?.mergeBy ?? []
      }

      if (isArray(data) && data.every(isPropertyDraft)) {
        payload.requestData = {
          label,
          properties: data,
          options: defaultOptions,
          ...(vectors?.length && { vectors })
        }
      } else if (isFlatObject(data)) {
        payload.requestData = { label, data, options: defaultOptions, ...(vectors?.length && { vectors }) }
      } else if (isObject(data)) {
        throw new Error(
          'Provided data is not a flat object. Upsert supports flat objects or property drafts array.'
        )
      } else {
        throw new Error('Provided data is not valid.')
      }

      this.logger?.({ requestId, path, ...payload })
      const response = await this.fetcher<ApiResponse<DBRecord<S> | undefined>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      if (response?.success && response?.data) {
        return new DBRecordInstance<S>(response.data)
      }

      return new DBRecordInstance<S>()
    }
  }

  /**
   * API methods for managing relations between records
   */
  public relationships = {
    /**
     * Reviews and manages relationship patterns inferred from the current project schema.
     */
    patterns: {
      /**
       * Lists inferred relationship patterns, schema relationships, and analysis status.
       */
      list: async () => {
        const path = `/relationships/patterns`
        const payload = { method: 'GET', headers: {} }
        const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
        this.logger?.({ requestId, path, ...payload })

        const response = await this.fetcher<ApiResponse<RelationshipPatternListResponse>>(path, payload)
        this.logger?.({ requestId, path, ...payload, responseData: response.data })

        return response
      },

      /**
       * Queues schema analysis to generate relationship pattern suggestions.
       */
      analyze: async () => {
        const path = `/relationships/patterns/analyze`
        const payload = { method: 'POST', headers: {}, requestData: {} }
        const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
        this.logger?.({ requestId, path, ...payload })

        const response = await this.fetcher<ApiResponse<{ queued: true }>>(path, payload)
        this.logger?.({ requestId, path, ...payload, responseData: response.data })

        return response
      },

      /**
       * Approves and applies a suggested relationship pattern.
       */
      approve: async (id: string) => {
        const path = `/relationships/patterns/${encodeURIComponent(id)}/approve`
        const payload = { method: 'POST', headers: {}, requestData: {} }
        const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
        this.logger?.({ requestId, path, ...payload })

        const response = await this.fetcher<ApiResponse<RelationshipPattern | undefined>>(path, payload)
        this.logger?.({ requestId, path, ...payload, responseData: response.data })

        return response
      },

      /**
       * Ignores a suggested relationship pattern without applying it.
       */
      ignore: async (id: string) => {
        const path = `/relationships/patterns/${encodeURIComponent(id)}/ignore`
        const payload = { method: 'POST', headers: {}, requestData: {} }
        const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
        this.logger?.({ requestId, path, ...payload })

        const response = await this.fetcher<ApiResponse<RelationshipPattern | undefined>>(path, payload)
        this.logger?.({ requestId, path, ...payload, responseData: response.data })

        return response
      },

      /**
       * Deletes a saved relationship pattern.
       * Pass deleteExisting to also remove relationships materialized by the pattern.
       */
      delete: async (id: string, options?: DeleteRelationshipPatternOptions) => {
        const query = options?.deleteExisting ? '?deleteExisting=true' : ''
        const path = `/relationships/patterns/${encodeURIComponent(id)}${query}`
        const payload = { method: 'DELETE', headers: {} }
        const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
        this.logger?.({ requestId, path, ...payload })

        const response = await this.fetcher<ApiResponse<{ deleted: true }>>(path, payload)
        this.logger?.({ requestId, path, ...payload, responseData: response.data })

        return response
      }
    },

    /**
     * Creates many relationships by matching source and target records by keys
     * @param data - Configuration of source/target match and relation details
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise with the API response message
     */
    createMany: async (
      data: {
        source: { label: string; key?: string; where?: Where }
        target: { label: string; key?: string; where?: Where }
        type?: string
        direction?: RelationDirection
        properties?: Record<string, unknown>
        manyToMany?: boolean
      },
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)
      const path = `/relationships/create-many`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: data ?? {}
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<{ message: string }>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    /**
     * Deletes many relationships by matching source and target records by keys or by manyToMany filter
     */
    deleteMany: async (
      data: {
        source: { label: string; key?: string; where?: Where }
        target: { label: string; key?: string; where?: Where }
        type?: string
        direction?: RelationDirection
        manyToMany?: boolean
      },
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)
      const path = `/relationships/delete-many`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: data ?? {}
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<{ message: string }>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },
    /**
     * Searches for relations matching the query criteria.
     * Pagination (`limit`/`skip`) is part of the search query body, same as records search.
     * @param searchQuery - Query to identify relations
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise with the API response containing matched relations
     */
    find: async <S extends Schema = any>(
      searchQuery: RelationshipSearchQuery<S> = {},
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)
      const path = `/relationships/search`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: searchQuery ?? {}
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
        requestData: searchQuery ?? {}
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
     * @param searchQuery - Query parameters to filter properties
     * @param searchQuery.query - Filter query for values
     * @param transaction - Optional transaction for atomic operations
     * @returns Promise with the API response containing the property and its values
     */
    values: async (
      id: string,
      searchQuery?: SearchQuery & { query?: string; orderBy?: OrderDirection },
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)
      const path = `/properties/${id}/values`

      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: searchQuery ?? {}
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<Property & PropertyValuesData>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    }
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
        requestData: searchQuery ?? {}
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
        requestData: isObject(config) && 'ttl' in config ? config : {}
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const transaction = await this.fetcher<ApiResponse<{ id: string }>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: transaction.data })

      return new Transaction(transaction.data.id)
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

      return new Transaction(transaction.data.id)
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

  public settings = {
    get: async () => {
      const path = `/sdk/settings`
      const payload = {
        method: 'GET'
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<State['serverSettings']>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    }
  }

  // Only for managed/custom db instances connected to cloud
  public query = {
    /**
     * Executes a raw query against the underlying database engine.
     *
     * NOTE: This endpoint is cloud-only — available only on the RushDB managed
     * service or when your project is connected to a custom database through
     * RushDB Cloud. It will not work for self-hosted or local-only deployments.
     *
     * @param param0 - Object containing the query and optional params
     * @param param0.query - Query string to execute
     * @param param0.params - Optional parameters to pass to the query
     * @param transaction - Optional transaction id or Transaction instance to run the query in
     * @returns ApiResponse<any> - Raw result returned by the server (wrapped in ApiResponse)
     */
    raw: async <T extends any = any>(
      { query, params }: { query: string; params?: FlatObject },
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)
      const path = `/query/raw`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: { query, params }
      }

      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<T>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    }
  }

  /**
   * API methods for AI-assisted graph exploration.
   */
  public ai = {
    /**
     * Returns the full graph schema as structured JSON.
     * Each item contains the label name, record count, properties with value ranges/samples,
     * and cross-label relationships with direction.
     * Properties may include a `vectorIndexes` array when one or more embedding indexes
     * exist for that property — a non-empty array means the property is queryable with
     * `db.records.vectorSearch()`. Each entry exposes: id, sourceType, similarityFunction, dimensions,
     * status, and modelKey.
     * Use property `id` fields to pass to db.properties.values() for deeper drill-down.
     * @param params - Optional filter. `labels` scopes to specific labels only.
     *                 `force: true` bypasses the 1-hour schema cache and triggers a full recalculation.
     * @param transaction - Optional transaction for atomic operations
     */
    getSchema: async (
      params?: { labels?: string[]; force?: boolean },
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)
      const path = `/ai/schema`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: params ?? {}
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<unknown[]>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },

    /**
     * Returns the full graph schema as compact Markdown tables.
     * Token-efficient — intended for direct LLM consumption.
     * Includes: labels with counts, properties with types and value ranges/samples,
     * cross-label relationship map, and a "Semantic Search" column per property that shows
     * `sourceType similarityFunction dimensionsd [status]` (e.g. `managed cosine 1536d [ready]`)
     * for indexed properties, or `—` when no embedding index exists.
     * @param params - Optional filter. `labels` scopes to specific labels only.
     *                 `force: true` bypasses the 1-hour schema cache and triggers a full recalculation.
     * @param transaction - Optional transaction for atomic operations
     */
    getSchemaMarkdown: async (
      params?: { labels?: string[]; force?: boolean },
      transaction?: Transaction | string
    ) => {
      const txId = pickTransactionId(transaction)
      const path = `/ai/schema/md`
      const payload = {
        headers: Object.assign({}, buildTransactionHeader(txId)),
        method: 'POST',
        requestData: params ?? {}
      }
      const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId, path, ...payload })

      const response = await this.fetcher<ApiResponse<string>>(path, payload)
      this.logger?.({ requestId, path, ...payload, responseData: response.data })

      return response
    },

    /**
     * Embedding Index management methods.
     */
    indexes: {
      /**
       * Lists all embedding index policies configured for the current project.
       */
      find: async () => {
        const path = `/ai/indexes`
        const payload = { method: 'GET', headers: {} }
        const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
        this.logger?.({ requestId, path, ...payload })

        const response = await this.fetcher<ApiResponse<EmbeddingIndex[]>>(path, payload)
        this.logger?.({ requestId, path, ...payload, responseData: response.data })

        return response
      },

      /**
       * Creates a new embedding index policy for a string property.
       * @param params.propertyName - Name of the property to index
       * @param params.modelKey - Embedding model identifier (e.g. 'text-embedding-3-small')
       * @param params.dimensions - Vector dimensionality produced by the model
       */
      create: async (params: CreateEmbeddingIndexParams) => {
        const path = `/ai/indexes`
        const { external, ...rest } = params
        const resolvedParams = external === true ? { ...rest, sourceType: 'external' as const } : rest
        const payload = {
          method: 'POST',
          headers: {},
          requestData: resolvedParams
        }
        const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
        this.logger?.({ requestId, path, ...payload })

        const response = await this.fetcher<ApiResponse<EmbeddingIndex>>(path, payload)
        this.logger?.({ requestId, path, ...payload, responseData: response.data })

        return response
      },

      /**
       * Upserts external vectors for a specific embedding index.
       * @param id - The target embedding index ID
       * @param params.items - Array of { recordId, vector }
       */
      upsertVectors: async (id: string, params: UpsertEmbeddingVectorsParams) => {
        const path = `/ai/indexes/${id}/vectors/upsert`
        const payload = {
          method: 'POST',
          headers: {},
          requestData: params
        }
        const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
        this.logger?.({ requestId, path, ...payload })

        const response = await this.fetcher<ApiResponse<UpsertEmbeddingVectorsResult>>(path, payload)
        this.logger?.({ requestId, path, ...payload, responseData: response.data })

        return response
      },

      /**
       * Deletes an embedding index policy by ID.
       * @param id - The ID of the embedding index to delete
       */
      delete: async (id: string) => {
        const path = `/ai/indexes/${id}`
        const payload = { method: 'DELETE', headers: {} }
        const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
        this.logger?.({ requestId, path, ...payload })

        const response = await this.fetcher<ApiResponse<{ deleted: boolean }>>(path, payload)
        this.logger?.({ requestId, path, ...payload, responseData: response.data })

        return response
      },

      /**
       * Returns Neo4j-level statistics for an embedding index.
       * @param id - The ID of the embedding index
       */
      stats: async (id: string) => {
        const path = `/ai/indexes/${id}/stats`
        const payload = { method: 'GET', headers: {} }
        const requestId = typeof this.logger === 'function' ? generateRandomId() : ''
        this.logger?.({ requestId, path, ...payload })

        const response = await this.fetcher<ApiResponse<EmbeddingIndexStats>>(path, payload)
        this.logger?.({ requestId, path, ...payload, responseData: response.data })

        return response
      }
    },

    /**
     * Performs AI-assisted smart search from natural language.
     *
     * RushDB converts the prompt into a SearchQuery using the project schema, executes it,
     * and returns matching records with the generated SearchQuery attached to the result.
     *
     * Use `db.records.vectorSearch({ ... })` for direct vector similarity over embedding indexes.
     *
     * @param prompt - Natural-language search request to convert into a SearchQuery and execute
     * @param options - Optional smart-search context and transaction
     */
    search: async <S extends Schema = any>(
      prompt: string,
      options?: SmartSearchOptions<S> & { transaction?: Transaction | string }
    ): Promise<DBRecordsArrayInstance<S>> => {
      const txId = pickTransactionId(options?.transaction)
      const generatePath = `/ai/search-query`
      const generatePayload = {
        method: 'POST',
        headers: Object.assign({}, buildTransactionHeader(txId)),
        requestData: {
          prompt,
          currentQuery: options?.currentQuery
        }
      }
      const generateRequestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId: generateRequestId, path: generatePath, ...generatePayload })

      const generated = await this.fetcher<ApiResponse<SmartSearchQueryResponse<S>>>(
        generatePath,
        generatePayload
      )
      this.logger?.({
        requestId: generateRequestId,
        path: generatePath,
        ...generatePayload,
        responseData: generated.data
      })

      const searchQuery = generated.data.searchQuery
      const searchPath = `/records/search`
      const searchPayload = {
        method: 'POST',
        headers: Object.assign({}, buildTransactionHeader(txId)),
        requestData: searchQuery ?? {}
      }
      const searchRequestId = typeof this.logger === 'function' ? generateRandomId() : ''
      this.logger?.({ requestId: searchRequestId, path: searchPath, ...searchPayload })

      const response = await this.fetcher<ApiResponse<Array<DBRecord<S>>>>(searchPath, searchPayload)
      this.logger?.({
        requestId: searchRequestId,
        path: searchPath,
        ...searchPayload,
        responseData: response.data
      })

      const dbRecordInstances = (response.data ?? []).map((r) => new DBRecordInstance<S>(r))
      return new DBRecordsArrayInstance<S>(
        dbRecordInstances,
        response.total ?? 0,
        searchQuery,
        generated.data.warnings
      )
    }
  }
}
