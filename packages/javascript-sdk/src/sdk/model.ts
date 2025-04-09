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

type RushDBInstance = {
  registerModel(model: Model): void
  toInstance<S extends Schema = Schema>(record: DBRecord<S>): DBRecordInstance<S, SearchQuery<S>>
}

export class Model<S extends Schema = any> extends RestApiProxy {
  public readonly label: string
  public readonly schema: S
  private readonly rushDBInstance?: RushDBInstance

  constructor(modelName: string, schema: S, RushDBInstance?: RushDBInstance) {
    super()
    this.label = modelName
    this.schema = schema
    this.rushDBInstance = RushDBInstance

    RushDBInstance?.registerModel(this)
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

  public getLabel() {
    return this.label
  }

  public toInstance(record: DBRecord<S>) {
    if (this.rushDBInstance) {
      return this.rushDBInstance.toInstance(record)
    } else {
      throw new Error('No RushDB instance was provided during model initialization.')
    }
  }

  async find<Q extends SearchQuery<S> = SearchQuery<S>>(
    searchQuery?: Q & { labels?: never },
    transaction?: Transaction | string
  ) {
    const query = (searchQuery ?? {}) as Q
    return this.apiProxy?.records.find<S, Q>({ ...query, labels: [this.label] }, transaction)
  }

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
    return this.apiProxy?.records.findOne<S, Q>({ ...query, labels: [this.label] }, transaction)
  }

  async findById(id: string, transaction?: Transaction | string) {
    return this.findOne({ where: { $id: id } }, transaction)
  }

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
    return this.apiProxy?.records.findUniq<S, Q>({ ...query, labels: [this.label] }, transaction)
  }

  async create(record: InferSchemaTypesWrite<S>, transaction?: Transaction | string) {
    const data = await mergeDefaultsWithPayload<S>(this.schema, record)

    const uniqFields = pickUniqFieldsFromRecord(this.schema, data)

    if (!isEmptyObject(uniqFields)) {
      const tx = transaction ?? (await this.apiProxy.tx.begin())
      const matchingRecords = await this.find({ where: uniqFields }, tx)
      const hasOwnTransaction = typeof transaction !== 'undefined'
      const canCreate = !matchingRecords?.data?.length

      if (canCreate) {
        const result = await this.apiProxy.records.create<S>({ label: this.label, data }, tx)
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
    return await this.apiProxy.records.create<S>({ label: this.label, data }, transaction)
  }

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
    return this.apiProxy.records.attach({ source, target, options }, transaction)
  }

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
    return this.apiProxy.records.detach({ source, target, options }, transaction)
  }

  private async handleSetOrUpdate(
    target: DBRecordTarget,
    record: Partial<InferSchemaTypesWrite<S>> | Array<PropertyDraft>,
    method: 'set' | 'update',
    transaction?: Transaction | string
  ) {
    // Consider Array as Array<PropertyDraft>
    if (isArray(record) && record.every(isPropertyDraft)) {
      // @TODO
      throw new Error(`Model.${method} with Array<PropertyDraft> as payload is not implemented yet.`)
    }

    const data = await mergeDefaultsWithPayload<S>(this.schema, record)
    const uniqFields = pickUniqFieldsFromRecord(this.schema, data)

    if (!isEmptyObject(uniqFields)) {
      const tx = transaction ?? (await this.apiProxy.tx.begin())
      const matchingRecords = await this.find({ where: uniqFields }, tx)
      const hasOwnTransaction = typeof transaction !== 'undefined'

      const canUpdate =
        !matchingRecords?.data?.length ||
        (matchingRecords.data.length === 1 && matchingRecords.data[0].__id === pickRecordId(target)!)

      if (canUpdate) {
        const result = await this.apiProxy.records[method]<S>({ target, label: this.label, data }, tx)

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

    return await this.apiProxy.records[method]<S>({ label: this.label, target, data }, transaction)
  }

  async set(
    // @TODO: Check target to match Model type
    target: DBRecordTarget,
    record: InferSchemaTypesWrite<S>,
    transaction?: Transaction | string
  ) {
    return await this.handleSetOrUpdate(target, record, 'set', transaction)
  }

  async update(
    // @TODO: Check target to match Model type
    target: DBRecordTarget,
    record: Partial<InferSchemaTypesWrite<S>>,
    transaction?: Transaction | string
  ) {
    return await this.handleSetOrUpdate(target, record, 'update', transaction)
  }

  async createMany(records: Array<InferSchemaTypesWrite<S>>, transaction?: Transaction | string) {
    // Begin a transaction if one isn't provided.
    const hasOwnTransaction = typeof transaction !== 'undefined'
    const tx = transaction ?? (await this.apiProxy.tx.begin())

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
      const createdRecords = await this.apiProxy.records.createMany<S>(
        { label: this.label, payload: recordsToStore },
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

  async delete(searchQuery: Omit<SearchQuery<S>, 'labels'>, transaction?: Transaction | string) {
    if (isEmptyObject(searchQuery.where)) {
      throw new EmptyTargetError(
        `You must specify criteria to delete records of type '${this.label}'. Empty criteria are not allowed. If this was intentional, use the Dashboard instead.`
      )
    }

    return await this.apiProxy.records.delete({ ...searchQuery, labels: [this.label] }, transaction)
  }

  async deleteById(idOrIds: MaybeArray<string>, transaction?: Transaction | string) {
    return await this.apiProxy.records.deleteById(idOrIds, transaction)
  }
}

export type InferType<M extends Model<any> = Model<any>> = FlattenTypes<InferSchemaTypesRead<M['schema']>>
