//! Embedded database node shared by protocol executors and transports.
//! All engines use one store and every write flows through deterministic
//! commit batches.

mod adapters;
mod backup;
mod catalog;
mod facets;
mod fusion;
mod hybrid;
mod indexes;
mod ingest;
mod metrics;
mod observability;
mod queries;
mod records;
mod relationship_reads;
mod writer_holds;
pub(crate) use records::UnlabeledUpsert;
mod relationships;
mod residency;
mod retention;
pub(crate) mod scope;
mod suggestions;
mod transactions;
mod usage;
pub use usage::UsageError;
mod vector_index_meta;
mod vector_mode;
mod vectors;
pub(crate) mod working_set;

use scope::RequestScope;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rushdb_lmpg::import;
use rushdb_lmpg::reader::try_access as access;
use rushdb_query::SearchRequest;
use rushdb_types::ids::{OwnerKind, RecordId, RelationshipId, TenantId};
use rushdb_types::keys::{key, P_FAMILY_SIG};

type PropertyMetadata =
    BTreeMap<String, Vec<(u64, String, String, rushdb_types::model::LogicalType)>>;
type IndexedRecord = (RecordId, u64, serde_json::Map<String, serde_json::Value>);

type PropertyAggregate = (String, u64);
type RelationshipAggregate = (u64, BTreeMap<String, PropertyAggregate>);

const MAX_CURSOR_PAGE_SIZE: u64 = 10_000;
/// Node-wide cap for serialized cursor request state retained alongside pinned
/// read views. The view itself is owned by storage and is bounded separately.
const MAX_RESIDENT_CURSOR_BYTES: usize = 64 * 1024 * 1024;
/// A Protocol v1 cursor holds a storage read view.  It is deliberately kept
/// server-local: its opaque id has no meaning after a process restart.
struct ResultCursor {
    tenant: TenantId,
    /// Policy generation captured when the cursor was created. A context-bound
    /// continuation must use the same generation or fail closed.
    policy_generation: u64,
    /// Ephemeral retention-consumer identity. Cursors die with the process,
    /// so this registration is intentionally not persisted with metadata.
    retention_consumer: String,
    view: Arc<rushdb_storage::ReadView>,
    search: SearchRequest,
    next_offset: u64,
    /// Total matches at cursor creation. The view is pinned, so the total
    /// is invariant across pages: continuations reuse it instead of
    /// re-scanning the match set for a count on every page.
    total: u64,
    expires_at: Instant,
    /// Shared logical-cache reservation for the serialized request state.
    /// Dropping the cursor releases this charge automatically.
    _reservation: Option<rushdb_storage::resources::ByteReservation>,
}

fn cursor_resident_bytes(cursor: &ResultCursor) -> usize {
    // SearchRequest is the variable-sized part of a cursor. Include a fixed
    // allowance for the cursor metadata and map/allocator overhead; the pinned
    // read view is accounted by storage's snapshot budget.
    serde_json::to_vec(&cursor.search).map_or(256, |bytes| bytes.len().saturating_add(256))
}

fn append_bounded_json_response(
    values: &mut Vec<serde_json::Value>,
    encoded_bytes: &mut usize,
    value: serde_json::Value,
    limit: usize,
) -> Result<(), String> {
    let bytes =
        serde_json::to_vec(&value).map_err(|error| format!("encode response row: {error}"))?;
    let next = encoded_bytes.saturating_add(bytes.len());
    if next > limit {
        return Err(format!(
            "response exceeds configured byte limit ({next} > {limit})"
        ));
    }
    *encoded_bytes = next;
    values.push(value);
    Ok(())
}

