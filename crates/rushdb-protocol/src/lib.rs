//! Transport-neutral, versioned `RushDB` database protocol.
//!
//! These types describe database capabilities. HTTP, gRPC/Connect, embedded
//! callers, and application adapters are transports around this contract;
//! product resources and SDK response shapes do not belong here.

use serde::{Deserialize, Serialize};

/// Wire protocol version spoken by these envelopes.
///
/// Bumped whenever the wire shape changes in a backward-incompatible way.
pub const PROTOCOL_VERSION: u16 = 1;

/// Constant-time comparison of two secrets (bearer tokens, passwords).
///
/// Returns `false` for differing lengths (a token's length is not itself a
/// secret). Use this instead of `==` for authentication so the comparison time
/// does not reveal how many leading bytes matched.
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    // Defeat an early-exit rewrite of the accumulate loop.
    std::hint::black_box(diff) == 0
}

/// Versioned request envelope wrapping a single [`Operation`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestEnvelope {
    /// Protocol version the caller speaks; must equal [`PROTOCOL_VERSION`].
    pub protocol_version: u16,
    /// Optional caller-chosen id echoed back on the response for correlation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Request-scoped context (tenancy).
    #[serde(default)]
    pub context: RequestContext,
    /// The operation to perform.
    pub operation: Operation,
}

/// Request-scoped context carried alongside every operation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestContext {
    /// Opaque tenancy value supplied by the trusted host. The configured
    /// discriminator property remains server-side configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Optional execution deadline requested by the trusted host, in
    /// milliseconds from request admission. The engine treats this as a
    /// work deadline, not an identity or authorization decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_ms: Option<u64>,
    /// Optional working-set cap of this request's queries, in bytes; it
    /// may only tighten the server's `query-memory-max`. Must be
    /// positive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_memory_max_bytes: Option<u64>,
    /// Optional limit on the records a filter without a usable index may
    /// check; `0` refuses every such walk. It may only tighten the server's
    /// `unindexed-walk-max-owners`, like `CockroachDB`'s
    /// `disallow_full_table_scans` with `large_full_scan_rows`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unindexed_walk_max_owners: Option<u64>,
}

/// A single protocol operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Operation {
    /// Execute a read-only query.
    Query {
        /// The query to execute.
        query: Query,
    },
    /// Execute a mutation.
    Mutate {
        /// The mutation to apply.
        mutation: Mutation,
    },
    /// Introspect schema, capabilities, or query plans.
    Describe {
        /// The description request.
        describe: Describe,
    },
    /// Ingest documents.
    Ingest {
        /// The ingest request.
        ingest: Ingest,
    },
    /// Manage vector indexes.
    Index {
        /// The index operation.
        index: IndexOperation,
    },
    /// Manage a multi-statement transaction.
    Transaction {
        /// The transaction operation.
        transaction: TransactionOperation,
    },
    /// Drop every tenant-scoped key for the request scope. Used by
    /// application-level project deletion so a deleted project's data cannot
    /// leak into a later reuse of the same scope value.
    PurgeTenant,
    /// Consistent logical backup. The graph is pinned at a commit index and
    /// exported as JSONL while the server keeps serving writes. On the public
    /// multitenant protocol the scopes must match the authenticated request
    /// scope; an empty list means that one scope. Fleet-wide export is an
    /// offline/operator operation.
    Backup {
        /// Backup selection parameters.
        request: BackupRequest,
    },
    /// Read or replace the request tenant's data-access policy. Operator
    /// only: over HTTP it needs the boot token, so a capability token cannot
    /// change the policy that limits it.
    Policy {
        /// The policy operation.
        policy: PolicyOperation,
    },
}

/// A data-access policy operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PolicyOperation {
    /// The tenant's active policy, if any.
    Get,
    /// Install a policy as the tenant's next generation. A transaction begun
    /// under the previous generation aborts at commit.
    Put {
        /// `allowedLabels`, `deniedRelTypes`, `visibleProperties`
        /// (label → the only properties a reader sees) and `maxResults`.
        policy: serde_json::Value,
        /// Whether reads and writes are filtered by the policy; `false`
        /// keeps tenant isolation only.
        enforced: bool,
    },
}

/// Parameters for a logical backup.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRequest {
    /// Scopes to include. In multitenant protocol mode every value must equal
    /// the authenticated request scope; empty means that one scope.
    #[serde(default)]
    pub scopes: Vec<String>,
}

/// Value-domain mode for property-value queries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PropertyValuesMode {
    /// Full distinct list (legacy behavior; `limit` may truncate output).
    #[default]
    Values,
    /// Exact distinct count only (`values` empty, `distinct_count` set).
    Count,
    /// Min/max only (`values` empty, `min`/`max` set).
    Range,
    /// Bounded most-frequent values with owner counts (`counts` aligned with
    /// `values`); exact, answered from the equality postings when available.
    TopK,
}

/// Text filter over the distinct values of a property-value query:
/// keeps the values whose text matches. A string matches as stored; a number,
/// boolean or datetime matches as the response renders it; an array matches
/// when any element does. A text that reads as a date (year first, at least a
/// month: `1993-11-12`, `1993/11/12`, `2010/2/22`, `2013-06`) also matches
/// values holding that date in another layout (`1993-11-12T00:00:00Z`,
/// `1993.11.12`, `19931112`, `201306`), at the text's precision. The
/// engine matches with `rushdb_types::value_match::ValueMatcher`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueSearch {
    /// The text to look for.
    pub text: String,
    /// How `text` must match a value (default `contains`).
    #[serde(default, rename = "match")]
    pub matching: ValueMatch,
    /// Compare case-sensitively (default: case-insensitive).
    #[serde(default)]
    pub case_sensitive: bool,
    /// When no value matches and the property has at most this many distinct
    /// values in scope, answer with all of them (by descending owner count,
    /// with counts) and set `searchMissed`, so a caller sees how the values
    /// are actually written instead of an empty result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_on_miss: Option<u64>,
    /// `similar` only: the least similarity a value needs, from 0 to 1
    /// (default 0.6). See [`ValueMatch::Similar`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_similarity: Option<f64>,
}

/// How [`ValueSearch::text`] must match a value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ValueMatch {
    /// The value contains the text.
    #[default]
    Contains,
    /// The value starts with the text.
    Prefix,
    /// The value equals the text.
    Exact,
    /// The value is spelled like the text: equal once case, punctuation and
    /// spacing are ignored, or sharing enough of the text's character
    /// trigrams (`minSimilarity`). Values rank by similarity, then by owner
    /// count. These are near matches, not exact ones.
    Similar,
}

/// A read-only query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Query {
    /// A Gremlin traversal as `TinkerPop` `GraphSON` 3 bytecode (what language
    /// variants send), run as the equivalent Gremlin text traversal.
    GremlinBytecode {
        /// The `g:Bytecode` value (or its untyped `{"step": [...]}` form).
        bytecode: serde_json::Value,
        /// Optional bind parameters.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parameters: Option<serde_json::Value>,
    },
    /// Textual query-language frontend. Every language is transported by the
    /// same operation and must lower into an engine-owned logical plan.
    Statement {
        /// The language the statement is written in.
        language: StatementLanguage,
        /// The statement text.
        statement: String,
        /// Optional bind parameters for the statement.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parameters: Option<serde_json::Value>,
    },
    /// Native structured query.
    Records {
        /// The native query document.
        query: QueryDocument,
    },
    /// Continue a tenant-bound, snapshot-pinned native records cursor.
    /// Cursors are deliberately a Protocol v1 feature; compatibility query
    /// languages retain their ordinary materialized response contracts.
    Cursor {
        /// Cursor token returned by a previous records response.
        cursor_id: String,
        /// Maximum number of records to return in this page.
        limit: u64,
    },
    /// Fetch a single record by id.
    Record {
        /// The record id.
        id: String,
    },
    /// Query relationships.
    Relationships {
        /// The relationship query.
        query: RelationshipQuery,
    },
    /// Distinct property values.
    PropertyValues {
        /// The property to collect values for.
        property: String,
        /// The query constraining the records considered.
        query: QueryDocument,
        /// Value-domain mode. `None` = `Values` (legacy full distinct
        /// list). `Count` returns only the exact distinct count; `Range`
        /// returns only min/max; `TopK` returns the `limit` most frequent
        /// values (exact).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode: Option<PropertyValuesMode>,
        /// Bounds shipped `values` in `Values` mode (by descending record
        /// count, ties by value).
        /// `distinct_count` always reports the untruncated total and
        /// `truncated` flags truncation. Ignored by `Count`/`Range`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<u64>,
        /// Keep only the distinct values whose text matches. Every
        /// mode then answers over the matching values: `distinct_count`
        /// counts matches, and `Values` lists them by descending owner count
        /// (ties by value), answered from the value dictionaries when they
        /// cover the property.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        search: Option<ValueSearch>,
    },
    /// Vector similarity query.
    Vector {
        /// The vector query.
        query: VectorQuery,
    },
    /// Pairwise join of two record sets.
    JoinPairs(JoinPairsQuery),
    /// Records of a label+property that are not yet bound to a vector index.
    /// The control plane uses this to drive embedding backfill: fetch a batch
    /// of unbound records, embed their text, then `upsertVectors`.
    VectorIndexCandidates {
        /// The candidate query.
        query: VectorIndexCandidatesQuery,
    },
    /// Lexical (full-text) top-k search against a declared text index.
    Text {
        /// The text index name.
        index: String,
        /// The query string.
        query: String,
        /// Maximum number of hits to return.
        #[serde(default = "default_text_limit")]
        limit: u64,
        /// Every term must match (AND); by default any term may (OR).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        all_terms: bool,
        /// Match the whole query as one phrase (terms adjacent, in order).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        phrase: bool,
    },
    /// Combined graph + vector + lexical query, fused by reciprocal-rank
    /// fusion over one shared, predicate-resolved candidate scope.
    Hybrid {
        /// The hybrid query.
        query: HybridQuery,
    },
    /// The records a native query selects, rendered as CSV text: an `__id`
    /// column, an `__label` column, then every property name in sorted order.
    /// `select` and `groupBy` are rejected (the export is of records).
    ExportCsv {
        /// The native query document.
        query: QueryDocument,
    },
    /// Value search across every property, without naming one:
    /// the stored values that match `search`, with the label and property
    /// that hold them and their owner counts, by descending count. `labels`
    /// narrows the labels searched (all when empty).
    SearchValues {
        /// The search (`domainOnMiss` is ignored here).
        search: ValueSearch,
        /// Labels to search (all labels when empty).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        labels: Vec<String>,
        /// Most matches returned (default 50, at most 1000); `total` counts
        /// them all.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<u64>,
    },
}

