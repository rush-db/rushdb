//! Transport-independent execution of `rushdb-protocol` operations.

mod capabilities;
mod csv;
mod errors;
mod index_ddl;
mod introspection;
mod mutation;
mod policy;
mod queries;
mod transactions;

pub(crate) use capabilities::{engine_capabilities, query_language_capabilities};
pub(crate) use errors::{
    execution_failed, language_execution_failed, query_failed, response_array, response_u64,
    scheduler_error_to_protocol_error, sql_failed,
};

use rushdb_protocol::{
    Describe, ErrorCode, IndexOperation, Ingest, JoinProjection, LabelDescription, Mutation,
    Operation, ProtocolError, PutMode, Query, QueryDocument, RequestEnvelope, Response,
    ResponseEnvelope, SchemaDescription, StatementLanguage, TransactionOperation,
    TransactionStatus, PROTOCOL_VERSION,
};

use crate::node::Node;

#[cfg(test)]
mod test_access {
    use rushdb_lmpg::reader::try_access;
    use rushdb_storage::ReadView;
    use rushdb_types::ids::TenantId;

    pub fn label_members(view: &ReadView, tenant: TenantId, label: &str) -> Vec<u64> {
        try_access::label_members(view, tenant, label).expect("test read")
    }

    pub fn record_by_ord(
        view: &ReadView,
        tenant: TenantId,
        ordinal: u64,
    ) -> Option<rushdb_types::occurrence::RecordHeader> {
        try_access::record_by_ord(view, tenant, ordinal).expect("test read")
    }
}

pub(crate) fn operation_permission(operation: &Operation) -> &'static str {
    match operation {
        Operation::Query { .. } | Operation::Describe { .. } | Operation::Backup { .. } => "read",
        Operation::Mutate { .. }
        | Operation::Ingest { .. }
        | Operation::Index { .. }
        | Operation::Transaction { .. }
        | Operation::PurgeTenant => "write",
        Operation::Policy { .. } => "operator",
    }
}

/// True for an operation a streaming-replication follower must reject
///: every write except starting or discarding a
/// transaction coordinator scope, mirroring the maintenance-mode gate's
/// own `Begin`/`Rollback` exception below.
fn is_write(operation: &Operation) -> bool {
    match operation {
        Operation::Mutate { .. }
        | Operation::Ingest { .. }
        | Operation::Index { .. }
        | Operation::PurgeTenant => true,
        Operation::Transaction { transaction } => !matches!(
            transaction,
            TransactionOperation::Begin { .. } | TransactionOperation::Rollback { .. }
        ),
        Operation::Policy { policy } => {
            matches!(policy, rushdb_protocol::PolicyOperation::Put { .. })
        }
        Operation::Query { .. } | Operation::Describe { .. } | Operation::Backup { .. } => false,
    }
}

fn execution_operation_class(operation: &Operation) -> rushdb_engine::OperationClass {
    match operation {
        Operation::Query { .. } | Operation::Describe { .. } => rushdb_engine::OperationClass::Read,
        Operation::Transaction { .. } => rushdb_engine::OperationClass::Transaction,
        Operation::Backup { .. } | Operation::Policy { .. } => {
            rushdb_engine::OperationClass::Maintenance
        }
        Operation::Mutate { .. }
        | Operation::Ingest { .. }
        | Operation::Index { .. }
        | Operation::PurgeTenant => rushdb_engine::OperationClass::Write,
    }
}