pub fn hex_simple(id: &[u8; 16]) -> String {
    use std::fmt::Write as _;
    id.iter().fold(String::with_capacity(32), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

/// Map a similarity metric string to the engine enum (cosine/dot/euclidean).
pub fn similarity_from_str(s: &str) -> Result<rushdb_types::occurrence::Similarity, String> {
    match s.to_ascii_lowercase().as_str() {
        "cosine" => Ok(rushdb_types::occurrence::Similarity::Cosine),
        "dot" => Ok(rushdb_types::occurrence::Similarity::Dot),
        "euclidean" | "l2" => Ok(rushdb_types::occurrence::Similarity::Euclidean),
        other => Err(format!(
            "unknown similarity metric `{other}` (cosine|dot|euclidean)"
        )),
    }
}

/// Validate a caller-supplied vector index name. The alias is an opaque
/// compatibility key and is never parsed, but a bounded, unparsed-safe charset
/// keeps it addressable and stops custom names from colliding with the engine's
/// derived `emb__...` form. `derived` is the alias this definition would get
/// without a name, which is allowed to use the reserved prefix.
fn validate_index_alias(name: &str, derived: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 64 {
        return Err("vector index name must be 1..=64 bytes".into());
    }
    if name.starts_with("emb__") && name != derived {
        return Err("vector index name must not use the reserved `emb__` prefix".into());
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
    {
        return Err("vector index name may contain only [A-Za-z0-9._:-]".into());
    }
    Ok(())
}

/// Normalize a vector index owner scope to `"record"` or `"relationship"`.
fn normalize_vector_owner(owner: Option<&str>) -> Result<&'static str, String> {
    match owner.map(str::to_ascii_lowercase).as_deref() {
        None | Some("record" | "node") => Ok("record"),
        Some("relationship" | "edge") => Ok("relationship"),
        Some(other) => Err(format!(
            "unknown vector index owner `{other}` (record|relationship)"
        )),
    }
}

/// Cached `(labels, per-label counts)` for one tenant at one commit index.
type CachedMetadata = std::sync::Arc<(Vec<String>, Vec<(String, u64)>)>;

/// Normalized property name -> declared type name (the property dictionary).
type PropertyTypeMap = std::collections::BTreeMap<String, String>;

/// A byte-accounted logical cache charged to the shared budget tree.
type LogicalCache<K, V> = rushdb_storage::cache::LogicalCache<K, V>;

/// Commit/policy-generation-keyed `(labels, counts)` logical cache.
type MetadataCache = std::sync::Mutex<Option<LogicalCache<(TenantId, u64), (u64, CachedMetadata)>>>;

/// Commit/policy-generation-keyed property-dictionary logical cache.
type PropertyCache =
    std::sync::Mutex<Option<LogicalCache<(TenantId, u64, u64), std::sync::Arc<PropertyTypeMap>>>>;

/// Embedded database node: one store plus the per-tenant engine instances,
/// metrics, request-scope state, and maintenance caches shared by every
/// transport adapter.
pub struct Node {
    store: Arc<rushdb_storage::Store>,
    base_tenant: TenantId,
    /// When true, a request scope (`X-RushDB-Tenant` / pgwire `database`) is
    /// required and selects the tenant keyspace via the full SHA-256 of the
    /// scope value — collision-free, so no discriminator property is ever
    /// stored on records. `false` is single-tenant mode: no scope, one
    /// fixed keyspace.
    multitenant: bool,
    maintenance: bool,
    transactions: crate::resource_config::TransactionConfig,
    languages: rushdb_protocol::LanguageSet,
    /// Per-tenant in-memory handles, admitted on demand and evicted when
    /// idle.
    residency: residency::TenantResidency,
    /// Emit per-tenant metric series (bounded by `metrics_tenant_limit`).
    metrics_per_tenant: Option<usize>,
    /// WAL archive directory: base backups register there so archive
    /// retention keeps what they need.
    wal_archive_dir: Option<std::path::PathBuf>,
    /// Declared text indexes of every tenant.
    text: std::sync::Arc<rushdb_textsearch::TextIndexes>,
    metrics: Metrics,
    bindings: Vec<(String, u16, bool)>,
    max_request_bytes: usize,
    cursors: std::sync::Mutex<HashMap<String, ResultCursor>>,
    cursor_bytes: std::sync::Mutex<usize>,
    /// Byte-accounted logical cache for `(labels, per-label counts)` keyed by
    /// tenant and policy generation. Pinned views make these results pure
    /// over the commit index, so entries are exact until the next commit or
    /// policy rotation. `None` when `resources.logical-cache-bytes` is 0.
    metadata_cache: MetadataCache,
    /// Byte-accounted logical cache for the property dictionary (normalized
    /// name -> type), keyed by `(tenant, commit index, policy generation)`.
    property_cache: PropertyCache,
    /// Last computed [`rushdb_storage::StorageStatistics`] and when. Refreshed
    /// at most once per scrape interval so `/metrics` never scans per scrape.
    storage_stats:
        std::sync::Mutex<Option<(std::time::Instant, rushdb_storage::StorageStatistics)>>,
    /// Cached per-tenant record-header counts and when. Refreshed off the
    /// request path (the count scans the whole keyspace).
    records_by_tenant_cache:
        std::sync::Mutex<Option<(std::time::Instant, std::collections::BTreeMap<u64, u64>)>>,
    /// Slow-call log threshold in milliseconds; 0 disables the log.
    slow_call_threshold_ms: u64,
    /// Per-tenant share of concurrent bulk requests.
    bulk_share: crate::bulk_share::BulkShare,
    /// Prefix applied to exported Prometheus metric names.
    metrics_prefix: Option<String>,
    /// Only export metric families whose name contains this substring.
    metrics_filter: Option<String>,
    /// Readiness fails below this free-disk percentage; 0 disables the guard.
    disk_space_threshold_percent: u8,
    /// Server-side request deadline in milliseconds (0 = none), applied to
    /// every transport by the protocol executor.
    request_timeout_ms: u64,
    /// Data directory the disk guard inspects (absent for memory stores).
    data_dir: Option<std::path::PathBuf>,
    /// Address advertised in build info; `None` echoes the bind address.
    advertised_address: Option<String>,
    /// Require an explicit index name on vector writes and searches (no
    /// derived `emb__...` alias fallback).
    require_vector_index_name: bool,
    /// See `resources.vector-exact-max-pool`.
    vector_exact_max_pool: u64,
    /// See `resources.vector-default-ef`.
    vector_default_ef: usize,
    /// See `resources.introspection-value-max-chars`.
    introspection_value_max_chars: usize,
    /// Acknowledged floors for active transaction/replica/backup/index
    /// consumers. Slow consumers protect history from retention pruning.
    retention_consumers: std::sync::Mutex<crate::retention::RetentionConsumers>,
    /// Owns the optional Raft runtime so it shuts down with the node instead
    /// of being leaked after bootstrap.
    raft_runtime: Option<tokio::runtime::Runtime>,
    /// Bounded blocking executor shared by network transports; embedded
    /// callers use the scheduler owned by `rushdb_engine::Engine` instead.
    scheduler: std::sync::Arc<rushdb_engine::BoundedScheduler>,
    /// `true` while a `--replica-of` task is polling/applying (streaming
    /// replication); writes are rejected meanwhile. Only the task clears it (never
    /// `request_promotion` directly), so a stale batch can never still be
    /// mid-apply once writes are admitted.
    following: std::sync::atomic::AtomicBool,
    /// Operator request (`promote`) the task observes at its next poll.
    follower_stop_requested: std::sync::atomic::AtomicBool,
    /// Maintenance closures on the blocking pool; shutdown drains them before
    /// the final checkpoint.
    maintenance_work: crate::lifecycle::MaintenanceWork,
    /// Wakes the index builder when an index is declared or dropped.
    index_build_wake: std::sync::Arc<tokio::sync::Notify>,
}

impl Drop for Node {
    fn drop(&mut self) {
        // The replicator owns the OpenRaft cluster, whose state-machine list
        // includes this store. Clear that back-edge before stopping the
        // runtime or the store remains in a strong cycle and redb cannot be
        // reopened in the same process.
        self.store.set_replicator(None);
        if let Some(runtime) = self.raft_runtime.take() {
            // OpenRaft tasks retain the store/backend while they wind down.
            // Waiting here is required before a same-process reopen; the
            // background-only shutdown path can return while redb still holds
            // its process lock.
            runtime.shutdown_timeout(std::time::Duration::from_secs(5));
        }
    }
}

use metrics::{disk_free_ok, Metrics};

/// Per-property statistics collected during the describe scan. `min`/`max`
/// use `serde_json`'s scalar ordering (numeric for numbers, lexical for
/// strings/ISO datetimes); `samples` is a bounded set of distinct scalars.
#[derive(Default)]
struct PropertyScanStats {
    records_count: usize,
    is_array: bool,
    samples: Vec<serde_json::Value>,
    min: Option<serde_json::Value>,
    max: Option<serde_json::Value>,
}

/// Bounded number of distinct sample values surfaced by `describe_properties`.
const SCHEMA_SAMPLE_CAP: usize = 10;

/// Index-derived value domain for a property (see
/// [`Node::value_domain_from_index`]).
#[derive(Debug, Clone)]
pub(crate) struct ValueDomainIndex {
    /// Exact distinct scalar values from the equality postings.
    pub distinct: u64,
    /// Whether an explicit-null occurrence exists (null is one distinct value).
    pub null_present: bool,
    pub min: Option<serde_json::Value>,
    pub max: Option<serde_json::Value>,
    /// True when every known definition is numeric, so `min`/`max` are valid.
    pub numeric_only: bool,
}

impl ValueDomainIndex {
    pub(crate) fn distinct_total(&self) -> u64 {
        self.distinct + u64::from(self.null_present)
    }
}

/// E2 entry pagination: slice of an already-sorted property-entry list.
/// `skip` past the end yields an empty list; `limit: None` takes the tail.
fn paginate_entries(
    entries: Vec<rushdb_protocol::PropertyDescription>,
    skip: usize,
    limit: Option<usize>,
) -> Vec<rushdb_protocol::PropertyDescription> {
    if skip >= entries.len() {
        return Vec::new();
    }
    let mut out = entries.into_iter().skip(skip);
    match limit {
        Some(n) => out.by_ref().take(n).collect(),
        None => out.collect(),
    }
}

impl PropertyScanStats {
    fn observe(&mut self, value: &serde_json::Value) {
        if !value.is_number() && !value.is_string() && !value.is_boolean() {
            return;
        }
        if self.samples.len() < SCHEMA_SAMPLE_CAP && !self.samples.contains(value) {
            self.samples.push(value.clone());
        }
        if !(value.is_number() || value.is_string()) {
            return;
        }
        let cmp = |current: &serde_json::Value| -> std::cmp::Ordering {
            match (value, current) {
                (serde_json::Value::Number(_), serde_json::Value::Number(_)) => {
                    rushdb_types::value::json_scalar_cmp(value, current)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }
                (serde_json::Value::String(a), serde_json::Value::String(b)) => a.cmp(b),
                _ => std::cmp::Ordering::Equal,
            }
        };
        if self
            .min
            .as_ref()
            .is_none_or(|current| cmp(current) == std::cmp::Ordering::Less)
        {
            self.min = Some(value.clone());
        }
        if self
            .max
            .as_ref()
            .is_none_or(|current| cmp(current) == std::cmp::Ordering::Greater)
        {
            self.max = Some(value.clone());
        }
    }
}

/// Lower bound of [`Node::read_lease_ms`].
const READ_LEASE_FLOOR_MS: u64 = 60_000;
/// Slack past the longest legitimate reader before its lease expires.
const READ_LEASE_GRACE_MS: u64 = 5_000;

impl Node {
    /// The node's text indexes, the one searcher every query surface uses.
    pub(crate) fn text_indexes(&self) -> &std::sync::Arc<rushdb_textsearch::TextIndexes> {
        &self.text
    }

    /// The shared storage engine. Used by background maintenance (GC worker).
    pub(crate) fn store(&self) -> Arc<rushdb_storage::Store> {
        self.store.clone()
    }

    /// Current committed commit index. Public for offline backup tooling.
    #[must_use]
    pub fn committed_version(&self) -> u64 {
        self.store.committed_version()
    }

    /// `true` while `--replica-of` is polling/applying;
    /// writes are rejected until [`Self::request_promotion`] stops it.
    #[must_use]
    pub fn is_following(&self) -> bool {
        self.following.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Tracker every background maintenance closure is spawned through.
    pub(crate) fn maintenance_work(&self) -> &crate::lifecycle::MaintenanceWork {
        &self.maintenance_work
    }

    /// The index builder's wake-up signal (see `http::spawn_index_builder`).
    pub(crate) fn index_build_wake(&self) -> std::sync::Arc<tokio::sync::Notify> {
        std::sync::Arc::clone(&self.index_build_wake)
    }

    /// Request this follower stop replicating and accept local writes.
    /// Asynchronous — only the task clears [`Self::is_following`] once it
    /// actually stops, so poll it until `false` before treating promotion
    /// as complete; this rules out a write racing a still-mid-apply batch.
    pub fn request_promotion(&self) {
        self.follower_stop_requested
            .store(true, std::sync::atomic::Ordering::Release);
    }

    /// Polled by the replication task itself; see [`Self::request_promotion`].
    pub(crate) fn follower_stop_requested(&self) -> bool {
        self.follower_stop_requested
            .load(std::sync::atomic::Ordering::Acquire)
    }

    /// Called right before the task exits on a stop request; this allows writes, not `request_promotion`.
    pub(crate) fn finish_following(&self) {
        self.following
            .store(false, std::sync::atomic::Ordering::Release);
    }

    /// Boot a node from `cfg`: validate/apply the stored config, open or create
    /// the store, optionally start raft replication, and build the runtime
    /// state.
    ///
    /// # Errors
    ///
    /// Returns a message when the stored config is missing or invalid, resource
    /// or transaction limits are rejected, the raft cluster cannot start, or
    /// the store cannot be opened or created.
    ///
    /// # Panics
    ///
    /// Panics only if the fixed nonzero transport scheduler configuration is
    /// rejected, which indicates a programming error.
    #[expect(
        clippy::too_many_lines,
        reason = "single linear bootstrap: store, optional raft, engine caches, metrics, and transports"
    )]
    pub fn bootstrap(cfg: &mut crate::ServerConfig) -> Result<Self, String> {
        cfg.validate_boot_security()?;
        let boot_started = std::time::Instant::now();
        let mut boot_phases = Vec::with_capacity(4);
        let phase_started = std::time::Instant::now();
        // Resource limits must take effect before opening redb or allocating
        // caches. Read once so a concurrent file edit cannot change the config
        // between preflight and the opened Store.
        let stored_config = cfg
            .data_dir
            .as_deref()
            .map(crate::config::ConfigFile::load)
            .transpose()?
            .flatten();
        if let Some(file) = &stored_config {
            crate::config::validate_config(file, cfg)?;
        }
        let resources = cfg.resources.storage()?;
        cfg.transactions.validate(&cfg.resources)?;
        if cfg.raft.as_ref().is_some_and(|raft| raft.size == 0) {
            return Err("raft replication requires at least one node".into());
        }
        boot_phases.push(("config", phase_started.elapsed()));
        let multitenant = cfg.multitenant;
        let tenant = TenantId::single();
        let mut raft_runtime = None;
        let phase_started = std::time::Instant::now();
        crate::boot::set_phase("opening-storage");
        let store = Arc::new(match &cfg.data_dir {
            Some(dir) => {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                rushdb_lmpg::open_store_with_resources(&dir.join("rushdb.redb"), resources)
                    .map_err(|e| format!("open redb store: {e}"))?
            }
            None => rushdb_storage::Store::new_with_resources(resources)
                .map_err(|e| format!("create memory store: {e}"))?,
        });
        boot_phases.push(("storage", phase_started.elapsed()));
        match store.copy_on_write() {
            Some(rushdb_storage::CopyOnWrite::On { paths, reason }) => tracing::warn!(
                ?paths,
                %reason,
                "data files still copy on write: checkpoint fsyncs can stall every tenant for seconds"
            ),
            Some(rushdb_storage::CopyOnWrite::Off) => {
                tracing::info!("data directory has copy-on-write off");
            }
            Some(rushdb_storage::CopyOnWrite::NotApplicable) | None => {}
        }

        if let Some(dir) = &cfg.data_dir {
            if stored_config.is_none() {
                if store.committed_version() > 0 {
                    return Err("existing data directory has no config.yml; migrate it explicitly before reuse".into());
                }
                crate::config::write_config(dir, cfg)?;
            }
        }

        // Register deterministic command expansion for forward commits and
        // OpenRaft state-machine application.
        {
            let _wire = rushdb_lmpg::Lmpg::new(Arc::clone(&store), tenant);
        }
        crate::boot::set_phase("starting-engines");

        // Optional raft replication: boot an in-process OpenRaft cluster and
        // route local commits through the elected leader. This node's store is
        // registered as node 0's state machine, so every replicated batch is
        // applied here before the commit returns; the cluster runtime is kept
        // alive for the node's lifetime.
        if let Some(raft_cfg) = &cfg.raft {
            let phase_started = std::time::Instant::now();
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?;
            let cluster = runtime
                .block_on(rushdb_raft::OpenRaftCluster::new_with_local_store(
                    raft_cfg.size,
                    std::time::Duration::from_millis(50),
                    std::time::Duration::from_millis(200),
                    Some(Arc::clone(&store)),
                ))
                .map_err(|e| format!("raft bootstrap: {e}"))?;
            let cluster = std::sync::Arc::new(cluster);
            store.set_replicator(Some(std::sync::Arc::new(
                rushdb_raft::ClusterReplicator::new(cluster.clone(), runtime.handle().clone()),
            )));
            runtime
                .block_on(cluster.leader(std::time::Duration::from_secs(5)))
                .map_err(|e| format!("raft leader election: {e}"))?;
            // Keep the runtime owned by the node; dropping the node now also
            // shuts down the replication workers and releases their handles.
            raft_runtime = Some(runtime);
            boot_phases.push(("raft", phase_started.elapsed()));
        }

        let metadata_cache = store
            .cache_budget()
            .map(rushdb_storage::cache::LogicalCache::new)
            .transpose()
            .map_err(|error| format!("create metadata cache: {error}"))?;
        let property_cache = store
            .cache_budget()
            .map(rushdb_storage::cache::LogicalCache::new)
            .transpose()
            .map_err(|error| format!("create property cache: {error}"))?;

        let node = Node {
            store: Arc::clone(&store),
            base_tenant: tenant,
            multitenant,
            maintenance: cfg.maintenance,
            transactions: cfg.transactions,
            languages: cfg.languages,
            residency: residency::TenantResidency::new(
                std::time::Duration::from_secs(cfg.resources.tenant_idle_seconds),
                cfg.resources.tenant_resident_bytes.0,
                Arc::new(Instant::now),
            ),
            wal_archive_dir: cfg.resources.wal_archive_dir.clone(),
            metrics_per_tenant: (cfg.resources.metrics_tenant_limit > 0)
                .then_some(cfg.resources.metrics_tenant_limit),
            text: std::sync::Arc::new(rushdb_textsearch::TextIndexes::new(Arc::clone(&store))),
            metrics: Metrics::new(),
            bindings: vec![
                ("http".to_string(), cfg.port, cfg.tls_cert.is_some()),
                ("pgwire".to_string(), cfg.pgwire_port, false),
                ("grpc".to_string(), cfg.grpc_port, false),
                ("bolt".to_string(), cfg.bolt_port, false),
            ],
            max_request_bytes: cfg.max_request_bytes,
            cursors: std::sync::Mutex::new(HashMap::new()),
            cursor_bytes: std::sync::Mutex::new(0),
            metadata_cache: std::sync::Mutex::new(metadata_cache),
            property_cache: std::sync::Mutex::new(property_cache),
            storage_stats: std::sync::Mutex::new(None),
            records_by_tenant_cache: std::sync::Mutex::new(None),
            slow_call_threshold_ms: cfg.resources.logs_query_threshold_ms,
            bulk_share: crate::bulk_share::BulkShare::new(cfg.resources.tenant_bulk_concurrency),
            metrics_prefix: cfg.resources.metrics_prefix.clone(),
            metrics_filter: cfg.resources.metrics_filter.clone(),
            disk_space_threshold_percent: cfg.resources.disk_space_threshold_percent,
            request_timeout_ms: cfg.request_timeout_ms,
            data_dir: cfg.data_dir.clone(),
            advertised_address: cfg.resources.advertised_address.clone(),
            require_vector_index_name: cfg.resources.require_vector_index_name,
            vector_exact_max_pool: cfg.resources.vector_exact_max_pool,
            vector_default_ef: cfg.resources.vector_default_ef,
            introspection_value_max_chars: cfg.resources.introspection_value_max_chars,
            retention_consumers: std::sync::Mutex::new(
                crate::retention::RetentionConsumers::with_lease(std::time::Duration::from_secs(
                    cfg.resources.retention_consumer_lease_seconds,
                )),
            ),
            raft_runtime,
            scheduler: {
                let (workers, queue_capacity) = cfg.resources.scheduler_shape();
                // RM-3: the scheduler is the single admission owner (no
                // separate Node-level tenant-slot gate). `0` means "no
                // configured per-tenant cap", which must mean "as unlimited
                // as the system allows" (the full queue), not a silently
                // different hardcoded default — a prior version of this
                // code capped at 64 here even when the operator asked for 0
                // (unlimited) at `Node::try_acquire_tenant_slot`, which this
                // consolidation also removes.
                let max_per_tenant = if cfg.max_concurrent_requests_per_tenant == 0 {
                    queue_capacity
                } else {
                    cfg.max_concurrent_requests_per_tenant
                };
                std::sync::Arc::new(
                    rushdb_engine::BoundedScheduler::new_with_tenant_limit(
                        workers,
                        queue_capacity,
                        max_per_tenant,
                    )
                    .expect("node scheduler configuration is valid"),
                )
            },
            following: std::sync::atomic::AtomicBool::new(cfg.resources.replica_of.is_some()),
            follower_stop_requested: std::sync::atomic::AtomicBool::new(false),
            maintenance_work: crate::lifecycle::MaintenanceWork::default(),
            index_build_wake: std::sync::Arc::default(),
        };
        node.restore_retention_consumers()?;
        // Every durable commit feeds the latency histogram (C0: it was never
        // observed, so `rushdb_commit_latency_seconds` stayed empty).
        node.store
            .set_commit_observer(node.metrics.commit_observer());
        for (phase, elapsed) in boot_phases {
            node.metrics
                .observe_boot_phase(phase, elapsed.as_secs_f64());
        }
        node.metrics
            .observe_boot_phase("total", boot_started.elapsed().as_secs_f64());
        crate::boot::set_phase("ready");
        Ok(node)
    }
    /// Version/build identity, transport bindings, and configured limits for
    /// `Describe::BuildInfo`. Budgets are read live from the store so the
    /// reported limits match the running node.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "these casts narrow engine counters and sizes that are bounded by the process address space or clamped before conversion, so truncation cannot occur on supported 64-bit targets"
    )]
    pub fn build_info(&self) -> rushdb_protocol::BuildInfo {
        let mut limits = std::collections::BTreeMap::new();
        limits.insert(
            "maxRequestBytes".to_string(),
            self.max_request_bytes.min(u64::MAX as usize) as u64,
        );
        limits.insert(
            "txMaxStagedBytes".to_string(),
            self.transactions.max_staged_bytes.0.min(u64::MAX as usize) as u64,
        );
        limits.insert(
            "storageBudgetBytes".to_string(),
            self.store
                .resource_budget()
                .snapshot()
                .limit
                .min(u64::MAX as usize) as u64,
        );
        limits.insert(
            "transactionBudgetBytes".to_string(),
            self.store
                .transaction_budget()
                .snapshot()
                .limit
                .min(u64::MAX as usize) as u64,
        );
        rushdb_protocol::BuildInfo {
            engine: "rushdb-lmpg".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_revision: option_env!("RUSHDB_BUILD_REVISION").map(str::to_string),
            protocol_version: rushdb_protocol::PROTOCOL_VERSION,
            transports: self
                .bindings
                .iter()
                .map(|(transport, port, tls)| rushdb_protocol::TransportBinding {
                    transport: transport.clone(),
                    port: *port,
                    tls: *tls,
                })
                .collect(),
            limits,
            advertised_address: self.advertised_address.clone(),
        }
    }

    /// Slow-call log threshold in milliseconds (0 = disabled).
    pub fn slow_call_threshold_ms(&self) -> u64 {
        self.slow_call_threshold_ms
    }

    /// The per-tenant bulk share.
    pub(crate) fn bulk_share(&self) -> &crate::bulk_share::BulkShare {
        &self.bulk_share
    }

    /// Server-side request deadline in milliseconds (0 = none).
    pub fn request_timeout_ms(&self) -> u64 {
        self.request_timeout_ms
    }

    /// How long a read view may live before GC treats it as abandoned and
    /// cancels it. It must outlast every legitimate reader: a transaction or
    /// cursor snapshot (both capped by `tx-max-age-seconds`) and a request
    /// running up to `request-timeout-ms`; never below one minute.
    pub fn read_lease_ms(&self) -> u64 {
        let transaction_ms =
            u64::try_from(self.transactions.max_ttl().as_millis()).unwrap_or(u64::MAX);
        READ_LEASE_FLOOR_MS
            .max(transaction_ms)
            .max(self.request_timeout_ms)
            .saturating_add(READ_LEASE_GRACE_MS)
    }

    /// Configured free-disk readiness threshold percentage (0 = disabled).
    pub fn disk_space_threshold_percent(&self) -> u8 {
        self.disk_space_threshold_percent
    }

    /// Configured prefix applied to exported Prometheus metric names.
    pub fn metrics_prefix(&self) -> Option<&str> {
        self.metrics_prefix.as_deref()
    }

    /// Configured substring filter selecting which metric families are exported.
    pub fn metrics_filter(&self) -> Option<&str> {
        self.metrics_filter.as_deref()
    }

    /// Readiness disk guard: true when the data filesystem has at least the
    /// configured free percentage (or the guard is disabled / unsupported).
    pub fn disk_space_ok(&self) -> bool {
        if self.disk_space_threshold_percent == 0 {
            return true;
        }
        let path = self
            .data_dir
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        disk_free_ok(&path, self.disk_space_threshold_percent).unwrap_or(true)
    }

    /// Reclaim old MVCC versions and compact the redb backing file.
    ///
    /// # Errors
    ///
    /// Returns a message when the store cannot complete compaction.
    pub fn compact_now(&self) -> Result<u64, String> {
        self.store.compact().map_err(|e| e.to_string())
    }

    /// The startup language-enablement set (`searchquery` is always on).
    pub fn languages(&self) -> rushdb_protocol::LanguageSet {
        self.languages
    }

    fn reap_idle_engines(&self) -> usize {
        self.residency.evict_idle().len()
    }

    /// Apply pending changes of every tenant's text indexes, within
    /// `budget`. Tenants without text indexes cost one catalog read. Returns
    /// the owners applied.
    pub(crate) fn apply_text_indexes(&self, budget: std::time::Duration) -> usize {
        let started = std::time::Instant::now();
        let tenants = match self.durable_tenants() {
            Ok(tenants) => tenants,
            Err(error) => {
                tracing::warn!(%error, "text applier cannot enumerate tenants");
                return 0;
            }
        };
        let mut applied = 0usize;
        for tenant in tenants {
            let remaining = budget.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                break;
            }
            let has_text = self
                .store
                .read_view()
                .map_err(|e| e.to_string())
                .and_then(|view| {
                    rushdb_lmpg::index::definitions(&view, tenant).map_err(|e| e.to_string())
                })
                .map(|defs| {
                    defs.iter()
                        .any(|def| matches!(def.kind, rushdb_types::index::IndexKind::Text { .. }))
                });
            match has_text {
                Ok(false) => continue,
                Ok(true) => {}
                Err(error) => {
                    tracing::warn!(%error, %tenant, "text applier cannot read the catalog");
                    continue;
                }
            }
            let lmpg = rushdb_lmpg::Lmpg::new(Arc::clone(&self.store), tenant);
            match self.text.apply_tenant(&lmpg, remaining) {
                Ok(count) => applied = applied.saturating_add(count),
                Err(error) => tracing::warn!(%error, %tenant, "text apply failed"),
            }
        }
        applied
    }
    /// Node-wide sweep; does not require or inspect request tenant scope.
    pub(crate) fn reap_expired_transactions(&self) -> usize {
        self.reap_expired_transactions_as(rushdb_engine::OperationClass::Maintenance)
    }

    /// Execute expiry as an explicitly classified maintenance operation. This
    /// keeps background expiry publication separate from transport writes when
    /// the reaper is called from shutdown or maintenance scheduling.
    fn reap_expired_transactions_as(&self, operation: rushdb_engine::OperationClass) -> usize {
        debug_assert_eq!(operation, rushdb_engine::OperationClass::Maintenance);
        let coordinators = self.residency.coordinators();
        let expired: usize = coordinators
            .iter()
            .map(|coordinator| {
                if self.maintenance() {
                    coordinator.reap_expired_without_autocommit()
                } else {
                    coordinator.reap_expired()
                }
            })
            .sum();
        drop(coordinators);
        expired + self.reap_expired_cursors() + self.reap_idle_engines()
    }

    pub(crate) fn reap_expired_cursors(&self) -> usize {
        let now = Instant::now();
        let mut cursors = self
            .cursors
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut expired = 0usize;
        let mut reclaimed = 0usize;
        let mut released = Vec::new();
        cursors.retain(|_, cursor| {
            if cursor.expires_at > now {
                true
            } else {
                expired = expired.saturating_add(1);
                reclaimed = reclaimed.saturating_add(cursor_resident_bytes(cursor));
                released.push(cursor.retention_consumer.clone());
                false
            }
        });
        drop(cursors);
        for consumer in released {
            self.release_ephemeral_retention_consumer(&consumer);
        }
        if reclaimed > 0 {
            let mut cursor_bytes = self
                .cursor_bytes
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *cursor_bytes = cursor_bytes.saturating_sub(reclaimed);
        }
        expired
    }
    /// True when the node requires a request scope selecting the tenant keyspace.
    pub fn multitenant(&self) -> bool {
        self.multitenant
    }

    pub(crate) fn max_response_bytes(&self) -> usize {
        self.max_request_bytes
    }

    pub(crate) fn base_tenant_id(&self) -> TenantId {
        self.base_tenant
    }

    /// Resolve a transport scope to its tenant ordinal. No scope is the
    /// node's base tenant. A scope the directory has never seen is registered
    /// on first use, except on a follower, which cannot commit and resolves it
    /// to [`TenantId::UNREGISTERED`] (owns no keys, so reads are empty).
    ///
    /// # Errors
    /// Returns the storage error when the directory cannot be read or the
    /// registration commit fails.
    pub(crate) fn resolve_scope(&self, scope: Option<&str>) -> Result<TenantId, String> {
        let Some(scope) = scope else {
            return Ok(self.base_tenant);
        };
        let digest = rushdb_types::ids::TenantScope::of(scope);
        if self.following.load(std::sync::atomic::Ordering::Acquire) {
            return self
                .store
                .resolve_tenant(&digest)
                .map(|tenant| tenant.unwrap_or(TenantId::UNREGISTERED))
                .map_err(|error| error.to_string());
        }
        rushdb_lmpg::tenancy::resolve_or_register(&self.store, &digest)
            .map_err(|error| format!("tenant directory: {error}"))
    }

    /// Scheduler fairness key for a transport scope: its resolved tenant, or
    /// the unregistered sentinel when resolution fails (the executor then
    /// reports the failure).
    pub(crate) fn schedule_key(&self, scope: Option<&str>) -> TenantId {
        self.resolve_scope(scope).unwrap_or(TenantId::UNREGISTERED)
    }

    /// True while maintenance mode is active (writes rejected, `/ready` 503).
    pub fn maintenance(&self) -> bool {
        self.maintenance
    }

    /// Convenience [`RequestScope`] resolving to the fixed base tenant.
    ///
    /// For single-tenant/embedded callers only — the production request path
    /// (transports → `ProtocolExecutor`) always constructs an explicit
    /// `RequestScope` from the transport-authenticated scope instead of this.
    /// Safe by construction, not by convention: in single-tenant mode a
    /// `RequestScope` always resolves to the same base tenant regardless of
    /// scope input, so there is no ambient/ordering hazard here — this is a
    /// pure, repeatable function of `self`, not state any earlier call could
    /// have left behind. In multitenant mode, callers that need a specific
    /// tenant must build their own `RequestScope::new(node, Some(scope))`.
    #[must_use]
    pub fn scope(&self) -> RequestScope<'_> {
        RequestScope::new(self, None)
    }

    /// Run a transport operation through the node-wide bounded scheduler.
    ///
    /// RM-3: this is the **single** admission owner for tenant concurrency.
    /// A prior version additionally gated on a separate Node-level
    /// `tenant_inflight` counter here, duplicating the scheduler's own
    /// per-tenant cap at every transport call site; the two were always
    /// configured from the same value and always checked together, so the
    /// duplicate added drift risk (see the scheduler-construction comment)
    /// without adding a distinct policy. Callers get one typed
    /// [`rushdb_engine::SchedulerError::QueueFull`] when the tenant is at
    /// capacity, not two different admission paths to keep in sync.
    pub(crate) fn schedule_for_tenant<R: Send + 'static>(
        &self,
        tenant: TenantId,
        job: impl FnOnce() -> R + Send + 'static,
    ) -> Result<R, rushdb_engine::SchedulerError> {
        self.scheduler.execute_for_tenant(tenant, job)
    }
}