fn default_text_limit() -> u64 {
    10
}

/// Query for records not yet bound to a vector index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorIndexCandidatesQuery {
    /// Label whose records are scanned.
    pub label: String,
    /// Property whose vector index bindings are checked.
    pub property: String,
    /// The index id (the canonical `emb__<label>__<property>__<sim>__<dims>`
    /// alias) to check bindings against.
    pub index: String,
    /// Number of matching records to skip.
    #[serde(default)]
    pub skip: u64,
    /// Maximum number of records to return.
    #[serde(default = "default_candidate_limit")]
    pub limit: u64,
}

fn default_candidate_limit() -> u64 {
    500
}

/// A textual query language frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatementLanguage {
    /// `RushDB`'s relational SQL frontend (also used by pgwire).
    Sql,
    /// ISO/IEC 39075 Graph Query Language. This is not GraphQL.
    Gql,
    /// openCypher-compatible graph query language.
    Cypher,
    /// Apache `TinkerPop` Gremlin text traversal language.
    Gremlin,
    /// SQL:2023 property graph queries using `GRAPH_TABLE`.
    SqlPgq,
    /// MongoDB-style query documents: find/aggregate + insert/update/delete
    /// commands, translated onto the native query/mutate surface.
    Mongo,
}

impl StatementLanguage {
    /// Machine-readable language id (matches `describe.capabilities`).
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            StatementLanguage::Sql => "sql",
            StatementLanguage::Gql => "gql",
            StatementLanguage::Cypher => "cypher",
            StatementLanguage::Gremlin => "gremlin",
            StatementLanguage::SqlPgq => "sqlPgq",
            StatementLanguage::Mongo => "mongo",
        }
    }
}

/// Startup language enablement. `searchquery` (the native `QueryDocument`) is
/// NOT listed here: it is always enabled and cannot be disabled. The default
/// enables the stable languages (`sql`, `gremlin`, `sql-pgq`, `mongodb`);
/// the release-candidate `cypher` and `gql` are opt-in. `config.yml`
/// (`languages:`) or `--languages` replaces the whole set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Language enablement is a fixed wire contract of independent opt-in flags; \
              collapsing them into enums would change the serialized `LanguageSet` shape"
)]
pub struct LanguageSet {
    /// `DataFusion` relational SQL + PostgreSQL wire protocol.
    pub sql: bool,
    /// ISO GQL.
    pub gql: bool,
    /// openCypher.
    pub cypher: bool,
    /// Apache `TinkerPop` Gremlin.
    pub gremlin: bool,
    /// SQL:2023 `GRAPH_TABLE` (read-only).
    pub sql_pgq: bool,
    /// MongoDB query language.
    pub mongodb: bool,
}

impl Default for LanguageSet {
    fn default() -> Self {
        Self {
            sql: true,
            gremlin: true,
            sql_pgq: true,
            mongodb: true,
            ..Self::NONE
        }
    }
}

impl LanguageSet {
    /// No statement language: only the native query document.
    pub const NONE: Self = Self {
        sql: false,
        gql: false,
        cypher: false,
        gremlin: false,
        sql_pgq: false,
        mongodb: false,
    };

    /// Parse a comma-separated `--languages` value. `searchquery` is implicit
    /// and rejected if explicitly listed as a removable language.
    ///
    /// # Errors
    ///
    /// Returns an error if the list explicitly names the always-enabled
    /// `searchquery` language or contains an unknown language name.
    pub fn from_list(s: &str) -> Result<Self, String> {
        let mut set = LanguageSet::NONE;
        for raw in s.split(',') {
            let name = raw.trim();
            if name.is_empty() {
                continue;
            }
            match name {
                "searchquery" => {
                    return Err("`searchquery` is always enabled and cannot be disabled".into())
                }
                "sql" => set.sql = true,
                "gql" => set.gql = true,
                "cypher" => set.cypher = true,
                "gremlin" => set.gremlin = true,
                "sql-pgq" => set.sql_pgq = true,
                "mongodb" => set.mongodb = true,
                other => return Err(format!("unknown language `{other}`")),
            }
        }
        Ok(set)
    }

    /// Whether a statement language is enabled.
    #[must_use]
    pub fn enabled(&self, language: StatementLanguage) -> bool {
        match language {
            StatementLanguage::Sql => self.sql,
            StatementLanguage::Gql => self.gql,
            StatementLanguage::Cypher => self.cypher,
            StatementLanguage::Gremlin => self.gremlin,
            StatementLanguage::SqlPgq => self.sql_pgq,
            StatementLanguage::Mongo => self.mongodb,
        }
    }

    /// The enabled statement languages, for capability advertisement.
    #[must_use]
    pub fn advertised(&self) -> Vec<StatementLanguage> {
        [
            (StatementLanguage::Sql, self.sql),
            (StatementLanguage::Gql, self.gql),
            (StatementLanguage::Cypher, self.cypher),
            (StatementLanguage::Gremlin, self.gremlin),
            (StatementLanguage::SqlPgq, self.sql_pgq),
            (StatementLanguage::Mongo, self.mongodb),
        ]
        .into_iter()
        .filter(|(_, on)| *on)
        .map(|(language, _)| language)
        .collect()
    }
}

/// `RushDB`'s structured query document. Its predicate/projection grammar is a
/// database concern and is parsed by `rushdb-query`. The document is an exact
/// contract: an unknown key is rejected, never skipped, so a typo
/// cannot silently widen a query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryDocument {
    /// Labels the records must carry (empty = any label).
    #[serde(default)]
    pub labels: Vec<String>,
    /// Predicate document filtering candidate records.
    #[serde(default = "empty_object")]
    pub predicate: serde_json::Value,
    /// Output-shaping select expression map (`select` in the SDK/API surface).
    /// The legacy top-level `aggregate` clause is extinct: `select` + `groupBy`
    /// is the canonical shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select: Option<serde_json::Value>,
    /// Group-key field names (`groupBy` in the SDK/API surface).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group_by: Vec<String>,
    /// Sort expression document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<serde_json::Value>,
    /// Number of matching records to skip.
    #[serde(default)]
    pub offset: u64,
    /// Maximum number of records to return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    /// Start a resumable native records cursor. A finite `limit` is required.
    /// The server caps the requested lease and binds the token to this tenant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_ttl_ms: Option<u64>,
}

impl Default for QueryDocument {
    fn default() -> Self {
        Self {
            labels: Vec::new(),
            predicate: empty_object(),
            select: None,
            group_by: Vec::new(),
            order: None,
            offset: 0,
            limit: None,
            cursor_ttl_ms: None,
        }
    }
}

/// Query over relationships between records.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipQuery {
    /// Labels the relationship source must carry.
    #[serde(default)]
    pub source_labels: Vec<String>,
    /// Labels the relationship target must carry.
    #[serde(default)]
    pub target_labels: Vec<String>,
    /// Restrict to this relationship type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation_type: Option<String>,
    /// Predicate document filtering relationships.
    #[serde(default = "empty_object")]
    pub predicate: serde_json::Value,
    /// Record predicate the source must satisfy (`query.records` syntax,
    /// `$id` included). Resolved through the source records' own links, so
    /// the cost follows their relationship count, not the store's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_predicate: Option<serde_json::Value>,
    /// Record predicate the target must satisfy; see `source_predicate`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_predicate: Option<serde_json::Value>,
    /// Number of matching relationships to skip.
    #[serde(default)]
    pub offset: u64,
    /// Maximum number of relationships to return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
}

/// A write operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Mutation {
    /// A Gremlin write traversal as `GraphSON` 3 bytecode; see
    /// [`Query::GremlinBytecode`].
    GremlinBytecode {
        /// The `g:Bytecode` value.
        bytecode: serde_json::Value,
        /// Optional bind parameters.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parameters: Option<serde_json::Value>,
    },
    /// Graph-language write. Keeping this under `Mutation` makes authorization
    /// independent of statement text and prevents a write from entering via a
    /// read-authorized `Query::Statement` operation.
    Statement {
        /// The language the statement is written in.
        language: StatementLanguage,
        /// The statement text.
        statement: String,
        /// Optional bind parameters for the statement.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parameters: Option<serde_json::Value>,
    },
    /// Create, merge, or replace a single record.
    PutRecord {
        /// How to reconcile with an existing record.
        mode: PutMode,
        /// Optional explicit record id.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Optional primary label.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// Record properties.
        properties: serde_json::Value,
        /// Optional vectors to associate with the record.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        vectors: Vec<serde_json::Value>,
        /// Put options document.
        #[serde(default = "empty_object")]
        options: serde_json::Value,
    },
    /// Delete a record by id.
    DeleteRecord {
        /// The record id.
        id: String,
    },
    /// Atomically change a record's single primary label. The stable record
    /// id, ordinal, fields, vectors, and relationships are retained.
    RetypeRecord {
        /// The record id.
        id: String,
        /// The current primary label.
        from_label: String,
        /// The new primary label.
        to_label: String,
    },
    /// Delete every record matching a query.
    DeleteRecords {
        /// The query selecting records to delete.
        query: QueryDocument,
    },
    /// Remove a property (field) from the tenant: every occurrence on records
    /// and relationships, plus the family and definitions. `name` is matched
    /// against the normalized property name.
    DeleteProperty {
        /// The normalized property name to delete.
        name: String,
    },
    /// Link records matched by label, key, and predicate.
    LinkRecords {
        /// The relationship mutation.
        command: RelationshipMutation,
    },
    /// Remove links matched by label, key, and predicate.
    UnlinkRecords {
        /// The relationship mutation.
        command: RelationshipMutation,
    },
    /// Retype relationships between two labels.
    RetypeRelations {
        /// The retype mutation.
        command: RetypeRelationsMutation,
    },
    /// Link records by explicit record IDs (attach semantics). Unlike
    /// `LinkRecords` (label + key + predicate join), the endpoints are named
    /// directly, so Core's attach/detach-by-id surface maps without a label
    /// join. Idempotent per pair (MERGE semantics).
    LinkRecordsById {
        /// The by-id relationship mutation.
        command: RelationshipIdsMutation,
    },
    /// Delete the relationships between explicit source and target records.
    UnlinkRecordsById {
        /// Which relationships to delete.
        command: UnlinkRecordsMutation,
    },
    /// Pairwise batch link: explicit (source, target) pairs, one edge per
    /// pair — unlike `LinkRecordsById`, which is cross-product. The whole
    /// batch resolves up front (dangling id fails the call) and commits
    /// atomically in one batch; existing edges are skipped and reported.
    /// Per-pair `relationType` overrides the top-level default, so one call
    /// can carry heterogeneous edge types.
    LinkRecordsPairs {
        /// The pairwise relationship mutation.
        command: RelationshipPairsMutation,
    },
}

