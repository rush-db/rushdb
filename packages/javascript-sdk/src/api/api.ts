import type { HttpClient } from '../network/HttpClient.js'
import type {
  DBRecord,
  RelationDetachOptions,
  RelationOptions,
  RelationTarget
} from '../sdk/record.js'
import type { UserProvidedConfig } from '../sdk/types.js'
import type {
  PropertyValue,
  PropertyWithValue,
  SearchQuery,
  Schema,
  InferSchemaTypesWrite,
  MaybeArray
} from '../types/index.js'
import type { ApiResponse, RecordsApi } from './types.js'

import {
  isArray,
  isEmptyObject,
  isObject,
  isObjectFlat,
  isString,
  toBoolean
} from '../common/utils.js'
import { createFetcher } from '../network/index.js'
import { EmptyTargetError } from '../sdk/errors.js'
import {
  DBRecordsBatchDraft,
  DBRecordDraft,
  DBRecordInstance,
  DBRecordsArrayInstance
} from '../sdk/record.js'
import { Transaction } from '../sdk/transaction.js'
import { createApi } from './create-api.js'
import {
  buildUrl,
  createSearchParams,
  isTransaction,
  normalizeRecord,
  pickTransaction,
  prepareProperties
} from './utils.js'

export class RestAPI {
  public api: ReturnType<typeof createApi>
  public fetcher: ReturnType<typeof createFetcher>

  public records: RecordsApi

