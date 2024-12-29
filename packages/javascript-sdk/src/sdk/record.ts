import type {
  AnyObject,
  PropertyType,
  PropertyValue,
  PropertyWithValue,
  SearchQuery,
  Schema,
  FlattenTypes,
  InferSchemaTypesRead,
  InferSchemaTypesWrite,
  MaybeArray,
  OptionalKeysRead,
  RequiredKeysRead
} from '../types/index.js'
import type { Transaction } from './transaction.js'

import { RestApiProxy } from '../api/rest-api-proxy.js'

type DBRecordInternalProps<S extends Schema = Schema> = {
  readonly __id: string
  readonly __label: string
  readonly __proptypes?: FlattenTypes<
    {
      [Key in RequiredKeysRead<S>]: S[Key]['type']
    } & {
      [Key in OptionalKeysRead<S>]?: S[Key]['type']
    }
  >
}

export type RecordProps<S extends Schema = Schema> =
  S extends S ? InferSchemaTypesRead<S>
  : {
      [K in keyof S]?: S[K]
    }

export type DBRecord<S extends Schema = Schema> = FlattenTypes<
  DBRecordInternalProps<S> & FlattenTypes<RecordProps<S>>
>

// For set, update, attach, detach, delete methods (extending plain id: string)
export type DBRecordTarget = DBRecord<any> | DBRecordInstance<any> | string

export type RelationTarget =
  | DBRecordsArrayInstance<any>
  | MaybeArray<DBRecord<any>>
  | MaybeArray<DBRecordInstance<any>>
  | MaybeArray<string>

export type Relation = {
  sourceId: string
  sourceLabel: string
  targetId: string
  targetLabel: string
  type: string
}

export type RelationDirection = 'in' | 'out'
export type RelationOptions = { direction?: RelationDirection; type?: string }
export type RelationDetachOptions = {
  direction?: RelationDirection
  typeOrTypes?: string | string[]
}

export class DBRecordsBatchDraft {
  label: string
  options?: {
    returnResult?: boolean
    suggestTypes?: boolean
  }
  payload: MaybeArray<AnyObject>

  constructor({
    label,
    options = {
      returnResult: true,
      suggestTypes: true
    },
    payload
  }: {
    label: string
    options?: {
      returnResult?: boolean
      suggestTypes?: boolean
    }
    payload: AnyObject
  }) {
    this.label = label
    this.options = options
    this.payload = payload
  }

  public toJson() {
    return {
      label: this.label,
      options: this.options,
      payload: this.payload
    }
  }
}

export class DBRecordDraft {
  label: string
  properties?: Array<{
    metadata?: string
    name: string
    type: PropertyType
    value: PropertyValue
    valueSeparator?: string
  }>

  constructor({
    label,
    properties = []
  }: {
    label: string
    properties?: Array<
      PropertyWithValue & {
        metadata?: string
        valueSeparator?: string
      }
    >
    relation?: RelationOptions
  }) {
    this.label = label
    this.properties = properties
  }

  public toJson() {
    return {
      label: this.label,
      properties: this.properties
    }
  }
}

export class DBRecordInstance<
  S extends Schema = Schema
> extends RestApiProxy {
  data?: DBRecord<S>
  searchParams?: SearchQuery<S>

  constructor(data?: DBRecord<S>, searchParams?: SearchQuery<S>) {
    super()
    this.data = data
    this.searchParams = searchParams
  }

  async delete(transaction?: Transaction | string) {
    if (this.data) {
      return await this.apiProxy.records.deleteById(this.data.__id, transaction)
    }
  }

  async update<S extends Schema = Schema>(
    partialData: DBRecordDraft | Partial<InferSchemaTypesWrite<S>>,
    transaction?: Transaction | string
  ) {
    if (this.data) {
      return this.apiProxy.records.update(this.data.__id, partialData, transaction)
    }
  }
  async set<S extends Schema = Schema>(
    data: DBRecordDraft | InferSchemaTypesWrite<S>,
    transaction?: Transaction | string
  ) {
    if (this.data) {
      return this.apiProxy.records.set(this.data.__id, data, transaction)
    }
  }

  async attach(
    target: RelationTarget,
    options?: RelationOptions,
    transaction?: Transaction | string
  ) {
    if (this.data) {
      return this.apiProxy.records.attach(this.data.__id, target, options, transaction)
    }
  }

  async detach(
    target: RelationTarget,
    options?: RelationDetachOptions,
    transaction?: Transaction | string
  ) {
    if (this.data) {
      return this.apiProxy.records.detach(this.data.__id, target, options, transaction)
    }
  }
}

export class DBRecordsArrayInstance<
  S extends Schema = Schema
> extends RestApiProxy {
  data?: DBRecord<S>[]
  total: number | undefined
  searchParams?: SearchQuery<S>

  constructor(data?: DBRecord<S>[], total?: number, searchParams?: SearchQuery<S>) {
    super()
    this.data = data
    this.total = total
    this.searchParams = searchParams
  }

  // @TODO: Bulk actions: Delete (by ids or searchParams?); Export to csv; Props update for found Records; Attach/Detach
}