/// Rename the relationship type between two labels: every existing edge of
/// `source_type` whose endpoints are a `source_label` record and a
/// `target_label` record is re-created as `target_type` (endpoints preserved,
/// direction honored), then the old edge is removed. Mirrors the Neo4j
/// retype semantics used by relationship-pattern approval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetypeRelationsMutation {
    /// Label the source endpoint must carry.
    pub source_label: String,
    /// Label the target endpoint must carry.
    pub target_label: String,
    /// Relationship type to rewrite from.
    #[serde(default = "default_rel_type")]
    pub source_type: String,
    /// Relationship type to rewrite to.
    pub target_type: String,
    /// Direction of the relationship.
    #[serde(default)]
    pub direction: Direction,
}

fn default_rel_type() -> String {
    "__RUSHDB__RELATION__DEFAULT__".to_string()
}

/// Record write reconciliation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PutMode {
    /// Fail if a record already exists.
    Create,
    /// Update the record if it exists, otherwise create it.
    Merge,
    /// Overwrite the record entirely.
    Replace,
}

/// Link records selected by label/key/predicate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipMutation {
    /// The relationship type to create.
    pub relation_type: String,
    /// Selector for the source endpoint.
    pub source: RelationshipSelector,
    /// Selector for the target endpoint.
    pub target: RelationshipSelector,
    /// Direction of the relationship.
    #[serde(default)]
    pub direction: Direction,
    /// Allow a cross-product when a selector matches multiple records.
    #[serde(default)]
    pub allow_cartesian: bool,
    /// Relationship properties.
    #[serde(default = "empty_object")]
    pub properties: serde_json::Value,
}

/// Selects records of a label matching a predicate and optional property key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipSelector {
    /// Label the selected records must carry.
    pub label: String,
    /// Optional property key used to identify records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub property: Option<String>,
    /// Predicate document filtering records.
    #[serde(default = "empty_object")]
    pub predicate: serde_json::Value,
}

/// By-ID relationship mutation (attach/detach). `source_ids` and `target_ids`
/// are record `_id` values (dashed UUID or compact hex). For each source→target
/// pair the engine creates/deletes a relationship of `relation_type`.
/// `direction: in` treats the pair as target→source (endpoint swap).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipIdsMutation {
    /// The relationship type to create or delete.
    pub relation_type: String,
    /// Direction of the relationship.
    #[serde(default)]
    pub direction: Direction,
    /// Source endpoint record ids.
    pub source_ids: Vec<String>,
    /// Target endpoint record ids.
    pub target_ids: Vec<String>,
    /// Relationship properties.
    #[serde(default = "empty_object")]
    pub properties: serde_json::Value,
}

/// Delete the relationships between every source and every target record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnlinkRecordsMutation {
    /// Relationship types to delete; empty deletes relationships of every
    /// type.
    #[serde(default)]
    pub relation_types: Vec<String>,
    /// Direction from source to target; absent deletes relationships in both
    /// directions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<Direction>,
    /// Source endpoint record ids.
    pub source_ids: Vec<String>,
    /// Target endpoint record ids.
    pub target_ids: Vec<String>,
}

/// One explicit edge of a pairwise batch link.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipPair {
    /// Source endpoint record id.
    pub source_id: String,
    /// Target endpoint record id.
    pub target_id: String,
    /// Per-pair edge type; absent = the batch `relation_type`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation_type: Option<String>,
}

/// Pairwise batch link: N explicit pairs → up to N edges in one atomic
/// commit (no cross-product expansion).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipPairsMutation {
    /// Default relationship type applied to pairs without their own.
    pub relation_type: String,
    /// Direction of the relationships.
    #[serde(default)]
    pub direction: Direction,
    /// Explicit source/target pairs.
    pub pairs: Vec<RelationshipPair>,
    /// Relationship properties.
    #[serde(default = "empty_object")]
    pub properties: serde_json::Value,
}

/// Direction of a relationship.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    /// Source → target.
    #[default]
    Out,
    /// Target → source.
    In,
}

/// An introspection request. Unknown fields are rejected, as in a query
/// document: a `schemaSummary` sent a `query` predicate used to ignore it and
/// summarize the whole tenant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Describe {
    /// Describe schema for the records matching a query.
    Schema {
        /// The query constraining the described records.
        query: QueryDocument,
        /// Property-entry detail level. `None`/`Full` reads up to
        /// 10k records for samples/min/max; `Catalog` answers names, types,
        /// exact support counts, array shape, and sibling flags from catalog
        /// indexes alone (no record scan).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<PropertyDetail>,
        /// Property-entry pagination: slice of the `properties` list.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skip: Option<u64>,
        /// Maximum number of property entries to return.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<u64>,
    },
    /// List labels.
    Labels {
        /// The query constraining the described labels.
        query: QueryDocument,
    },
    /// Describe properties.
    Properties {
        /// The query constraining the described properties.
        query: QueryDocument,
        /// Entry detail level: `None`/`Full` reads up to 10k
        /// records for samples/min/max; `Catalog` answers names, types,
        /// exact support counts, array shape, and sibling flags from catalog
        /// indexes alone (no record scan) and rejects a non-empty `where`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<PropertyDetail>,
        /// Property-entry pagination: slice of the returned entry list.
        /// Independent of the stats scan window.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skip: Option<u64>,
        /// Maximum number of property entries to return.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<u64>,
    },
    /// Machine-readable statement-language profiles. Clients must use this
    /// instead of inferring compatibility from a server version string.
    QueryLanguages,
    /// Feature-capability report (see [`Response::Capabilities`]).
    Capabilities,
    /// Version/build identity, transport bindings, and configured limits
    /// (see [`Response::BuildInfo`]).
    BuildInfo,
    /// Query plan introspection for a `QueryDocument` (see
    /// [`Response::Explain`]).
    Explain {
        /// The query document to explain.
        query: QueryDocument,
    },
    /// Query plan introspection for a vector query (see
    /// [`Response::ExplainVector`]): access path, candidate count, top-k.
    ExplainVector {
        /// The vector query to explain.
        query: VectorQuery,
    },
    /// List declared (non-vector) indexes with their state.
    Indexes,
    /// List vector indexes.
    VectorIndexes,
    /// Describe a single vector index by name.
    VectorIndex {
        /// The vector index name.
        name: String,
    },
    /// Labels and properties whose names are spelled like `text`: the
    /// answer to "no such table/column". Ignores case, separators
    /// and plural endings, then ranks by spelling similarity.
    Names {
        /// The name to resolve, e.g. `patients` or `Patient.birth_date`.
        text: String,
        /// Most suggestions (default 5).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<u64>,
    },
    /// Every label with its properties in one compact read: types,
    /// exact support and distinct counts, numeric/date ranges, the values of
    /// properties holding few, and relationship shapes.
    SchemaSummary {
        /// Labels to summarize (all when empty).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        labels: Vec<String>,
        /// List a property's values when it holds at most this many
        /// distinct ones (default 10, at most 100; 0 lists none).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        few_values: Option<u64>,
    },
    /// Value overlap between property pairs: for each pair, how
    /// many of the source's distinct values the target also holds, over the
    /// exact value dictionaries. Evidence only: deciding which pairs are
    /// references is the caller's (the Core platform's suggestions).
    ValueOverlap {
        /// The pairs to compare (at most 10,000).
        pairs: Vec<JoinPair>,
    },
}

/// One side of a join: a label and one of its properties.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinEnd {
    /// The label.
    pub label: String,
    /// The property.
    pub property: String,
}

/// A pair of properties to compare: does the source's values name records
/// of the target by the target property?
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinPair {
    /// The referencing side.
    pub source: JoinEnd,
    /// The referenced side (usually a key).
    pub target: JoinEnd,
}

/// Property-entry detail level for `Describe::Properties`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PropertyDetail {
    /// Full statistics: samples, min/max, and array shape. Unfiltered reads
    /// sample up to 10k records and combine that sample with exact catalog
    /// counts; filtered reads scan the complete match set within the query
    /// resource limit so support counts are never silently truncated.
    #[default]
    Full,
    /// Catalog facts only (names, types, exact support counts, array shape,
    /// sibling flags) with no record scan; rejects a non-empty `where`.
    Catalog,
}

/// A document ingest request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Ingest {
    /// Ingest JSON documents.
    Documents {
        /// Explicit label for a top-level record or array. May be omitted for
        /// a non-empty container object whose values are all nested objects
        /// or arrays of nested objects; those top-level keys become labels.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// The document or documents to ingest.
        documents: serde_json::Value,
        /// Ingest options document.
        #[serde(default = "empty_object")]
        options: serde_json::Value,
    },
    /// Ingest CSV text: every data row becomes one record of `label`. Rows
    /// are decoded as Papa Parse 5 decodes them, header names are made into
    /// property names, then the rows are ingested as documents.
    Csv {
        /// The label of every imported record.
        label: String,
        /// The CSV text.
        data: String,
        /// How the text is decoded.
        #[serde(default)]
        parse_config: CsvParseConfig,
        /// Ingest options document, as for documents.
        #[serde(default = "empty_object")]
        options: serde_json::Value,
        /// Per-row inline vectors: entry `i` becomes data row `i`'s `$vectors`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        vectors: Vec<serde_json::Value>,
    },
}