impl RequestScope<'_> {
    /// Query plan introspection for `Describe::Explain`: the selected access
    /// path, residual filter, and a bounded scan estimate. Does not execute the
    /// query.
    ///
    /// # Errors
    ///
    /// Returns a message when the label catalog cannot be read for the current
    /// tenant.
    pub fn explain(
        &self,
        query: &rushdb_protocol::QueryDocument,
    ) -> Result<rushdb_protocol::ExplainPlan, String> {
        let snap = self.store.read_view().map_err(|e| e.to_string())?;
        self.explain_in_view(query, &snap)
    }

    /// Explain a query using a caller-owned pinned snapshot.
    pub(crate) fn explain_in_view(
        &self,
        query: &rushdb_protocol::QueryDocument,
        snap: &rushdb_storage::ReadView,
    ) -> Result<rushdb_protocol::ExplainPlan, String> {
        let tenant = self.tenant();
        let mut estimate: u64 = 0;
        for label in &query.labels {
            // Exact O(1) count from the on-disk label counter.
            let count = rushdb_lmpg::derived::label_count(snap, tenant, label)
                .map_err(|e| e.to_string())?;
            estimate = estimate.saturating_add(count);
        }
        let parsed_where = rushdb_query::document::parse_where(&query.predicate).ok();
        // Exact most-selective predicate-leaf cardinality from the maintained
        // derived postings; `None` when the predicate is not
        // index-eligible.
        let estimated_cardinality = parsed_where.as_ref().and_then(|expr| {
            rushdb_query::explain_cardinality(snap, tenant, &query.labels, expr)
                .ok()
                .flatten()
        });
        // Separate, approximate selectivity estimate for a range leaf
        // (never the same field as `estimated_cardinality` above).
        let range_selectivity_estimate = parsed_where.as_ref().and_then(|expr| {
            rushdb_query::explain_range_selectivity(snap, tenant, expr)
                .ok()
                .flatten()
        });
        // A declared ordered index on a predicate leaf is the access
        // path; otherwise records queries scan label membership.
        let index = parsed_where.as_ref().and_then(|expr| {
            rushdb_query::explain_index(snap, tenant, &query.labels, expr)
                .ok()
                .flatten()
        });
        let unindexed_walk = match (&index, parsed_where.as_ref()) {
            (None, Some(expr)) => rushdb_query::explain_walk(snap, tenant, &query.labels, expr)
                .map_err(|e| e.to_string())?
                .map(
                    |(property, owners, labels)| rushdb_protocol::UnindexedWalk {
                        suggested_indexes: labels
                            .into_iter()
                            .map(|label| rushdb_protocol::IndexSuggestion {
                                label,
                                property: property.clone(),
                            })
                            .collect(),
                        property,
                        owners,
                    },
                ),
            _ => None,
        };
        Ok(rushdb_protocol::ExplainPlan {
            access_path: match (&index, &unindexed_walk) {
                (Some(name), _) => format!("index:{name}"),
                (None, Some(walk)) => format!("property_walk:{}", walk.property),
                (None, None) if query.labels.is_empty() => "full_scan".to_string(),
                (None, None) => "label_scan".to_string(),
            },
            unindexed_walk,
            labels: query.labels.clone(),
            predicate: query.predicate.clone(),
            scan_estimate: estimate,
            estimated_cardinality,
            range_selectivity_estimate,
            limit: query.limit,
            offset: query.offset,
        })
    }
    /// Vector query plan introspection for `Describe::ExplainVector`: the
    /// access path (`vector_index`), the resolved alias, the predicate-resolved
    /// candidate count (filter-before-top-k), and the top-k. Does not execute.
    ///
    /// # Errors
    ///
    /// Returns a message when the read view cannot be created or the filter
    /// document cannot be parsed/evaluated.
    pub fn explain_vector(
        &self,
        query: &rushdb_protocol::VectorQuery,
    ) -> Result<rushdb_protocol::ExplainVectorPlan, String> {
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        self.explain_vector_in_view(query, snap)
    }

    /// Explain vector access using a caller-owned pinned snapshot.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "these casts narrow engine counters and sizes that are bounded by the process address space or clamped before conversion, so truncation cannot occur on supported 64-bit targets"
    )]
    pub(crate) fn explain_vector_in_view(
        &self,
        query: &rushdb_protocol::VectorQuery,
        snap: Arc<rushdb_storage::ReadView>,
    ) -> Result<rushdb_protocol::ExplainVectorPlan, String> {
        let labels = query.labels.clone();
        let label = labels.first().cloned().unwrap_or_default();
        let dims = query.vector.len();
        let alias = rushdb_lmpg::import::external_space_alias(
            &label,
            &query.property,
            &query.similarity,
            dims,
        );
        let mut filter_document = serde_json::json!({ "labels": labels });
        if !query.predicate.is_null() {
            filter_document["where"] = query.predicate.clone();
        }
        let parsed =
            rushdb_query::document::parse_document(&filter_document).map_err(|e| e.to_string())?;
        let page =
            rushdb_query::find_records_in_view(&self.lmpg(), &parsed.search, &|| false, snap)
                .map_err(|e| e.to_string())?;
        Ok(rushdb_protocol::ExplainVectorPlan {
            access_path: "vector_index".to_string(),
            alias,
            label,
            candidate_count: page.records.len().min(u64::MAX as usize) as u64,
            k: (query.offset as usize).saturating_add(query.limit as usize),
            predicate: query.predicate.clone(),
        })
    }

    /// Build a tenant-scoped engine handle bound to this node's store,
    /// authorizer and the request's bounds.
    #[must_use]
    pub fn lmpg(&self) -> rushdb_lmpg::Lmpg {
        rushdb_lmpg::Lmpg::new_with_authorizer(
            std::sync::Arc::clone(&self.store),
            self.tenant(),
            self.authorizer(),
        )
        .with_request_limits(self.request_limits())
    }

    fn vector_engine(&self) -> Result<std::sync::Arc<rushdb_vector::VectorEngine>, String> {
        let tenant = self.tenant();
        self.residency.vector(tenant, || {
            // The engine outlives this request and serves later ones, so its
            // handle carries the server's bounds only, not this request's.
            let engine =
                rushdb_vector::VectorEngine::new(Arc::new(rushdb_lmpg::Lmpg::new_with_authorizer(
                    std::sync::Arc::clone(&self.store),
                    tenant,
                    self.authorizer(),
                )));
            // A pure in-memory store (no data_dir) has nowhere to persist a
            // frozen ANN segment; segments then stay resident-only.
            match &self.data_dir {
                Some(dir) => engine.with_segments_dir(
                    dir.join("vector-segments")
                        .join(Self::tenant_dir_name(tenant)),
                ),
                None => engine,
            }
        })
    }

    /// Filesystem-safe, collision-free directory name for one tenant's
    /// on-disk ANN segments (`<data_dir>/vector-segments/<this>/space-<id>/`):
    /// the tenant ordinal in hex. Ordinals are never reused, so a purged
    /// tenant's directory can never be adopted by a new tenant.
    fn tenant_dir_name(tenant: TenantId) -> String {
        rushdb_types::keys::tenant_hex(tenant)
    }

    fn tx_coordinator(&self) -> Result<std::sync::Arc<rushdb_tx::TxCoordinator>, String> {
        let tenant = self.tenant();
        self.residency.tx(tenant, || {
            rushdb_tx::TxCoordinator::with_limits_and_max_ttl(
                Arc::clone(&self.store),
                tenant,
                self.transactions.ttl(),
                self.transactions.max_ttl(),
                Some(std::sync::Arc::new(rushdb_authz::ActiveAuthorizer::new(
                    Arc::clone(&self.store),
                    tenant,
                ))),
                self.transactions.limits(),
                self.transactions.max_staged_bytes_per_tenant.0,
            )
        })
    }

    fn existing_tx_coordinator(&self) -> Option<Arc<rushdb_tx::TxCoordinator>> {
        self.residency.existing_tx(self.tenant())
    }

    /// Engine authorizer bound to the persisted active policy (default-deny
    /// in `PolicyEnforced` mode). Evaluated live at every mutation/commit.
    #[must_use]
    pub fn authorizer(&self) -> Option<std::sync::Arc<dyn rushdb_lmpg::Authorizer>> {
        Some(std::sync::Arc::new(rushdb_authz::ActiveAuthorizer::new(
            std::sync::Arc::clone(&self.store),
            self.tenant(),
        )))
    }

    /// Intersect a records search with the active policy in a pinned view.
    /// This narrows labels and relationship traversals before candidate
    /// planning, so denied data cannot affect totals or pagination.
    pub(crate) fn authorize_search_in_view(
        &self,
        view: &rushdb_storage::ReadView,
        request: &SearchRequest,
    ) -> Result<SearchRequest, String> {
        let Some(active) = rushdb_authz::load_active_policy_in_view(view, self.tenant())
            .map_err(|error| error.to_string())?
        else {
            return Ok(request.clone());
        };
        if active.mode != rushdb_authz::AuthorizationMode::PolicyEnforced {
            return Ok(request.clone());
        }
        // Under `*` an "all labels" request stays one (no labels).
        let every = request.labels.is_empty() && active.policy.allows_every_label();
        let labels = if every {
            Vec::new()
        } else if request.labels.is_empty() {
            // The labels present that the policy allows, as stored (the
            // policy may spell them differently).
            rushdb_lmpg::reader::try_access::labels(view, self.tenant())
                .map_err(|error| error.to_string())?
                .into_iter()
                .filter(|label| active.policy.allows_label(label))
                .collect()
        } else {
            request
                .labels
                .iter()
                .filter(|label| active.policy.allows_label(label))
                .cloned()
                .collect()
        };
        if labels.is_empty() && !every {
            if request.labels.is_empty() && !active.policy.allowed_labels.is_empty() {
                // Allowed labels that hold no records yet: nothing to read.
                return Ok(SearchRequest {
                    labels: active.policy.allowed_labels,
                    ..request.clone()
                });
            }
            return Err("policy denied all record labels".into());
        }
        let mut related = Vec::with_capacity(request.related.len());
        for spec in &request.related {
            if let Some(rel_type) = &spec.rel_type {
                if active
                    .policy
                    .denied_rel_types
                    .iter()
                    .any(|denied| denied == rel_type)
                {
                    return Err(format!("relationship type `{rel_type}` denied by policy"));
                }
            }
            related.push(spec.clone());
        }
        Ok(SearchRequest {
            labels,
            related,
            ..request.clone()
        })
    }

    /// Apply the same pinned policy intersection to an aggregate request.
    pub(crate) fn authorize_aggregate_in_view(
        &self,
        view: &rushdb_storage::ReadView,
        request: &rushdb_query::AggregateRequest,
    ) -> Result<rushdb_query::AggregateRequest, String> {
        let search = SearchRequest {
            labels: request.labels.clone(),
            where_: request.where_.clone(),
            related: request.related.clone(),
            pagination: rushdb_query::Pagination {
                skip: 0,
                limit: None,
            },
            ..SearchRequest::default()
        };
        let restricted = self.authorize_search_in_view(view, &search)?;
        Ok(rushdb_query::AggregateRequest {
            labels: restricted.labels,
            related: restricted.related,
            ..request.clone()
        })
    }

    /// Current active policy + generation + fingerprint, or an error if no
    /// policy has been persisted yet.
    ///
    /// # Errors
    ///
    /// Returns a message when the active policy cannot be loaded or when no
    /// policy has been persisted.
    pub fn security_policy(&self) -> Result<serde_json::Value, String> {
        let lmpg = self.lmpg();
        match rushdb_authz::load_active_policy(&lmpg).map_err(|e| e.to_string())? {
            Some(ap) => Ok(serde_json::json!({
                "generation": ap.generation,
                "mode": format!("{:?}", ap.mode),
                "fingerprint": ap.fingerprint,
                "policy": ap.policy,
            })),
            None => Err("no active policy".to_string()),
        }
    }

    /// Persist a policy (bumping the generation); returned JSON carries the
    /// new generation and fingerprint.
    ///
    /// # Errors
    ///
    /// Returns a message when the policy cannot be stored in the engine.
    pub fn security_put_policy(
        &self,
        policy: &rushdb_authz::DataAccessPolicy,
        mode: rushdb_authz::AuthorizationMode,
    ) -> Result<serde_json::Value, String> {
        self.security_put_policy_with_min_count(policy, mode, 0)
    }

    /// [`Self::security_put_policy`] with a small-count threshold for catalog
    /// reads: groups counted over fewer than `min_count` records are
    /// withheld. 0 is no threshold.
    ///
    /// # Errors
    ///
    /// Returns a message when the policy cannot be stored in the engine.
    pub fn security_put_policy_with_min_count(
        &self,
        policy: &rushdb_authz::DataAccessPolicy,
        mode: rushdb_authz::AuthorizationMode,
        min_count: u64,
    ) -> Result<serde_json::Value, String> {
        let ap = rushdb_authz::store_policy_with_min_count(&self.lmpg(), policy, mode, min_count)?;
        Ok(serde_json::json!({
            "generation": ap.generation,
            "mode": format!("{:?}", ap.mode),
            "fingerprint": ap.fingerprint,
            "minCount": ap.min_count,
        }))
    }

    /// Create the typed engine execution state for one already-admitted
    /// transport request. Network adapters still own identity and scope
    /// mapping; this boundary owns the resolved tenant, pinned view, operation
    /// class, and response working-byte budget.
    pub(crate) fn execution_context(
        &self,
        operation: rushdb_engine::OperationClass,
        deadline: Option<std::time::Duration>,
    ) -> Result<rushdb_engine::ExecutionContext, String> {
        rushdb_engine::ExecutionContext::new(
            &self.store,
            self.tenant(),
            operation,
            deadline,
            Some(self.max_response_bytes()),
        )
        .map_err(|error| error.to_string())
    }

    /// [`Self::purge_tenant`] polling `is_cancelled` between purge chunks so a
    /// long purge can be aborted mid-flight (E4 operator trust). Applied chunks
    /// stay applied; the purge is resumable.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    ///
    /// A poisoned tenant engine or transaction registry mutex is recovered.
    pub fn purge_tenant_with_cancel(&self, is_cancelled: &dyn Fn() -> bool) -> Result<u64, String> {
        let lmpg = self.lmpg();
        // tenant() enforces a present request scope in multi-tenant mode; the
        // authorizer bound to the Lmpg is the default-deny gate on mutation.
        let tenant = self.tenant();
        let commit_index = lmpg
            .purge_tenant_cancellable(is_cancelled)
            .map_err(|e| e.to_reason())?;
        // A purge invalidates every tenant-local derived projection and cache.
        // Remove handles even when an external Arc keeps an in-flight request
        // alive; subsequent requests must construct fresh engines.
        if let Err(error) = self.text.remove_tenant(tenant) {
            tracing::warn!(%error, %tenant, "cannot remove purged tenant's text indexes");
        }
        self.residency.remove(tenant);
        // On-disk ANN segments are a rebuildable cache keyed by tenant, not
        // canonical data — same purge boundary as the in-memory engine
        // handle just dropped above. Best-effort: a failure here wastes
        // disk space, never correctness (a stale file under a reused
        // tenant id would still be caught as a fingerprint mismatch).
        if let Some(dir) = &self.data_dir {
            let _ = std::fs::remove_dir_all(
                dir.join("vector-segments")
                    .join(Self::tenant_dir_name(tenant)),
            );
        }
        Ok(commit_index)
    }

    /// Drop every tenant-scoped key for the current request scope in one
    /// deterministic batch commit. Multi-tenant mode requires a request scope
    /// (enforced by `tenant()`); the global commit log/outcome keys survive.
    /// Returns the commit index of the purge.
    ///
    /// # Errors
    ///
    /// Returns a message when the purge batch cannot be committed (for example
    /// when the authorizer denies the mutation).
    pub fn purge_tenant(&self) -> Result<u64, String> {
        self.purge_tenant_with_cancel(&|| false)
    }

    /// Purge the current tenant through the caller-owned execution context.
    /// Deadline expiry is treated like transport cancellation between bounded
    /// purge chunks, while the context remains the authoritative request
    /// boundary for the protocol path.
    pub(crate) fn purge_tenant_with_execution_context(
        &self,
        execution_context: &mut rushdb_engine::ExecutionContext,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<u64, String> {
        execution_context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let commit_index = self.purge_tenant_with_cancel(&|| {
            is_cancelled() || execution_context.check_deadline().is_err()
        })?;
        Ok(commit_index)
    }

    /// Consistent logical backup. The graph is pinned at the current commit
    /// index and exported as JSONL from a snapshot view while the server keeps
    /// serving writes. In multitenant protocol mode, an empty scope list means
    /// the current request scope and every listed scope must equal it.
    ///
    /// # Errors
    ///
    /// Returns a message when the read view cannot be created, no tenant scope
    /// is set in multitenant mode, the requested scopes do not match the
    /// current scope, or a tenant cannot be exported.
    pub fn backup(
        &self,
        request: &rushdb_protocol::BackupRequest,
    ) -> Result<(u64, Vec<rushdb_protocol::TenantBackup>), String> {
        let snap = self.store.read_view().map_err(|e| e.to_string())?;
        self.backup_in_view(request, &snap)
    }

    /// Export a logical backup from a caller-owned pinned snapshot.
    pub(crate) fn backup_in_view(
        &self,
        request: &rushdb_protocol::BackupRequest,
        snap: &rushdb_storage::ReadView,
    ) -> Result<(u64, Vec<rushdb_protocol::TenantBackup>), String> {
        let commit_index = snap.commit_index();

        // Determine which tenants to export.
        let tenants: Vec<(TenantId, Option<String>)> = if self.multitenant {
            let current = self
                .scope_value()
                .map(str::to_string)
                .ok_or("backup requires a tenant scope")?;
            if request.scopes.iter().any(|scope| scope != &current) {
                return Err("backup scopes must match the current tenant scope".into());
            }
            vec![(self.tenant(), Some(current))]
        } else {
            vec![(self.base_tenant, None)]
        };

        let mut out: Vec<rushdb_protocol::TenantBackup> = Vec::new();
        for (tenant, scope) in tenants {
            let (jsonl, records, relationships) = rushdb_backup::export_tenant_jsonl(snap, tenant)
                .map_err(|e| format!("backup tenant {tenant:?}: {e}"))?;
            out.push(rushdb_protocol::TenantBackup {
                scope,
                jsonl,
                records,
                relationships,
            });
        }
        Ok((commit_index, out))
    }

    /// Exact vector search over one space (DB primitive; embedding
    /// generation stays in the control plane). Returns scored record ids.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn vector_search(
        &self,
        alias: &str,
        vector: &[f32],
        k: usize,
    ) -> Result<Vec<(String, f32)>, String> {
        let tenant = self.tenant();
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        let vectors = self.vector_engine()?;
        let space = vectors
            .lookup_space_in_view(&snap, alias)
            .map_err(|e| e.to_string())?
            .ok_or("index space missing")?;
        let hits = vectors
            .search_exact_in_view(&snap, &space, vector, k, None)
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for h in hits {
            if h.source.kind != OwnerKind::Record {
                continue;
            }
            if let Some(hdr) = access::record_by_ord(&snap, tenant, h.source.owner_ord)
                .map_err(|e| e.to_string())?
            {
                out.push((crate::node::hex_simple(&hdr.record_id), h.score));
            }
        }
        Ok(out)
    }
}