/// Fine-grained operation label for metrics and slow-call logging. The
/// hierarchy is `area.detail` so dashboards can aggregate by prefix; unknown
/// shapes must never appear — every variant is listed explicitly.
pub(crate) fn operation_kind(operation: &Operation) -> &'static str {
    match operation {
        Operation::Query { query } => match query {
            Query::Statement { .. } => "query.statement",
            Query::GremlinBytecode { .. } => "query.gremlinBytecode",
            Query::Records { .. } => "query.records",
            Query::ExportCsv { .. } => "query.exportCsv",
            Query::Cursor { .. } => "query.cursor",
            Query::Record { .. } => "query.record",
            Query::Relationships { .. } => "query.relationships",
            Query::PropertyValues { .. } => "query.propertyValues",
            Query::Vector { .. } => "query.vector",
            Query::JoinPairs(_) => "query.joinPairs",
            Query::VectorIndexCandidates { .. } => "query.vectorIndexCandidates",
            Query::Text { .. } => "query.text",
            Query::Hybrid { .. } => "query.hybrid",
            Query::SearchValues { .. } => "query.searchValues",
        },
        Operation::Mutate { mutation } => match mutation {
            Mutation::Statement { .. } => "mutate.statement",
            Mutation::GremlinBytecode { .. } => "mutate.gremlinBytecode",
            Mutation::PutRecord { .. } => "mutate.putRecord",
            Mutation::DeleteRecord { .. } => "mutate.deleteRecord",
            Mutation::RetypeRecord { .. } => "mutate.retypeRecord",
            Mutation::DeleteRecords { .. } => "mutate.deleteRecords",
            Mutation::DeleteProperty { .. } => "mutate.deleteProperty",
            Mutation::LinkRecords { .. } => "mutate.linkRecords",
            Mutation::UnlinkRecords { .. } => "mutate.unlinkRecords",
            Mutation::RetypeRelations { .. } => "mutate.retypeRelations",
            Mutation::LinkRecordsById { .. } => "mutate.linkRecordsById",
            Mutation::LinkRecordsPairs { .. } => "mutate.linkRecordsPairs",
            Mutation::UnlinkRecordsById { .. } => "mutate.unlinkRecordsById",
        },
        Operation::Describe { describe } => match describe {
            Describe::Schema { .. } => "describe.schema",
            Describe::Labels { .. } => "describe.labels",
            Describe::Properties { .. } => "describe.properties",
            Describe::QueryLanguages => "describe.queryLanguages",
            Describe::Capabilities => "describe.capabilities",
            Describe::BuildInfo => "describe.buildInfo",
            Describe::Explain { .. } => "describe.explain",
            Describe::ExplainVector { .. } => "describe.explainVector",
            Describe::Indexes => "describe.indexes",
            Describe::VectorIndexes => "describe.vectorIndexes",
            Describe::VectorIndex { .. } => "describe.vectorIndex",
            Describe::Names { .. } => "describe.names",
            Describe::SchemaSummary { .. } => "describe.schemaSummary",
            Describe::ValueOverlap { .. } => "describe.valueOverlap",
        },
        Operation::Ingest { ingest } => match ingest {
            Ingest::Documents { .. } => "ingest.documents",
            Ingest::Csv { .. } => "ingest.csv",
        },
        Operation::Index { index } => match index {
            IndexOperation::Create { .. } => "index.create",
            IndexOperation::Drop { .. } => "index.drop",
            IndexOperation::CreateVector { .. } => "index.createVector",
            IndexOperation::DropVector { .. } => "index.dropVector",
            IndexOperation::UpsertVectors { .. } => "index.upsertVectors",
            IndexOperation::RevalidateVector { .. } => "index.revalidateVector",
        },
        Operation::Transaction { transaction } => match transaction {
            TransactionOperation::Begin { .. } => "transaction.begin",
            TransactionOperation::Stage { .. } => "transaction.stage",
            TransactionOperation::Commit { .. } => "transaction.commit",
            TransactionOperation::Rollback { .. } => "transaction.rollback",
        },
        Operation::PurgeTenant => "purgeTenant",
        Operation::Backup { .. } => "backup",
        Operation::Policy { policy } => match policy {
            rushdb_protocol::PolicyOperation::Get => "policy.get",
            rushdb_protocol::PolicyOperation::Put { .. } => "policy.put",
        },
    }
}

/// Executes canonical protocol operations against a booted [`Node`].
///
/// RM-1: `node` is an explicit [`RequestScope`], not a bare `&Node` — the
/// tenant for this executor's whole lifetime is resolved once, at
/// construction, from the scope the transport already authenticated. Every
/// existing `self.node.foo()` call site keeps compiling unchanged whether
/// `foo` lives on `Node` (via `RequestScope`'s `Deref`) or on `RequestScope`
/// itself; nothing reads an ambient thread-local scope anymore.
pub struct ProtocolExecutor<'a> {
    node: crate::node::scope::RequestScope<'a>,
}

/// E3 `Range` mode extrema: numeric order for numbers, lexical order for
/// strings/ISO datetimes (the same ordering `describe` uses for its
/// per-property min/max). Non-comparable values (bool/null/array/object)
/// never become extrema; with no comparable scalars both are `None`.
/// Min and max of one ordered class of `values`: the numbers if there are
/// any, else the texts that are datetimes (by instant), else the texts.
/// Classes never mix (a text `min` beside a datetime `max` was returned for
/// a property stored both ways).
fn values_range(
    values: &[serde_json::Value],
) -> (Option<serde_json::Value>, Option<serde_json::Value>) {
    if !values.iter().any(serde_json::Value::is_number) {
        let dated: Vec<(i64, &serde_json::Value)> = values
            .iter()
            .filter_map(|v| Some((rushdb_types::datetime::parse_datetime(v.as_str()?)?, v)))
            .collect();
        if !dated.is_empty() {
            let min = dated
                .iter()
                .min_by_key(|(ms, _)| *ms)
                .map(|(_, v)| (*v).clone());
            let max = dated
                .iter()
                .max_by_key(|(ms, _)| *ms)
                .map(|(_, v)| (*v).clone());
            return (min, max);
        }
    }
    let numbers = values.iter().any(serde_json::Value::is_number);
    let comparable = |v: &serde_json::Value| {
        if numbers {
            v.is_number()
        } else {
            v.is_string()
        }
    };
    let cmp = |a: &serde_json::Value, b: &serde_json::Value| -> std::cmp::Ordering {
        match (a, b) {
            (serde_json::Value::Number(_), serde_json::Value::Number(_)) => {
                rushdb_types::value::json_scalar_cmp(a, b).unwrap_or(std::cmp::Ordering::Equal)
            }
            (serde_json::Value::String(x), serde_json::Value::String(y)) => x.cmp(y),
            _ => std::cmp::Ordering::Equal,
        }
    };
    let mut min: Option<&serde_json::Value> = None;
    let mut max: Option<&serde_json::Value> = None;
    for v in values.iter().filter(|v| comparable(v)) {
        if min.is_none_or(|m| cmp(v, m) == std::cmp::Ordering::Less) {
            min = Some(v);
        }
        if max.is_none_or(|m| cmp(v, m) == std::cmp::Ordering::Greater) {
            max = Some(v);
        }
    }
    (min.cloned(), max.cloned())
}