/// How CSV text is decoded (`parseConfig` in the SDK/API surface). Every
/// field is optional; the defaults read a header row, split on `,` and skip
/// empty lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CsvParseConfig {
    /// Field separator; any non-empty string.
    #[serde(default = "default_csv_delimiter")]
    pub delimiter: String,
    /// The first row names the fields. Without it, fields are named
    /// `column_1`, `column_2`, ...
    #[serde(default = "default_true")]
    pub header: bool,
    /// Which lines are skipped as empty.
    #[serde(default)]
    pub skip_empty_lines: CsvSkipEmptyLines,
    /// Turn numbers, `true`/`false` and ISO datetimes into typed values.
    /// Defaults to the `suggestTypes` import option.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_typing: Option<bool>,
    /// The quote character.
    #[serde(default = "default_csv_quote")]
    pub quote_char: String,
    /// The character that escapes a quote inside a quoted field; defaults to
    /// the quote character (a doubled quote).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escape_char: Option<String>,
    /// Line ending: `\n`, `\r` or `\r\n`. Any other value, or none, is
    /// detected from the text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newline: Option<String>,
}

impl Default for CsvParseConfig {
    fn default() -> Self {
        CsvParseConfig {
            delimiter: default_csv_delimiter(),
            header: true,
            skip_empty_lines: CsvSkipEmptyLines::default(),
            dynamic_typing: None,
            quote_char: default_csv_quote(),
            escape_char: None,
            newline: None,
        }
    }
}

fn default_csv_delimiter() -> String {
    ",".to_string()
}

fn default_csv_quote() -> String {
    "\"".to_string()
}

fn default_true() -> bool {
    true
}

/// Which CSV lines are skipped: `true` skips lines with no characters,
/// `"greedy"` also skips lines whose fields are all whitespace, `false` keeps
/// every line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CsvSkipEmptyLines {
    /// Keep every line.
    Keep,
    /// Skip lines with no characters.
    #[default]
    Empty,
    /// Skip lines whose fields are all whitespace.
    Greedy,
}

impl Serialize for CsvSkipEmptyLines {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            CsvSkipEmptyLines::Keep => serializer.serialize_bool(false),
            CsvSkipEmptyLines::Empty => serializer.serialize_bool(true),
            CsvSkipEmptyLines::Greedy => serializer.serialize_str("greedy"),
        }
    }
}

impl<'de> Deserialize<'de> for CsvSkipEmptyLines {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match serde_json::Value::deserialize(deserializer)? {
            serde_json::Value::Bool(true) => Ok(CsvSkipEmptyLines::Empty),
            serde_json::Value::Bool(false) => Ok(CsvSkipEmptyLines::Keep),
            serde_json::Value::String(value) if value == "greedy" => Ok(CsvSkipEmptyLines::Greedy),
            _ => Err(serde::de::Error::custom(
                "skipEmptyLines must be true, false or \"greedy\"",
            )),
        }
    }
}

/// A vector index management operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum IndexOperation {
    /// Declare an ordered or text index on a label's property. It is
    /// built online; `describe.indexes` reports `building` until it is
    /// `ready`.
    Create {
        /// The index definition.
        definition: IndexDefinition,
    },
    /// Drop a declared index by name (removed online).
    Drop {
        /// The index name.
        name: String,
    },
    /// Create a vector index.
    CreateVector {
        /// The index definition.
        definition: VectorIndexDefinition,
    },
    /// Drop a vector index by name.
    DropVector {
        /// The index name.
        name: String,
    },
    /// Insert or update vectors in an index.
    UpsertVectors {
        /// The index name.
        name: String,
        /// The vectors to upsert.
        items: Vec<VectorItem>,
    },
    /// Reconcile a managed vector index's binding statuses against live data
    /// (deleted/updated sources become Retired/Stale), so a stale `ready`
    /// index reports accurate counts and can be re-embedded. For managed
    /// (occurrence-backed) indexes only; external indexes are skipped.
    RevalidateVector {
        /// The index name.
        name: String,
    },
}

/// A transaction management operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TransactionOperation {
    /// Begin a transaction.
    Begin {
        /// Optional per-transaction TTL override (milliseconds). Absent uses
        /// the server default; either value is capped by the server maximum.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ttl_ms: Option<u64>,
        /// When true, an expired transaction is COMMITTED (best effort) instead
        /// of rolled back when its TTL lapses.
        #[serde(default)]
        autocommit: bool,
    },
    /// Stage a mutation in the transaction.
    Stage {
        /// The transaction id.
        transaction_id: String,
        /// The mutation to stage.
        mutation: TransactionMutation,
    },
    /// Commit the transaction.
    Commit {
        /// The transaction id.
        transaction_id: String,
    },
    /// Roll back the transaction.
    Rollback {
        /// The transaction id.
        transaction_id: String,
    },
}

/// A mutation staged inside a transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TransactionMutation {
    /// Create a record.
    CreateRecord {
        /// The record label.
        label: String,
        /// Optional pinned record id (dashed UUID or compact hex) so other
        /// services can reference the record before commit.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Record properties.
        #[serde(default = "empty_object")]
        properties: serde_json::Value,
        /// Value typing options, as for `putRecord` (`suggestTypes`,
        /// `convertNumericValuesToNumbers`, `skipEmptyValues`, ...).
        #[serde(default = "empty_object")]
        options: serde_json::Value,
    },
    /// Update an existing record.
    UpdateRecord {
        /// Reference to the record to update.
        record: RecordReference,
        /// Properties to apply; values are typed as `putRecord` merge and
        /// replace type them.
        #[serde(default = "empty_object")]
        properties: serde_json::Value,
        /// `true` also deletes every property `properties` does not name, as
        /// `putRecord` mode `replace` does; `false` (default) merges.
        #[serde(default)]
        replace: bool,
    },
    /// Transactional counterpart to the native `retypeRecord` mutation.
    RetypeRecord {
        /// Reference to the record to retype.
        record: RecordReference,
        /// The current primary label.
        from_label: String,
        /// The new primary label.
        to_label: String,
    },
    /// Delete a record.
    DeleteRecord {
        /// Reference to the record to delete.
        record: RecordReference,
    },
    /// Create a relationship between two records.
    CreateRelationship {
        /// The relationship type.
        relation_type: String,
        /// The source endpoint.
        source: RecordReference,
        /// The target endpoint.
        target: RecordReference,
        /// Relationship properties.
        #[serde(default = "empty_object")]
        properties: serde_json::Value,
    },
    /// Stage an import (JSON documents or CSV) as plain record creates and
    /// the relationships between them. Merge imports (`mergeBy`,
    /// `mergeStrategy`) and inline vectors are not staged.
    Ingest {
        /// The import, as for the `ingest` operation.
        ingest: Box<Ingest>,
    },
    /// Read one record inside the transaction: the result merges committed
    /// state with the transaction's staged writes (read-your-writes) and the
    /// read's key footprint is recorded so `commit` detects concurrent changes.
    GetRecord {
        /// Reference to the record to read.
        record: RecordReference,
    },
    /// Stage a graph-language mutation (GQL/Cypher/Gremlin) into the
    /// transaction. The statement runs against the committed graph, its exact
    /// delta is staged, and it commits or rolls back with the transaction.
    Statement {
        /// The language the statement is written in.
        language: StatementLanguage,
        /// The statement text.
        statement: String,
        /// Optional bind parameters for the statement.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parameters: Option<serde_json::Value>,
    },
}

/// A reference to a record by its public id. The engine resolves the id
/// inside the transaction, so records staged earlier in the same transaction
/// can be referenced too.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordReference {
    /// The record id (dashed UUID or compact hex).
    pub id: String,
}

/// Definition of a declared (non-vector) index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexDefinition {
    /// Tenant-unique index name.
    pub name: String,
    /// Label whose records are indexed.
    pub label: String,
    /// Indexed property.
    pub property: String,
    /// Index kind: `ordered` (equality, `$in`, ranges, `startsWith`),
    /// `text` (BM25 full-text; `property` may be `*` for every string
    /// property of the record) or `structural` (one vector per record from
    /// the graph around it, searched with `query.vector`; `property` is `*`).
    #[serde(default = "IndexDefinition::default_kind")]
    pub kind: String,
    /// Text indexes only: `eventual` (default; searches see what the applier
    /// has indexed, typically within 250 ms) or `immediate` (read-your-writes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consistency: Option<String>,
    /// Structural indexes only: vector dimensions (8..=4096).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
}

impl IndexDefinition {
    fn default_kind() -> String {
        "ordered".into()
    }
}

/// Definition of a vector index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorIndexDefinition {
    /// Label whose records are indexed.
    pub label: String,
    /// Property holding the vector.
    pub property: String,
    /// Vector dimensionality.
    pub dimensions: u32,
    /// Similarity metric name.
    #[serde(default = "default_similarity")]
    pub similarity: String,
    /// Stable opaque index name (the source of truth is the metadata record:
    /// label/property/similarity/dimensions). When absent the engine derives a
    /// deterministic name from the definition for legacy compatibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Optional caller-declared embedding provenance (model id/revision). The
    /// engine does not interpret or verify it, but rejects an accidental
    /// change under an existing name so incomparable vectors cannot be mixed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Owner kind the index covers: `"record"` (default) or `"relationship"`.
    /// For a relationship index, `label` is the relationship type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