/// Format compact record identity bytes as the canonical dashed UUID string.
fn dashed_from_hex(hex: &str) -> String {
    let b = hex.as_bytes();
    let mut out = String::with_capacity(36);
    for (i, &c) in b.iter().enumerate() {
        if i == 8 || i == 12 || i == 16 || i == 20 {
            out.push('-');
        }
        out.push(c as char);
    }
    out
}

fn bytes_from_hex(hex: &str) -> Result<[u8; 16], String> {
    // accept dashed UUIDs and compact hex alike
    let hex: String = hex.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return Err("id must be a 32-char hex string (or dashed UUID)".into());
    }
    let mut out = [0u8; 16];
    if hex.len() != 32 {
        return Err(format!("id length wrong: {}", hex.len()));
    }
    for (i, byte) in out.iter_mut().enumerate() {
        let hi = hex_byte(hex.as_bytes()[i * 2])?;
        let lo = hex_byte(hex.as_bytes()[i * 2 + 1])?;
        *byte = (hi << 4) | lo;
    }
    Ok(out)
}

fn type_name_str(ty: rushdb_types::model::LogicalType) -> String {
    match ty {
        rushdb_types::model::LogicalType::Boolean => "boolean".to_string(),
        rushdb_types::model::LogicalType::Int64 | rushdb_types::model::LogicalType::Float64 => {
            "number".to_string()
        }
        rushdb_types::model::LogicalType::Utf8 => "string".to_string(),
        rushdb_types::model::LogicalType::DateTime => "datetime".to_string(),
    }
}