fn operation_label(operation: &Operation) -> &'static str {
    match operation {
        Operation::Query { .. } => "query",
        Operation::Mutate { .. } => "mutate",
        Operation::Describe { .. } => "describe",
        Operation::Ingest { .. } => "ingest",
        Operation::Index { .. } => "index",
        Operation::Transaction { .. } => "transaction",
        Operation::PurgeTenant => "purge_tenant",
        Operation::Backup { .. } => "backup",
        Operation::Policy { .. } => "policy",
    }
}

fn response_row_count(response: &Response) -> u64 {
    match response {
        Response::Records { records, .. }
        | Response::Rows { rows: records, .. }
        | Response::TextHits { rows: records, .. }
        | Response::StatementMutation { rows: records, .. }
        | Response::Ingested { records, .. } => records.len() as u64,
        Response::Relationships { relationships, .. } => relationships.len() as u64,
        Response::PropertyValues { values, .. } => values.len() as u64,
        _ => 0,
    }
}

fn observe_response_work(node: &Node, operation: &str, response: &Response) {
    node.observe_work(operation, "response_rows", response_row_count(response));
    let bytes = serde_json::to_vec(response).map_or(0, |encoded| encoded.len() as u64);
    node.observe_work(operation, "response_bytes", bytes);
}

impl<'a> ProtocolExecutor<'a> {
    /// Wrap a node for protocol execution under the given, already-
    /// authenticated transport scope (RM-1: explicit, not the former ambient
    /// thread-local — the transport resolves `scope` from its own channel,
    /// e.g. an HTTP header or gRPC/Bolt metadata, before this call).
    pub fn new(node: &'a Node, scope: Option<String>) -> Self {
        Self {
            node: crate::node::scope::RequestScope::new(node, scope),
        }
    }

    /// Execute one request to completion, without cancellation.
    ///
    /// # Errors
    ///
    /// Returns a [`ProtocolError`] when the request's protocol version, tenant
    /// scope, or operation is invalid or the underlying engine call fails.
    pub fn execute(&self, request: RequestEnvelope) -> Result<ResponseEnvelope, ProtocolError> {
        self.execute_cancellable(request, &|| false)
    }

