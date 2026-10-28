//! Configuration for currently integrated resource consumers. These limits
//! govern reservations, not process RSS or all provider allocations.
use serde::{Deserialize, Serialize};
use std::{str::FromStr, time::Duration};

/// A byte count parsed from a bare integer or a `B`/`KiB`/`MiB`/`GiB` suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ByteSize(pub usize);

impl FromStr for ByteSize {
    type Err = String;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let input = input.trim();
        let digits = input.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return Err("byte size requires a nonnegative integer".into());
        }
        let value = input[..digits]
            .parse::<usize>()
            .map_err(|_| "byte size overflow")?;
        let multiplier = match input[digits..].trim() {
            "" | "B" => 1usize,
            "KiB" => 1024,
            "MiB" => 1024 * 1024,
            "GiB" => 1024 * 1024 * 1024,
            _ => return Err("byte size suffix must be B, KiB, MiB, or GiB".into()),
        };
        value
            .checked_mul(multiplier)
            .map(Self)
            .ok_or_else(|| "byte size overflow".into())
    }
}

impl<'de> Deserialize<'de> for ByteSize {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Input {
            Bytes(u64),
            Text(String),
        }
        match Input::deserialize(deserializer)? {
            Input::Bytes(value) => usize::try_from(value)
                .map(Self)
                .map_err(serde::de::Error::custom),
            Input::Text(value) => value.parse().map_err(serde::de::Error::custom),
        }
    }
}

