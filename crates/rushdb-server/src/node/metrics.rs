//! Metrics registry and rendering, isolated from request dispatch.

use std::sync::Arc;

use prometheus::Encoder as _;

pub struct Metrics {
    registry: prometheus::Registry,
    requests: prometheus::IntCounterVec,
    duration: prometheus::HistogramVec,
    commits: prometheus::IntCounter,
    text_pending: prometheus::IntGaugeVec,
    /// Tenant labels currently emitted on `rushdb_records_stored`.
    record_tenant_labels: std::sync::Mutex<std::collections::BTreeSet<String>>,
    tenants_resident: prometheus::IntGauge,
    ann_exact_fallbacks: prometheus::IntCounter,
    unindexed_walks: prometheus::IntCounter,
    tenant_resident_bytes: prometheus::IntGauge,
    budget_used: prometheus::IntGaugeVec,
    budget_peak: prometheus::IntGaugeVec,
    budget_limit: prometheus::IntGaugeVec,
    rows: prometheus::IntCounterVec,
    records: prometheus::IntGaugeVec,
    bytes: prometheus::IntGaugeVec,
    commit_latency: prometheus::Histogram,
    commit_phase: prometheus::HistogramVec,
    alloc_allocated: prometheus::IntGauge,
    alloc_total: prometheus::IntCounter,
    alloc_active: prometheus::IntGauge,
    alloc_metadata: prometheus::IntGauge,
    alloc_resident: prometheus::IntGauge,
    alloc_retained: prometheus::IntGauge,
    storage_file_bytes: prometheus::IntGauge,
    storage_data_bytes: prometheus::IntGauge,
    storage_history_bytes: prometheus::IntGauge,
    storage_log_bytes: prometheus::IntGauge,
    storage_receipt_bytes: prometheus::IntGauge,
    storage_admission_waits: prometheus::IntCounter,
    storage_admission_wait_ms: prometheus::IntCounter,
    storage_checkpoints: prometheus::IntCounter,
    storage_checkpoint_wait_ms: prometheus::IntCounter,
    storage_backend_mutex_wait_ms: prometheus::IntCounter,
    storage_backend_mutex_hold_ms: prometheus::IntCounter,
    storage_backend_mutex_hold_max_ms: prometheus::IntGauge,
    /// C0 (cloud-capacity F12): WAL bytes retained on disk.
    wal_retained_bytes: prometheus::IntGauge,
    /// C0: bytes held by the in-memory conflict window.
    conflict_window_bytes: prometheus::IntGauge,
    /// C0: HTTP request-body bytes reserved in flight.
    http_inflight_body_bytes: prometheus::IntGauge,
    /// C0: duration of each maintenance (GC/retention/checkpoint) tick.
    gc_tick_seconds: prometheus::Histogram,
    retention_consumers: prometheus::IntGauge,
    retention_protected_floor: prometheus::IntGauge,
    retention_oldest_reader: prometheus::IntGauge,
    sequencer_inflight_keys: prometheus::IntGauge,
    planner_stats_bytes: prometheus::IntGauge,
    planner_stats_entries: prometheus::IntGauge,
    group_commits_total: prometheus::IntCounter,
    scheduler_inflight: prometheus::IntGauge,
    scheduler_capacity: prometheus::IntGauge,
    log_rows_pruned: prometheus::IntGauge,
    receipts_swept: prometheus::IntGauge,
    compactions_total: prometheus::IntCounter,
    work: prometheus::IntCounterVec,
    boot_phase: prometheus::HistogramVec,
    maintenance_step: prometheus::HistogramVec,
    vector_searches: prometheus::IntCounterVec,
    writer_holds: std::sync::Arc<super::writer_holds::WriterHoldCollector>,
}

/// Registers a collector the metrics also keep a handle to.
struct SharedCollector<C>(std::sync::Arc<C>);

impl<C: prometheus::core::Collector> prometheus::core::Collector for SharedCollector<C> {
    fn desc(&self) -> Vec<&prometheus::core::Desc> {
        self.0.desc()
    }

    fn collect(&self) -> Vec<prometheus::proto::MetricFamily> {
        self.0.collect()
    }
}