    /// Execute one request, polling `is_cancelled` for query/describe work.
    /// The request's bounds ride on every engine handle it uses.
    ///
    /// # Errors
    ///
    /// Returns a [`ProtocolError`] when the request's protocol version, tenant
    /// scope, bounds or operation are invalid, when cancellation is observed
    /// before a cancellable operation starts, or when the underlying engine
    /// call fails.
    pub fn execute_cancellable(
        &self,
        request: RequestEnvelope,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<ResponseEnvelope, ProtocolError> {
        let limits = request_limits(&request.context, self.node.request_timeout_ms())?;
        ProtocolExecutor {
            node: self.node.clone().with_request_limits(limits),
        }
        .execute_scoped(request, is_cancelled)
    }

    #[expect(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        reason = "single linear operation/reader; splitting it would scatter shared borrows and hurt auditability"
    )]
    fn execute_scoped(
        &self,
        request: RequestEnvelope,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<ResponseEnvelope, ProtocolError> {
        if let Some(error) = self.node.resolution_error() {
            return Err(ProtocolError::new(
                ErrorCode::Unavailable,
                format!("tenant scope could not be resolved: {error}"),
            ));
        }
        if request.protocol_version != PROTOCOL_VERSION {
            return Err(ProtocolError::new(
                ErrorCode::UnsupportedVersion,
                format!(
                    "unsupported protocol version {}; expected {PROTOCOL_VERSION}",
                    request.protocol_version
                ),
            ));
        }
        let request = bytecode::as_statement(request)?;
        // E4 stall observability: every successful envelope echoes the
        // server-side execution time so operators can tell slow from stuck
        // without a second measurement channel.
        let started = std::time::Instant::now();
        let operation = operation_label(&request.operation);
        self.validate_scope(request.context.scope.as_deref())?;
        // Rollback is cleanup. It must remain available when the ordinary
        // query phase pool is exhausted, so do not admit it through an
        // ExecutionContext (which reserves an 8 MiB query phase token).
        // Scope validation above and the tenant-bound coordinator still
        // enforce the request's authorization boundary.
        if let Operation::Transaction {
            transaction: TransactionOperation::Rollback { transaction_id },
        } = &request.operation
        {
            self.node
                .tx_rollback(transaction_id)
                .map_err(|error| self.transaction_failed(transaction_id, error))?;
            let response = Response::TransactionOutcome {
                transaction_id: transaction_id.clone(),
                status: TransactionStatus::RolledBack,
                commit_index: None,
                reason: None,
                retryable: false,
            };
            let encoded_bytes = serde_json::to_vec(&response).map_err(|error| {
                ProtocolError::new(
                    ErrorCode::ExecutionFailed,
                    format!("encode response for size check: {error}"),
                )
            })?;
            if encoded_bytes.len() > self.node.max_response_bytes() {
                return Err(ProtocolError::new(
                    ErrorCode::ResourceExhausted,
                    format!(
                        "response exceeds configured byte limit ({} > {})",
                        encoded_bytes.len(),
                        self.node.max_response_bytes()
                    ),
                ));
            }
            observe_response_work(&self.node, operation, &response);
            return Ok(ResponseEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: request.request_id,
                server_timing_ms: Some(
                    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
                ),
                response,
            });
        }
        // Disk-space guard, for every transport: a write fails safe while
        // free space is below the configured threshold rather than run into
        // a mid-commit ENOSPC.
        if is_write(&request.operation) && !self.node.disk_space_ok() {
            return Err(ProtocolError::new(
                ErrorCode::Unavailable,
                "free disk below the configured threshold; writes rejected",
            ));
        }
        let deadline_exempt = matches!(
            &request.operation,
            Operation::Transaction {
                transaction: TransactionOperation::Rollback { .. }
            }
        );
        let deadline_ms =
            effective_deadline_ms(request.context.deadline_ms, self.node.request_timeout_ms());
        let mut execution_context = self
            .node
            .execution_context(
                execution_operation_class(&request.operation),
                (!deadline_exempt)
                    .then_some(deadline_ms)
                    .flatten()
                    .map(std::time::Duration::from_millis),
            )
            .map_err(execution_failed)?;
        // Capture the policy generation from the same pinned view used by all
        // reads in this request.  The generation is metadata for downstream
        // authorization and cache keys; it must never come from a fresh view.
        let active_policy = rushdb_authz::load_active_policy_in_view(
            execution_context.read_view().as_ref(),
            execution_context.tenant(),
        )
        .map_err(|error| execution_failed(error.to_string()))?;
        let policy_enforced = active_policy
            .as_ref()
            .is_some_and(|active| active.mode == rushdb_authz::AuthorizationMode::PolicyEnforced);
        // Catalog reads (describe, value search, SQL's tables) see only what
        // an enforced policy allows, through one access check.
        let catalog_access =
            rushdb_lmpg::CatalogAccess::new(active_policy.clone().map(std::sync::Arc::new));
        let sql_policy = catalog_access.policy().cloned().map(std::sync::Arc::new);
        let policy_generation = active_policy.map_or(0, |active| active.generation);
        execution_context.set_policy_generation(policy_generation);
        if !deadline_exempt {
            execution_context
                .check_deadline()
                .map_err(|error| execution_failed(error.to_string()))?;
        }
        // A cancellation observed before dispatch must prevent every
        // mutation from entering its write path as well as reads. Rollback is
        // deliberately exempt because it is cleanup and must remain usable
        // under pressure.
        if !deadline_exempt && is_cancelled() {
            return Err(ProtocolError::new(
                ErrorCode::Cancelled,
                "operation cancelled",
            ));
        }
        // Fail closed for operations that cannot yet intersect the active
        // data-access policy. Every other read path routes through
        // `authorize_search_in_view` or an equivalent policy check; these do
        // not, so allowing them would leak labeled/typed data outside the
        // policy's allowed set.
        if policy_enforced
            && matches!(
                &request.operation,
                Operation::Backup { .. }
                    | Operation::Query {
                        query: Query::Statement {
                            language: StatementLanguage::Gql
                                | StatementLanguage::Cypher
                                | StatementLanguage::Gremlin
                                | StatementLanguage::SqlPgq
                                | StatementLanguage::Mongo,
                            ..
                        } | Query::JoinPairs(_)
                            | Query::VectorIndexCandidates { .. }
                            | Query::Text { .. }
                    }
                    | Operation::Describe {
                        describe: Describe::Explain { .. }
                    }
            )
        {
            return Err(ProtocolError::new(
                ErrorCode::Unauthorized,
                "policy-enforced reads are unavailable for this adapter",
            ));
        }
        // A streaming-replication follower is read-only.
        // `Transaction::Begin`/`Rollback` are exempted (same as the
        // maintenance-mode gate below) — they touch no durable state, so
        // blocking them would only stop a client from even attempting a
        // transaction before learning it must go to the leader.
        if self.node.is_following() && is_write(&request.operation) {
            return Err(ProtocolError::new(
                ErrorCode::Unavailable,
                "this node is a streaming-replication follower; writes must go to the leader",
            ));
        }
        let cancellable = matches!(
            &request.operation,
            Operation::Query { .. } | Operation::Describe { .. }
        );
        let mut response = match request.operation {
            Operation::Query { query } => match query {
                Query::GremlinBytecode { .. } => return Err(bytecode::not_rewritten()),
                Query::Statement {
                    language,
                    statement,
                    parameters,
                } => 'statement: {
                    if !self.node.languages().enabled(language) {
                        return Err(ProtocolError::new(
                            ErrorCode::LanguageDisabled,
                            format!("query language `{}` is not enabled", language.as_str()),
                        ));
                    }
                    if matches!(language, StatementLanguage::Cypher | StatementLanguage::Gql) {
                        if let Some(ddl) = rushdb_language::ddl::parse_index_ddl(&statement) {
                            if !matches!(ddl, Ok(rushdb_language::ddl::IndexDdl::Show) | Err(_)) {
                                return Err(ProtocolError::new(
                                    ErrorCode::InvalidRequest,
                                    "index DDL is a mutation; send it as mutate.statement",
                                ));
                            }
                            break 'statement self.index_ddl(ddl, &mut execution_context)?;
                        }
                    }
                    let mut mongo_matched: Option<u64> = None;
                    let rows = match language {
                        StatementLanguage::Sql => self
                            .node
                            .sql_with_parameters_budget_in_view_cancellable(
                                std::sync::Arc::clone(execution_context.read_view()),
                                &statement,
                                parameters.as_ref(),
                                execution_context.remaining_budget(),
                                &|| execution_context.check_active().is_err(),
                                sql_policy,
                            )
                            .map_err(sql_failed)?
                            .into_iter()
                            .map(|row| {
                                serde_json::Value::Object(
                                    row.into_iter()
                                        .collect::<serde_json::Map<String, serde_json::Value>>(),
                                )
                            })
                            .collect::<Vec<_>>(),
                        StatementLanguage::Gql
                        | StatementLanguage::Cypher
                        | StatementLanguage::Gremlin
                        | StatementLanguage::SqlPgq => {
                            let language = match language {
                                StatementLanguage::Gql => rushdb_language::QueryLanguage::Gql,
                                StatementLanguage::Cypher => rushdb_language::QueryLanguage::Cypher,
                                StatementLanguage::Gremlin => {
                                    rushdb_language::QueryLanguage::Gremlin
                                }
                                StatementLanguage::SqlPgq => rushdb_language::QueryLanguage::SqlPgq,
                                StatementLanguage::Sql | StatementLanguage::Mongo => unreachable!(),
                            };
                            rushdb_language::execute_in_view_with_budget_cancellable(
                                execution_context.read_view(),
                                execution_context.tenant(),
                                language,
                                &statement,
                                parameters.as_ref(),
                                execution_context.remaining_budget(),
                                &|| execution_context.check_active().is_err(),
                                Some(self.node.text_indexes()),
                            )
                            .map_err(language_execution_failed)?
                            .rows
                        }
                        StatementLanguage::Mongo => {
                            let (total, rows) = rushdb_mongodb::execute_query_in_view_with_budget(
                                &self.node.lmpg(),
                                std::sync::Arc::clone(execution_context.read_view()),
                                &statement,
                                &|| execution_context.check_active().is_err(),
                                execution_context.remaining_budget(),
                                Some(self.node.text_indexes()),
                            )
                            .map_err(execution_failed)?;
                            mongo_matched = Some(total);
                            rows
                        }
                    };
                    Response::Rows {
                        total: mongo_matched.unwrap_or(rows.len() as u64),
                        rows,
                    }
                }
                Query::JoinPairs(query) => {
                    let (left_property, right_property) = match &query.predicate {
                        rushdb_protocol::JoinPredicate::PropertyOverlap {
                            left_property,
                            right_property,
                        } => (left_property, right_property),
                    };
                    if query.left.label.trim().is_empty()
                        || query.right.label.trim().is_empty()
                        || left_property.trim().is_empty()
                        || right_property.trim().is_empty()
                        || !query.left.filter.is_object()
                        || !query.right.filter.is_object()
                        || query.limit == 0
                        || query.limit > 10_000
                    {
                        return Err(ProtocolError::new(
                            ErrorCode::InvalidRequest,
                            "join labels must be non-empty and limit must be between 1 and 10000",
                        ));
                    }
                    match query.projection {
                        JoinProjection::Count => {
                            let value = self
                                .node
                                .query_join_pairs_with_execution_context(
                                    &mut execution_context,
                                    &query,
                                    is_cancelled,
                                )
                                .map_err(execution_failed)?;
                            Response::Count { value }
                        }
                    }
                }
                Query::Records { query } => self.query_records_with_context(
                    &query,
                    is_cancelled,
                    Some(&mut execution_context),
                )?,
                Query::ExportCsv { query } => {
                    self.export_csv(&query, is_cancelled, &mut execution_context)?
                }
                Query::Cursor { cursor_id, limit } => self.query_cursor_with_context(
                    &cursor_id,
                    limit,
                    is_cancelled,
                    &mut execution_context,
                )?,
                Query::Record { id } => Response::Record {
                    record: self
                        .node
                        .get_record_with_execution_context(&mut execution_context, &id)
                        .map_err(|error| {
                            if error == "record denied by policy" {
                                ProtocolError::new(ErrorCode::Unauthorized, error)
                            } else {
                                execution_failed(error)
                            }
                        })?,
                },
                Query::Relationships { query } => {
                    self.query_relationships(&query, &mut execution_context)?
                }
                Query::PropertyValues {
                    property,
                    query,
                    mode,
                    limit,
                    search,
                } => self.query_property_values(
                    &property,
                    &query,
                    mode,
                    limit,
                    search.as_ref(),
                    is_cancelled,
                    &mut execution_context,
                    &catalog_access,
                )?,
                Query::SearchValues {
                    search,
                    labels,
                    limit,
                } => self.query_search_values(
                    &search,
                    &labels,
                    limit,
                    is_cancelled,
                    &mut execution_context,
                    &catalog_access,
                )?,
                Query::Vector { query } => {
                    self.query_vector_with_context(&mut execution_context, &query)?
                }
                Query::VectorIndexCandidates { query } => {
                    self.query_vector_index_candidates_with_context(&mut execution_context, &query)?
                }
                Query::Text {
                    index,
                    query,
                    limit,
                    all_terms,
                    phrase,
                } => self.query_text_with_context(
                    &index,
                    &query,
                    limit,
                    rushdb_textsearch::LexicalOptions { all_terms, phrase },
                    is_cancelled,
                    &execution_context,
                )?,
                Query::Hybrid { query } => {
                    self.query_hybrid_with_context(&mut execution_context, &query)?
                }
            },
            Operation::Mutate { mutation } => {
                if self.node.maintenance() {
                    return Err(ProtocolError::new(
                        ErrorCode::Unavailable,
                        "maintenance mode: writes are rejected until maintenance is cleared",
                    ));
                }
                self.mutate(mutation, is_cancelled, &mut execution_context)?
            }
            Operation::Describe { describe } => self.describe_with_access(
                describe,
                &catalog_access,
                is_cancelled,
                &mut execution_context,
            )?,
            Operation::Ingest { ingest } => {
                if self.node.maintenance() {
                    return Err(ProtocolError::new(
                        ErrorCode::Unavailable,
                        "maintenance mode: writes are rejected until maintenance is cleared",
                    ));
                }
                self.ingest(ingest, &mut execution_context, is_cancelled)?
            }
            Operation::Index { .. } | Operation::PurgeTenant if self.node.maintenance() => {
                return Err(ProtocolError::new(
                    ErrorCode::Unavailable,
                    "maintenance mode: writes are rejected until maintenance is cleared",
                ));
            }
            Operation::Index { index } => {
                self.index(index, &mut execution_context, is_cancelled)?
            }
            Operation::Transaction { transaction } => {
                if self.node.maintenance()
                    && !matches!(
                        transaction,
                        TransactionOperation::Begin { .. } | TransactionOperation::Rollback { .. }
                    )
                {
                    return Err(ProtocolError::new(
                        ErrorCode::Unavailable,
                        "maintenance mode: writes are rejected until maintenance is cleared",
                    ));
                }
                self.transaction(transaction, &mut execution_context, is_cancelled)?
            }
            Operation::PurgeTenant => Response::PurgedTenant {
                commit_index: self
                    .node
                    .purge_tenant_with_execution_context(&mut execution_context, is_cancelled)
                    .map_err(execution_failed)?,
            },
            Operation::Policy { policy } => self.policy(policy, &execution_context)?,
            Operation::Backup { request } => {
                self.validate_backup_scopes(&request)?;
                let (commit_index, tenants) = self
                    .node
                    .backup_in_view(&request, execution_context.read_view())
                    .map_err(execution_failed)?;
                Response::Backup {
                    commit_index,
                    tenants,
                }
            }
        };
        if cancellable && is_cancelled() {
            return Err(ProtocolError::new(
                ErrorCode::Cancelled,
                "operation cancelled",
            ));
        }
        // Re-check the shared execution context after the transport-specific
        // executor returns. A query may have spent its whole budget inside a
        // storage or language adapter; do not publish a late result after the
        // context deadline has elapsed.
        let terminal_mutation = matches!(
            &response,
            Response::Record { .. }
                | Response::Mutation { .. }
                | Response::StatementMutation { .. }
                | Response::Ingested { .. }
                | Response::VectorsUpserted { .. }
                | Response::PurgedTenant { .. }
                | Response::TransactionOutcome {
                    status: TransactionStatus::Committed
                        | TransactionStatus::Aborted
                        | TransactionStatus::Expired
                        | TransactionStatus::RolledBack,
                    ..
                }
        );
        if !deadline_exempt && !terminal_mutation {
            execution_context
                .check_deadline()
                .map_err(|error| execution_failed(error.to_string()))?;
        }
        let mut encoded_bytes = serde_json::to_vec(&response).map_err(|error| {
            ProtocolError::new(
                ErrorCode::ExecutionFailed,
                format!("encode response for size check: {error}"),
            )
        })?;
        if encoded_bytes.len() > self.node.max_response_bytes() {
            // The record itself is already durably committed. Fall back to a
            // compact terminal acknowledgement rather than reporting a
            // retryable failure that could duplicate the write. Reads and
            // other large responses still fail before transport encoding.
            if matches!(response, Response::Record { .. }) {
                response = Response::Mutation { affected: 1 };
                encoded_bytes = serde_json::to_vec(&response).map_err(|error| {
                    ProtocolError::new(
                        ErrorCode::ExecutionFailed,
                        format!("encode compact mutation response: {error}"),
                    )
                })?;
            }
        }
        if encoded_bytes.len() > self.node.max_response_bytes() {
            let error = ProtocolError::new(
                ErrorCode::ResourceExhausted,
                format!(
                    "response exceeds configured byte limit ({} > {})",
                    encoded_bytes.len(),
                    self.node.max_response_bytes()
                ),
            );
            // A terminal mutation is already durably committed. Reporting a
            // retryable error would invite a client retry that duplicates the
            // write, so mark it non-retryable: the caller must discover the
            // outcome, not re-execute.
            return Err(if terminal_mutation {
                error.with_retryable(false)
            } else {
                error
            });
        }
        // `charge_bytes` also checks the request deadline. Once a mutation
        // has published a terminal outcome, preserve that outcome even when
        // the response budget accounting happens after the deadline. The
        // explicit response-size check above still enforces the hard limit.
        if !terminal_mutation {
            execution_context
                .charge_bytes(encoded_bytes.len())
                .map_err(|error| execution_failed(error.to_string()))?;
        }
        observe_response_work(&self.node, operation, &response);
        Ok(ResponseEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: request.request_id,
            server_timing_ms: Some(started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
            response,
        })
    }

    fn validate_scope(&self, requested: Option<&str>) -> Result<(), ProtocolError> {
        match (self.node.multitenant(), requested) {
            (false, None) => Ok(()),
            (false, Some(_)) => Err(ProtocolError::new(
                ErrorCode::ScopeMismatch,
                "scope must be omitted in single-tenant mode",
            )),
            (true, None) => Err(ProtocolError::new(
                ErrorCode::MissingScope,
                "scope is required in multitenant mode",
            )),
            (true, Some(requested)) => {
                if self.node.scope_value() == Some(requested) {
                    Ok(())
                } else {
                    Err(ProtocolError::new(
                        ErrorCode::ScopeMismatch,
                        "protocol scope does not match the authenticated transport scope",
                    ))
                }
            }
        }
    }

    /// A tenant-scoped request may back up only its own keyspace. `scopes` is
    /// an operator-facing selector in offline tooling; accepting it unchecked
    /// on the public multitenant protocol would let tenant A export tenant B.
    /// An empty list therefore means the current request scope, never "all".
    fn validate_backup_scopes(
        &self,
        request: &rushdb_protocol::BackupRequest,
    ) -> Result<(), ProtocolError> {
        if !self.node.multitenant() {
            return Ok(());
        }
        let current = self.node.scope_value().map(str::to_string).ok_or_else(|| {
            ProtocolError::new(
                ErrorCode::MissingScope,
                "scope is required in multitenant mode",
            )
        })?;
        if request.scopes.iter().all(|scope| scope == &current) {
            Ok(())
        } else {
            Err(ProtocolError::new(
                ErrorCode::ScopeMismatch,
                "backup scopes must match the authenticated tenant scope",
            ))
        }
    }
}