  constructor(token?: string, config?: UserProvidedConfig & { httpClient: HttpClient }) {
    this.fetcher = null as unknown as ReturnType<typeof createFetcher>

    if (config?.httpClient) {
      const url = buildUrl(config)
      this.fetcher = createFetcher({
        httpClient: config.httpClient,
        token,
        url
      })
    }
    this.api = createApi(this.fetcher)

    this.records = {
      attach: async (
        sourceId: string,
        target: RelationTarget,
        options?: RelationOptions,
        transaction?: Transaction | string
      ) => {
        // target is MaybeArray<DBRecordInstance>
        if (target instanceof DBRecordInstance) {
          const id = target.data?.__id
          if (id) {
            return await this.api.records.attach(sourceId, id, options, transaction)
          } else {
            throw new EmptyTargetError('Attach error: Target id is empty')
          }
        } else if (isArray(target) && target.every((r) => r instanceof DBRecordInstance)) {
          const ids = target.map((r) => (r as DBRecordInstance).data?.__id).filter(toBoolean)
          if (ids.length) {
            return await this.api.records.attach(sourceId, ids as string[], options, transaction)
          } else {
            throw new EmptyTargetError('Attach error: Target ids are empty')
          }
        }

        // target is DBRecordsArrayInstance
        else if (target instanceof DBRecordsArrayInstance) {
          const ids = target.data?.map((r) => r.__id).filter(Boolean)
          if (ids?.length) {
            return await this.api.records.attach(sourceId, ids, options, transaction)
          } else {
            throw new EmptyTargetError('Attach error: Target ids are empty')
          }
        }

        // target is MaybeArray<DBRecord>
        else if (isObject(target) && '__id' in target) {
          return await this.api.records.attach(sourceId, target.__id, options, transaction)
        } else if (isArray(target) && target.every((r) => isObject(r) && '__id' in r)) {
          const ids = target?.map((r) => (r as DBRecord).__id).filter(Boolean)
          if (ids?.length) {
            return await this.api.records.attach(sourceId, ids, options, transaction)
          } else {
            throw new EmptyTargetError('Attach error: Target ids are empty')
          }
        }

        // target is MaybeArray<string>
        else {
          return await this.api.records.attach(
            sourceId,
            target as MaybeArray<string>,
            options,
            transaction
          )
        }
      },

      create: async <S extends Schema = any>(
        labelOrData: DBRecordDraft | string,
        maybeDataOrTransaction?: Transaction | InferSchemaTypesWrite<S> | string,
        transaction?: Transaction | string
      ): Promise<DBRecordInstance<S>> => {
        let response

        if (labelOrData instanceof DBRecordDraft) {
          response = await this.api?.records.create<S>(
            labelOrData,
            pickTransaction(maybeDataOrTransaction)
          )
        }

        if (!response && isString(labelOrData)) {
          if (isObjectFlat(maybeDataOrTransaction)) {
            const normalizedRecord = normalizeRecord({
              label: labelOrData,
              payload: maybeDataOrTransaction as Record<string, PropertyValue>
            })

            response = await this.api?.records.create<S>(
              new DBRecordDraft(
                normalizedRecord as { label: string; properties: PropertyWithValue[] }
              ),
              transaction
            )
          } else if (isObject(maybeDataOrTransaction)) {
            throw Error('Provided data is not a flat object. Consider to use `createMany` method.')
          }
        }

        if (response?.success && response?.data) {
          const result = new DBRecordInstance<S>(response.data)
          result.init(this)
          return result
        }

        return new DBRecordInstance<S>()
      },

      createMany: async <S extends Schema = any>(
        labelOrData: DBRecordsBatchDraft | string,
        maybeDataOrTransaction?:
          | Transaction
          | MaybeArray<InferSchemaTypesWrite<S>>
          | string,
        transaction?: Transaction | string
      ): Promise<DBRecordsArrayInstance<S>> => {
        let response

        if (labelOrData instanceof DBRecordsBatchDraft) {
          response = await this.api?.records.createMany<S>(
            labelOrData,
            pickTransaction(maybeDataOrTransaction)
          )
        }

        if (
          !response &&
          isString(labelOrData) &&
          (isArray(maybeDataOrTransaction) || isObject(maybeDataOrTransaction))
        ) {
          const data = new DBRecordsBatchDraft({
            label: labelOrData,
            payload: maybeDataOrTransaction
          })
          response = await this.api?.records.createMany<S>(data, transaction)
        }

        if (response?.success && response?.data) {
          const result = new DBRecordsArrayInstance<S>(response.data, response.total)
          result.init(this)
          return result
        }

        return new DBRecordsArrayInstance<S>([])
      },

      delete: async <S extends Schema = any>(
        searchParams: SearchQuery<S>,
        transaction?: Transaction | string
      ) => {
        if (isEmptyObject(searchParams.where)) {
          throw new EmptyTargetError(
            `You must specify criteria to delete records. Empty criteria are not allowed. If this was intentional, use the Dashboard instead.`
          )
        }

        return this.api?.records.delete(searchParams, transaction)
      },

      deleteById: async (
        idOrIds: MaybeArray<string>,
        transaction?: Transaction | string
      ) => {
        return this.api?.records.deleteById(idOrIds, transaction)
      },

      detach: async (
        sourceId: string,
        target: RelationTarget,
        options?: RelationDetachOptions,
        transaction?: Transaction | string
      ) => {
        // target is MaybeArray<DBRecordInstance>
        if (target instanceof DBRecordInstance) {
          const id = target.data?.__id
          if (id) {
            return await this.api.records.detach(sourceId, id, options, transaction)
          } else {
            throw new EmptyTargetError('Detach error: Target id is empty')
          }
        } else if (isArray(target) && target.every((r) => r instanceof DBRecordInstance)) {
          const ids = target.map((r) => (r as DBRecordInstance).data?.__id).filter(Boolean)
          if (ids.length) {
            return await this.api.records.detach(sourceId, ids as string[], options, transaction)
          } else {
            throw new EmptyTargetError('Detach error: Target ids are empty')
          }
        }

        // target is DBRecordsArrayInstance
        else if (target instanceof DBRecordsArrayInstance) {
          const ids = target.data?.map((r) => r.__id).filter(Boolean)
          if (ids?.length) {
            return await this.api.records.detach(sourceId, ids, options, transaction)
          } else {
            throw new EmptyTargetError('Detach error: Target ids are empty')
          }
        }

        // target is MaybeArray<DBRecord>
        else if (isObject(target) && '__id' in target) {
          return await this.api.records.detach(sourceId, target.__id, options, transaction)
        } else if (isArray(target) && target.every((r) => isObject(r) && '__id' in r)) {
          const ids = target?.map((r) => (r as DBRecord).__id).filter(Boolean)
          if (ids?.length) {
            return await this.api.records.detach(sourceId, ids, options, transaction)
          } else {
            throw new EmptyTargetError('Detach error: Target ids are empty')
          }
        }

        // target is MaybeArray<string>
        else {
          return await this.api.records.detach(
            sourceId,
            target as MaybeArray<string>,
            options,
            transaction
          )
        }
      },

      export: async <S extends Schema = any>(
        searchParams: SearchQuery<S>,
        transaction?: Transaction | string
      ) => {
        return this.api?.records.export(searchParams, transaction)
      },

      find: async <S extends Schema = any>(
        labelOrSearchParams?: SearchQuery<S> | string,
        searchParamsOrTransaction?: SearchQuery<S> | Transaction | string,
        transaction?: Transaction | string
      ): Promise<DBRecordsArrayInstance<S>> => {
        const isTransactionParam = isTransaction(searchParamsOrTransaction)
        const { id, searchParams } = createSearchParams<S>(
          labelOrSearchParams,
          searchParamsOrTransaction
        )
        const tx = isTransactionParam ? searchParamsOrTransaction : transaction
        const response = await this.api?.records.find<S>({ id, searchParams }, tx)

        const result = new DBRecordsArrayInstance<S>(
          response.data,
          response.total,
          searchParamsOrTransaction as SearchQuery<S>
        )
        result.init(this)
        return result
      },

      findById: async <
        S extends Schema = Schema,
        Arg extends MaybeArray<string> = MaybeArray<string>,
        Result = Arg extends string[] ? DBRecordsArrayInstance<S>
        : DBRecordInstance<S>
      >(
        idOrIds: Arg,
        transaction?: Transaction | string
      ): Promise<Result> => {
        if (isArray(idOrIds)) {
          const response = (await this.api?.records.findById<S>(
            idOrIds,
            transaction
          )) as ApiResponse<DBRecord<S>[]>
          const result = new DBRecordsArrayInstance<S>(response.data, response.total)
          result.init(this)
          return result as Result
        } else {
          const response = (await this.api?.records.findById<S>(
            idOrIds,
            transaction
          )) as ApiResponse<DBRecord<S>>
          const result = new DBRecordInstance<S>(response.data)
          result.init(this)
          return result as Result
        }
      },

      findOne: async <S extends Schema = any>(
        labelOrSearchParams?: SearchQuery<S> | string,
        searchParamsOrTransaction?: SearchQuery<S> | Transaction | string,
        transaction?: Transaction | string
      ): Promise<DBRecordInstance<S>> => {
        const isTransactionParam = isTransaction(searchParamsOrTransaction)
        const { searchParams } = createSearchParams<S>(
          labelOrSearchParams,
          searchParamsOrTransaction
        )
        const tx = isTransactionParam ? searchParamsOrTransaction : transaction
        const response = await this.api?.records.findOne<S>(searchParams, tx)

        const result = new DBRecordInstance<S>(
          response.data,
          searchParamsOrTransaction as SearchQuery<S>
        )
        result.init(this)
        return result
      },

      findUniq: async <S extends Schema = any>(
        labelOrSearchParams?: SearchQuery<S> | string,
        searchParamsOrTransaction?: SearchQuery<S> | Transaction | string,
        transaction?: Transaction | string
      ): Promise<DBRecordInstance<S>> => {
        const isTransactionParam = isTransaction(searchParamsOrTransaction)
        const { searchParams } = createSearchParams<S>(
          labelOrSearchParams,
          searchParamsOrTransaction
        )
        const tx = isTransactionParam ? searchParamsOrTransaction : transaction
        const response = await this.api?.records.findUniq<S>(searchParams, tx)

        const result = new DBRecordInstance<S>(
          response.data,
          searchParamsOrTransaction as SearchQuery<S>
        )
        result.init(this)
        return result
      },

      properties: async (id: string, transaction?: Transaction | string) => {
        return this.api?.records.properties(id, transaction)
      },

      relations: async (id: string, transaction?: Transaction | string) => {
        return await this.api.records.relations(id, transaction)
      },

      set: async <S extends Schema = any>(
        id: string,
        data: DBRecordDraft | InferSchemaTypesWrite<S>,
        transaction?: Transaction | string
      ) => {
        let response

        if (data instanceof DBRecordDraft) {
          response = await this.api?.records.set<S>(id, data, transaction)
        } else if (isObjectFlat(data)) {
          const properties = prepareProperties(data)

          response = await this.api?.records.set<S>(
            id,
            new DBRecordDraft({ properties } as {
              label: string
              properties: PropertyWithValue[]
            }),
            transaction
          )
        } else if (isObject(data)) {
          throw Error('Provided data is not a flat object. Consider to use `createMany` method.')
        }

        if (response?.success && response?.data) {
          const result = new DBRecordInstance<S>(response.data)
          result.init(this)
          return result
        }

        return new DBRecordInstance<S>()
      },

      update: async <S extends Schema = any>(
        id: string,
        data: DBRecordDraft | Partial<InferSchemaTypesWrite<S>>,
        transaction?: Transaction | string
      ) => {
        let response

        if (data instanceof DBRecordDraft) {
          response = await this.api?.records.update<S>(id, data, transaction)
        } else if (isObjectFlat(data)) {
          const properties = prepareProperties(data)

          const recordDraft = new DBRecordDraft({ properties } as {
            label: string
            properties: PropertyWithValue[]
          })

          response = await this.api?.records.update<S>(id, recordDraft, transaction)
        } else if (isObject(data)) {
          throw Error('Provided data is not a flat object. Consider to use `createMany` method.')
        }

        if (response?.success && response?.data) {
          const result = new DBRecordInstance<S>(response.data)
          result.init(this)
          return result
        }

        return new DBRecordInstance<S>()
      }
    }
  }