/// Storage resource budgets and maintenance knobs (CLI `resources` section and
/// the `config.yml` `resources` section).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, clap::Args)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct ResourceConfig {
    /// Engine-managed reservations only; not a process RSS limit.
    #[arg(long, env = "RUSHDB_MANAGED_LIMIT", default_value = "512MiB")]
    pub managed_limit: ByteSize,
    /// Managed budget for the storage page cache.
    #[arg(long, env = "RUSHDB_STORAGE_CACHE", default_value = "128MiB")]
    pub storage_cache: ByteSize,
    /// Managed budget for the transaction staging pool.
    #[arg(long, env = "RUSHDB_TRANSACTION_POOL", default_value = "128MiB")]
    pub transaction_pool: ByteSize,
    /// Optional operator-declared deployment cap, not automatic OS detection.
    #[arg(long, env = "RUSHDB_DEPLOYMENT_LIMIT")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_limit: Option<ByteSize>,
    /// Required untracked allowance when a deployment limit is specified.
    #[arg(long, env = "RUSHDB_MEMORY_HEADROOM", default_value = "128MiB")]
    pub memory_headroom: ByteSize,
    /// Logical durability contract: `local` (fsync before ack, default) or
    /// `async` (no fsync; not persisted until a later local commit or
    /// checkpoint, so an acknowledged commit can be lost on a crash). `memory`
    /// is only for in-memory stores.
    #[arg(long, env = "RUSHDB_DURABILITY", default_value = "local")]
    pub durability: DurabilityArg,
    /// Physical storage shards (independent redb files / writers). 1 = single
    /// file (default). > 1 routes tenants across per-shard files.
    #[arg(long, env = "RUSHDB_SHARDS", default_value = "1")]
    pub shards: usize,
    /// Acknowledge that `shards > 1` is experimental: multi-shard commit
    /// groups are persisted per shard and are not crash-atomic. Required to
    /// enable more than one shard.
    #[arg(long, env = "RUSHDB_EXPERIMENTAL_SHARDING")]
    pub experimental_sharding: bool,
    /// Max commits coalesced into one group-commit txn (0 = unbounded).
    #[arg(long, env = "RUSHDB_MAX_GROUP_COUNT", default_value = "0")]
    pub max_group_count: usize,
    /// Max staged op bytes coalesced into one group-commit txn (0 = unbounded).
    #[arg(long, env = "RUSHDB_MAX_GROUP_BYTES", default_value_t = 0)]
    pub max_group_bytes: usize,
    /// Max time the group-commit publisher lingers before flushing, letting
    /// concurrent commits share one fsync (0 = flush immediately). Trades a
    /// little write latency for higher bulk throughput.
    #[arg(long, env = "RUSHDB_MAX_GROUP_DELAY_MS", default_value_t = 0)]
    pub max_group_delay_ms: u64,
    /// Bytes reserved for byte-accounted logical caches (decoded records,
    /// adjacency, property dictionary), charged to the shared budget tree.
    /// 0 disables them. Default 128 MiB.
    #[arg(long, env = "RUSHDB_LOGICAL_CACHE_BYTES", default_value = "128MiB")]
    pub logical_cache_bytes: ByteSize,
    /// Byte budget of the evictable planner-statistics cache (value
    /// distributions loaded on demand), a child of the logical cache budget.
    /// 0 disables caching. Default 32 MiB.
    #[arg(long, env = "RUSHDB_PLANNER_STATS_BYTES", default_value = "32MiB")]
    pub planner_stats_bytes: ByteSize,
    /// Seconds an unused planner-statistics entry stays cached. Default 600.
    #[arg(long, env = "RUSHDB_PLANNER_STATS_IDLE_SECONDS", default_value_t = 600)]
    pub planner_stats_idle_seconds: u64,
    /// Commits after which a cached planner-statistics entry is recomputed.
    /// Default 10000.
    #[arg(
        long,
        env = "RUSHDB_PLANNER_STATS_MAX_AGE_COMMITS",
        default_value_t = 10_000
    )]
    pub planner_stats_max_age_commits: u64,
    /// Seconds an unused tenant's in-memory handles (vector engine,
    /// transaction coordinator) stay resident. Default 300.
    #[arg(long, env = "RUSHDB_TENANT_IDLE_SECONDS", default_value_t = 300)]
    pub tenant_idle_seconds: u64,
    /// Byte budget for resident tenant handles; when only active tenants fill
    /// it, new tenants get a retryable 503 `tenant_capacity`. Default 512 MiB.
    #[arg(long, env = "RUSHDB_TENANT_RESIDENT_BYTES", default_value = "512MiB")]
    pub tenant_resident_bytes: ByteSize,
    /// Per-tenant metric series (records per tenant) for at most this many
    /// most recently served tenants; series of evicted or purged tenants are
    /// removed. 0 (default) = no per-tenant series.
    #[arg(long, env = "RUSHDB_METRICS_TENANT_LIMIT", default_value_t = 0)]
    pub metrics_tenant_limit: usize,
    /// Process RSS watchdog threshold (e.g. `2GiB`); 0 = disabled. Policy
    /// signal, not a hard allocation cap.
    #[arg(long, env = "RUSHDB_RSS_MAX", default_value = "0")]
    pub rss_max: ByteSize,
    /// Per-query working-set cap in bytes; 0 = off. A structured query that
    /// would materialize more returns `resource exhausted` instead of
    /// ballooning RSS (distinct from transaction staging). Defaults to a
    /// bounded 256MiB so one query cannot exhaust a small host.
    #[arg(long, env = "RUSHDB_QUERY_MEMORY_MAX", default_value = "256MiB")]
    pub query_memory_max: ByteSize,
    /// Largest record population a filter without a usable index may check;
    /// a larger one is refused at planning with the index to declare.
    /// 0 (default) = no limit: such a filter walks the records, like
    /// PostgreSQL's sequential scan, bounded by `request-timeout-ms` and
    /// `query-memory-max`. A request may set a tighter limit.
    #[arg(long, env = "RUSHDB_UNINDEXED_WALK_MAX_OWNERS", default_value_t = 0)]
    pub unindexed_walk_max_owners: u64,
    /// Maximum time to wait for the write-admission lock before failing a
    /// commit with a retryable `resource exhausted`; 0 = wait indefinitely.
    #[arg(long, env = "RUSHDB_LOCK_TIMEOUT_MS", default_value_t = 0)]
    pub lock_timeout_ms: u64,
    /// Max bytes of one commit's expanded write set (encoded op-set plus the
    /// serialized command payload) before it is rejected outright with a
    /// clean, non-retryable `resource exhausted` error — the bulk-load
    /// boundary: below this, a commit too
    /// big for one 64 MiB WAL record is transparently chained across
    /// several; above it, the expectation is an external bulk-import
    /// pipeline, not the live transactional path.
    #[arg(
        long = "max-expanded-commit-bytes",
        env = "RUSHDB_MAX_EXPANDED_COMMIT_BYTES",
        default_value = "512MiB"
    )]
    pub max_expanded_commit_bytes: ByteSize,
    /// Idle-window compaction period in seconds; 0 = disabled. Acts as the
    /// minimum interval between compactions. On by default: a compaction runs only when the store is quiet and the file grew or a
    /// tenant purge freed space, so purged tenants give their space back.
    #[arg(long, env = "RUSHDB_COMPACT_INTERVAL_SECONDS", default_value_t = 300)]
    pub compact_interval_seconds: u64,
    /// Minimum quiet period after the last commit before compaction runs.
    #[arg(long, env = "RUSHDB_COMPACT_QUIESCE_SECONDS", default_value_t = 10)]
    pub compact_quiesce_seconds: u64,
    /// Compact only when the file grew by at least this percent since the last
    /// compaction (>= 100).
    #[arg(long, env = "RUSHDB_COMPACT_GROWTH_PERCENT", default_value_t = 150)]
    pub compact_growth_percent: u32,
    /// History/GC sweep period in seconds (checkpoint interval); 0 = 5s.
    #[arg(long, env = "RUSHDB_CHECKPOINT_INTERVAL_SECONDS", default_value_t = 0)]
    pub checkpoint_interval_seconds: u64,
    /// Directory for continuous logical-WAL archiving. When set, a worker
    /// writes append-only segment files `wal-<from>-<to>.seg` plus an
    /// `archive.state` watermark, enabling off-box durability and PITR when
    /// combined with a base backup. None = off.
    #[arg(long, env = "RUSHDB_WAL_ARCHIVE_DIR")]
    pub wal_archive_dir: Option<std::path::PathBuf>,
    /// Retry window: a retried commit within this many seconds returns the
    /// original outcome; after it, the retry is a new commit.
    #[arg(
        long,
        env = "RUSHDB_IDEMPOTENCY_WINDOW_SECONDS",
        default_value_t = 24 * 3600
    )]
    pub idempotency_window_seconds: u64,
    /// Archive retention window in seconds: PITR is possible to any
    /// point inside it. Segments older than the window, and not needed by
    /// the base backup that covers the window's start, are deleted.
    #[arg(
        long,
        env = "RUSHDB_WAL_ARCHIVE_RETENTION_SECONDS",
        default_value_t = 7 * 24 * 3600
    )]
    pub wal_archive_retention_seconds: u64,
    /// Hard byte cap on the WAL archive; 0 = none. Over it the oldest
    /// segments go even inside the window (the window shrinks, logged).
    #[arg(long, env = "RUSHDB_WAL_ARCHIVE_RETENTION_BYTES", default_value = "0")]
    pub wal_archive_retention_bytes: ByteSize,
    /// Streaming replication: the leader's base URL (e.g.
    /// `http://leader:3000`) to follow. When set, a background task polls
    /// the leader's WAL from this node's last applied commit and applies
    /// each returned commit batch. Start from an empty store, or from a
    /// base backup restored from the leader (mid-stream attach) while the
    /// leader still retains the WAL after it. None = not a follower.
    #[arg(long, env = "RUSHDB_REPLICA_OF")]
    pub replica_of: Option<String>,
    /// Stable identity this follower reports to the leader
    /// (`RUSHDB_REPLICA_ID`; default `"default"`). The leader protects WAL
    /// retention per follower identity, so two distinct followers of the
    /// same leader MUST use distinct ids or each will silently un-protect
    /// the WAL range the other still needs.
    #[arg(long, env = "RUSHDB_REPLICA_ID", default_value = "default")]
    pub replica_id: String,
    /// Bearer token presented to the leader's replication endpoint
    /// (`RUSHDB_REPLICA_TOKEN`; defaults to this node's own `--token` when
    /// unset, for the common same-cluster shared-secret case).
    #[arg(long, env = "RUSHDB_REPLICA_TOKEN")]
    pub replica_token: Option<String>,
    /// Hard cap: the WAL never holds more than the last N commits for any
    /// consumer (a replica, archiver or backup below it must resync).
    #[arg(
        long,
        env = "RUSHDB_TX_LOG_RETENTION_COMMITS",
        default_value_t = 100_000
    )]
    pub tx_log_retention_commits: u64,
    /// Commits the WAL keeps even when no consumer or open reader needs them;
    /// 0 = keep only what consumers and readers pin (the PostgreSQL
    /// `wal_keep_size = 0` default). Bounded by the hard caps.
    #[arg(long, env = "RUSHDB_TX_LOG_KEEP_COMMITS", default_value_t = 0)]
    pub tx_log_keep_commits: u64,
    /// Retain all per-commit metadata (wins over `tx-log-retention-commits`).
    #[arg(long, env = "RUSHDB_TX_LOG_KEEP_ALL", default_value_t = false)]
    pub tx_log_keep_all: bool,
    /// Byte-target for retained command-log rows (WAL); 0 = off. The oldest
    /// rows are pruned until the log fits, in addition to the commit floor.
    /// Defaults to 4GiB so a bulk-ingest burst cannot fill the disk with
    /// retained WAL before the commit-count window is reached.
    #[arg(long, env = "RUSHDB_TX_LOG_RETENTION_BYTES", default_value = "4GiB")]
    pub tx_log_retention_bytes: ByteSize,
    /// Slow-call log threshold in milliseconds; 0 = disabled.
    #[arg(long, env = "RUSHDB_LOGS_QUERY_THRESHOLD_MS", default_value_t = 5_000)]
    pub logs_query_threshold_ms: u64,
    /// Bulk (ingest) requests one tenant may run at a time; further ones
    /// wait in line, so one tenant's bulk load does not queue every other
    /// tenant's commits behind its batches. `0` = unlimited.
    #[arg(long, env = "RUSHDB_TENANT_BULK_CONCURRENCY", default_value_t = 2)]
    pub tenant_bulk_concurrency: usize,
    /// Prefix applied to every exported Prometheus metric name.
    #[arg(long, env = "RUSHDB_METRICS_PREFIX")]
    pub metrics_prefix: Option<String>,
    /// Export only metric families whose name contains this substring.
    #[arg(long, env = "RUSHDB_METRICS_FILTER")]
    pub metrics_filter: Option<String>,
    /// Writes on every transport are rejected and readiness fails below this free-disk
    /// percentage; 0 = disabled. Default 10: fail safe before a full disk
    /// stops writes for every tenant.
    #[arg(
        long,
        env = "RUSHDB_DISK_SPACE_THRESHOLD_PERCENT",
        default_value_t = 10
    )]
    #[serde(default = "default_disk_space_threshold_percent")]
    pub disk_space_threshold_percent: u8,
    /// Request-scheduler and HTTP runtime worker threads; 0 = the host's
    /// available parallelism.
    #[arg(long, env = "RUSHDB_THREADS", default_value_t = 0)]
    pub threads: usize,
    /// Seconds a retention consumer (backup, WAL archive, replica, vector
    /// index) may go without acknowledging before its claim on the WAL
    /// expires; 0 = never expire. Default 24h.
    #[arg(
        long,
        env = "RUSHDB_RETENTION_CONSUMER_LEASE_SECONDS",
        default_value_t = 86_400
    )]
    pub retention_consumer_lease_seconds: u64,
    /// Bytes of HTTP request bodies that may be in flight at once, across all
    /// connections. A request whose declared length does not fit is refused
    /// with a retryable `resource_exhausted` before its body is read.
    #[arg(
        long,
        env = "RUSHDB_HTTP_INFLIGHT_BODY_BYTES",
        default_value = "256MiB"
    )]
    pub http_inflight_body_bytes: ByteSize,
    /// Address advertised in build info for clients; defaults to the bind
    /// address.
    #[arg(long, env = "RUSHDB_ADVERTISED_ADDRESS")]
    pub advertised_address: Option<String>,
    /// Require an explicit index name on vector writes and searches. When on,
    /// `createVector` / `search.vector` / inline `$vectors` without a
    /// `name`/`index` are refused instead of falling back to the derived
    /// `emb__...` alias (which cannot distinguish two same-shape models). On
    /// by default, matching how every comparable vector database (e.g.
    /// Qdrant's collections) always requires an explicit name; disable only
    /// to keep a deployment that still relies on implicit derivation
    /// working unchanged (`--require-vector-index-name false`).
    #[arg(
        long,
        env = "RUSHDB_REQUIRE_VECTOR_INDEX_NAME",
        default_value_t = true,
        action = clap::ArgAction::Set
    )]
    pub require_vector_index_name: bool,
    /// Largest index (ready vectors) a vector search in `auto` mode scores
    /// exactly; above it, `auto` uses the ANN graph once it is resident and
    /// builds it in the background meanwhile. At 5,000 vectors an exact
    /// search takes ~9 ms at 768 dimensions and ~13 ms at 1,536.
    #[arg(long, env = "RUSHDB_VECTOR_EXACT_MAX_POOL", default_value_t = 5_000)]
    pub vector_exact_max_pool: u64,
    /// ANN gather breadth when a search does not set `ef`.
    #[arg(long, env = "RUSHDB_VECTOR_DEFAULT_EF", default_value_t = 64)]
    pub vector_default_ef: usize,
    /// Longest text, in characters, that catalog introspection shows for a
    /// sample, listed value or bound (`describe` schema and properties,
    /// `describe.schemaSummary`): a longer one is cut there and ends with
    /// `…`, so a column of long texts cannot flood a schema read. Value
    /// queries (`propertyValues`, `searchValues`) and records are never cut.
    /// 0 = no cap.
    #[arg(
        long,
        env = "RUSHDB_INTROSPECTION_VALUE_MAX_CHARS",
        default_value_t = 64
    )]
    pub introspection_value_max_chars: usize,
}