const BUCKETS: &[f64] = &[
    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

impl Metrics {
    pub(super) fn observe_state(
        &self,
        stats: Option<rushdb_storage::StorageStatistics>,
        consumers: &crate::retention::RetentionConsumers,
        scheduler: &rushdb_engine::BoundedScheduler,
    ) {
        if let Some(stats) = stats {
            let clamp = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
            self.storage_data_bytes.set(clamp(stats.data_bytes));
            self.storage_history_bytes.set(clamp(stats.history_bytes));
            self.storage_log_bytes.set(clamp(stats.log_bytes));
            self.storage_receipt_bytes.set(clamp(stats.receipt_bytes));
            advance(&self.storage_admission_waits, clamp(stats.admission_waits));
            advance(
                &self.storage_admission_wait_ms,
                clamp(stats.admission_wait_ms),
            );
            advance(&self.storage_checkpoints, clamp(stats.checkpoints));
            advance(
                &self.storage_checkpoint_wait_ms,
                clamp(stats.checkpoint_wait_ms),
            );
        }
        self.retention_consumers
            .set(i64::try_from(consumers.len()).unwrap_or(i64::MAX));
        self.retention_protected_floor.set(
            consumers
                .protected_floor()
                .map_or(0, |floor| i64::try_from(floor).unwrap_or(i64::MAX)),
        );
        self.scheduler_inflight
            .set(i64::try_from(scheduler.inflight()).unwrap_or(i64::MAX));
        self.scheduler_capacity
            .set(i64::try_from(scheduler.capacity()).unwrap_or(i64::MAX));
    }

    /// Records: an "all" series always; per-tenant series only when
    /// enabled, for the bounded set of recently served tenants. Labels that
    /// left that set (evicted or purged tenants) are removed.
    fn observe_records(
        &self,
        records: &std::collections::BTreeMap<u64, u64>,
        per_tenant: Option<&[rushdb_types::ids::TenantId]>,
    ) {
        let total: u64 = records.values().sum();
        self.records
            .with_label_values(&["all"])
            .set(i64::try_from(total).unwrap_or(i64::MAX));
        let desired: std::collections::BTreeSet<String> = per_tenant
            .unwrap_or_default()
            .iter()
            .map(|tenant| tenant.0.to_string())
            .collect();
        {
            let mut emitted = self
                .record_tenant_labels
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for stale in emitted.difference(&desired) {
                let _ = self.records.remove_label_values(&[stale]);
            }
            for label in &desired {
                let count = label
                    .parse::<u64>()
                    .ok()
                    .and_then(|tenant| records.get(&tenant))
                    .copied()
                    .unwrap_or(0);
                self.records
                    .with_label_values(&[label])
                    .set(i64::try_from(count).unwrap_or(i64::MAX));
            }
            *emitted = desired;
        }
    }

    /// Resident tenants and their charged bytes.
    pub(super) fn observe_residency(&self, tenants: usize, bytes: usize) {
        advance(
            &self.ann_exact_fallbacks,
            i64::try_from(rushdb_vector::ann::ann_exact_fallbacks_total()).unwrap_or(i64::MAX),
        );
        advance(
            &self.unindexed_walks,
            i64::try_from(rushdb_query::unindexed_walks_total()).unwrap_or(i64::MAX),
        );
        self.tenants_resident
            .set(i64::try_from(tenants).unwrap_or(i64::MAX));
        self.tenant_resident_bytes
            .set(i64::try_from(bytes).unwrap_or(i64::MAX));
    }

    /// Observe store-level gauges (sequencer in-flight keys,
    /// group commits, oldest live reader).
    pub(super) fn observe_store(&self, store: &rushdb_storage::Store) {
        let clamp = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
        let planner_stats = store.planner_stats().stats();
        self.planner_stats_bytes.set(clamp(planner_stats.bytes));
        self.planner_stats_entries.set(clamp(planner_stats.entries));
        self.sequencer_inflight_keys.set(clamp(
            u64::try_from(store.sequencer_inflight_keys()).unwrap_or(u64::MAX),
        ));
        advance(&self.group_commits_total, clamp(store.group_commit_count()));
        self.retention_oldest_reader
            .set(store.retention_summary().0.map_or(0, clamp));
        // Live (no scan): backend lock contention, retained WAL, conflict
        // window. These used to refresh only with the 15-minute storage walk.
        let locks = store.backend_lock_times();
        advance(
            &self.storage_backend_mutex_wait_ms,
            clamp(locks.wait_ns / 1_000_000),
        );
        advance(
            &self.storage_backend_mutex_hold_ms,
            clamp(locks.hold_ns / 1_000_000),
        );
        self.storage_backend_mutex_hold_max_ms
            .set(clamp(locks.hold_max_ns / 1_000_000));
        self.writer_holds.observe(store);
        if let Ok(bytes) = store.retained_log_bytes() {
            self.wal_retained_bytes.set(clamp(bytes));
        }
        self.conflict_window_bytes.set(clamp(
            u64::try_from(store.conflict_window_bytes()).unwrap_or(u64::MAX),
        ));
    }

    /// Record the bytes of request bodies currently reserved in flight.
    pub(crate) fn observe_inflight_body_bytes(&self, bytes: usize) {
        self.http_inflight_body_bytes
            .set(i64::try_from(bytes).unwrap_or(i64::MAX));
    }

    /// Record one maintenance tick's duration.
    pub(crate) fn observe_gc_tick(&self, seconds: f64) {
        self.gc_tick_seconds.observe(seconds);
    }

    /// Store commit observer feeding the durable-latency histogram and the
    /// per-phase histogram.
    pub(super) fn commit_observer(&self) -> std::sync::Arc<rushdb_storage::CommitObserver> {
        let latency = self.commit_latency.clone();
        let phases: Vec<(rushdb_storage::CommitPhase, prometheus::Histogram)> =
            rushdb_storage::CommitPhase::ALL
                .iter()
                .map(|phase| {
                    (
                        *phase,
                        self.commit_phase.with_label_values(&[phase.label()]),
                    )
                })
                .collect();
        std::sync::Arc::new(move |phase, seconds| {
            if phase == rushdb_storage::CommitPhase::Durable {
                latency.observe(seconds);
            }
            if let Some((_, histogram)) = phases.iter().find(|(known, _)| *known == phase) {
                histogram.observe(seconds);
            }
        })
    }

    #[expect(
        clippy::too_many_lines,
        reason = "constructs and registers every Prometheus family in one linear list so the metric set is auditable in a single place"
    )]
    pub(super) fn new() -> Self {
        let registry = prometheus::Registry::new();
        let requests = prometheus::IntCounterVec::new(
            prometheus::Opts::new("rushdb_requests_total", "Database requests by route"),
            &["route"],
        )
        .expect("valid Prometheus metric");
        let duration = prometheus::HistogramVec::new(
            prometheus::HistogramOpts::new(
                "rushdb_request_duration_seconds",
                "Database request latency in seconds",
            )
            .buckets(BUCKETS.to_vec()),
            &["route"],
        )
        .expect("valid Prometheus metric");
        let commits = prometheus::IntCounter::new(
            "rushdb_commits_total",
            "Current durable RushDB commit index",
        )
        .expect("valid Prometheus metric");
        let text_pending = prometheus::IntGaugeVec::new(
            prometheus::Opts::new(
                "rushdb_text_pending",
                "Changes not yet applied to a text index",
            ),
            &["index"],
        )
        .expect("valid Prometheus metric");
        // P01 resource gauges: managed envelope + transaction staging pool.
        // Expose used/peak/limit per pool so exhaustion has an observable,
        // stable signal (reasons come from ResourceError::Exhausted).
        let budget_used = prometheus::IntGaugeVec::new(
            prometheus::Opts::new(
                "rushdb_budget_used_bytes",
                "Current managed bytes charged to a resource pool",
            ),
            &["pool"],
        )
        .expect("valid Prometheus metric");
        let budget_peak = prometheus::IntGaugeVec::new(
            prometheus::Opts::new(
                "rushdb_budget_peak_bytes",
                "Peak managed bytes charged to a resource pool",
            ),
            &["pool"],
        )
        .expect("valid Prometheus metric");
        let budget_limit = prometheus::IntGaugeVec::new(
            prometheus::Opts::new(
                "rushdb_budget_limit_bytes",
                "Configured byte limit of a resource pool",
            ),
            &["pool"],
        )
        .expect("valid Prometheus metric");
        // Pure usage metrics (3.0): rows returned per query, records stored,
        // approximate bytes stored, and commit latency — all as numbers.
        let rows = prometheus::IntCounterVec::new(
            prometheus::Opts::new("rushdb_rows_returned_total", "Rows returned by query route"),
            &["route"],
        )
        .expect("valid Prometheus metric");
        let records = prometheus::IntGaugeVec::new(
            prometheus::Opts::new(
                "rushdb_records_stored",
                "Records stored: tenant=\"all\" always; per tenant only with metrics-per-tenant",
            ),
            &["tenant"],
        )
        .expect("valid Prometheus metric");
        let ann_exact_fallbacks = prometheus::IntCounter::new(
            "rushdb_ann_exact_fallbacks_total",
            "ANN searches answered exactly because the ANN budget could not admit the index",
        )
        .expect("valid Prometheus metric");
        let unindexed_walks = prometheus::IntCounter::new(
            "rushdb_unindexed_walks_total",
            "Queries whose unindexed predicate walked more owners than the threshold \
             (declare an ordered index; EXPLAIN names it)",
        )
        .expect("valid Prometheus metric");
        let tenants_resident = prometheus::IntGauge::new(
            "rushdb_tenants_resident",
            "Tenants with resident in-memory handles",
        )
        .expect("valid Prometheus metric");
        let tenant_resident_bytes = prometheus::IntGauge::new(
            "rushdb_tenant_resident_bytes",
            "Bytes charged by resident tenant handles",
        )
        .expect("valid Prometheus metric");
        let bytes = prometheus::IntGaugeVec::new(
            prometheus::Opts::new(
                "rushdb_bytes_stored",
                "Approximate bytes stored by tenant (0 = single-tenant aggregate)",
            ),
            &["tenant"],
        )
        .expect("valid Prometheus metric");
        let commit_latency = prometheus::Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "rushdb_commit_latency_seconds",
                "Durable commit latency in seconds",
            )
            .buckets(BUCKETS.to_vec()),
        )
        .expect("valid Prometheus metric");
        let commit_phase = prometheus::HistogramVec::new(
            prometheus::HistogramOpts::new(
                "rushdb_commit_phase_seconds",
                "Commit pipeline time by phase (committer phases per commit, publisher phases per group)",
            )
            .buckets(BUCKETS.to_vec()),
            &["phase"],
        )
        .expect("valid Prometheus metric");
        // Allocator introspection (jemalloc `stats.*`, bytes). Unlike the
        // managed-budget gauges these report bytes actually mapped, so
        // allocator retention after bulk-load peaks is observable. On the
        // system allocator (`--no-default-features`) no counters exist and
        // the gauges stay zero.
        let alloc_total = prometheus::IntCounter::new(
            "rushdb_allocator_bytes_allocated_total",
            "Cumulative bytes allocated since start (alloc-accounting builds only; 0 otherwise)",
        )
        .expect("valid Prometheus metric");
        let alloc_allocated = prometheus::IntGauge::new(
            "rushdb_allocator_allocated_bytes",
            "Application bytes allocated (jemalloc stats.allocated)",
        )
        .expect("valid Prometheus metric");
        let alloc_active = prometheus::IntGauge::new(
            "rushdb_allocator_active_bytes",
            "Bytes in active pages (jemalloc stats.active)",
        )
        .expect("valid Prometheus metric");
        let alloc_metadata = prometheus::IntGauge::new(
            "rushdb_allocator_metadata_bytes",
            "Bytes used for allocator metadata (jemalloc stats.metadata)",
        )
        .expect("valid Prometheus metric");
        let alloc_resident = prometheus::IntGauge::new(
            "rushdb_allocator_resident_bytes",
            "Physically resident bytes (jemalloc stats.resident)",
        )
        .expect("valid Prometheus metric");
        let alloc_retained = prometheus::IntGauge::new(
            "rushdb_allocator_retained_bytes",
            "Bytes retained in arenas but not active (jemalloc stats.retained)",
        )
        .expect("valid Prometheus metric");
        // Storage-file growth and retention/compaction activity: the durable
        // file should track live data once per-commit metadata is pruned.
        let storage_file_bytes = prometheus::IntGauge::new(
            "rushdb_storage_file_bytes",
            "Apparent size of the primary durable file (0 when in-memory)",
        )
        .expect("valid Prometheus metric");
        let storage_data_bytes = prometheus::IntGauge::new(
            "rushdb_storage_data_bytes",
            "Live canonical data row bytes (key+value), excluding history/log",
        )
        .expect("valid Prometheus metric");
        let storage_history_bytes = prometheus::IntGauge::new(
            "rushdb_storage_history_bytes",
            "Retained MVCC history bytes",
        )
        .expect("valid Prometheus metric");
        let storage_log_bytes =
            prometheus::IntGauge::new("rushdb_storage_log_bytes", "Retained command-log bytes")
                .expect("valid Prometheus metric");
        let storage_receipt_bytes = prometheus::IntGauge::new(
            "rushdb_storage_receipt_bytes",
            "Retained retry-receipt bytes",
        )
        .expect("valid Prometheus metric");
        let storage_admission_waits = prometheus::IntCounter::new(
            "rushdb_storage_admission_waits_total",
            "Cumulative bounded storage admission-lock waits",
        )
        .expect("valid Prometheus metric");
        let storage_admission_wait_ms = prometheus::IntCounter::new(
            "rushdb_storage_admission_wait_ms_total",
            "Cumulative milliseconds spent waiting for storage admission",
        )
        .expect("valid Prometheus metric");
        let storage_checkpoints = prometheus::IntCounter::new(
            "rushdb_storage_checkpoints_total",
            "Cumulative explicit durability checkpoints",
        )
        .expect("valid Prometheus metric");
        let storage_checkpoint_wait_ms = prometheus::IntCounter::new(
            "rushdb_storage_checkpoint_wait_ms_total",
            "Cumulative milliseconds spent in durability checkpoints",
        )
        .expect("valid Prometheus metric");
        let storage_backend_mutex_wait_ms = prometheus::IntCounter::new(
            "rushdb_storage_backend_mutex_wait_ms_total",
            "Cumulative milliseconds spent waiting for the backend database mutex",
        )
        .expect("valid Prometheus metric");
        let storage_backend_mutex_hold_ms = prometheus::IntCounter::new(
            "rushdb_storage_backend_mutex_hold_ms_total",
            "Cumulative milliseconds the backend database mutex was held",
        )
        .expect("valid Prometheus metric");
        let wal_retained_bytes =
            prometheus::IntGauge::new("rushdb_wal_retained_bytes", "WAL bytes retained on disk")
                .expect("valid Prometheus metric");
        let conflict_window_bytes = prometheus::IntGauge::new(
            "rushdb_conflict_window_bytes",
            "Bytes held by the in-memory conflict-validation window",
        )
        .expect("valid Prometheus metric");
        let http_inflight_body_bytes = prometheus::IntGauge::new(
            "rushdb_http_inflight_body_bytes",
            "HTTP request-body bytes reserved in flight",
        )
        .expect("valid Prometheus metric");
        let gc_tick_seconds = prometheus::Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "rushdb_gc_tick_seconds",
                "Duration of one maintenance (GC, retention, checkpoint) tick",
            )
            .buckets(BUCKETS.to_vec()),
        )
        .expect("valid Prometheus metric");
        let storage_backend_mutex_hold_max_ms = prometheus::IntGauge::new(
            "rushdb_storage_backend_mutex_hold_max_ms",
            "Maximum single backend database-mutex hold in milliseconds",
        )
        .expect("valid Prometheus metric");
        let retention_consumers = prometheus::IntGauge::new(
            "rushdb_retention_consumers",
            "Active retention consumers protecting history",
        )
        .expect("valid Prometheus metric");
        let retention_protected_floor = prometheus::IntGauge::new(
            "rushdb_retention_protected_floor",
            "Lowest acknowledged commit protected by retention consumers",
        )
        .expect("valid Prometheus metric");
        let retention_oldest_reader = prometheus::IntGauge::new(
            "rushdb_retention_oldest_reader_commit",
            "Oldest commit pinned by a live read view (0 when none)",
        )
        .expect("valid Prometheus metric");
        let sequencer_inflight_keys = prometheus::IntGauge::new(
            "rushdb_sequencer_inflight_keys",
            "Keys in the sequencer's assigned-but-unpublished overlay",
        )
        .expect("valid Prometheus metric");
        let planner_stats_bytes = prometheus::IntGauge::new(
            "rushdb_planner_stats_bytes",
            "Bytes held by the evictable planner-statistics cache",
        )
        .expect("valid Prometheus metric");
        let planner_stats_entries = prometheus::IntGauge::new(
            "rushdb_planner_stats_entries",
            "Value distributions held by the planner-statistics cache",
        )
        .expect("valid Prometheus metric");
        let group_commits_total = prometheus::IntCounter::new(
            "rushdb_storage_group_commits_total",
            "Cumulative group-commit write transactions persisted",
        )
        .expect("valid Prometheus metric");
        let scheduler_inflight = prometheus::IntGauge::new(
            "rushdb_scheduler_inflight",
            "Transport jobs admitted to the bounded scheduler",
        )
        .expect("valid Prometheus metric");
        let scheduler_capacity = prometheus::IntGauge::new(
            "rushdb_scheduler_capacity",
            "Global bounded scheduler admission capacity",
        )
        .expect("valid Prometheus metric");
        let log_rows_pruned = prometheus::IntGauge::new(
            "rushdb_commit_log_rows_pruned",
            "Cumulative command-log rows reclaimed by retention",
        )
        .expect("valid Prometheus metric");
        let receipts_swept = prometheus::IntGauge::new(
            "rushdb_receipts_swept",
            "Cumulative retry receipts removed after the idempotency window",
        )
        .expect("valid Prometheus metric");
        let compactions_total = prometheus::IntCounter::new(
            "rushdb_storage_compactions_total",
            "Cumulative completed storage compactions",
        )
        .expect("valid Prometheus metric");
        let work = prometheus::IntCounterVec::new(
            prometheus::Opts::new(
                "rushdb_operation_work_total",
                "Bounded work units observed by operation and counter",
            ),
            &["operation", "counter"],
        )
        .expect("valid Prometheus metric");
        let maintenance_step = prometheus::HistogramVec::new(
            prometheus::HistogramOpts::new(
                "rushdb_maintenance_step_seconds",
                "Duration of one step of a maintenance tick",
            )
            .buckets(BUCKETS.to_vec()),
            &["step"],
        )
        .expect("valid Prometheus metric");
        let vector_searches = prometheus::IntCounterVec::new(
            prometheus::Opts::new(
                "rushdb_vector_searches_total",
                "Vector searches by the path that answered and why",
            ),
            &["path", "reason"],
        )
        .expect("valid Prometheus metric");
        let boot_phase = prometheus::HistogramVec::new(
            prometheus::HistogramOpts::new(
                "rushdb_boot_phase_duration_seconds",
                "Startup phase duration in seconds",
            )
            .buckets(BUCKETS.to_vec()),
            &["phase"],
        )
        .expect("valid Prometheus metric");
        for collector in [
            Box::new(requests.clone()) as Box<dyn prometheus::core::Collector>,
            Box::new(duration.clone()),
            Box::new(commits.clone()),
            Box::new(text_pending.clone()),
            Box::new(tenants_resident.clone()),
            Box::new(ann_exact_fallbacks.clone()),
            Box::new(unindexed_walks.clone()),
            Box::new(tenant_resident_bytes.clone()),
            Box::new(budget_used.clone()),
            Box::new(budget_peak.clone()),
            Box::new(budget_limit.clone()),
            Box::new(rows.clone()),
            Box::new(records.clone()),
            Box::new(bytes.clone()),
            Box::new(commit_latency.clone()),
            Box::new(commit_phase.clone()),
            Box::new(alloc_allocated.clone()),
            Box::new(alloc_total.clone()),
            Box::new(alloc_active.clone()),
            Box::new(alloc_metadata.clone()),
            Box::new(alloc_resident.clone()),
            Box::new(alloc_retained.clone()),
            Box::new(storage_file_bytes.clone()),
            Box::new(storage_data_bytes.clone()),
            Box::new(storage_history_bytes.clone()),
            Box::new(storage_log_bytes.clone()),
            Box::new(storage_receipt_bytes.clone()),
            Box::new(storage_admission_waits.clone()),
            Box::new(storage_admission_wait_ms.clone()),
            Box::new(storage_checkpoints.clone()),
            Box::new(storage_checkpoint_wait_ms.clone()),
            Box::new(storage_backend_mutex_wait_ms.clone()),
            Box::new(storage_backend_mutex_hold_ms.clone()),
            Box::new(storage_backend_mutex_hold_max_ms.clone()),
            Box::new(wal_retained_bytes.clone()),
            Box::new(conflict_window_bytes.clone()),
            Box::new(http_inflight_body_bytes.clone()),
            Box::new(gc_tick_seconds.clone()),
            Box::new(retention_consumers.clone()),
            Box::new(retention_protected_floor.clone()),
            Box::new(retention_oldest_reader.clone()),
            Box::new(sequencer_inflight_keys.clone()),
            Box::new(planner_stats_bytes.clone()),
            Box::new(planner_stats_entries.clone()),
            Box::new(group_commits_total.clone()),
            Box::new(scheduler_inflight.clone()),
            Box::new(scheduler_capacity.clone()),
            Box::new(log_rows_pruned.clone()),
            Box::new(receipts_swept.clone()),
            Box::new(compactions_total.clone()),
            Box::new(work.clone()),
            Box::new(boot_phase.clone()),
            Box::new(maintenance_step.clone()),
            Box::new(vector_searches.clone()),
        ] {
            registry
                .register(collector)
                .expect("unique Prometheus metric");
        }
        let writer_holds = std::sync::Arc::new(super::writer_holds::WriterHoldCollector::new());
        registry
            .register(Box::new(SharedCollector(std::sync::Arc::clone(
                &writer_holds,
            ))))
            .expect("unique Prometheus metric");
        Self {
            registry,
            requests,
            duration,
            commits,
            text_pending,
            record_tenant_labels: std::sync::Mutex::default(),
            tenants_resident,
            ann_exact_fallbacks,
            unindexed_walks,
            tenant_resident_bytes,
            budget_used,
            budget_peak,
            budget_limit,
            rows,
            records,
            bytes,
            commit_latency,
            commit_phase,
            alloc_allocated,
            alloc_total,
            alloc_active,
            alloc_metadata,
            alloc_resident,
            alloc_retained,
            storage_file_bytes,
            storage_data_bytes,
            storage_history_bytes,
            storage_log_bytes,
            storage_receipt_bytes,
            storage_admission_waits,
            storage_admission_wait_ms,
            storage_checkpoints,
            storage_checkpoint_wait_ms,
            storage_backend_mutex_wait_ms,
            storage_backend_mutex_hold_ms,
            storage_backend_mutex_hold_max_ms,
            wal_retained_bytes,
            conflict_window_bytes,
            http_inflight_body_bytes,
            gc_tick_seconds,
            retention_consumers,
            retention_protected_floor,
            retention_oldest_reader,
            sequencer_inflight_keys,
            planner_stats_bytes,
            planner_stats_entries,
            group_commits_total,
            scheduler_inflight,
            scheduler_capacity,
            log_rows_pruned,
            receipts_swept,
            compactions_total,
            work,
            boot_phase,
            maintenance_step,
            vector_searches,
            writer_holds,
        }
    }

    pub(crate) fn observe(&self, route: &str, seconds: f64) {
        self.requests.with_label_values(&[route]).inc();
        self.duration.with_label_values(&[route]).observe(seconds);
    }

    pub(crate) fn observe_rows(&self, route: &str, rows: u64) {
        self.rows.with_label_values(&[route]).inc_by(rows);
    }

    pub(crate) fn observe_work(&self, operation: &str, counter: &str, units: u64) {
        self.work
            .with_label_values(&[operation, counter])
            .inc_by(units);
    }

    /// Record one maintenance-tick step's duration.
    pub(crate) fn observe_maintenance_step(&self, step: &str, seconds: f64) {
        self.maintenance_step
            .with_label_values(&[step])
            .observe(seconds);
    }

    /// Count one vector search by the path that answered (`ann`/`exact`)
    /// and its reason (`none`, or the fallback).
    pub(crate) fn observe_vector_search(&self, path: &str, reason: &str) {
        self.vector_searches
            .with_label_values(&[path, reason])
            .inc();
    }

    pub(crate) fn observe_boot_phase(&self, phase: &str, seconds: f64) {
        self.boot_phase.with_label_values(&[phase]).observe(seconds);
    }

    #[expect(
        clippy::cast_possible_wrap,
        reason = "every value is clamped to i64::MAX (or saturating) before conversion; metrics and byte counts cannot exceed i64 in practice"
    )]
    pub(super) fn render(
        &self,
        store: &Arc<rushdb_storage::Store>,
        text: &std::collections::BTreeMap<String, u64>,
        records: &std::collections::BTreeMap<u64, u64>,
        per_tenant: Option<&[rushdb_types::ids::TenantId]>,
        prefix: Option<&str>,
        filter: Option<&str>,
    ) -> String {
        let commit_index = store.committed_version();
        advance(&self.commits, commit_index.min(i64::MAX as u64) as i64);
        // Rebuilt every scrape: a dropped index's series goes with it.
        self.text_pending.reset();
        for (index, pending) in text {
            self.text_pending
                .with_label_values(&[index])
                .set((*pending).min(i64::MAX as u64) as i64);
        }
        // P01: expose resource-pool usage so exhaustion has a stable signal.
        for (pool, budget) in [
            ("managed", store.resource_budget()),
            ("transaction", store.transaction_budget()),
        ] {
            let snap = budget.snapshot();
            self.budget_used
                .with_label_values(&[pool])
                .set(snap.used as i64);
            self.budget_peak
                .with_label_values(&[pool])
                .set(snap.peak as i64);
            self.budget_limit
                .with_label_values(&[pool])
                .set(snap.limit as i64);
        }
        // Per-adapter attribution children, same gauge family. Each
        // is `None` iff its shared parent (cache/phase) is disabled.
        for (pool, budget) in [
            ("vector", store.vector_cache_budget()),
            ("sql", store.sql_phase_budget()),
            ("graph", store.graph_phase_budget()),
        ] {
            let Some(budget) = budget else { continue };
            let snap = budget.snapshot();
            self.budget_used
                .with_label_values(&[pool])
                .set(snap.used as i64);
            self.budget_peak
                .with_label_values(&[pool])
                .set(snap.peak as i64);
            self.budget_limit
                .with_label_values(&[pool])
                .set(snap.limit as i64);
        }
        // Allocator reality check: real mapped bytes behind the reservation
        // tokens above. Absent on the system allocator (gauges stay zero).
        if let Some(total) = crate::allocator::bytes_allocated_total() {
            advance(&self.alloc_total, i64::try_from(total).unwrap_or(i64::MAX));
        }
        if let Some(stats) = crate::allocator::stats() {
            self.alloc_allocated.set(stats.allocated as i64);
            self.alloc_active.set(stats.active as i64);
            self.alloc_metadata.set(stats.metadata as i64);
            self.alloc_resident.set(stats.resident as i64);
            self.alloc_retained.set(stats.retained as i64);
        }
        // Storage-file size and retention/compaction counters (0 in-memory).
        self.storage_file_bytes
            .set(store.storage_file_bytes().unwrap_or(0).min(i64::MAX as u64) as i64);
        let (log_pruned, payload_pruned, compactions) = store.maintenance_counters();
        self.log_rows_pruned
            .set(log_pruned.min(i64::MAX as u64) as i64);
        self.receipts_swept
            .set(payload_pruned.min(i64::MAX as u64) as i64);
        advance(
            &self.compactions_total,
            compactions.min(i64::MAX as u64) as i64,
        );
        self.observe_records(records, per_tenant);
        // Approximate bytes stored (redb map bytes), node-wide.
        let (map_bytes, _) = store.approximate_map_bytes();
        self.bytes
            .with_label_values(&["all"])
            .set(map_bytes.min(i64::MAX as u64) as i64);
        let mut families = self.registry.gather();
        if let Some(filter) = filter.filter(|value| !value.is_empty()) {
            families.retain(|family| family.name().contains(filter));
        }
        if let Some(prefix) = prefix.filter(|value| !value.is_empty()) {
            for family in &mut families {
                family.set_name(format!("{prefix}{}", family.name()));
            }
        }
        let mut encoded = Vec::new();
        prometheus::TextEncoder::new()
            .encode(&families, &mut encoded)
            .expect("encoding metrics into Vec cannot fail");
        String::from_utf8(encoded).expect("Prometheus text format is UTF-8")
    }
}