/// A vector to associate with a record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorItem {
    /// Record the vector belongs to.
    pub record_id: String,
    /// The vector components.
    pub vector: Vec<f32>,
    /// Hash of the source property captured before an external embedding
    /// provider ran. When present, the engine conditionally attaches the
    /// vector and rejects stale completions after a source update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_hash: Option<u64>,
    /// Opaque index-generation token captured when the embedding job started.
    /// A completion from a dropped/recreated index is rejected when the token
    /// no longer matches the durable index metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_generation: Option<String>,
}

/// A vector similarity query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorQuery {
    /// Labels the candidate records must carry.
    pub labels: Vec<String>,
    /// Property holding the indexed vector.
    pub property: String,
    /// The query vector.
    pub vector: Vec<f32>,
    /// Similarity metric name.
    #[serde(default = "default_similarity")]
    pub similarity: String,
    /// Index/space name. Required while `resources.require-vector-index-name`
    /// is on (the default): two models of the same shape are only told apart
    /// by name. With the gate off, an absent name falls back to the alias
    /// derived from `labels`/`property`/`similarity`/vector length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,
    /// Predicate document filtering candidates before the top-k search.
    #[serde(default = "empty_object")]
    pub predicate: serde_json::Value,
    /// Number of top results to skip.
    #[serde(default)]
    pub offset: u64,
    /// Number of top results to return.
    pub limit: u64,
    /// How the search ranks candidates; see [`VectorSearchMode`]. Absent
    /// means `ann` when `ef` is positive, `auto` otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<VectorSearchMode>,
    /// ANN gather breadth whenever the search runs on the ANN graph (at
    /// least `offset + limit` is gathered). Absent: the server's
    /// `resources.vector-default-ef`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ef: Option<usize>,
    /// Keep only hits scoring at least this much. Every metric scores
    /// "higher is more similar", so one threshold fits all; a page may hold
    /// fewer than `limit` records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_threshold: Option<f32>,
}

/// How a vector search ranks its candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VectorSearchMode {
    /// The server chooses: exact search while the index holds at most
    /// `resources.vector-exact-max-pool` vectors (or its metric has no ANN
    /// graph), ANN above. A search never waits for the ANN graph to be
    /// built: until it is resident, `auto` answers exactly and builds it in
    /// the background.
    Auto,
    /// Score every candidate.
    Exact,
    /// Gather from the ANN graph and rerank exactly. Only cosine indexes
    /// have one; a budget or confidence shortfall is still answered exactly
    /// and reported.
    Ann,
}

/// The path that answered a vector search, reported with its results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorSearchReport {
    /// `exact` or `ann`: the path that produced the results.
    pub mode: String,
    /// The gather breadth, when the search ran on the ANN graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ef: Option<usize>,
    /// Why a search that would use the ANN graph was answered exactly:
    /// `metric` (no ANN graph for this metric), `budget` (the graph does not
    /// fit the vector cache), `lowConfidence` (too few filtered candidates
    /// gathered), or `annWarming` (`auto` only: the graph is being built in
    /// the background; later searches use it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
    /// Vectors in the index (its ready bindings), which `auto` decides on.
    pub pool: u64,
}

/// The vector leg of a [`HybridQuery`]. Same shape as the similarity-search
/// fields of [`VectorQuery`], minus the fields [`HybridQuery`] already
/// carries once for both legs (`labels`, `predicate`, pagination).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HybridVectorLeg {
    /// Property holding the indexed vector.
    pub property: String,
    /// The query vector.
    pub vector: Vec<f32>,
    /// Similarity metric name.
    #[serde(default = "default_similarity")]
    pub similarity: String,
    /// Explicit index/space name; see [`VectorQuery::index`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,
    /// See [`VectorQuery::mode`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<VectorSearchMode>,
    /// See [`VectorQuery::ef`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ef: Option<usize>,
    /// Keep only vector hits scoring at least this much, before fusion; see
    /// [`VectorQuery::score_threshold`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_threshold: Option<f32>,
    /// This leg's share of a fused score (both legs set); default 1.
    #[serde(default = "default_leg_weight")]
    pub weight: f64,
    /// Candidates this leg ranks before fusion (1..=10,000); default
    /// `4 × (offset + limit)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefetch_limit: Option<u64>,
}

/// The lexical leg of a [`HybridQuery`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HybridTextLeg {
    /// The text index name.
    pub index: String,
    /// The query string.
    pub query: String,
    /// See [`Query::Text`]'s `all_terms`.
    #[serde(default)]
    pub all_terms: bool,
    /// See [`Query::Text`]'s `phrase`.
    #[serde(default)]
    pub phrase: bool,
    /// This leg's share of a fused score (both legs set); default 1.
    #[serde(default = "default_leg_weight")]
    pub weight: f64,
    /// Candidates this leg ranks before fusion (1..=10,000); default
    /// `4 × (offset + limit)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefetch_limit: Option<u64>,
}

/// How the lexical leg of a hybrid search ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSearchReport {
    /// Records matching the query among the filtered candidates.
    pub matches: u64,
    /// Changes the index had not applied when the search ran.
    pub pending: u64,
}

/// A combined graph + vector + lexical query.
///
/// `labels`/`predicate` are resolved exactly once, through the same
/// cardinality-based access-path planner ordinary graph queries use
/// (`matching_ordinals`), into one candidate owner-ordinal set. Both the
/// vector leg and the lexical leg search only inside that set — ranking
/// happens after filtering, never before, so a predicate cannot be
/// circumvented by either retrieval channel. The two independently correct
/// rankings are then combined by [`Fusion`]. At least one of `vector`/`text`
/// must be set; both may be set for a true hybrid query.
///
/// Record-scoped only for v1: relationship vectors remain reachable through
/// the plain [`VectorQuery`], and text indexes cover records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HybridQuery {
    /// Labels a candidate record must carry; empty matches every label.
    #[serde(default)]
    pub labels: Vec<String>,
    /// Predicate document filtering candidates before either leg ranks them.
    #[serde(default = "empty_object")]
    pub predicate: serde_json::Value,
    /// The vector leg, when ranking by similarity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector: Option<HybridVectorLeg>,
    /// The lexical leg, when ranking by BM25.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<HybridTextLeg>,
    /// Fused results to skip before `limit` takes over; 0 = first page.
    /// Each leg's own over-fetch is sized from `offset + limit`, not
    /// `limit` alone, so a deep page is not silently under-filled by a
    /// gather that only ever considered the first page's candidates.
    #[serde(default)]
    pub offset: u64,
    /// Number of fused results to return.
    pub limit: u64,
    /// How the two legs' rankings combine.
    #[serde(default)]
    pub fusion: Fusion,
    /// Reciprocal-rank-fusion smoothing constant ([`Fusion::Rrf`] only).
    #[serde(default = "default_rrf_k")]
    pub rrf_k: f64,
}

/// How a hybrid query combines its legs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Fusion {
    /// Reciprocal rank fusion: `Σ weight / (rrfK + rank + 1)`; only ranks
    /// cross legs.
    #[default]
    Rrf,
    /// Distribution-based score fusion: each leg's scores normalized by its
    /// own mean ± 3 standard deviations to `[0, 1]`, then `Σ weight × score`.
    Dbsf,
}

fn default_rrf_k() -> f64 {
    60.0
}

fn default_leg_weight() -> f64 {
    1.0
}

/// A pairwise join of two record sets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinPairsQuery {
    /// Left-hand record set.
    pub left: RecordSet,
    /// Right-hand record set.
    pub right: RecordSet,
    /// How the two sets are joined.
    pub predicate: JoinPredicate,
    /// Exclude pairs whose endpoints are the same record.
    #[serde(default)]
    pub exclude_same_record: bool,
    /// Output projection.
    pub projection: JoinProjection,
    /// Maximum number of pairs to return.
    pub limit: u32,
}

/// One side of a pairwise join.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordSet {
    /// Label the records must carry.
    pub label: String,
    /// Native predicate document for this record set.
    #[serde(default = "empty_object")]
    pub filter: serde_json::Value,
}

/// Join condition between two record sets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum JoinPredicate {
    /// Join records whose given properties share values.
    PropertyOverlap {
        /// Property on the left record.
        left_property: String,
        /// Property on the right record.
        right_property: String,
    },
}

/// Output projection for a pairwise join.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JoinProjection {
    /// Return only the number of matching pairs.
    Count,
}

/// Versioned response envelope returned for a [`RequestEnvelope`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseEnvelope {
    /// Protocol version the server speaks.
    pub protocol_version: u16,
    /// Correlation id echoed from the request, when supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Server-side execution time in milliseconds, echoed by the executor on
    /// success (E4 stall observability). Absent on envelopes built by older
    /// servers; clients must treat it as informational, not a deadline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_timing_ms: Option<u64>,
    /// The operation result.
    pub response: Response,
}

