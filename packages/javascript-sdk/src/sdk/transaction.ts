import { RushDB } from './sdk.js'

/**
 * Represents a database transaction that groups multiple operations.
 * Transactions ensure that a series of operations are executed atomically,
 * meaning either all operations succeed or none of them are applied.
 *
 * Use transactions when you need to perform multiple related operations
 * that should be treated as a single unit of work. For example, creating
 * related records, updating multiple records, or performing operations
 * that need to be rolled back if any part fails.
 */
export class Transaction {
  readonly id: string

  /**
   * Creates a new Transaction instance.
   * Typically, you should not create Transaction instances directly.
   * Instead, use the tx.begin() method from a RushDB instance.
   *
   * @param id - The transaction ID provided by the database
   */
  constructor(id: string) {
    this.id = id
  }

  /**
   * Rolls back (cancels) all operations performed within this transaction.
   * Use this when an error occurs or when you want to undo changes.
   *
   * @returns Promise resolving to the result of the rollback operation
   */
  async rollback() {
    const instance = RushDB.init()
    return await instance.tx.rollback(this.id)
  }

  /**
   * Commits (applies) all operations performed within this transaction.
   * This finalizes all changes made within the transaction.
   *
   * @returns Promise resolving to the result of the commit operation
   */
  async commit() {
    const instance = RushDB.init()
    return await instance.tx.commit(this.id)
  }
}
