/**
 * Default server-side budget for a Neo4j transaction (request-scoped sessions,
 * user-defined transactions, background/side-effect work). Neo4j terminates the
 * transaction and rolls it back when the budget is exceeded.
 */
export const DEFAULT_TRANSACTION_TIMEOUT_MS = 60_000