fn is_label_less_container(documents: &serde_json::Value) -> bool {
    let Some(container) = documents.as_object() else {
        return false;
    };
    !container.is_empty()
        && container.values().all(|value| {
            value.is_object()
                || value.as_array().is_some_and(|items| {
                    !items.is_empty()
                        && items.iter().any(|item| item.is_object() || item.is_array())
                })
        })
}

fn query_document_to_parser_input(query: &QueryDocument) -> serde_json::Value {
    let mut document = serde_json::json!({
        "labels": query.labels,
        "where": query.predicate,
        "skip": query.offset,
        "limit": query.limit,
    });
    if let Some(select) = &query.select {
        document["select"] = select.clone();
    }
    if !query.group_by.is_empty() {
        document["groupBy"] = serde_json::Value::Array(
            query
                .group_by
                .iter()
                .map(|g| serde_json::Value::String(g.clone()))
                .collect(),
        );
    }
    if let Some(order) = &query.order {
        document["orderBy"] = order.clone();
    }
    document
}

mod bytecode;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod facet_tests;

#[cfg(test)]
mod contract_tests;

#[cfg(test)]
mod atomic_write_tests;

#[cfg(test)]
mod csv_ingest_tests;
#[cfg(test)]
mod merge_key_tests;
mod request_bounds;
pub(crate) use request_bounds::effective_deadline_ms;
use request_bounds::request_limits;
#[cfg(test)]
mod request_bounds_tests;
#[cfg(test)]
mod statement_text_search_tests;
#[cfg(test)]
mod text_search_tests;
#[cfg(test)]
mod transaction_ingest_tests;
#[cfg(test)]
mod transaction_reference_tests;
#[cfg(test)]
mod unlink_tests;