/// Config-surface spelling of the durability contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum DurabilityArg {
    /// Durable before acknowledgement (default).
    Local,
    /// Acknowledge after queuing; persisted by a later durable commit or
    /// checkpoint.
    Async,
    /// In-memory only; no durability guarantee (in-memory stores only).
    Memory,
}

/// `config.yml` default for the free-disk guard. A config file that omits the
/// key gets the production fail-safe (10%); only an explicit `0` disables it.
/// The library `Default` below is 0 for tests and embedders, so the two are
/// intentionally separate.
fn default_disk_space_threshold_percent() -> u8 {
    10
}

impl Default for ResourceConfig {
    fn default() -> Self {
        let storage = rushdb_storage::resources::StorageResources::default();
        Self {
            managed_limit: ByteSize(storage.managed_limit_bytes),
            storage_cache: ByteSize(storage.page_cache_bytes),
            transaction_pool: ByteSize(storage.transaction_pool_bytes),
            deployment_limit: None,
            memory_headroom: ByteSize(128 * 1024 * 1024),
            durability: DurabilityArg::Local,
            shards: 1,
            experimental_sharding: false,
            max_group_count: 0,
            max_group_bytes: 0,
            max_group_delay_ms: 0,
            logical_cache_bytes: ByteSize(128 * 1024 * 1024),
            planner_stats_bytes: ByteSize(storage.planner_stats_bytes),
            planner_stats_idle_seconds: storage.planner_stats_idle_seconds,
            planner_stats_max_age_commits: storage.planner_stats_max_age_commits,
            tenant_idle_seconds: 300,
            tenant_resident_bytes: ByteSize(512 * 1024 * 1024),
            metrics_tenant_limit: 0,
            rss_max: ByteSize(0),
            query_memory_max: ByteSize(256 * 1024 * 1024),
            unindexed_walk_max_owners: 0,
            lock_timeout_ms: 0,
            max_expanded_commit_bytes: ByteSize(storage.max_expanded_commit_bytes),
            compact_interval_seconds: 300,
            compact_quiesce_seconds: 10,
            compact_growth_percent: 150,
            checkpoint_interval_seconds: 0,
            wal_archive_dir: None,
            idempotency_window_seconds: 24 * 3600,
            wal_archive_retention_seconds: 7 * 24 * 3600,
            wal_archive_retention_bytes: ByteSize(0),
            replica_of: None,
            replica_id: "default".into(),
            replica_token: None,
            tx_log_retention_commits: 100_000,
            tx_log_keep_commits: 0,
            tx_log_keep_all: false,
            tx_log_retention_bytes: ByteSize(4 * 1024 * 1024 * 1024),
            logs_query_threshold_ms: 5_000,
            tenant_bulk_concurrency: 2,
            metrics_prefix: None,
            metrics_filter: None,
            // The library default leaves the guard off: `ServerConfig::default()`
            // backs in-process tests and embedders, which must not depend on the
            // host's free space. The production defaults (CLI flag and config
            // file) are 10 — see `default_disk_space_threshold_percent`.
            disk_space_threshold_percent: 0,
            threads: 0,
            retention_consumer_lease_seconds: 86_400,
            http_inflight_body_bytes: ByteSize(256 * 1024 * 1024),
            advertised_address: None,
            require_vector_index_name: true,
            vector_exact_max_pool: 5_000,
            vector_default_ef: 64,
            introspection_value_max_chars: 64,
        }
    }
}
impl ResourceConfig {
    /// Request-scheduler shape `(workers, queue capacity)`. `threads = 0`
    /// sizes the pool from the host's available parallelism; the queue holds
    /// 32 requests per worker, clamped to `64..=4096`.
    #[must_use]
    pub fn scheduler_shape(&self) -> (usize, usize) {
        let workers = if self.threads == 0 {
            std::thread::available_parallelism().map_or(4, std::num::NonZero::get)
        } else {
            self.threads
        }
        .clamp(2, 256);
        (workers, workers.saturating_mul(32).clamp(64, 4096))
    }