/// Free-space decision for the readiness disk guard, factored out for tests.
#[expect(
    clippy::cast_possible_truncation,
    reason = "avail/total is a fraction in 0..=100 after integer division, so the u8 conversion cannot truncate a meaningful value"
)]
pub(super) fn free_percent_ok(total: u128, avail: u128, threshold_percent: u8) -> bool {
    if total == 0 {
        return true;
    }
    (avail * 100 / total) as u8 >= threshold_percent
}

/// Read filesystem free space and compare it to `threshold_percent`. `None`
/// means the platform/filesystem could not report usage.
#[cfg(unix)]
pub(super) fn disk_free_ok(path: &std::path::Path, threshold_percent: u8) -> Option<bool> {
    use std::ffi::CString;
    let c = CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
    // SAFETY: the buffer is zero-initialized and `c` is a valid NUL-terminated
    // path; `statvfs` only writes into the provided struct.
    unsafe {
        let mut stat: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c.as_ptr(), &raw mut stat) != 0 {
            return None;
        }
        let block = u128::from(stat.f_frsize.max(1));
        let total = block.checked_mul(u128::from(stat.f_blocks))?;
        let avail = block.checked_mul(u128::from(stat.f_bavail))?;
        Some(free_percent_ok(total, avail, threshold_percent))
    }
}

#[cfg(not(unix))]
pub(super) fn disk_free_ok(_path: &std::path::Path, _threshold_percent: u8) -> Option<bool> {
    None
}

/// Advance a counter to a cumulative total read from the engine. Totals only
/// grow; a smaller reading (never expected) leaves the counter unchanged.
fn advance(counter: &prometheus::IntCounter, total: i64) {
    let total = u64::try_from(total).unwrap_or(0);
    let current = counter.get();
    if total > current {
        counter.inc_by(total - current);
    }
}