#[cfg(test)]
mod structural_index_tests;

#[cfg(test)]
mod gremlin_bytecode_tests;

#[cfg(test)]
mod value_domain_index_tests {
    use super::*;
    use rushdb_lmpg::plan::FieldChange;

    #[test]
    fn value_domain_index_matches_scan_and_falls_back_for_arrays() {
        use rushdb_protocol::PropertyValuesMode;
        let mut cfg = crate::ServerConfig::default();
        let node = Node::bootstrap(&mut cfg).unwrap();
        let lmpg = node.scope().lmpg();
        let set = |k: &str, v: serde_json::Value| (k.to_string(), FieldChange::Set(v));
        lmpg.create_record(
            "P",
            None,
            vec![
                set("n", serde_json::json!(1)),
                set("s", serde_json::json!("a")),
                set("arr", serde_json::json!([1, 2])),
            ],
        )
        .unwrap();
        lmpg.create_record(
            "P",
            None,
            vec![
                set("n", serde_json::json!(2)),
                set("s", serde_json::json!("b")),
            ],
        )
        .unwrap();
        lmpg.create_record(
            "P",
            None,
            vec![
                set("n", serde_json::json!(2.5)),
                set("s", serde_json::json!("a")),
            ],
        )
        .unwrap();
        lmpg.create_record(
            "P",
            None,
            vec![("nv".to_string(), FieldChange::ExplicitNull)],
        )
        .unwrap();

        let call = |property: &str, mode: PropertyValuesMode, labels: Vec<String>| {
            let Response::PropertyValues {
                distinct_count,
                min,
                max,
                ..
            } = ProtocolExecutor::new(&node, None)
                .execute(RequestEnvelope {
                    protocol_version: PROTOCOL_VERSION,
                    request_id: None,
                    context: rushdb_protocol::RequestContext::default(),
                    operation: Operation::Query {
                        query: Query::PropertyValues {
                            property: property.to_string(),
                            query: QueryDocument {
                                labels,
                                ..Default::default()
                            },
                            mode: Some(mode),
                            limit: None,
                            search: None,
                        },
                    },
                })
                .unwrap()
                .response
            else {
                panic!("expected propertyValues response")
            };
            (
                distinct_count.unwrap_or(0),
                min.and_then(|v| v.as_f64()),
                max.and_then(|v| v.as_f64()),
            )
        };
        for property in ["n", "s", "arr", "nv"] {
            for mode in [PropertyValuesMode::Count, PropertyValuesMode::Range] {
                let indexed = call(property, mode, Vec::new());
                let scanned = call(property, mode, vec!["P".to_string()]);
                assert_eq!(indexed, scanned, "{property} {mode:?}");
            }
        }
        assert_eq!(call("n", PropertyValuesMode::Count, Vec::new()).0, 3);
        assert_eq!(
            call("n", PropertyValuesMode::Range, Vec::new()),
            (3, Some(1.0), Some(2.5))
        );
        assert_eq!(call("nv", PropertyValuesMode::Count, Vec::new()).0, 1);
        assert_eq!(call("arr", PropertyValuesMode::Count, Vec::new()).0, 1);
    }