/// The result of an [`Operation`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Response {
    /// Schema description.
    Schema {
        /// The schema description.
        schema: SchemaDescription,
    },
    /// A count result.
    Count {
        /// The count.
        value: u64,
    },
    /// A single record.
    Record {
        /// The record as a JSON object.
        record: serde_json::Value,
    },
    /// A page of records.
    Records {
        /// The records as JSON objects.
        records: Vec<serde_json::Value>,
        /// Total number of matching records.
        total: u64,
        /// Present while a cursor has more records. Send it in
        /// `query.cursor` with a new bounded page size.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cursor_id: Option<String>,
        /// For a vector or hybrid query: the path that answered it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        vector_search: Option<VectorSearchReport>,
        /// For a hybrid query with a text leg: how that leg ran.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text_search: Option<TextSearchReport>,
        /// For a hybrid query with both legs: how they were combined.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fusion: Option<Fusion>,
    },
    /// Ranked hits of a text search.
    TextHits {
        /// One `{id, score}` object per hit, best first.
        rows: Vec<serde_json::Value>,
        /// Number of hits returned.
        total: u64,
        /// Changes to the index not yet applied when the search ran: an
        /// `eventual` index may miss up to this many recent writes.
        pending: u64,
    },
    /// Rows returned by a statement query.
    Rows {
        /// The rows as JSON objects.
        rows: Vec<serde_json::Value>,
        /// Total number of matching rows.
        total: u64,
    },
    /// Label names.
    Labels {
        /// The labels.
        labels: Vec<String>,
    },
    /// Property descriptions.
    Properties {
        /// The property descriptions.
        properties: Vec<PropertyDescription>,
    },
    /// Statement-language capability profiles.
    QueryLanguages {
        /// The capabilities.
        languages: Vec<QueryLanguageCapability>,
    },
    /// Feature-capability report for clients that must not infer support from a
    /// version string (Core's Rust-mode negotiation). Flags describe protocol
    /// features a client may rely on; an absent flag means unsupported.
    Capabilities {
        /// The capability flags.
        capabilities: Vec<String>,
    },
    /// Build identity, transport bindings, and configured limits. Clients use
    /// this with [`Response::QueryLanguages`] to negotiate a pinned contract
    /// instead of guessing from a server version string.
    BuildInfo {
        /// The build info.
        build: BuildInfo,
    },
    /// Query plan introspection (see [`Describe::Explain`]).
    Explain {
        /// The query plan.
        plan: ExplainPlan,
    },
    /// Vector query plan introspection (see [`Describe::ExplainVector`]).
    ExplainVector {
        /// The vector query plan.
        plan: ExplainVectorPlan,
    },
    /// Relationships as JSON objects.
    Relationships {
        /// The relationships.
        relationships: Vec<serde_json::Value>,
        /// Total number of matching relationships.
        total: u64,
    },
    /// Distinct property values for a property.
    PropertyValues {
        /// The property the values belong to.
        property: String,
        /// The returned distinct values.
        values: Vec<serde_json::Value>,
        /// Exact distinct total, always reported. Equals `values.len()`
        /// when untruncated; the untruncated total when `truncated`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        distinct_count: Option<u64>,
        /// Minimum distinct value (`Range` mode; numeric order for numbers,
        /// lexical order otherwise). Absent when no comparable scalars.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<serde_json::Value>,
        /// Maximum distinct value (`Range` mode).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<serde_json::Value>,
        /// True when `limit` truncated the shipped `values`.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        truncated: bool,
        /// Owner counts aligned with `values` (`TopK` mode); empty otherwise.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        counts: Vec<u64>,
        /// True when a value search matched nothing and `values`/`counts`
        /// hold the property's whole domain instead (`domainOnMiss`).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        search_missed: bool,
    },
    /// Number of records affected by a mutation.
    Mutation {
        /// Number of records affected.
        affected: u64,
    },
    /// Result of a link operation: relationships created, and requested
    /// pairs already linked by the same type (links are idempotent per pair).
    Linked {
        /// Relationships created.
        created: u64,
        /// Requested pairs that were already linked.
        skipped_existing: u64,
    },
    /// Result of a graph-language mutation.
    StatementMutation {
        /// Rows returned by the statement.
        rows: Vec<serde_json::Value>,
        /// Total number of rows returned.
        total: u64,
        /// Number of records affected.
        affected: u64,
        /// Commit index the mutation landed at.
        commit_index: u64,
        /// Optional statement status string.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    /// Result of ingesting documents.
    Ingested {
        /// Number of records ingested.
        count: u64,
        /// Number of relationships ingested.
        #[serde(default)]
        relationships: u64,
        /// The ingested records as JSON objects.
        records: Vec<serde_json::Value>,
    },
    /// Records rendered as CSV text.
    Csv {
        /// The CSV text: a header row, then one row per record, joined by
        /// `\r\n`. Empty when no record matched.
        content: String,
        /// Number of records rendered.
        count: u64,
    },
    /// A vector index as a JSON object.
    VectorIndex {
        /// The index.
        index: serde_json::Value,
    },
    /// A declared index as a JSON object.
    Index {
        /// The index.
        index: serde_json::Value,
    },
    /// Declared indexes as JSON objects.
    Indexes {
        /// The indexes.
        indexes: Vec<serde_json::Value>,
    },
    /// Vector indexes as JSON objects.
    VectorIndexes {
        /// The indexes.
        indexes: Vec<serde_json::Value>,
    },
    /// Statistics for a vector index.
    VectorIndexStats {
        /// The statistics.
        stats: serde_json::Value,
    },
    /// Result of upserting vectors.
    VectorsUpserted {
        /// Number of vectors updated.
        updated: u64,
        /// Number of vectors requested.
        requested: u64,
    },
    /// Records not yet bound to a vector index.
    VectorIndexCandidates {
        /// The candidate records.
        records: Vec<serde_json::Value>,
        /// Total number of matching candidates.
        total: u64,
    },
    /// A transaction was begun.
    TransactionBegun {
        /// The transaction id.
        transaction_id: String,
    },
    /// A mutation was staged in a transaction.
    TransactionStaged {
        /// The transaction id.
        transaction_id: String,
    },
    /// An import was staged in a transaction.
    TransactionIngested {
        /// The transaction id.
        transaction_id: String,
        /// Records staged.
        count: u64,
        /// Relationships staged.
        relationships: u64,
    },
    /// A record read inside a transaction.
    TransactionGet {
        /// The transaction id.
        transaction_id: String,
        /// The record as a JSON object.
        record: serde_json::Value,
    },
    /// The outcome of committing or rolling back a transaction.
    TransactionOutcome {
        /// The transaction id.
        transaction_id: String,
        /// The final status.
        status: TransactionStatus,
        /// Commit index when committed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        commit_index: Option<u64>,
        /// Human-readable reason for a non-committed outcome.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        /// Whether the client may retry.
        retryable: bool,
    },
    /// A tenant's keys were purged.
    PurgedTenant {
        /// Commit index the purge landed at.
        commit_index: u64,
    },
    /// A tenant's data-access policy.
    Policy {
        /// The policy, or `None` when the tenant has none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        policy: Option<serde_json::Value>,
        /// Whether the policy is enforced.
        enforced: bool,
        /// The policy generation (the commit index that installed it).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        generation: Option<u64>,
    },
    /// A logical backup.
    Backup {
        /// Commit index the backup was pinned at.
        commit_index: u64,
        /// One backup per scope.
        tenants: Vec<TenantBackup>,
    },
    /// Matches of a value search across properties (`query.searchValues`).
    ValueMatches {
        /// Matches by descending owner count (ties by label, property, value).
        matches: Vec<ValueMatchRow>,
        /// Number of matches before `limit`.
        total: u64,
        /// Properties that could not be searched (e.g. over a resource limit),
        /// as `label.property`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        skipped: Vec<String>,
    },
    /// Name suggestions (`describe.names`), best first.
    Names {
        /// The suggestions.
        names: Vec<NameSuggestion>,
    },
    /// A compact schema (`describe.schemaSummary`).
    SchemaSummary {
        /// One entry per label, by name.
        labels: Vec<LabelSummary>,
    },
    /// Value overlap per requested pair (`describe.valueOverlap`), in
    /// request order.
    ValueOverlap {
        /// One entry per pair.
        pairs: Vec<ValueOverlap>,
    },
}

/// One stored value matching a cross-property value search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueMatchRow {
    /// The label holding the value.
    pub label: String,
    /// The property holding the value.
    pub property: String,
    /// The value, as the records response renders it.
    pub value: serde_json::Value,
    /// Records of `label` holding it.
    pub count: u64,
}

/// A label or property spelled like a requested name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NameSuggestion {
    /// The label (the suggestion itself, or the property's label).
    pub label: String,
    /// The property, when the suggestion is a property.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub property: Option<String>,
    /// Spelling similarity, from 0 to 1 (1: equal ignoring case, separators
    /// and plural endings).
    pub score: f64,
}

/// One label of a compact schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelSummary {
    /// The label.
    pub name: String,
    /// Records with the label.
    pub records_count: u64,
    /// Its properties, by name.
    pub properties: Vec<PropertySummary>,
    /// Relationship shapes touching the label.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationships: Vec<RelationshipSummary>,
}

/// One property of a [`LabelSummary`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertySummary {
    /// The property name.
    pub name: String,
    /// Display type (`string`, `number`, `boolean`, `datetime`, `null`).
    #[serde(rename = "type")]
    pub value_type: String,
    /// Records of the label carrying it.
    pub records_count: u64,
    /// True when any occurrence is an array.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_array: bool,
    /// The other stored types of a type-split property, as
    /// `describe.properties` lists them (e.g. `["float64"]` on an `int64`
    /// entry).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sibling_types: Vec<String>,
    /// Exact distinct values under the label (absent for arrays, whose
    /// values are whole arrays). Values too long for the value dictionary
    /// are counted from the occurrences.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distinct_count: Option<u64>,
    /// Smallest value (numbers and datetimes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<serde_json::Value>,
    /// Largest value (numbers and datetimes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<serde_json::Value>,
    /// Every value, by descending count, when there are at most
    /// `fewValues` of them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<serde_json::Value>,
}

/// A relationship shape of a [`LabelSummary`], seen from that label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipSummary {
    /// The relationship type.
    #[serde(rename = "type")]
    pub relationship_type: String,
    /// The label at the other end.
    pub label: String,
    /// `out` when this label is the source, `in` when it is the target.
    pub direction: String,
    /// Relationships of this shape.
    pub count: u64,
}

/// How much of a source property's values a target property holds, from
/// the exact value dictionaries (`describe.valueOverlap`). Values compare as
/// text, so a numeric id matches the same id stored as a string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueOverlap {
    /// The source property.
    pub source: JoinEnd,
    /// The target property.
    pub target: JoinEnd,
    /// Distinct source values (absent when not compared).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_distinct: Option<u64>,
    /// Distinct target values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_distinct: Option<u64>,
    /// Distinct source values the target also holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_distinct: Option<u64>,
    /// Source records whose value the target holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_records: Option<u64>,
    /// `matchedDistinct / sourceDistinct`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<f64>,
    /// Source distinct values per source record carrying the property (1:
    /// every record has its own value).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_uniqueness: Option<f64>,
    /// Target distinct values per target record carrying the property (1:
    /// a key).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_uniqueness: Option<f64>,
    /// When (nearly) all values of both are numbers: the source's value span
    /// as a share of the target's (codes 1..9 inside ids 1..11,060 score
    /// about 0.001).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numeric_span: Option<f64>,
    /// Why the values were not compared (no such property, arrays, long
    /// values, too many values, or withheld under a policy's `minCount`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_compared: Option<String>,
}