    /// Build and validate the storage resource envelope from this config.
    ///
    /// # Errors
    ///
    /// Returns a message when the shard/group limits are invalid, the
    /// memory-headroom is zero, the disk threshold exceeds 100, the managed
    /// limit plus headroom overflows, or that sum exceeds the declared
    /// deployment limit.
    pub fn storage(&self) -> Result<rushdb_storage::resources::StorageResources, String> {
        let storage = rushdb_storage::resources::StorageResources {
            managed_limit_bytes: self.managed_limit.0,
            page_cache_bytes: self.storage_cache.0,
            transaction_pool_bytes: self.transaction_pool.0,
            durability: match self.durability {
                DurabilityArg::Local => rushdb_storage::resources::Durability::Local,
                DurabilityArg::Async => rushdb_storage::resources::Durability::Async,
                DurabilityArg::Memory => rushdb_storage::resources::Durability::Memory,
            },
            shard_count: self.shards,
            experimental_multi_shard: self.experimental_sharding,
            max_group_count: self.max_group_count,
            max_group_bytes: self.max_group_bytes,
            max_group_delay_ms: self.max_group_delay_ms,
            logical_cache_bytes: self.logical_cache_bytes.0,
            // Phase admission is bounded by the whole managed envelope; the
            // shared parent budget still caps the sum of all children. This
            // decouples concurrency from the (smaller) logical-cache budget.
            phase_budget_bytes: self.managed_limit.0,
            query_memory_max_bytes: u64::try_from(self.query_memory_max.0).unwrap_or(u64::MAX),
            unindexed_walk_max_owners: self.unindexed_walk_max_owners,
            lock_acquisition_timeout_ms: self.lock_timeout_ms,
            max_expanded_commit_bytes: self.max_expanded_commit_bytes.0,
            planner_stats_bytes: self.planner_stats_bytes.0,
            planner_stats_idle_seconds: self.planner_stats_idle_seconds,
            planner_stats_max_age_commits: self.planner_stats_max_age_commits,
            // Segmented-WAL rotation target (log-first). Not yet an
            // operator knob; the default matches the storage default.
            log_segment_bytes: 64 * 1024 * 1024,
            idempotency_window_seconds: self.idempotency_window_seconds,
        };
        storage.validate().map_err(|e| e.to_string())?;
        if self.memory_headroom.0 == 0 {
            return Err("memory-headroom must be positive".into());
        }
        if self.disk_space_threshold_percent > 100 {
            return Err("disk-space-threshold-percent must be between 0 and 100".into());
        }
        let total = self
            .managed_limit
            .0
            .checked_add(self.memory_headroom.0)
            .ok_or("managed-limit plus memory-headroom overflows")?;
        if self.deployment_limit.is_some_and(|limit| total > limit.0) {
            return Err("managed-limit plus memory-headroom exceeds deployment-limit".into());
        }
        Ok(storage)
    }