    #[test]
    fn values_mode_returns_values_of_a_mixed_case_property() {
        use rushdb_protocol::PropertyValuesMode;
        let mut cfg = crate::ServerConfig::default();
        let node = Node::bootstrap(&mut cfg).unwrap();
        let lmpg = node.scope().lmpg();
        for grade in ["K", "9", "K"] {
            lmpg.create_record(
                "School",
                None,
                vec![(
                    "Low Grade".to_string(),
                    FieldChange::Set(serde_json::json!(grade)),
                )],
            )
            .unwrap();
        }
        let values = |property: &str, labels: Vec<String>| {
            let Response::PropertyValues {
                values,
                distinct_count,
                ..
            } = ProtocolExecutor::new(&node, None)
                .execute(RequestEnvelope {
                    protocol_version: PROTOCOL_VERSION,
                    request_id: None,
                    context: rushdb_protocol::RequestContext::default(),
                    operation: Operation::Query {
                        query: Query::PropertyValues {
                            property: property.to_string(),
                            query: QueryDocument {
                                labels,
                                ..Default::default()
                            },
                            mode: Some(PropertyValuesMode::Values),
                            limit: None,
                            search: None,
                        },
                    },
                })
                .unwrap()
                .response
            else {
                panic!("expected propertyValues response")
            };
            let mut values: Vec<String> = values
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect();
            values.sort();
            (values, distinct_count)
        };
        let expected = (vec!["9".to_string(), "K".to_string()], Some(2));
        assert_eq!(values("Low Grade", Vec::new()), expected);
        assert_eq!(values("Low Grade", vec!["School".to_string()]), expected);
        assert_eq!(values("low_grade", vec!["School".to_string()]), expected);
    }