/// Query plan introspection for a `QueryDocument`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplainPlan {
    /// Access path selected by the planner: `index:<name>` for a declared
    /// ordered index, `property_walk:<property>` when an unindexed predicate
    /// leaf narrows the candidates (see `unindexedWalk`), else `label_scan`
    /// or `full_scan`.
    pub access_path: String,
    /// The unindexed walk the access path starts with: the property, the
    /// owners carrying it that are visited, and the ordered indexes that
    /// would answer the leaf instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unindexed_walk: Option<UnindexedWalk>,
    /// Labels the scan is constrained to (empty = all labels).
    pub labels: Vec<String>,
    /// Residual filter applied after the access path (the predicate).
    pub predicate: serde_json::Value,
    /// Bounded estimate of rows the scan would visit (sum of label members).
    pub scan_estimate: u64,
    /// Exact cardinality of the most selective index-eligible predicate leaf
    ///, or `None` when the predicate is not index-eligible. Derived
    /// from the maintained posting cardinality; never an approximation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_cardinality: Option<u64>,
    /// Selectivity fraction (`0.0`-`1.0`) for the first range leaf
    /// (`$gt`/`$gte`/`$lt`/`$lte`) in the predicate, from the maintained
    /// value-distribution histogram, or `None` when the predicate has no
    /// range leaf or no usable histogram exists yet. A genuinely separate
    /// field from `estimated_cardinality`: this one is always approximate,
    /// by design, and must never be read as if it were exact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_selectivity_estimate: Option<f64>,
    /// Page bound applied by the query.
    pub limit: Option<u64>,
    /// Offset applied by the query.
    pub offset: u64,
}

/// An unindexed predicate walk reported by `EXPLAIN`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnindexedWalk {
    /// The (normalized) property the walk reads.
    pub property: String,
    /// Owners carrying the property that the walk visits, one point read
    /// each.
    pub owners: u64,
    /// One ordered index per walked label that would answer the leaf.
    pub suggested_indexes: Vec<IndexSuggestion>,
}

/// An ordered index `EXPLAIN` suggests declaring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexSuggestion {
    /// The label the index belongs to.
    pub label: String,
    /// The indexed property.
    pub property: String,
}

/// Vector query plan introspection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplainVectorPlan {
    /// Access path (`vector_index`).
    pub access_path: String,
    /// The canonical index alias the query resolves to.
    pub alias: String,
    /// Label the query is scoped to.
    pub label: String,
    /// Number of records matching the predicate (the candidate set the vector
    /// search is filtered to — filter-before-top-k).
    pub candidate_count: u64,
    /// Top-k requested (offset + limit).
    pub k: usize,
    /// Residual predicate applied as the candidate filter.
    pub predicate: serde_json::Value,
}

/// A bound transport listener on the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportBinding {
    /// Transport name: `http`, `pgwire`, `grpc`, or `bolt`.
    pub transport: String,
    /// Bound port (0 when the transport is disabled).
    pub port: u16,
    /// Whether the listener serves TLS.
    pub tls: bool,
}

/// Version/build identity, transport bindings, and configured limits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    /// Engine identifier.
    pub engine: String,
    /// Engine semantic version (`CARGO_PKG_VERSION`).
    pub version: String,
    /// Optional build revision (git commit) embedded at compile time.
    pub build_revision: Option<String>,
    /// The wire protocol version this build speaks.
    pub protocol_version: u16,
    /// Listeners bound at startup.
    pub transports: Vec<TransportBinding>,
    /// Configured limits as a flat map of numeric values
    /// (`maxRequestBytes`, `storageBudgetBytes`, `transactionBudgetBytes`,
    /// `transactionTimeoutMs`).
    pub limits: std::collections::BTreeMap<String, u64>,
    /// Address clients should use to reach this node, when it differs from the
    /// bind address (multi-host / NAT deployments).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advertised_address: Option<String>,
}

/// Honest delivery status for a statement-language profile. The engine
/// advertises what it already does (`Stable`) and labels the rest explicitly
/// (`ReleaseCandidate` / `ComingSoon`) instead of implying full support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageStatus {
    /// The documented profile is delivered and exercised by the test suite.
    Stable,
    /// Read+write implemented via the community planner, but the standard's
    /// conformance gate (TCK / public corpus) is not yet green.
    ReleaseCandidate,
    /// Not implemented; clients can plan against the advertised shape.
    ComingSoon,
}

/// Capability advertisement for a single statement language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryLanguageCapability {
    /// The language this profile describes.
    pub language: StatementLanguage,
    /// Honest delivery status: Stable (already done for the documented
    /// profile), `ReleaseCandidate` (implemented but conformance gates not yet
    /// green), or `ComingSoon` (advertised but not implemented).
    pub status: LanguageStatus,
    /// Versioned standards/dialect profile implemented by the frontend.
    pub profile: String,
    /// Community planner/executor that owns parsing and semantics.
    pub implementation: String,
    /// Read/write access supported by the profile.
    pub access: StatementAccess,
    /// Parameter styles the profile accepts.
    #[serde(default)]
    pub parameter_styles: Vec<ParameterStyle>,
    /// Transports exposing the read-authorized statement operation.
    #[serde(default)]
    pub transports: Vec<StatementTransport>,
    /// Transports that expose the write-authorized statement operation. This
    /// is empty for read-only profiles and can be narrower than `transports`.
    #[serde(default)]
    pub mutation_transports: Vec<StatementTransport>,
    /// Stable feature identifiers implemented by this profile.
    #[serde(default)]
    pub features: Vec<String>,
    /// Explicit boundaries clients must not mistake for full-standard support.
    #[serde(default)]
    pub limitations: Vec<String>,
}

/// Read/write access supported by a statement-language profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatementAccess {
    /// Only read statements are permitted.
    ReadOnly,
    /// Read and write statements are permitted.
    ReadWrite,
}

/// Parameter binding style accepted by a statement-language profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParameterStyle {
    /// Positional placeholders.
    Positional,
    /// Named placeholders.
    Named,
}

/// Transport exposing a statement-language profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatementTransport {
    /// The JSON protocol itself.
    ProtocolJson,
    /// The PostgreSQL wire protocol.
    Pgwire,
}

/// A single tenant's portion of a logical backup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TenantBackup {
    /// Scope value, or `None` in single-tenant mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// JSONL payload: one logical line per record/relationship as of the
    /// pinned commit index. Restorable via `rushdb-backup::restore_backup`.
    pub jsonl: String,
    /// Number of records in the payload.
    pub records: u64,
    /// Number of relationships in the payload.
    pub relationships: u64,
}

/// Schema summary for a scoped graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaDescription {
    /// Label summaries.
    pub labels: Vec<LabelDescription>,
    /// Property summaries.
    pub properties: Vec<PropertyDescription>,
    /// Relationship summaries.
    pub relationships: Vec<RelationshipDescription>,
}

/// Summary of a label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelDescription {
    /// Label name.
    pub name: String,
    /// Number of records carrying the label.
    pub records_count: u64,
}

/// Summary of a property.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyDescription {
    /// Stable property id.
    pub id: String,
    /// Normalized property name.
    pub name: String,
    /// Display type name.
    #[serde(rename = "type")]
    pub value_type: String,
    /// Number of records carrying the property.
    pub records_count: u64,
    /// The label this property occurrence was observed on (present when the
    /// describe scan attributed occurrences per label; the Core schema cache
    /// groups properties by label without per-label record scans).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// True when any occurrence of this property is stored as an array
    /// (`OrderedMany` cardinality); schema consumers render it as e.g. `string[]`.
    #[serde(default)]
    pub is_array: bool,
    /// Other canonical types sharing this normalized property name (E8
    /// type-split siblings, e.g. `["float64"]` on the `(X,int64)` entry when
    /// X also exists as float with partitioned support). Empty when the name
    /// has a single type; a non-empty marker tells clients not to trust a
    /// cached single-type definition. Logical names (`int64`/`float64`) are
    /// used deliberately: the display `type` collapses both to `number`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sibling_types: Vec<String>,
    /// Bounded distinct scalar values observed during the describe scan
    /// (cap `SCHEMA_SAMPLE_CAP`), so schema consumers do not need to sample
    /// records themselves.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sample_values: Vec<serde_json::Value>,
    /// Minimum scalar value observed (numeric order for numbers, lexical
    /// order for strings/ISO datetimes). `None` when the scan saw no
    /// comparable scalars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<serde_json::Value>,
    /// Maximum scalar value observed (numeric order for numbers, lexical
    /// order for strings/ISO datetimes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<serde_json::Value>,
}

/// One directed relationship shape present in the scoped graph.
///
/// Schema descriptions aggregate concrete edges by `(source label, type,
/// target label)`. Tenant scope is enforced before aggregation and its
/// differentiator property is never exposed as relationship metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipDescription {
    /// Label of the source endpoint.
    pub source_label: String,
    /// Label of the target endpoint.
    pub target_label: String,
    /// Relationship type.
    #[serde(rename = "type")]
    pub relationship_type: String,
    /// Number of edges with this shape.
    pub count: u64,
    /// Properties observed on these edges.
    pub properties: Vec<RelationshipPropertyDescription>,
}

/// Summary of a property on a relationship shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipPropertyDescription {
    /// Normalized property name.
    pub name: String,
    /// Display type name.
    #[serde(rename = "type")]
    pub value_type: String,
    /// Number of relationships carrying the property.
    pub relationships_count: u64,
}

/// Final status of a transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    /// Committed successfully.
    Committed,
    /// Rolled back explicitly by the client.
    RolledBack,
    /// Aborted due to a conflict or error.
    Aborted,
    /// Expired before completion.
    Expired,
    /// Outcome could not be determined.
    Unknown,
}