  public relations = {
    find: async <S extends Schema = any>({
      pagination,
      search,
      transaction
    }: {
      pagination?: Pick<SearchQuery, 'limit' | 'skip'>
      search?: SearchQuery<S>
      transaction?: Transaction | string
    }) => {
      const { searchParams } = createSearchParams<S>(search)

      const tx = pickTransaction(transaction)

      return await this.api.relations.find(searchParams, pagination, tx)
    }
  }

  public properties = {
    delete: async (id: string, transaction?: Transaction | string) => {
      return this.api?.properties.delete(id, transaction)
    },
    find: async <S extends Schema = any>(
      searchParams: SearchQuery<S>,
      transaction?: Transaction | string
    ) => {
      return this.api?.properties.find<S>(searchParams, transaction)
    },
    findById: async (id: string, transaction?: Transaction | string) => {
      return this.api?.properties.findById(id, transaction)
    },
    values: async (id: string, transaction?: Transaction | string) => {
      return this.api?.properties.values(id, transaction)
    }
  }

  public labels = {
    find: async <S extends Schema = any>(
      searchParams: SearchQuery<S>,
      transaction?: Transaction | string
    ) => {
      return this.api.labels.find<S>(searchParams, transaction)
    }
  }

  public tx = {
    begin: async (config?: Partial<{ ttl: number }>) => {
      const transaction = await this.api?.tx.begin(config)

      const result = new Transaction(transaction.data.id)
      result.init(this)
      return result
    },
    commit: async (id: string) => this.api?.tx.commit(id),
    get: async (id: string) => {
      const transaction = await this.api?.tx.get(id)

      const result = new Transaction(transaction.data.id)
      result.init(this)
      return result
    },
    rollback: async (id: string) => this.api?.tx.commit(id)
  }
}
