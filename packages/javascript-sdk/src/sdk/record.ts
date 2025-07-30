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

import { idToDate, idToTimestamp } from './utils.js'
import { toBoolean } from '../common/utils'
import { RushDB } from './sdk.js'

/**
 * Internal properties that are present on all database records.
 * These are system fields managed by RushDB.
 */
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

/**
 * Properties defined by the user's schema.
 */
export type RecordProps<S extends Schema = Schema> =
  S extends S ? InferSchemaTypesRead<S>
  : {
      [K in keyof S]?: S[K]
    }

/**
 * DBRecord with possible aggregation fields based on a search query.
 */
export type DBRecordInferred<S extends Schema, Q extends SearchQuery<S>> =
  Q extends { aggregate: infer A extends Record<string, any> } ? DBRecord<S> & ExtractAggregateFields<A>
  : DBRecord<S>

/**
 * Represents a database record with both system fields and user-defined fields.
 * This is the raw data structure returned from database queries.
 */
export type DBRecord<S extends Schema = Schema> = FlattenTypes<
  DBRecordInternalProps<S> & FlattenTypes<RecordProps<S>>
>

/**
 * Target for operations like update, delete, etc.
 * Can be a record object, a record instance, or a record ID.
 */
export type DBRecordTarget = DBRecord<any> | DBRecordInstance<any> | string

/**
 * Target for relationship operations.
 * Can be an array of records, record instances, or record IDs.
 */
export type RelationTarget =
  | DBRecordsArrayInstance<any>
  | MaybeArray<DBRecord<any>>
  | MaybeArray<DBRecordInstance<any>>
  | MaybeArray<string>

/**
 * Represents a relationship between two records.
 */
export type Relation = {
  sourceId: string
  sourceLabel: string
  targetId: string
  targetLabel: string
  type: string
}

/**
 * Direction of a relationship.
 * 'in' for incoming relationships, 'out' for outgoing relationships.
 */
export type RelationDirection = 'in' | 'out'

/**
 * Options for creating or modifying relationships.
 */
export type RelationOptions = { direction?: RelationDirection; type?: string }

/**
 * Options for detaching (removing) relationships.
 */
export type RelationDetachOptions = {
  direction?: RelationDirection
  typeOrTypes?: MaybeArray<string>
}

/**
 * Options for creating database records.
 */
export type DBRecordCreationOptions = {
  returnResult?: boolean
  suggestTypes?: boolean
  castNumberArraysToVectors?: boolean
  convertNumericValuesToNumbers?: boolean
  capitalizeLabels?: boolean
  relationshipType?: string
}

export class DBRecordInstance<S extends Schema = Schema, Q extends SearchQuery<S> = SearchQuery<S>> {
  data: DBRecordInferred<S, Q>

  /**
   * Creates a new DBRecordInstance.
   * Typically, you don't create these directly but receive them from query results.
   *
   * @param data - The raw record data
   */
  constructor(data: DBRecordInferred<S, Q> = {} as DBRecordInferred<S, Q>) {
    this.data = data
  }

  /**
   *
   * Checks if this record instance exists.
   * A record is considered to exist if it has a valid ID and label.
   *
   * @returns True if the record exists, false otherwise
   */
  exists() {
    return toBoolean(this.data.__id) && toBoolean(this.data.__label)
  }

  /**
   * Gets the unique ID of this record.
   *
   * @returns The record's ID
   * @throws Error if the ID is missing or invalid
   */
  id() {
    if (!toBoolean(this.data.__id)) {
      throw new Error(
        `DBRecordInstance: Unable to access 'id'. The Record's \`data.__id\` is missing or incorrect.`
      )
    }
    return this.data.__id
  }

  /**
   * Gets the label (type) of this record.
   *
   * @returns The record's label
   * @throws Error if the label is missing or invalid
   */
  label() {
    if (!toBoolean(this.data.__label)) {
      throw new Error(
        `DBRecordInstance: Unable to access 'label'. The Record's \`data.__label\` is missing or incorrect.`
      )
    }
    return this.data.__label
  }

  /**
   * Gets the property types of this record.
   *
   * @returns The record's property types
   * @throws Error if the property types are missing or invalid
   */
  proptypes() {
    if (!toBoolean(this.data.__proptypes)) {
      throw new Error(
        `DBRecordInstance: Unable to access 'proptypes'. The Record's \`data.__proptypes\` is missing or incorrect.`
      )
    }
    return this.data.__proptypes
  }

  /**
   * Gets the creation date of this record.
   * The date is derived from the record's ID.
   *
   * @returns The record's creation date
   * @throws Error if the ID is missing or invalid
   */
  date() {
    try {
      return idToDate(this.id())
    } catch {
      throw new Error(
        `DBRecordInstance: Unable to access 'date'. The Record's \`data.__id\` is missing or incorrect.`
      )
    }
  }

