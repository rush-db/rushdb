const envTransactionTimeout = Number(process.env.NEO4J_TRANSACTION_TIMEOUT_MS)

/**
 * Default server-side budget for a Neo4j transaction (request-scoped sessions,
 * user-defined transactions, background/side-effect work). Neo4j terminates the
 * transaction and rolls it back when the budget is exceeded.
 *
 * The default sits below the 60s managed-gateway limit on purpose: Neo4j must
 * kill a runaway query first so the API can answer with a structured 408
 * (see GlobalExceptionFilter) instead of the gateway's body-less 504 — and so
 * the query stops consuming shared-cluster resources once the client can no
 * longer receive a response anyway. Override with NEO4J_TRANSACTION_TIMEOUT_MS
 * (e.g. for self-hosted setups without a gateway in front).
 */
export const DEFAULT_TRANSACTION_TIMEOUT_MS =
  Number.isFinite(envTransactionTimeout) && envTransactionTimeout > 0 ? envTransactionTimeout : 55_000