    #[test]
    fn values_mode_renders_datetimes_by_their_own_definition() {
        use rushdb_protocol::PropertyValuesMode;
        let mut cfg = crate::ServerConfig::default();
        let node = Node::bootstrap(&mut cfg).unwrap();
        let run = |operation: Operation| {
            ProtocolExecutor::new(&node, None)
                .execute(RequestEnvelope {
                    protocol_version: PROTOCOL_VERSION,
                    request_id: None,
                    context: rushdb_protocol::RequestContext::default(),
                    operation,
                })
                .unwrap()
                .response
        };
        // The same name is a string on one label (not a parseable datetime)
        // and a datetime on another, the first definition being the string.
        for (label, created) in [("Comment", "2010-07-19 19:15:52.0"), ("Vote", "2010-07-19")] {
            run(Operation::Ingest {
                ingest: rushdb_protocol::Ingest::Documents {
                    label: Some(label.to_string()),
                    documents: serde_json::json!([{ "CreationDate": created }]),
                    options: serde_json::json!({ "suggestTypes": true }),
                },
            });
        }
        let Response::PropertyValues { values, .. } = run(Operation::Query {
            query: Query::PropertyValues {
                property: "CreationDate".to_string(),
                query: QueryDocument {
                    labels: vec!["Vote".to_string()],
                    ..Default::default()
                },
                mode: Some(PropertyValuesMode::Values),
                limit: None,
                search: None,
            },
        }) else {
            panic!("expected propertyValues response")
        };
        assert_eq!(values, vec![serde_json::json!("2010-07-19")]);
    }

    #[test]
    fn values_range_preserves_large_integer_ordering() {
        let low = serde_json::Value::Number(9_007_199_254_740_992i64.into());
        let high = serde_json::Value::Number(9_007_199_254_740_993i64.into());
        let (min, max) = values_range(&[high.clone(), low.clone()]);
        assert_eq!(min, Some(low));
        assert_eq!(max, Some(high));
    }
}