    /// Maintenance policy derived from the config surface (replaces the
    /// environment-only knobs). Retention is commit-count based; byte/time
    /// policies are deliberately not exposed because the logical log has no
    /// per-row size/time index. `boot_token` is the fallback replication
    /// token when `--replica-token` is unset (the common same-cluster
    /// shared-secret case).
    #[must_use]
    pub fn maintenance(&self, boot_token: &str) -> crate::retention::MaintenanceConfig {
        use crate::retention::RetentionPolicy;
        let retention = if self.tx_log_keep_all {
            RetentionPolicy::KeepAll
        } else if self.tx_log_retention_commits == 0 {
            RetentionPolicy::KeepNone
        } else {
            RetentionPolicy::Commits(self.tx_log_retention_commits)
        };
        crate::retention::MaintenanceConfig {
            retention,
            keep_commits: self.tx_log_keep_commits,
            retention_bytes: (self.tx_log_retention_bytes.0 > 0)
                .then_some(self.tx_log_retention_bytes.0 as u64),
            compact_interval: (self.compact_interval_seconds > 0)
                .then(|| Duration::from_secs(self.compact_interval_seconds)),
            compact_quiesce: Duration::from_secs(self.compact_quiesce_seconds),
            compact_growth_percent: self.compact_growth_percent,
            rss_max_bytes: (self.rss_max.0 > 0).then_some(self.rss_max.0 as u64),
            wal_archive_dir: self.wal_archive_dir.clone(),
            wal_archive_retention: std::time::Duration::from_secs(
                self.wal_archive_retention_seconds,
            ),
            wal_archive_retention_bytes: (self.wal_archive_retention_bytes.0 > 0)
                .then_some(self.wal_archive_retention_bytes.0 as u64),
            replica: self
                .replica_of
                .clone()
                .map(|leader_url| crate::retention::ReplicaConfig {
                    leader_url,
                    follower_id: self.replica_id.clone(),
                    token: self
                        .replica_token
                        .clone()
                        .unwrap_or_else(|| boot_token.to_string()),
                }),
        }
    }

    /// History/GC sweep period (default 5s when unset).
    #[must_use]
    pub fn checkpoint_interval(&self) -> Duration {
        if self.checkpoint_interval_seconds == 0 {
            Duration::from_secs(5)
        } else {
            Duration::from_secs(self.checkpoint_interval_seconds)
        }
    }
}

