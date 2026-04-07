import type {
  SearchQuery,
  Schema,
  FlattenTypes,
  InferSchemaTypesRead,
  InferSchemaTypesWrite,
  MaybeArray,
  PropertyDraft
} from '../types/index.js'
import type {
  DBRecord,
  DBRecordInstance,
  DBRecordsArrayInstance,
  DBRecordTarget,
  RelationDetachOptions,
  RelationOptions,
  RelationTarget
} from './record.js'
import type { Transaction } from './transaction.js'

import { isArray, isEmptyObject, toBoolean } from '../common/utils.js'
import { EmptyTargetError, UniquenessError } from './errors.js'
import { mergeDefaultsWithPayload, pickUniqFieldsFromRecord, pickUniqFieldsFromRecords } from './utils.js'
import { isPropertyDraft, pickRecordId } from '../api/utils'
import { RushDB } from './sdk'

/**
 * Configuration options for a Model instance.
 */
export type ModelConfig = {
  /**
   * When true, `set` and `update` will verify the target record's label
   * matches this model's label before proceeding. Adds one extra DB read.
   * @default false
   */
  validateTargetLabel?: boolean
}

/**
 * Options for the `create` method.
 */
export type CreateOptions = {
  /**
   * Behavior when a uniqueness conflict is detected.
   * - `'throw'` (default) — throws `UniquenessError`
   * - `'upsert'` — updates the conflicting record instead
   * - `'skip'` — returns `null` without throwing
   */
  onConflict?: 'throw' | 'upsert' | 'skip'
}

/**
 * Represents a model in the RushDB database.
 * A model defines the structure and behavior of a specific type of record in the database.
 * It provides methods for CRUD operations and relationship management.
 *
 * @typeParam S - The schema type that defines the structure of the model's records
 */
export class Model<S extends Schema = any> {
  public readonly label: string
  public readonly schema: S
  private readonly config: ModelConfig

  /**
   * Creates a new Model instance.
   *
   * @param modelName - The name/label of the model in the database
   * @param schema - The schema definition that describes the model's structure
   * @param config - Optional model configuration
   */
  constructor(modelName: string, schema: S, config?: ModelConfig) {
    this.label = modelName
    this.schema = schema
    this.config = config ?? {}
  }

  /** @description
   * Type helper for a draft version of the schema.
   * Represents a flat object containing only the record's own properties
   * (defined by the schema), excluding system fields such as `__id`, `__label`,
   * and `__proptypes`. This type does not yet have a representation on the
   * database side.
   */
  readonly draft!: InferType<Model<S>>

  /** @description
   * Type helper for a SearchQuery of the schema.
   * Represents a structured query input that enables filtering,
   * sorting, pagination, and aggregation of records based on schema-defined fields.
   * Useful for composing reusable, type-safe search expressions.
   */
  readonly searchQuery!: SearchQuery<S>

  /** @description
   * Type helper for a fully defined record with database representation.
   * Similar to the draft, but includes all fields that come with the record's
   * database-side representation, such as `__id`, `__label`, and `__proptypes`.
   */
  readonly record!: DBRecord<S>

  /** @description
   * Type helper for a single record instance.
   * Extends the record by providing additional methods to operate on this specific
   * record, such as saving, updating, or deleting it.
   */
  readonly recordInstance!: DBRecordInstance<S>

  /** @description
   * Type helper for an array of record instances.
   * Similar to a single record instance but supports batch or bulk operations,
   * allowing efficient management of multiple records simultaneously.
   */
  readonly recordsArrayInstance!: DBRecordsArrayInstance<S>

  /**
   * Retrieves the model's label.
   *
   * @returns The label/name of the model
   */
  public getLabel() {
    return this.label
  }

  /**
   * Retrieves the initialized RushDB instance or waits for initialization to complete.
   *
   * This method is called internally by all data access methods in the Model class.
   * It ensures that the database connection is properly established before executing
   * any operations. For this to work, make sure you've created a RushDB instance
   * somewhere in your code and imported that module in your application's entry point.
   *
   * Note: If you get "No RushDB instance found" errors, it usually means the module
   * where you create your RushDB instance hasn't been imported yet.
   *
   * @returns Promise resolving to the initialized RushDB instance
   * @throws Error if no RushDB instance has been created yet
   */

  /**
   * Converts a database record to a record instance (DBRecordInstance) with additional methods.
   *
   * @param record - The database record to convert
   * @returns A record instance with additional methods
   * @throws Error if no RushDB instance was provided during initialization
   */
  public async toDBRecordInstance(record: DBRecord<S>) {
    try {
      const instance = RushDB.getInstance()
      return instance.toDBRecordInstance(record)
    } catch (error) {
      throw new Error('No RushDB instance was provided during model initialization.')
    }
  }

  /**
   * Finds records that match the given search criteria.
   *
   * @param searchQuery - Optional search criteria
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to matching records
   */
  async find<Q extends SearchQuery<S> = SearchQuery<S>>(
    searchQuery?: Q & { labels?: never },
    transaction?: Transaction | string
  ) {
    const query = (searchQuery ?? {}) as Q
    const instance = RushDB.getInstance()
    return instance.records.find<S, Q>({ ...query, labels: [this.label] }, transaction)
  }