/// Machine-readable error taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The request's protocol version is not supported.
    UnsupportedVersion,
    /// The request violated the protocol contract.
    ProtocolViolation,
    /// The request was malformed.
    InvalidRequest,
    /// The caller is not authorized.
    Unauthorized,
    /// A required scope was missing.
    MissingScope,
    /// The request scope did not match the authorized scope.
    ScopeMismatch,
    /// The target does not exist.
    NotFound,
    /// The request conflicts with current state.
    Conflict,
    /// The operation was cancelled.
    Cancelled,
    /// A server resource budget was exhausted.
    ResourceExhausted,
    /// The service is temporarily unavailable.
    Unavailable,
    /// Execution failed.
    ExecutionFailed,
    /// A query language is not enabled at startup (config.yml / --languages).
    LanguageDisabled,
    /// The requested form is recognized but not implemented by this engine
    /// (e.g. Mongo `$lookup`/`$text`/`$geoNear`/full `$unwind`). Append-only.
    Unsupported,
    /// A write conflict under snapshot isolation (first-committer-wins). The
    /// transaction was aborted at commit and may be retried. Distinct from
    /// [`ErrorCode::Conflict`], which is a non-retryable constraint/state
    /// conflict. Append-only.
    WriteConflict,
}

/// A structured protocol error with a stable [`ErrorCode`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
#[serde(rename_all = "camelCase")]
pub struct ProtocolError {
    /// Machine-readable error code.
    pub code: ErrorCode,
    /// Human-readable error message.
    pub message: String,
    /// Whether the caller may safely retry.
    pub retryable: bool,
}

/// Executes a canonical envelope under an optional tenant scope. Implemented
/// by the engine side (the server crate); gRPC and Bolt transports consume it
/// so neither depends on the server crate (no dependency cycle).
pub trait EnvelopeHandler: Send + Sync + 'static {
    /// Execute one request envelope under an optional tenant scope.
    ///
    /// # Errors
    ///
    /// Returns a [`ProtocolError`] when the envelope is rejected or the
    /// operation fails.
    fn execute(
        &self,
        envelope: RequestEnvelope,
        scope: Option<String>,
    ) -> Result<ResponseEnvelope, ProtocolError>;

    /// Whether the owning service has begun shutdown and new transport work
    /// should stop. Transports use this optional lifecycle hook to drain
    /// active requests without coupling the protocol crate to storage.
    fn shutdown_requested(&self) -> bool {
        false
    }
}

impl ProtocolError {
    /// Create an error, deriving `retryable` from the error code.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        let retryable = matches!(
            code,
            ErrorCode::ResourceExhausted | ErrorCode::Unavailable | ErrorCode::WriteConflict
        );
        Self {
            code,
            message: message.into(),
            retryable,
        }
    }

    /// Override the derived `retryable` flag.
    #[must_use]
    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }
}

fn empty_object() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

fn default_similarity() -> String {
    "cosine".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_matches_only_equal_secrets() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secrez"));
        assert!(!constant_time_eq(b"secret", b"secret-longer"));
        assert!(!constant_time_eq(b"", b"x"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn join_query_roundtrips_with_stable_camel_case_shape() {
        let envelope = RequestEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: Some("request-1".into()),
            context: RequestContext {
                scope: Some("scope-a".into()),
                deadline_ms: Some(250),
                ..Default::default()
            },
            operation: Operation::Query {
                query: Query::JoinPairs(JoinPairsQuery {
                    left: RecordSet {
                        label: "CHARACTER".into(),
                        filter: serde_json::json!({ "active": true }),
                    },
                    right: RecordSet {
                        label: "PLANET".into(),
                        filter: serde_json::json!({}),
                    },
                    predicate: JoinPredicate::PropertyOverlap {
                        left_property: "homePlanetId".into(),
                        right_property: "id".into(),
                    },
                    exclude_same_record: true,
                    projection: JoinProjection::Count,
                    limit: 100,
                }),
            },
        };
        let json = serde_json::to_value(&envelope).unwrap();
        assert_eq!(json["protocolVersion"], 1);
        assert_eq!(json["operation"]["kind"], "query");
        assert_eq!(json["operation"]["query"]["kind"], "joinPairs");
        assert_eq!(json["operation"]["query"]["excludeSameRecord"], true);
        assert_eq!(json["context"]["deadlineMs"], 250);
        assert_eq!(
            json["operation"]["query"]["predicate"]["leftProperty"],
            "homePlanetId"
        );
        assert_eq!(
            serde_json::from_value::<RequestEnvelope>(json).unwrap(),
            envelope
        );
    }

    #[test]
    fn statement_languages_share_one_versioned_wire_shape() {
        for language in [
            StatementLanguage::Sql,
            StatementLanguage::Gql,
            StatementLanguage::Cypher,
            StatementLanguage::Gremlin,
            StatementLanguage::SqlPgq,
        ] {
            let envelope = RequestEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: Some(format!("language-{language:?}")),
                context: RequestContext::default(),
                operation: Operation::Query {
                    query: Query::Statement {
                        language,
                        statement: "MATCH (n) RETURN n".into(),
                        parameters: Some(serde_json::json!({"limit": 10})),
                    },
                },
            };
            let encoded = serde_json::to_value(&envelope).unwrap();
            assert_eq!(encoded["operation"]["query"]["kind"], "statement");
            assert!(encoded["operation"]["query"]["language"].is_string());
            assert_eq!(
                serde_json::from_value::<RequestEnvelope>(encoded).unwrap(),
                envelope
            );
        }
    }

    #[test]
    fn query_language_capabilities_are_versioned_protocol_data() {
        let response = ResponseEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: Some("languages".into()),
            server_timing_ms: None,
            response: Response::QueryLanguages {
                languages: vec![QueryLanguageCapability {
                    language: StatementLanguage::Sql,
                    status: LanguageStatus::Stable,
                    profile: "PostgreSQL-dialect relational SQL".into(),
                    implementation: "Apache DataFusion".into(),
                    access: StatementAccess::ReadWrite,
                    parameter_styles: vec![ParameterStyle::Positional, ParameterStyle::Named],
                    transports: vec![StatementTransport::ProtocolJson, StatementTransport::Pgwire],
                    mutation_transports: vec![StatementTransport::ProtocolJson],
                    features: vec!["select".into(), "preparedStatements".into()],
                    limitations: vec!["pgwireReadOnly".into()],
                }],
            },
        };
        let encoded = serde_json::to_value(&response).unwrap();
        assert_eq!(encoded["response"]["kind"], "queryLanguages");
        assert_eq!(encoded["response"]["languages"][0]["access"], "readWrite");
        assert_eq!(
            encoded["response"]["languages"][0]["transports"][1],
            "pgwire"
        );
        assert_eq!(
            encoded["response"]["languages"][0]["mutationTransports"][0],
            "protocolJson"
        );
        assert_eq!(
            serde_json::from_value::<ResponseEnvelope>(encoded).unwrap(),
            response
        );
    }

    #[test]
    fn writable_statements_have_a_distinct_write_authorized_wire_shape() {
        let envelope = RequestEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: Some("cypher-write".into()),
            context: RequestContext::default(),
            operation: Operation::Mutate {
                mutation: Mutation::Statement {
                    language: StatementLanguage::Cypher,
                    statement: "CREATE (:Person {name: $name})".into(),
                    parameters: Some(serde_json::json!({"name": "Ada"})),
                },
            },
        };
        let encoded = serde_json::to_value(&envelope).unwrap();
        assert_eq!(encoded["operation"]["kind"], "mutate");
        assert_eq!(encoded["operation"]["mutation"]["kind"], "statement");
        assert_eq!(
            serde_json::from_value::<RequestEnvelope>(encoded).unwrap(),
            envelope
        );
    }

    #[test]
    fn typed_transaction_roundtrips_without_an_embedded_json_command_language() {
        let request = RequestEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: Some("tx-stage-1".into()),
            context: RequestContext::default(),
            operation: Operation::Transaction {
                transaction: TransactionOperation::Stage {
                    transaction_id: "tx-1".into(),
                    mutation: TransactionMutation::CreateRecord {
                        label: "Person".into(),
                        id: None,
                        properties: serde_json::json!({ "name": "Ada" }),
                        options: serde_json::json!({}),
                    },
                },
            },
        };
        let encoded = serde_json::to_value(&request).unwrap();
        assert_eq!(encoded["operation"]["kind"], "transaction");
        assert_eq!(encoded["operation"]["transaction"]["kind"], "stage");
        assert_eq!(
            encoded["operation"]["transaction"]["mutation"]["kind"],
            "createRecord"
        );
        assert_eq!(
            serde_json::from_value::<RequestEnvelope>(encoded).unwrap(),
            request
        );
    }
}

#[cfg(test)]
mod compatibility {
    /// A protocol compatibility test that rejects accidental breaking changes:
    /// pins the wire version and the discriminant order of the error taxonomy.
    /// Adding a variant to `ErrorCode` must go at the END; reordering or
    /// removing a variant is a breaking change and must bump `PROTOCOL_VERSION`.
    #[test]
    fn protocol_version_is_one_and_error_code_order_is_stable() {
        assert_eq!(super::PROTOCOL_VERSION, 1);
        let codes = [
            super::ErrorCode::UnsupportedVersion,
            super::ErrorCode::ProtocolViolation,
            super::ErrorCode::InvalidRequest,
            super::ErrorCode::Unauthorized,
            super::ErrorCode::MissingScope,
            super::ErrorCode::ScopeMismatch,
            super::ErrorCode::NotFound,
            super::ErrorCode::Conflict,
            super::ErrorCode::Cancelled,
            super::ErrorCode::ResourceExhausted,
            super::ErrorCode::Unavailable,
            super::ErrorCode::ExecutionFailed,
            super::ErrorCode::LanguageDisabled,
            super::ErrorCode::Unsupported,
            super::ErrorCode::WriteConflict,
        ];
        for (i, code) in codes.iter().enumerate() {
            // Discriminant order is the wire contract; assert it explicitly.
            assert_eq!(
                *code as u8,
                u8::try_from(i).expect("ErrorCode count fits in u8"),
                "ErrorCode discriminant {i} moved"
            );
        }
    }
}
