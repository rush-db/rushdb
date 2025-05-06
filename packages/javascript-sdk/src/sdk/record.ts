import type {
  ExtractAggregateFields,
  FlattenTypes,
  InferSchemaTypesRead,
  InferSchemaTypesWrite,
  MaybeArray,
  OptionalKeysRead,
  PropertyDraft,
  RequiredKeysRead,
  Schema,
  SearchQuery
} from '../types/index.js'
import type { Transaction } from './transaction.js'

import { RestApiProxy } from '../api/rest-api-proxy.js'
import { idToDate, idToTimestamp } from './utils.js'

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

export type DBRecordInferred<S extends Schema, Q extends SearchQuery<S>> =
  Q extends { aggregate: infer A extends Record<string, any> } ? DBRecord<S> & ExtractAggregateFields<A>
  : DBRecord<S>

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
  typeOrTypes?: MaybeArray<string>
}

export type DBRecordWriteOptions = {
  returnResult?: boolean
  suggestTypes?: boolean
  castNumberArraysToVectors?: boolean
  convertNumericValuesToNumbers?: boolean
  capitalizeLabels?: boolean
  relationshipType?: string
}

export class DBRecordInstance<
  S extends Schema = Schema,
  Q extends SearchQuery<S> = SearchQuery<S>
> extends RestApiProxy {
  data?: DBRecordInferred<S, Q>

  constructor(data?: DBRecordInferred<S, Q>) {
    super()
    this.data = data
  }

  id() {
    try {
      return this.data!.__id
    } catch {
      throw new Error(
        `DBRecordInstance: Unable to access 'id'. The Record's \`data.__id\` is missing or incorrect.`
      )
    }
  }

  label() {
    try {
      return this.data!.__label
    } catch {
      throw new Error(
        `DBRecordInstance: Unable to access 'label'. The Record's \`data.__label\` is missing or incorrect.`
      )
    }
  }

  proptypes() {
    try {
      return this.data!.__proptypes
    } catch {
      throw new Error(
        `DBRecordInstance: Unable to access 'proptypes'. The Record's \`data.__proptypes\` is missing or incorrect.`
      )
    }
  }

  date() {
    try {
      return idToDate(this.id())
    } catch {
      throw new Error(
        `DBRecordInstance: Unable to access 'date'. The Record's \`data.__id\` is missing or incorrect.`
      )
    }
  }

  timestamp() {
    try {
      return idToTimestamp(this.id())
    } catch {
      throw new Error(
        `DBRecordInstance: Unable to access 'timestamp'. The Record's \`data.__id\` is missing or incorrect.`
      )
    }
  }

  async delete(transaction?: Transaction | string) {
    if (!this.data) {
      throw new Error('DBRecordInstance: Unable to delete Record. The Record data is undefined.')
    }

    return await this.apiProxy.records.deleteById(this.id(), transaction)
  }

  async update<S extends Schema = Schema>(
    data: Partial<InferSchemaTypesWrite<S>> | Array<PropertyDraft>,
    transaction?: Transaction | string
  ) {
    if (!this.data) {
      throw new Error('DBRecordInstance: Unable to update Record. The Record data is undefined.')
    }

    return this.apiProxy.records.update({ label: this.label(), target: this.id(), data }, transaction)
  }

  async set<S extends Schema = Schema>(
    data: InferSchemaTypesWrite<S> | Array<PropertyDraft>,
    transaction?: Transaction | string
  ) {
    if (!this.data) {
      throw new Error('DBRecordInstance: Unable to set. The Record data is undefined.')
    }

    return this.apiProxy.records.set({ label: this.label(), target: this.id(), data }, transaction)
  }

  async attach(target: RelationTarget, options?: RelationOptions, transaction?: Transaction | string) {
    if (!this.data) {
      throw new Error('DBRecordInstance: Unable to attach Record. The Record data is undefined.')
    }

    return this.apiProxy.records.attach({ source: this.id(), target, options }, transaction)
  }

  async detach(target: RelationTarget, options?: RelationDetachOptions, transaction?: Transaction | string) {
    if (!this.data) {
      throw new Error('DBRecordInstance: Unable to detach Record. The Record data is undefined.')
    }

    return this.apiProxy.records.detach({ source: this.id(), target, options }, transaction)
  }
}

export class DBRecordsArrayInstance<
  S extends Schema = Schema,
  Q extends SearchQuery<S> = SearchQuery<S>
> extends RestApiProxy {
  data?: Array<DBRecordInferred<S, Q>>
  total: number | undefined
  searchParams?: SearchQuery<S>

  constructor(data?: Array<DBRecordInferred<S, Q>>, total?: number, searchParams?: SearchQuery<S>) {
    super()
    this.data = data
    this.total = total
    this.searchParams = searchParams
  }

  // @TODO: Bulk actions: Delete (by ids or searchParams?); Export to csv; Props update for found Records; Attach/Detach
}