  /**
   * Finds a single record that matches the given search criteria.
   *
   * @param searchQuery - Optional search criteria
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the matching record or null if not found
   */
  async findOne<Q extends SearchQuery<S> = SearchQuery<S>>(
    searchQuery?: Q & {
      labels?: never
      limit?: never
      skip?: never
    },
    transaction?: Transaction | string
  ) {
    const query = (searchQuery ?? {}) as Q & {
      labels?: never
      limit?: never
      skip?: never
    }
    const instance = RushDB.getInstance()
    return instance.records.findOne<S, Q>({ ...query, labels: [this.label] }, transaction)
  }

  /**
   * Finds a record by its ID.
   *
   * @param id - The ID of the record to find
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the matching record or null if not found
   */
  async findById(id: string, transaction?: Transaction | string) {
    return this.findOne({ where: { $id: id } }, transaction)
  }

  /**
   * Finds a unique record that matches the given search criteria.
   *
   * @param searchQuery - Optional search criteria
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the unique matching record or null if not found
   */
  async findUniq<Q extends SearchQuery<S> = SearchQuery<S>>(
    searchQuery?: Q & {
      labels?: never
      limit?: never
      skip?: never
    },
    transaction?: Transaction | string
  ) {
    const query = (searchQuery ?? {}) as Q & {
      labels?: never
      limit?: never
      skip?: never
    }
    const instance = RushDB.getInstance()
    return instance.records.findUniq<S, Q>({ ...query, labels: [this.label] }, transaction)
  }

  /**
   * Creates a new record in the database.
   *
   * @param record - The record data to create
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the created record
   * @throws UniquenessError if the record violates uniqueness constraints
   */
  async create(
    record: InferSchemaTypesWrite<S>,
    transaction?: Transaction | string,
    options?: CreateOptions
  ) {
    const data = await mergeDefaultsWithPayload<S>(this.schema, record)

    const uniqFields = pickUniqFieldsFromRecord(this.schema, data)
    const hasUniqFields = toBoolean(uniqFields)
    const instance = RushDB.getInstance()

    if (hasUniqFields) {
      const tx = transaction ?? (await instance.tx.begin())
      const matchingRecords = await this.find({ where: uniqFields }, tx)
      const hasOwnTransaction = typeof transaction !== 'undefined'
      const canCreate = !matchingRecords?.data?.length

      if (canCreate) {
        const result = await instance.records.create<S>({ label: this.label, data }, tx)
        if (!hasOwnTransaction) {
          await (tx as Transaction).commit()
        }

        return result
      } else {
        const onConflict = options?.onConflict ?? 'throw'

        switch (onConflict) {
          case 'upsert': {
            const conflictingRecord = matchingRecords.data[0]
            const result = await instance.records.set<S>(
              { target: conflictingRecord.id, label: this.label, data },
              tx
            )
            if (!hasOwnTransaction) {
              await (tx as Transaction).commit()
            }
            return result
          }
          case 'skip':
            if (!hasOwnTransaction) {
              await (tx as Transaction).commit()
            }
            return null
          default:
            if (!hasOwnTransaction) {
              await (tx as Transaction).commit()
            }
            throw new UniquenessError(this.label, uniqFields)
        }
      }
    }
    return await instance.records.create<S>({ label: this.label, data }, transaction)
  }

  /**
   * Attaches a relationship between records.
   *
   * @param params - Object containing source, target, and relationship options
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the result of the attach operation
   */
  async attach(
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
  ) {
    const instance = RushDB.getInstance()
    return await instance.records.attach({ source, target, options }, transaction)
  }

  /**
   * Detaches a relationship between records.
   *
   * @param params - Object containing source, target, and detach options
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the result of the detach operation
   */
  async detach(
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
  ) {
    const instance = RushDB.getInstance()
    return await instance.records.detach({ source, target, options }, transaction)
  }

  /**
   * Validates that the target record belongs to this model's label.
   * @private
   */
  private async validateTargetLabel(
    target: DBRecordTarget,
    transaction?: Transaction | string
  ): Promise<void> {
    const id = pickRecordId(target)
    if (!id) return

    const instance = RushDB.getInstance()
    const record = await instance.records.findById(id, transaction)
    if (!record) {
      throw new Error(`Record with id "${id}" not found.`)
    }
    if (record.data.__label !== this.label) {
      throw new TypeError(
        `Target record label "${record.data.__label}" does not match model label "${this.label}".`
      )
    }
  }