/// Limits for the transaction coordinator (CLI `transactions` section and the
/// `config.yml` `transactions` section).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::Args)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct TransactionConfig {
    /// Maximum concurrently active transactions.
    #[arg(
        long = "tx-max-active",
        env = "RUSHDB_TX_MAX_ACTIVE",
        default_value_t = 128
    )]
    pub max_active: usize,
    /// Maximum staged operations per transaction.
    #[arg(
        long = "tx-max-operations",
        env = "RUSHDB_TX_MAX_OPERATIONS",
        default_value_t = 10_000
    )]
    pub max_operations: usize,
    /// Maximum staged payload bytes per transaction.
    #[arg(
        long = "tx-max-staged-bytes",
        env = "RUSHDB_TX_MAX_STAGED_BYTES",
        default_value = "8MiB"
    )]
    pub max_staged_bytes: ByteSize,
    /// Maximum expired transaction ids tracked per sweep.
    #[arg(
        long = "tx-max-expired-ids",
        env = "RUSHDB_TX_MAX_EXPIRED_IDS",
        default_value_t = 128
    )]
    pub max_expired_ids: usize,
    /// Default client transaction TTL in seconds.
    #[arg(
        long = "tx-default-ttl-seconds",
        env = "RUSHDB_TX_DEFAULT_TTL_SECONDS",
        default_value_t = 60
    )]
    pub default_ttl_seconds: u64,
    /// Maximum lifetime accepted for a client-provided transaction TTL.
    #[arg(
        long = "tx-max-age-seconds",
        env = "RUSHDB_TX_MAX_AGE_SECONDS",
        default_value_t = 60
    )]
    pub max_age_seconds: u64,
    /// Maximum staged transaction bytes any single tenant's coordinator may
    /// claim from the shared transaction pool in multitenant mode (0 = share
    /// the pool directly, today's behavior — one tenant's heavy staging can
    /// claim the whole pool and starve another tenant's admission).
    #[arg(
        long = "tx-max-staged-bytes-per-tenant",
        env = "RUSHDB_TX_MAX_STAGED_BYTES_PER_TENANT",
        default_value = "0"
    )]
    pub max_staged_bytes_per_tenant: ByteSize,
}
impl Default for TransactionConfig {
    fn default() -> Self {
        let limits = rushdb_tx::TransactionLimits::default();
        Self {
            max_active: limits.max_active,
            max_operations: limits.max_operations,
            max_staged_bytes: ByteSize(limits.max_staged_bytes),
            max_expired_ids: limits.max_expired_ids,
            default_ttl_seconds: 60,
            max_age_seconds: 60,
            max_staged_bytes_per_tenant: ByteSize(0),
        }
    }
}
impl TransactionConfig {
    /// Translate this config into the coordinator's [`rushdb_tx::TransactionLimits`].
    #[must_use]
    pub fn limits(&self) -> rushdb_tx::TransactionLimits {
        rushdb_tx::TransactionLimits {
            max_active: self.max_active,
            max_operations: self.max_operations,
            max_staged_bytes: self.max_staged_bytes.0,
            max_expired_ids: self.max_expired_ids,
        }
    }
    /// Default transaction TTL offered to clients.
    #[must_use]
    pub fn ttl(&self) -> Duration {
        Duration::from_secs(self.default_ttl_seconds)
    }
    /// The maximum lifetime accepted for a client-provided transaction TTL.
    #[must_use]
    pub fn max_ttl(&self) -> Duration {
        Duration::from_secs(self.max_age_seconds)
    }
    /// Validate the limits against each other and the storage resource pool.
    ///
    /// # Errors
    ///
    /// Returns a message when the limits are internally inconsistent (for
    /// example a zero maximum), the default TTL is zero or exceeds the maximum
    /// TTL, the maximum TTL overflows `Instant`, or the transaction pool cannot
    /// fit coordinator plus active-transaction metadata with payload headroom.
    pub fn validate(&self, resources: &ResourceConfig) -> Result<(), String> {
        self.limits().validate(self.ttl())?;
        if self.max_ttl().is_zero()
            || self.ttl() > self.max_ttl()
            || std::time::Instant::now()
                .checked_add(self.max_ttl())
                .is_none()
        {
            return Err("transaction default TTL must be positive and no greater than transaction maximum TTL".into());
        }
        // At least one coordinator plus one active transaction must be
        // admissible. This does not promise every maximum-sized payload fits.
        let minimum = self
            .limits()
            .metadata_allowance()?
            .checked_add(1024)
            .ok_or("transaction metadata size overflow")?;
        if minimum >= resources.transaction_pool.0 {
            return Err("transaction-pool cannot fit coordinator and active transaction metadata with payload headroom".into());
        }
        if self.max_staged_bytes_per_tenant.0 != 0 && self.max_staged_bytes_per_tenant.0 < minimum {
            return Err("tx-max-staged-bytes-per-tenant cannot fit one coordinator's own metadata reservation".into());
        }
        if self.max_staged_bytes_per_tenant.0 > resources.transaction_pool.0 {
            return Err(
                "tx-max-staged-bytes-per-tenant cannot exceed the shared transaction-pool".into(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};

    #[test]
    fn bytes_are_checked_and_unambiguous() {
        for (text, expected) in [
            ("0", 0),
            ("64KiB", 65_536),
            (" 2 MiB ", 2_097_152),
            ("3B", 3),
        ] {
            assert_eq!(text.parse::<ByteSize>().unwrap(), ByteSize(expected));
        }
        for invalid in [
            "",
            "-1",
            "+1",
            "1.5MiB",
            "1MB",
            "2kb",
            "1TiB",
            "∞",
            "18446744073709551616",
        ] {
            assert!(invalid.parse::<ByteSize>().is_err(), "{invalid}");
        }
        assert!(format!("{}GiB", usize::MAX).parse::<ByteSize>().is_err());
    }

    #[test]
    fn yaml_accepts_numeric_or_suffixed_bytes_and_rejects_typos() {
        let config: ResourceConfig =
            serde_yml::from_str("managed-limit: 1GiB\nstorage-cache: 67108864\n").unwrap();
        assert_eq!(config.managed_limit, ByteSize(1024 * 1024 * 1024));
        assert_eq!(config.storage_cache, ByteSize(64 * 1024 * 1024));
        assert_eq!(
            config.transaction_pool,
            ResourceConfig::default().transaction_pool
        );
        for yaml in [
            "storage-cahce: 64MiB",
            "storage-cache: -1",
            "storage-cache: 1.5",
            "storage-cache: null",
        ] {
            assert!(
                serde_yml::from_str::<ResourceConfig>(yaml).is_err(),
                "{yaml}"
            );
        }
        assert!(serde_yml::from_str::<TransactionConfig>("max-operation: 1").is_err());
    }

    #[test]
    fn disk_guard_defaults_are_off_for_the_library_and_on_for_config_files() {
        // `ServerConfig::default()` backs tests and embedders; the host's free
        // space must not decide whether their writes succeed.
        assert_eq!(ResourceConfig::default().disk_space_threshold_percent, 0);
        // A config file that omits the key keeps the production fail-safe.
        let parsed: ResourceConfig = serde_yml::from_str("shards: 1").unwrap();
        assert_eq!(parsed.disk_space_threshold_percent, 10);
    }

    #[test]
    fn aggregate_and_deployment_headroom_are_checked() {
        let defaults = ResourceConfig::default();
        defaults.storage().unwrap();
        for config in [
            ResourceConfig {
                storage_cache: ByteSize(0),
                ..defaults.clone()
            },
            ResourceConfig {
                transaction_pool: ByteSize(0),
                ..defaults.clone()
            },
            ResourceConfig {
                managed_limit: ByteSize(256 * 1024 * 1024),
                ..defaults.clone()
            },
            ResourceConfig {
                managed_limit: ByteSize(usize::MAX),
                ..defaults.clone()
            },
            ResourceConfig {
                memory_headroom: ByteSize(0),
                ..defaults.clone()
            },
            ResourceConfig {
                deployment_limit: Some(ByteSize(512 * 1024 * 1024)),
                ..defaults.clone()
            },
        ] {
            assert!(config.storage().is_err(), "{config:?}");
        }
        ResourceConfig {
            deployment_limit: Some(ByteSize(640 * 1024 * 1024)),
            ..defaults
        }
        .storage()
        .unwrap();
    }

    #[test]
    fn maintenance_policy_maps_retention_intervals_and_rss() {
        let mut config = ResourceConfig::default();
        let policy = config.maintenance("test-token");
        assert_eq!(
            policy.retention,
            crate::retention::RetentionPolicy::Commits(100_000)
        );
        assert_eq!(policy.compact_interval, Some(Duration::from_secs(300)));
        assert_eq!(policy.rss_max_bytes, None);
        assert_eq!(config.checkpoint_interval(), Duration::from_secs(5));

        config.tx_log_keep_all = true;
        assert_eq!(
            config.maintenance("test-token").retention,
            crate::retention::RetentionPolicy::KeepAll
        );
        config.tx_log_keep_all = false;
        config.tx_log_retention_commits = 0;
        assert_eq!(
            config.maintenance("test-token").retention,
            crate::retention::RetentionPolicy::KeepNone
        );
        config.tx_log_retention_commits = 42;
        config.compact_interval_seconds = 30;
        config.checkpoint_interval_seconds = 15;
        config.rss_max = ByteSize(2 * 1024 * 1024 * 1024);
        let policy = config.maintenance("test-token");
        assert_eq!(
            policy.retention,
            crate::retention::RetentionPolicy::Commits(42)
        );
        assert_eq!(policy.compact_interval, Some(Duration::from_secs(30)));
        assert_eq!(policy.rss_max_bytes, Some(2 * 1024 * 1024 * 1024));
        assert_eq!(config.checkpoint_interval(), Duration::from_secs(15));
    }

    #[test]
    fn new_operational_fields_round_trip_and_validate() {
        let config = ResourceConfig {
            rss_max: ByteSize(1024),
            metrics_prefix: Some("rushdb_".into()),
            metrics_filter: Some("budget".into()),
            disk_space_threshold_percent: 90,
            threads: 8,
            advertised_address: Some("db.internal".into()),
            logs_query_threshold_ms: 250,
            ..Default::default()
        };
        let yaml = serde_yml::to_string(&config).unwrap();
        let parsed: ResourceConfig = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(parsed, config);
        assert!(parsed.storage().is_ok());

        let invalid = ResourceConfig {
            disk_space_threshold_percent: 101,
            ..config
        };
        assert!(invalid.storage().is_err());
    }

    #[test]
    fn transaction_config_rejects_impossible_metadata_and_deadlines() {
        let resources = ResourceConfig::default();
        let defaults = TransactionConfig::default();
        defaults.validate(&resources).unwrap();
        for config in [
            TransactionConfig {
                max_active: 0,
                ..defaults
            },
            TransactionConfig {
                max_operations: 0,
                ..defaults
            },
            TransactionConfig {
                max_staged_bytes: ByteSize(0),
                ..defaults
            },
            TransactionConfig {
                default_ttl_seconds: 0,
                ..defaults
            },
            TransactionConfig {
                max_age_seconds: 0,
                ..defaults
            },
            TransactionConfig {
                default_ttl_seconds: 61,
                max_age_seconds: 60,
                ..defaults
            },
            TransactionConfig {
                max_age_seconds: u64::MAX,
                ..defaults
            },
            TransactionConfig {
                max_expired_ids: usize::MAX,
                ..defaults
            },
            TransactionConfig {
                // Too small to fit even one coordinator's own metadata
                // reservation against its per-tenant child budget.
                max_staged_bytes_per_tenant: ByteSize(1),
                ..defaults
            },
            TransactionConfig {
                // Exceeds the shared transaction-pool itself.
                max_staged_bytes_per_tenant: ByteSize(resources.transaction_pool.0 + 1),
                ..defaults
            },
        ] {
            assert!(config.validate(&resources).is_err(), "{config:?}");
        }
        assert!(defaults
            .validate(&ResourceConfig {
                transaction_pool: ByteSize(2048),
                ..resources.clone()
            })
            .is_err());
        TransactionConfig {
            max_expired_ids: 0,
            ..defaults
        }
        .validate(&resources)
        .unwrap();
        TransactionConfig {
            max_staged_bytes_per_tenant: ByteSize(resources.transaction_pool.0 / 2),
            ..defaults
        }
        .validate(&resources)
        .unwrap();
    }

    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        resources: ResourceConfig,
        #[command(flatten)]
        transactions: TransactionConfig,
    }

    #[test]
    fn every_environment_variable_is_named_after_its_flag() {
        let command = Cli::command();
        for arg in command.get_arguments() {
            let (Some(long), Some(env)) = (arg.get_long(), arg.get_env()) else {
                continue;
            };
            let expected = format!("RUSHDB_{}", long.to_uppercase().replace('-', "_"));
            assert_eq!(env.to_str(), Some(expected.as_str()), "--{long}");
        }
    }

    #[test]
    fn require_vector_index_name_defaults_on_and_can_be_turned_off() {
        let on = Cli::try_parse_from(["test"]).unwrap();
        assert!(on.resources.require_vector_index_name);
        let off = Cli::try_parse_from(["test", "--require-vector-index-name", "false"]).unwrap();
        assert!(!off.resources.require_vector_index_name);
    }

    #[test]
    fn unindexed_walks_run_unless_a_limit_is_set() {
        let cli = Cli::try_parse_from(["test"]).unwrap();
        assert_eq!(cli.resources.unindexed_walk_max_owners, 0);
        assert_eq!(ResourceConfig::default().unindexed_walk_max_owners, 0);
        let storage = ResourceConfig::default().storage().unwrap();
        assert_eq!(storage.unindexed_walk_max_owners, 0);
        let bounded =
            Cli::try_parse_from(["test", "--unindexed-walk-max-owners", "1000000"]).unwrap();
        assert_eq!(bounded.resources.unindexed_walk_max_owners, 1_000_000);
    }

    #[test]
    fn cli_parses_limits_and_documents_environment_bindings() {
        let cli = Cli::try_parse_from([
            "test",
            "--managed-limit",
            "1GiB",
            "--storage-cache",
            "64MiB",
            "--transaction-pool",
            "32MiB",
            "--deployment-limit",
            "2GiB",
            "--memory-headroom",
            "256MiB",
            "--shards",
            "2",
            "--experimental-sharding",
            "--max-group-count",
            "16",
            "--max-group-bytes",
            "4096",
            "--tx-max-active",
            "3",
            "--tx-max-operations",
            "4",
            "--tx-max-staged-bytes",
            "4096",
            "--tx-max-expired-ids",
            "0",
            "--tx-default-ttl-seconds",
            "5",
            "--tx-max-age-seconds",
            "7",
        ])
        .unwrap();
        let storage = cli.resources.storage().unwrap();
        assert_eq!(storage.shard_count, 2);
        assert_eq!(storage.max_group_count, 16);
        assert_eq!(storage.max_group_bytes, 4096);
        cli.transactions.validate(&cli.resources).unwrap();
        assert_eq!(cli.transactions.max_active, 3);
        assert_eq!(cli.transactions.max_operations, 4);
        assert_eq!(cli.transactions.ttl(), Duration::from_secs(5));
        assert_eq!(cli.transactions.max_ttl(), Duration::from_secs(7));
        assert_eq!(cli.transactions.max_staged_bytes, ByteSize(4096));
        let command = Cli::command();
        for (id, env) in [
            ("managed_limit", "RUSHDB_MANAGED_LIMIT"),
            ("default_ttl_seconds", "RUSHDB_TX_DEFAULT_TTL_SECONDS"),
            ("max_age_seconds", "RUSHDB_TX_MAX_AGE_SECONDS"),
        ] {
            assert_eq!(
                command
                    .get_arguments()
                    .find(|arg| arg.get_id() == id)
                    .unwrap()
                    .get_env()
                    .unwrap(),
                env
            );
        }
        assert!(Cli::try_parse_from(["test", "--managed-limit", "3MB"]).is_err());
    }

    #[test]
    fn multi_shard_requires_experimental_acknowledgement() {
        let cli = Cli::try_parse_from(["test", "--shards", "2"]).unwrap();
        assert!(
            cli.resources.storage().is_err(),
            "shards > 1 must be refused without --experimental-sharding"
        );
        let cli =
            Cli::try_parse_from(["test", "--shards", "2", "--experimental-sharding"]).unwrap();
        assert_eq!(cli.resources.storage().unwrap().shard_count, 2);
    }
}
