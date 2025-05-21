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

import { RestApiProxy } from '../api/rest-api-proxy.js'
import { isArray, isEmptyObject } from '../common/utils.js'
import { EmptyTargetError, UniquenessError } from './errors.js'
import { mergeDefaultsWithPayload, pickUniqFieldsFromRecord, pickUniqFieldsFromRecords } from './utils.js'
import { isPropertyDraft, pickRecordId } from '../api/utils'
import type { initSDK } from './sdk'

type RushDBInstance = ReturnType<typeof initSDK>
/**
 * Represents a model in the RushDB database.
 * A model defines the structure and behavior of a specific type of record in the database.
 * It provides methods for CRUD operations and relationship management.
 *
 * @typeParam S - The schema type that defines the structure of the model's records
 */
export class Model<S extends Schema = any> extends RestApiProxy {
  public readonly label: string
  public readonly schema: S
  private rushDBInstance?: RushDBInstance['instance']

  /**
   * Creates a new Model instance.
   *
   * @param modelName - The name/label of the model in the database
   * @param schema - The schema definition that describes the model's structure
   * @param rushDBInstance - Optional RushDB instance for model registration.
   *                        This is the recommended way to register models as it automatically
   *                        registers the model with the RushDB instance during creation.
   */
  constructor(modelName: string, schema: S, rushDBInstance?: RushDBInstance['instance']) {
    super()
    this.label = modelName
    this.schema = schema

    // If a RushDB instance is explicitly provided, use it
    if (rushDBInstance) {
      this.rushDBInstance = rushDBInstance
      this.rushDBInstance.registerModel(this)
      return
    }

    // Otherwise, try to get the instance synchronously
    if (!this.tryGetRushDBInstance()) {
      console.warn(
        `No RushDB instance available for automatic model registration. The model '${modelName}' will need to be manually initialized.`
      )
      console.info(`You may need to initialize this model manually using 'model.initialize(rushDBInstance)'`)
    }
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
   * Type helper for a fully-defined record with database representation.
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
   * Manually initializes the Model with a RushDB instance.
   * This can be called if automatic initialization fails or if you need to replace the instance.
   *
   * @param rushDBInstance - The RushDB instance to initialize the model with
   * @returns The model instance for method chaining
   */
  public initialize(rushDBInstance: RushDBInstance['instance']): this {
    this.rushDBInstance = rushDBInstance
    this.rushDBInstance.registerModel(this)
    return this
  }

  /**
   * Retrieves the model's label.
   *
   * @returns The label/name of the model
   */
  public getLabel() {
    return this.label
  }

  /**
   * Checks if the RushDB instance is initialized
   *
   * @returns True if the RushDB instance is initialized, false otherwise
   */
  public isInitialized(): boolean {
    return !!this.rushDBInstance
  }

  /**
   * Ensures that the RushDB instance is initialized
   *
   * @returns The RushDB instance
   * @throws Error if the RushDB instance is not initialized and cannot be obtained
   * @private
   */
  private ensureInitialized(): RushDBInstance['instance'] {
    // If we already have an instance, use it
    if (this.rushDBInstance) {
      return this.rushDBInstance
    }

    // Try to get the instance one more time
    if (this.tryGetRushDBInstance() && this.rushDBInstance) {
      return this.rushDBInstance
    }

    // If we still don't have an instance, throw an error
    throw new Error(
      'RushDB instance is not initialized. Make sure the Model is properly initialized before calling methods that require RushDB. ' +
        'You can initialize it by creating a RushDB instance or calling model.initialize(rushDBInstance).'
    )
  }

  /**
   * Tries to find and use a RushDB instance if one exists in the global scope
   *
   * @returns true if an instance was found and initialized, false otherwise
   */
  private tryGetRushDBInstance(): boolean {
    try {
      // Try to get the RushDB instance from either the browser or Node environment
      let RushDBStatic: { instance: RushDBInstance['instance'] } | undefined

      if (typeof window !== 'undefined') {
        // Browser environment
        RushDBStatic = require('../index.worker').RushDB
      } else {
        // Node.js environment
        RushDBStatic = require('../index.node').RushDB
      }

      if (RushDBStatic && RushDBStatic.instance) {
        this.rushDBInstance = RushDBStatic.instance
        if (this.rushDBInstance) {
          this.rushDBInstance.registerModel(this)
          return true
        }
      }
      return false
    } catch (error) {
      return false
    }
  }

  /**
   * Converts a database record to a record instance with additional methods.
   *
   * @param record - The database record to convert
   * @returns A record instance with additional methods
   * @throws Error if no RushDB instance was provided during initialization
   */
  public toInstance(record: DBRecord<S>) {
    return this.ensureInitialized().toInstance(record)
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
    return this.ensureInitialized().records.find<S, Q>({ ...query, labels: [this.label] }, transaction)
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
    return this.ensureInitialized().records.findOne<S, Q>({ ...query, labels: [this.label] }, transaction)
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
    return this.ensureInitialized().records.findUniq<S, Q>({ ...query, labels: [this.label] }, transaction)
  }

  /**
   * Creates a new record in the database.
   *
   * @param record - The record data to create
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the created record
   * @throws UniquenessError if the record violates uniqueness constraints
   */
  async create(record: InferSchemaTypesWrite<S>, transaction?: Transaction | string) {
    const rushDB = this.ensureInitialized()
    const data = await mergeDefaultsWithPayload<S>(this.schema, record)

    const uniqFields = pickUniqFieldsFromRecord(this.schema, data)

    if (!isEmptyObject(uniqFields)) {
      const tx = transaction ?? (await rushDB.tx.begin())
      const matchingRecords = await this.find({ where: uniqFields }, tx)
      const hasOwnTransaction = typeof transaction !== 'undefined'
      const canCreate = !matchingRecords?.data?.length

      if (canCreate) {
        const result = await rushDB.records.create<S>({ label: this.label, data }, tx)
        if (!hasOwnTransaction) {
          await (tx as Transaction).commit()
        }
        return result
      } else {
        if (!hasOwnTransaction) {
          await (tx as Transaction).commit()
        }

        // @TODO: Make it optional
        throw new UniquenessError(this.label, uniqFields)
      }
    }
    return await rushDB.records.create<S>({ label: this.label, data }, transaction)
  }

  /**
   * Attaches a relationship between records.
   *
   * @param params - Object containing source, target, and relationship options
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the result of the attach operation
   */
  attach(
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
    return this.ensureInitialized().records.attach({ source, target, options }, transaction)
  }

  /**
   * Detaches a relationship between records.
   *
   * @param params - Object containing source, target, and detach options
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the result of the detach operation
   */
  detach(
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
    return this.ensureInitialized().records.detach({ source, target, options }, transaction)
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
    const rushDB = this.ensureInitialized()

    // Consider Array as Array<PropertyDraft>
    if (isArray(record) && record.every(isPropertyDraft)) {
      // @TODO
      throw new Error(`Model.${method} with Array<PropertyDraft> as payload is not implemented yet.`)
    }

    const data = await mergeDefaultsWithPayload<S>(this.schema, record)
    const uniqFields = pickUniqFieldsFromRecord(this.schema, data)

    if (!isEmptyObject(uniqFields)) {
      const tx = transaction ?? (await rushDB.tx.begin())
      const matchingRecords = await this.find({ where: uniqFields }, tx)
      const hasOwnTransaction = typeof transaction !== 'undefined'

      const canUpdate =
        !matchingRecords?.data?.length ||
        (matchingRecords.data.length === 1 && matchingRecords.data[0]?.id() === pickRecordId(target)!)

      if (canUpdate) {
        const result = await rushDB.records[method]<S>({ target, label: this.label, data }, tx)

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

    return await rushDB.records[method]<S>({ label: this.label, target, data }, transaction)
  }

  /**
   * Sets all fields of a record to the provided values.
   *
   * @param target - The target record to modify
   * @param record - The new values to set
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the modified record
   */
  async set(
    // @TODO: Check target to match Model type
    target: DBRecordTarget,
    record: InferSchemaTypesWrite<S>,
    transaction?: Transaction | string
  ) {
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
    // @TODO: Check target to match Model type
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
    const rushDB = this.ensureInitialized()

    // Begin a transaction if one isn't provided.
    const hasOwnTransaction = typeof transaction !== 'undefined'
    const tx = transaction ?? (await rushDB.tx.begin())

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
      const createdRecords = await rushDB.records.createMany<S>(
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

    return await this.ensureInitialized().records.delete(
      { ...searchQuery, labels: [this.label] },
      transaction
    )
  }

  /**
   * Deletes one or more records by their IDs.
   *
   * @param idOrIds - Single ID or array of IDs to delete
   * @param transaction - Optional transaction or transaction ID
   * @returns Promise resolving to the result of the delete operation
   */
  async deleteById(idOrIds: MaybeArray<string>, transaction?: Transaction | string) {
    return await this.ensureInitialized().records.deleteById(idOrIds, transaction)
  }
}

/**
 * Helper type that infers the type structure from a Model instance.
 *
 * @typeParam M - The Model type to infer from
 */
export type InferType<M extends Model<any> = Model<any>> = FlattenTypes<InferSchemaTypesRead<M['schema']>>