  /**
   * Internal method to handle both set and update operations.
   *
   * @param target - The target record to modify
   * @param record - The data to set or update
   * @param method - The operation type ('set' or 'update')
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the modified record
   * @throws UniquenessError if the modification violates uniqueness constraints
   * @private
   */
  private async handleSetOrUpdate(
    target: DBRecordTarget,
    record: Partial<InferSchemaTypesWrite<S>> | Array<PropertyDraft>,
    method: 'set' | 'update',
    transaction?: Transaction | string
  ) {
    if (this.config.validateTargetLabel) {
      await this.validateTargetLabel(target, transaction)
    }

    // Consider Array as Array<PropertyDraft>
    if (isArray(record) && record.every(isPropertyDraft)) {
      const instance = RushDB.getInstance()
      return await instance.records[method]<S>({ label: this.label, target, data: record }, transaction)
    }

    const instance = RushDB.getInstance()
    const data = await mergeDefaultsWithPayload<S>(this.schema, record)
    const uniqFields = pickUniqFieldsFromRecord(this.schema, data)

    if (!isEmptyObject(uniqFields)) {
      const tx = transaction ?? (await instance.tx.begin())
      const matchingRecords = await this.find({ where: uniqFields }, tx)
      const hasOwnTransaction = typeof transaction !== 'undefined'

      const canUpdate =
        !matchingRecords?.data?.length ||
        (matchingRecords.data.length === 1 && matchingRecords.data[0]?.id === pickRecordId(target)!)

      if (canUpdate) {
        const result = await instance.records[method]<S>({ target, label: this.label, data }, tx)

        if (!hasOwnTransaction) {
          await (tx as Transaction).commit()
        }
        return result
      } else {
        if (!hasOwnTransaction) {
          await (tx as Transaction).commit()
        }
        throw new UniquenessError(this.label, uniqFields)
      }
    }

    return await instance.records[method]<S>({ label: this.label, target, data }, transaction)
  }

  /**
   * Sets all fields of a record to the provided values.
   *
   * @param target - The target record to modify
   * @param record - The new values to set
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the modified record
   */
  async set(target: DBRecordTarget, record: InferSchemaTypesWrite<S>, transaction?: Transaction | string) {
    return await this.handleSetOrUpdate(target, record, 'set', transaction)
  }

  /**
   * Updates specified fields of a record.
   *
   * @param target - The target record to modify
   * @param record - The fields to update and their new values
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the modified record
   */
  async update(
    target: DBRecordTarget,
    record: Partial<InferSchemaTypesWrite<S>>,
    transaction?: Transaction | string
  ) {
    return await this.handleSetOrUpdate(target, record, 'update', transaction)
  }

  /**
   * Creates multiple records in a single operation.
   *
   * @param records - Array of records to create
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the created records
   * @throws UniquenessError if any record violates uniqueness constraints
   */
  async createMany(records: Array<InferSchemaTypesWrite<S>>, transaction?: Transaction | string) {
    const instance = RushDB.getInstance()

    // Begin a transaction if one isn't provided.
    const hasOwnTransaction = typeof transaction !== 'undefined'
    const tx = transaction ?? (await instance.tx.begin())

    try {
      // Apply defaults in parallel
      const recordsToStore = await Promise.all(
        records.map(async (record) => {
          return await mergeDefaultsWithPayload<S>(this.schema, record)
        })
      )

      // Check uniqueness
      const uniqProperties = pickUniqFieldsFromRecords(
        recordsToStore as Array<Partial<InferSchemaTypesWrite<S>>>,
        this.schema,
        this.label
      )
      if (!isEmptyObject(uniqProperties)) {
        const criteria = Object.entries(uniqProperties).map(([key, values]) => ({
          [key]: { $in: values }
        }))
        const matchingRecords = await this.find(
          {
            where: {
              $or: criteria
            }
          },
          tx
        )
        if (matchingRecords?.data?.length) {
          throw new UniquenessError(this.label, Object.keys(uniqProperties))
        }
      }

      // Create records in the database
      const createdRecords = await instance.records.createMany<S>(
        { label: this.label, data: recordsToStore },
        tx
      )

      // Commit the transaction if it was created internally
      if (!hasOwnTransaction) {
        await (tx as Transaction).commit()
      }

      return createdRecords
    } catch (error) {
      if (!hasOwnTransaction) {
        await (tx as Transaction).rollback()
      }
      throw error
    }
  }

  /**
   * Deletes records that match the given search criteria.
   *
   * @param searchQuery - Search criteria for records to delete
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the result of the delete operation
   * @throws EmptyTargetError if no search criteria are provided
   */
  async delete(searchQuery: Omit<SearchQuery<S>, 'labels'>, transaction?: Transaction | string) {
    if (isEmptyObject(searchQuery.where)) {
      throw new EmptyTargetError(
        `You must specify criteria to delete records of type '${this.label}'. Empty criteria are not allowed. If this was intentional, use the Dashboard instead.`
      )
    }
    const instance = RushDB.getInstance()
    return await instance.records.delete({ ...searchQuery, labels: [this.label] }, transaction)
  }

  /**
   * Deletes one or more records by their IDs.
   *
   * @param idOrIds - Single ID or array of IDs to delete
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the result of the delete operation
   */
  async deleteById(idOrIds: MaybeArray<string>, transaction?: Transaction | string) {
    const instance = RushDB.getInstance()
    return await instance.records.deleteById(idOrIds, transaction)
  }
}

/**
 * Helper type that infers the type structure from a Model instance.
 *
 * @typeParam M - The Model type to infer from
 */
export type InferType<M extends Model<any> = Model<any>> = FlattenTypes<InferSchemaTypesRead<M['schema']>>