  /**
   * Gets the creation timestamp of this record.
   * The timestamp is derived from the record's ID.
   *
   * @returns The record's creation timestamp
   * @throws Error if the ID is missing or invalid
   */
  timestamp() {
    try {
      return idToTimestamp(this.id())
    } catch {
      throw new Error(
        `DBRecordInstance: Unable to access 'timestamp'. The Record's \`data.__id\` is missing or incorrect.`
      )
    }
  }

  /**
   * Deletes this record from the database.
   *
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the result of the delete operation
   * @throws Error if the record data is undefined
   */
  async delete(transaction?: Transaction | string) {
    if (!toBoolean(this.data)) {
      throw new Error('DBRecordInstance: Unable to delete Record. The Record data is undefined.')
    }

    const instance = RushDB.init()
    return await instance.records.deleteById(this.id(), transaction)
  }

  /**
   * Updates specific fields of this record.
   *
   * @param data - The fields to update and their new values
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the updated record
   * @throws Error if the record data is undefined
   */
  async update<S extends Schema = Schema>(
    data: Partial<InferSchemaTypesWrite<S>> | Array<PropertyDraft>,
    transaction?: Transaction | string
  ) {
    if (!toBoolean(this.data)) {
      throw new Error('DBRecordInstance: Unable to update Record. The Record data is undefined.')
    }

    const instance = RushDB.init()
    return instance.records.update({ label: this.label(), target: this.id(), data }, transaction)
  }

  /**
   * Sets all fields of this record to the provided values.
   * This replaces all existing field values.
   *
   * @param data - The new field values
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the updated record
   * @throws Error if the record data is undefined
   */
  async set<S extends Schema = Schema>(
    data: InferSchemaTypesWrite<S> | Array<PropertyDraft>,
    transaction?: Transaction | string
  ) {
    if (!toBoolean(this.data)) {
      throw new Error('DBRecordInstance: Unable to set. The Record data is undefined.')
    }

    const instance = RushDB.init()
    return instance.records.set({ label: this.label(), target: this.id(), data }, transaction)
  }

  /**
   * Creates a relationship between this record and another record or records.
   *
   * @param target - The target record(s) to create a relationship with
   * @param options - Optional relationship options
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the result of the attach operation
   * @throws Error if the record data is undefined
   */
  async attach(target: RelationTarget, options?: RelationOptions, transaction?: Transaction | string) {
    if (!toBoolean(this.data)) {
      throw new Error('DBRecordInstance: Unable to attach Record. The Record data is undefined.')
    }

    const instance = RushDB.init()
    return instance.records.attach({ source: this.id(), target, options }, transaction)
  }

  /**
   * Removes a relationship between this record and another record or records.
   *
   * @param target - The target record(s) to remove the relationship from
   * @param options - Optional detach options
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the result of the detach operation
   * @throws Error if the record data is undefined
   */
  async detach(target: RelationTarget, options?: RelationDetachOptions, transaction?: Transaction | string) {
    if (!toBoolean(this.data)) {
      throw new Error('DBRecordInstance: Unable to detach Record. The Record data is undefined.')
    }

    const instance = RushDB.init()
    return instance.records.detach({ source: this.id(), target, options }, transaction)
  }
}

/**
 * Represents a collection of database records with additional methods for batch operations.
 * This class is typically returned from queries that match multiple records.
 *
 * @typeParam S - The schema type that defines the records' structure
 * @typeParam Q - The search query type that was used to retrieve these records
 */
export class DBRecordsArrayInstance<S extends Schema = Schema, Q extends SearchQuery<S> = SearchQuery<S>> {
  /** The array of record instances */
  data: Array<DBRecordInstance<S, Q>>
  /** The total number of records matching the query (may be more than the number in data) */
  total: number
  /** The search query that was used to retrieve these records */
  searchQuery?: SearchQuery<S>

  /**
   * Creates a new DBRecordsArrayInstance.
   * Typically, you don't create these directly but receive them from query results.
   *
   * @param data - The array of record instances
   * @param total - The total number of records matching the query
   * @param searchQuery - The search query that was used to retrieve these records
   */
  constructor(data: Array<DBRecordInstance<S, Q>> = [], total: number = 0, searchQuery?: SearchQuery<S>) {
    this.data = data
    this.total = total
    this.searchQuery = searchQuery
  }

  /**
   * TODO: Future enhancements for this class:
   * - Bulk actions: Delete multiple records by IDs or search query
   * - Export records to CSV format
   * - Update properties across multiple records at once
   * - Batch relationship operations (attach/detach)
   * - Pagination support with a next() method to fetch additional results based on searchQuery
   */
}
