import type {
  DBRecordsBatchDraft,
  DBRecordDraft,
  DBRecordInstance,
  DBRecordTarget,
  DBRecordsArrayInstance,
  Relation,
  RelationDetachOptions,
  RelationOptions,
  RelationTarget
} from '../sdk/record.js'
import type { Transaction } from '../sdk/transaction.js'
import type {
  Property,
  SearchQuery,
  Schema,
  InferSchemaTypesWrite,
  MaybeArray
} from '../types/index.js'

export type ApiResponse<T, E = Record<string, any>> = {
  data: T
  success: boolean
  total?: number
} & E

export type RecordsApi = {
  attach(
    source: DBRecordTarget,
    target: RelationTarget,
    options?: RelationOptions,
    transaction?: Transaction | string
  ): Promise<ApiResponse<{ message: string }>>

  create<S extends Schema = any>(
    label: string,
    data: InferSchemaTypesWrite<S>,
    transaction?: Transaction | string
  ): Promise<DBRecordInstance<S>>
  create<S extends Schema = any>(
    labelOrData: DBRecordDraft | string,
    maybeDataOrTransaction?: Transaction | InferSchemaTypesWrite<S> | string,
    transaction?: Transaction | string
  ): Promise<DBRecordInstance<S>>

  createMany<S extends Schema = any>(
    label: string,
    data: InferSchemaTypesWrite<S>[],
    transaction?: Transaction | string
  ): Promise<DBRecordsArrayInstance<S>>
  createMany<S extends Schema = any>(
    labelOrData: DBRecordsBatchDraft | string,
    maybeDataOrTransaction?:
      | Transaction
      | MaybeArray<InferSchemaTypesWrite<S>>
      | string,
    transaction?: Transaction | string
  ): Promise<DBRecordsArrayInstance<S>>

  delete<S extends Schema = any>(
    searchParams: SearchQuery<S>,
    transaction?: Transaction | string
  ): Promise<ApiResponse<{ message: string }>>

  deleteById(
    idOrIds: MaybeArray<string>,
    transaction?: Transaction | string
  ): Promise<ApiResponse<{ message: string }>>

  detach(
    source: DBRecordTarget,
    target: RelationTarget,
    options?: RelationDetachOptions,
    transaction?: Transaction | string
  ): Promise<ApiResponse<{ message: string }>>

  export<S extends Schema = any>(
    searchParams?: SearchQuery<S>,
    transaction?: Transaction | string
  ): Promise<ApiResponse<{ dateTime: string; fileContent: string }>>

  find<S extends Schema = any>(
    label: string,
    searchParams?: SearchQuery<S>,
    transaction?: Transaction | string
  ): Promise<DBRecordsArrayInstance<S>>
  find<S extends Schema = any>(
    labelOrSearchParams: SearchQuery<S> | string,
    transaction?: Transaction | string
  ): Promise<DBRecordsArrayInstance<S>>
  find<S extends Schema = any>(
    searchParams: SearchQuery<S>,
    transaction?: Transaction | string
  ): Promise<DBRecordsArrayInstance<S>>

  findById<S extends Schema = any>(
    id: string,
    transaction?: Transaction | string
  ): Promise<DBRecordInstance<S>>
  findById<S extends Schema = any>(
    ids: string[],
    transaction?: Transaction | string
  ): Promise<DBRecordsArrayInstance<S>>

  findOne<S extends Schema = any>(
    label: string,
    searchParams: SearchQuery<S> & { labels?: never; limit?: never; skip?: never },
    transaction?: Transaction | string
  ): Promise<DBRecordInstance<S>>
  findOne<S extends Schema = any>(
    labelOrSearchParams: (SearchQuery<S> & { limit?: never; skip?: never }) | string,
    transaction?: Transaction | string
  ): Promise<DBRecordInstance<S>>
  findOne<S extends Schema = any>(
    searchParams: SearchQuery<S> & { limit?: never; skip?: never },
    transaction?: Transaction | string
  ): Promise<DBRecordInstance<S>>

  findUniq<S extends Schema = any>(
    label: string,
    searchParams: SearchQuery<S> & { labels?: never; limit?: never; skip?: never },
    transaction?: Transaction | string
  ): Promise<DBRecordInstance<S>>
  findUniq<S extends Schema = any>(
    labelOrSearchParams: (SearchQuery<S> & { limit?: never; skip?: never }) | string,
    transaction?: Transaction | string
  ): Promise<DBRecordInstance<S>>
  findUniq<S extends Schema = any>(
    searchParams: SearchQuery<S> & { limit?: never; skip?: never },
    transaction?: Transaction | string
  ): Promise<DBRecordInstance<S>>

  properties(
    target: DBRecordTarget,
    transaction?: Transaction | string
  ): Promise<ApiResponse<Property[]>>

  relations(
    target: DBRecordTarget,
    transaction?: Transaction | string
  ): Promise<ApiResponse<Array<Relation>>>

  // overwrite whole record
  set<S extends Schema = any>(
    target: DBRecordTarget,
    data: DBRecordDraft | InferSchemaTypesWrite<S>,
    transaction?: Transaction | string
  ): Promise<DBRecordInstance<S>>

  // partially update
  update<S extends Schema = any>(
    target: DBRecordTarget,
    data: DBRecordDraft | Partial<InferSchemaTypesWrite<S>>,
    transaction?: Transaction | string
  ): Promise<DBRecordInstance<S>>
}