/// Canonical logical type name for E8 sibling flags. Unlike `type_name_str`
/// (the display vocabulary, where int and float collapse to `number`), this
/// preserves the exact split so an int/float coexistence is detectable.
fn logical_type_name(ty: rushdb_types::model::LogicalType) -> String {
    match ty {
        rushdb_types::model::LogicalType::Boolean => "boolean".to_string(),
        rushdb_types::model::LogicalType::Int64 => "int64".to_string(),
        rushdb_types::model::LogicalType::Float64 => "float64".to_string(),
        rushdb_types::model::LogicalType::Utf8 => "utf8".to_string(),
        rushdb_types::model::LogicalType::DateTime => "datetime".to_string(),
    }
}

fn schema_json_type(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Array(values) => {
            let item_type = values
                .iter()
                .find(|value| !value.is_null())
                .map_or_else(|| "unknown".to_string(), schema_json_type);
            format!("{item_type}[]")
        }
        value => rushdb_types::value::json_type_name(value).to_string(),
    }
}

fn hex_byte(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("invalid hex byte '{c}'")),
    }
}

#[cfg(test)]
#[path = "node_read_tests.rs"]
mod node_read_tests;

#[cfg(test)]
#[path = "node/transaction_reaper_tests.rs"]
mod transaction_reaper_tests;
