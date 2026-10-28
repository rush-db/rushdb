//! HTTP adapter for `RushDB`'s transport-neutral operation protocol.
//! Axum/Hyper own HTTP parsing, connection handling, limits, and tracing.

use std::net::TcpListener;
use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::node::Node;
use crate::retention::{over_rss_limit, MaintenanceConfig, RetentionPolicy};
use crate::{ApiError, ServerConfig};

/// P11 output budget: a successful response larger than this is rejected with
/// an explicit error rather than silently truncated. This bounds slow-client
/// and oversized-result memory at the transport boundary.
/// P11 output budget is `ServerConfig::max_request_bytes` (see `bounded_response`).

#[derive(Clone)]
struct AppState {
    cfg: ServerConfig,
    node: Arc<Node>,
    /// Node-wide budget for request bodies being read or executed.
    inflight_bodies: rushdb_storage::resources::ByteBudget,
    /// Bound on concurrently admitted protocol requests (scheduler workers +
    /// queue). Each admitted request holds a blocking-pool thread while it
    /// waits for the scheduler, so the bound also caps those threads; excess
    /// requests are shed immediately with a retryable 503.
    inflight_requests: Arc<tokio::sync::Semaphore>,
}

/// A bound HTTP protocol adapter: listener, router, and background workers.
pub struct HttpRouter {
    /// Resolved listen address (ephemeral port 0 becomes the assigned port).
    addr: std::net::SocketAddr,
    /// Pre-bound listener for the plaintext path (None when TLS is enabled —
    /// `axum-server` binds its own listener).
    listener: Option<TcpListener>,
    app: Router,
    node: std::sync::Weak<Node>,
    /// PEM certificate chain + private key paths when HTTPS is enabled.
    tls: Option<(std::path::PathBuf, std::path::PathBuf)>,
    /// Growth/memory maintenance policy resolved from `resources`.
    maintenance: MaintenanceConfig,
    /// HTTP worker threads; 0 = adapter default.
    threads: usize,
    /// History/GC sweep period (checkpoint interval).
    checkpoint_interval: std::time::Duration,
}

/// Boot a node from `cfg` and bind the HTTP adapter to its configured address.
///
/// # Errors
///
/// Returns a message when the node cannot bootstrap (for example an invalid
/// data directory) or the listener cannot bind the configured address.
pub fn new_router(mut cfg: ServerConfig) -> Result<(HttpRouter, u16), String> {
    let node = Arc::new(Node::bootstrap(&mut cfg)?);
    bind_router(cfg, node)
}

/// Bind the HTTP adapter around an already-bootstrapped node. `new_router`
/// is the usual entry point (bootstraps its own node); this is exposed
/// separately for callers that need their own handle to the same node
/// alongside the bound server (e.g. a test driving the node directly while
/// also exercising its HTTP surface).
///
/// # Errors
/// Returns a message when the listener cannot bind the configured address.
pub fn bind_router(cfg: ServerConfig, node: Arc<Node>) -> Result<(HttpRouter, u16), String> {
    let listener =
        TcpListener::bind((cfg.listen_addr.as_str(), cfg.port)).map_err(|e| e.to_string())?;
    bind_router_with_listener(cfg, node, listener)
}

/// [`bind_router`] over an already-bound socket — the one a
/// [`crate::boot::BootListener`] answered probes on while the node booted, so
/// the port is never closed between boot and serving.
///
/// # Errors
/// Returns a message when the socket cannot be configured.
pub fn bind_router_with_listener(
    cfg: ServerConfig,
    node: Arc<Node>,
    listener: TcpListener,
) -> Result<(HttpRouter, u16), String> {
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let port = addr.port();
    let weak_node = Arc::downgrade(&node);
    let max_request_bytes = cfg.max_request_bytes;
    let tls = match (&cfg.tls_cert, &cfg.tls_key) {
        (Some(cert), Some(key)) => Some((cert.clone(), key.clone())),
        _ => None,
    };
    let maintenance = cfg.resources.maintenance(&cfg.token);
    let threads = cfg.resources.threads;
    let checkpoint_interval = cfg.resources.checkpoint_interval();
    let inflight_bodies = rushdb_storage::resources::ByteBudget::new(
        "http-inflight-bodies",
        cfg.resources
            .http_inflight_body_bytes
            .0
            .max(cfg.max_request_bytes),
    )
    .map_err(|e| e.to_string())?;
    let (workers, queue) = cfg.resources.scheduler_shape();
    let state = AppState {
        cfg,
        node,
        inflight_bodies,
        inflight_requests: Arc::new(tokio::sync::Semaphore::new(workers + queue)),
    };
    let app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/metrics", get(metrics))
        .route("/protocol/v1/execute", post(execute))
        .route("/internal/replication/wal", get(replication_wal_tail))
        .route("/internal/replication/promote", post(replication_promote))
        .route("/internal/backup/base", post(backup_base_online))
        .route("/internal/usage", post(tenant_usage))
        .fallback(not_found)
        .layer(RequestBodyLimitLayer::new(max_request_bytes))
        // The `Bytes` extractor (and any body-reading extractor) is gated by
        // axum's `DefaultBodyLimit`, which defaults to 2MB — independent of the
        // tower-http layer above. Without raising it, batch imports >2MB were
        // rejected with 413 "length limit exceeded" despite the configured
        // body limit (`server.body-limit`, 16 MiB by default).
        .layer(DefaultBodyLimit::max(max_request_bytes))
        .layer(TraceLayer::new_for_http())
        .with_state(state);
    Ok((
        HttpRouter {
            addr,
            listener: Some(listener),
            app,
            node: weak_node,
            tls,
            maintenance,
            threads,
            checkpoint_interval,
        },
        port,
    ))
}

impl HttpRouter {
    /// Return the shared stop signal for all transports and maintenance work.
    pub fn shutdown_handle(&self) -> impl Fn() + 'static {
        let node = self.node.clone();
        move || {
            if let Some(node) = node.upgrade() {
                node.store().begin_shutdown();
            }
        }
    }
    /// Serve HTTP(S) until an unrecoverable error stops the listener, running
    /// the transaction reaper, GC, and optional compaction/watchdog workers.
    ///
    /// # Panics
    ///
    /// Panics if the multi-threaded Tokio runtime cannot be built, if the TLS
    /// certificate/key files cannot be loaded, or if the pre-bound listener
    /// cannot be adopted by Tokio.
    #[expect(
        clippy::too_many_lines,
        reason = "serve lifecycle keeps listener, maintenance task ownership, cancellation and final checkpoint together"
    )]
    /// # Errors
    /// Returns listener, checkpoint, or bounded shutdown failures.
    pub fn serve_forever(&mut self) -> Result<(), String> {
        let Some(listener) = self.listener.take() else {
            return Ok(());
        };
        let app = self.app.clone();
        let weak_node = self.node.clone();
        let tls = self.tls.clone();
        let self_addr = self.addr;
        let maintenance = self.maintenance.clone();
        let threads = self.threads;
        let checkpoint_interval = self.checkpoint_interval;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(if threads == 0 {
                std::thread::available_parallelism().map_or(4, std::num::NonZero::get)
            } else {
                threads.max(1)
            })
            .build()
            .expect("HTTP runtime");
        let outcome = runtime.block_on(async move {
            let reaper =
                spawn_transaction_reaper(weak_node.clone(), std::time::Duration::from_secs(1));
            // Continuous incremental GC: bounded, resumable history sweeps and
            // policy-driven per-commit metadata retention.
            let gc = spawn_gc_worker(
                weak_node.clone(),
                checkpoint_interval,
                512,
                maintenance.retention,
                maintenance.keep_commits,
                maintenance.retention_bytes,
            );
            // Optional idle-window compaction (opt-in): operators choose when
            // the rewrite cost is acceptable.
            let compaction = maintenance.compact_interval.map(|period| {
                spawn_compaction_worker(
                    weak_node.clone(),
                    period,
                    maintenance.compact_quiesce,
                    maintenance.compact_growth_percent,
                )
            });
            // Durable byte accounting is refreshed off the request path so a
            // `/metrics` scrape never blocks on a full keyspace scan. The scan
            // is O(keyspace) CPU/IO, so on a large store it runs infrequently.
            let storage_stats =
                spawn_storage_stats_worker(weak_node.clone(), std::time::Duration::from_mins(15));
            // Index applier: applies pending text and structural
            // index changes with a ≤ 250 ms linger; idle tenants cost one
            // catalog read and never commit.
            let index_applier =
                spawn_index_applier(weak_node.clone(), std::time::Duration::from_millis(250));
            // Declared-index builds run continuously while any is pending.
            let index_builder = spawn_index_builder(weak_node.clone(), checkpoint_interval);
            // Continuous logical-WAL archiving: append-only segment files
            // plus a durable watermark, for off-box durability and PITR.
            let wal_archiver = maintenance.wal_archive_dir.clone().map(|dir| {
                spawn_wal_archiver(
                    weak_node.clone(),
                    std::time::Duration::from_secs(5),
                    dir,
                    maintenance.wal_archive_retention,
                    maintenance.wal_archive_retention_bytes,
                )
            });
            // Streaming replication follower: tails a
            // configured leader's WAL and replays it locally.
            let replication_follower = maintenance.replica.clone().map(|replica| {
                tokio::spawn(crate::replication::run_replication_follower(
                    weak_node.clone(),
                    replica,
                ))
            });
            // Kept for the shutdown path (the watchdog below consumes the other
            // handle).
            let shutdown_node = weak_node.clone();
            // Optional RSS watchdog (opt-in): when resident memory exceeds the
            // threshold, reclaim allocator pages and compact if idle.
            let watchdog = maintenance.rss_max_bytes.map(|limit| {
                spawn_memory_watchdog(
                    weak_node,
                    std::time::Duration::from_secs(10),
                    limit,
                    maintenance.compact_quiesce,
                )
            });
            let addr = self_addr;
            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
            let tls_handle = axum_server::Handle::new();
            let tls_handle_for_server = tls_handle.clone();
            // Serve until an unrecoverable error OR a shutdown signal. On a
            // signal we fall through and drop the node, which closes the redb
            // file cleanly — an unclean kill makes redb repair (O(file)) on the
            // next open.
            let server = async move {
                if let Some((cert, key)) = tls {
                    tracing::debug!("HTTPS enabled on {addr}");
                    let config = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert, key)
                        .await
                        .expect("load TLS certificate/key");
                    axum_server::tls_rustls::from_tcp_rustls(listener, config)
                        .handle(tls_handle_for_server)
                        .serve(app.into_make_service())
                        .await.map_err(|error| error.to_string())
                } else {
                    let listener =
                        tokio::net::TcpListener::from_std(listener).expect("HTTP listener");
                    axum::serve(listener, app)
                        .with_graceful_shutdown(async {
                            let _ = shutdown_rx.await;
                        })
                        .await.map_err(|error| error.to_string())
                }
            };
            tokio::pin!(server);
            let stop = async {
                tokio::select! {
                    () = shutdown_signal() => {},
                    () = async {
                        loop {
                            if shutdown_node.upgrade().is_none_or(|node| node.store().maintenance_aborted()) { break; }
                            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                        }
                    } => {},
                }
            };
            let mut outcome = tokio::select! {
                result = &mut server => result,
                () = stop => {
                    tracing::info!("shutdown signal received; closing the store cleanly");
                    // Stop in-flight maintenance scans so the store closes
                    // promptly instead of waiting for a full keyspace scan.
                    if let Some(node) = shutdown_node.upgrade() {
                        node.store().begin_shutdown();
                    }
                    let _ = shutdown_tx.send(());
                    tls_handle.graceful_shutdown(Some(std::time::Duration::from_secs(30)));
                    tokio::time::timeout(std::time::Duration::from_secs(30), &mut server)
                        .await.unwrap_or_else(|_| Err("HTTP transport drain deadline elapsed".into()))
                }
            };
            reaper.abort();
            gc.abort();
            storage_stats.abort();
            index_applier.abort();
            index_builder.abort();
            if let Some(archiver) = wal_archiver.as_ref() {
                archiver.abort();
            }
            if let Some(follower) = replication_follower.as_ref() {
                follower.abort();
            }
            if let Some(compaction) = compaction.as_ref() {
                compaction.abort();
            }
            if let Some(watchdog) = watchdog.as_ref() {
                watchdog.abort();
            }
            // Make shutdown deterministic: cancel maintenance, wait for every
            // task to observe cancellation, then flush buffered durability
            // before the owning Node/runtime is dropped.
            if let Some(node) = shutdown_node.upgrade() {
                node.store().begin_shutdown();
                // Aborting the loops above does not stop a closure already on
                // the blocking pool. Wait for every one to return (they observe
                // `begin_shutdown`) so no detached closure still holds the
                // Store when the runtime exits; otherwise redb is never closed
                // and the next open runs a full repair.
                if !node.maintenance_work().drained(std::time::Duration::from_secs(30)).await {
                    outcome = Err(format!(
                        "{} maintenance task(s) still running at the shutdown deadline",
                        node.maintenance_work().inflight()
                    ));
                }
                let checkpoint = tokio::task::spawn_blocking(move || node.store().checkpoint());
                match tokio::time::timeout(std::time::Duration::from_secs(5), checkpoint).await {
                    Ok(Ok(Ok(()))) => {},
                    failed => { outcome = Err(format!("final durability checkpoint failed: {failed:?}")); }
                }
            }
            let _ = reaper.await;
            let _ = gc.await;
            let _ = storage_stats.await;
            let _ = index_applier.await;
            let _ = index_builder.await;
            if let Some(archiver) = wal_archiver {
                let _ = archiver.await;
            }
            if let Some(follower) = replication_follower {
                let _ = follower.await;
            }
            if let Some(compaction) = compaction {
                let _ = compaction.await;
            }
            if let Some(watchdog) = watchdog {
                let _ = watchdog.await;
            }
            outcome
        });
        runtime.shutdown_timeout(std::time::Duration::from_secs(1));
        outcome
    }
}

pub(crate) fn spawn_transaction_reaper(
    node: std::sync::Weak<Node>,
    period: std::time::Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(period);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let Some(node) = node.upgrade() else {
                break;
            };
            // One sweep at a time, off the async executor; do not retain a
            // strong node reference while sleeping or create per-tenant tasks.
            if let Err(error) = node
                .maintenance_work()
                .clone()
                .spawn(move || node.reap_expired_transactions())
                .await
            {
                tracing::error!(%error, "transaction reaper stopped");
                break;
            }
        }
    })
}

/// Continuous incremental GC (W5, D8): a background loop drives bounded,
/// resumable `retire_before_batch` sweeps so history reclamation never
/// performs one monolithic redb txn. Each tick is a small write transaction
/// and a full sweep ends with a published per-shard checkpoint. The same tick
/// drives bounded per-commit metadata retention (command-log rows and outcome
/// payloads) so the durable file tracks live data instead of commit count.
pub(crate) fn spawn_gc_worker(
    node: std::sync::Weak<Node>,
    period: std::time::Duration,
    max_keys_per_tick: usize,
    retention: RetentionPolicy,
    keep_commits: u64,
    retention_bytes: Option<u64>,
) -> tokio::task::JoinHandle<()> {
    let sweep_budget = period / 5;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(period);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let Some(node) = node.upgrade() else {
                break;
            };
            if let Some(acknowledged) = node.store().replication_acknowledged_commit() {
                if let Err(error) = node.acknowledge_retention_consumer("replica", acknowledged) {
                    tracing::error!(%error, "replication acknowledgement fell behind the retention cap");
                }
            }
            let result = node.maintenance_work().clone().spawn(move || {
                let tick_started = std::time::Instant::now();
                let observe_node = std::sync::Arc::clone(&node);
                let _observe = scopeguard_tick(move || {
                    observe_node.observe_gc_tick(tick_started.elapsed().as_secs_f64());
                });
                // Pull the store off the node's store and run one GC tick,
                // expiring abandoned read leases so history can advance.
                let store = node.store();
                // Each step is timed (`rushdb_maintenance_step_seconds`), so a
                // long tick names the step that held the writer.
                let step = std::time::Instant::now();
                store.expire_read_pins(node.read_lease_ms());
                // Planner statistics unused past their idle TTL go.
                store.planner_stats().evict_idle();
                // Freeze and merge ANN segments off the write path.
                let step = node.observe_maintenance_step("read_pins", step);
                node.maintain_vector_segments();
                let step = node.observe_maintenance_step("vector_segments", step);
                // Records-per-tenant gauge source: label counters, so cheap
                // enough to refresh twice a minute.
                node.refresh_records_by_tenant(std::time::Duration::from_secs(30));
                // C5: a consumer that stopped acknowledging (dead backup job,
                // removed replica, disabled archiver) must not pin the WAL.
                node.expire_retention_consumers();
                // Read-pin expiry must happen before sampling this floor;
                // otherwise one abandoned view can delay metadata pruning for
                // an extra maintenance interval.
                //
                // Tenants are enumerated ONCE per tick (an O(tenants)
                // skip-scan) and shared by every per-tenant step below.
                let tenants = match node.durable_tenants() {
                    Ok(tenants) => Some(tenants),
                    Err(error) => {
                        tracing::warn!(%error, "cannot enumerate tenants; protecting full history this tick");
                        None
                    }
                };
                let protected_floor = node.protected_retention_floor();
                // History reclamation in batches until the sweep completes or
                // a fifth of the tick period has passed.
                let step = node.observe_maintenance_step("housekeeping", step);
                let done = store.gc_tick_for(max_keys_per_tick, sweep_budget)?;
                let step = node.observe_maintenance_step("history_gc", step);
                // Retention runs after history GC so the sweep sees the newest
                // watermark. The commit floor is fixed for this tick; the loop
                // drains the bounded backlog. A byte target additionally prunes
                // the oldest command-log rows until the log fits.
                let floor = retention.capped_floor(
                    store.committed_version(),
                    keep_commits,
                    retention_bytes
                        .map(|max| store.log_byte_floor(max))
                        .transpose()?,
                    protected_floor,
                );
                // The caps are hard. Consumers the floor passed are
                // lost and must resync.
                node.enforce_retention_floor(floor);
                let mut pruned = 0usize;
                loop {
                    if store.maintenance_aborted() {
                        return Ok(done);
                    }
                    let n = store.retire_commit_metadata_before(floor, max_keys_per_tick)?;
                    pruned += n;
                    if n == 0 {
                        break;
                    }
                }
                if pruned > 0 {
                    tracing::debug!(pruned, "retention advanced past retired commits");
                }
                let step = node.observe_maintenance_step("retention", step);
                // Retry receipts past the idempotency window: batches of
                // `max_keys_per_tick` until the backlog is gone or a fifth
                // of the tick period has passed (SOAK-1: a fixed batch per
                // tick fell behind sustained writes).
                let swept = store.sweep_expired_receipts(max_keys_per_tick, sweep_budget)?;
                if swept > 0 {
                    tracing::debug!(swept, "expired retry receipts removed");
                }
                let step = node.observe_maintenance_step("receipts", step);
                // Reclaim embedding slots superseded below the history floor.
                // Their bytes were retained only so pinned views could read the
                // historical binding; once the floor passes that version they
                // are garbage. Best-effort: a failure must not stop GC.
                if let Some(tenants) = tenants.as_deref() {
                    if let Err(error) = node.reclaim_vector_slots_for(tenants, store.gc_watermark()) {
                        tracing::warn!(%error, "embedding-slot reclamation failed");
                    }
                }
                let step = node.observe_maintenance_step("vector_slots", step);
                // The periodic checkpoint is the only point at which redb's
                // materialization becomes durable (commits are WAL-fsynced and
                // applied to redb with `Durability::None`), and the sole
                // trigger for WAL segment reclamation (`truncate_log`, clamped to the retention cursor this tick's
                // sweep just advanced).
                if let Err(error) = store.checkpoint() {
                    tracing::warn!(%error, "durability checkpoint failed");
                }
                node.observe_maintenance_step("checkpoint", step);
                Ok::<_, rushdb_storage::StorageError>(done)
            })
            .await;
            match result {
                Ok(Ok(_done)) => {}
                Ok(Err(error)) => {
                    tracing::warn!(%error, "incremental GC tick failed");
                }
                Err(error) => {
                    tracing::error!(%error, "incremental GC worker stopped");
                    break;
                }
            }
        }
    })
}

/// Optional idle-window compaction. Disabled unless
/// `RUSHDB_COMPACT_INTERVAL_SECONDS` is a positive integer; then a background
/// loop calls `Store::compact_if_needed` every `period` (the ticker is the
/// rate limit), which defers while commits are flowing (quiescence) and only
/// rewrites the file once it has grown by `compact.growth_percent` or enough
/// keys were purged. Compaction reclaims
/// the copy-on-write high-water mark that retention cannot, and is logged with
/// before/after file size.
pub(crate) fn spawn_compaction_worker(
    node: std::sync::Weak<Node>,
    period: std::time::Duration,
    quiesce: std::time::Duration,
    growth_percent: u32,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(period);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let Some(node) = node.upgrade() else {
                break;
            };
            let result = node
                .maintenance_work()
                .clone()
                .spawn(move || {
                    let store = node.store();
                    let before = store.storage_file_bytes();
                    match store.compact_if_needed(quiesce, growth_percent) {
                        Ok(rushdb_storage::CompactOutcome::Compacted(ci)) => {
                            let after = store.storage_file_bytes();
                            if let (Some(before), Some(after)) = (before, after) {
                                tracing::info!(
                                    commit_index = ci,
                                    before_bytes = before,
                                    after_bytes = after,
                                    "storage compaction complete"
                                );
                            }
                        }
                        // Commits flowing, a pinned reader, or below the
                        // thresholds: retry next tick.
                        Ok(deferred) => {
                            tracing::debug!(reason = ?deferred, "compaction not run");
                        }
                        // Includes readers that outlived the backend's drain
                        // wait; the next tick retries.
                        Err(error) => {
                            tracing::info!(%error, "compaction deferred");
                        }
                    }
                })
                .await;
            if let Err(error) = result {
                tracing::error!(%error, "compaction worker stopped");
                break;
            }
        }
    })
}

/// Optional RSS watchdog. When resident memory exceeds `rss_max_bytes`, log a
/// warning and reclaim: a bounded GC tick plus a compaction under the same
/// quiescence gate as the compaction worker (never while commits flow).
/// This is a policy signal, never a hard allocation cap.
pub(crate) fn spawn_memory_watchdog(
    node: std::sync::Weak<Node>,
    period: std::time::Duration,
    rss_max_bytes: u64,
    quiesce: std::time::Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(period);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let Some(node) = node.upgrade() else {
                break;
            };
            let resident = crate::allocator::stats().map(|stats| stats.resident as u64);
            if !over_rss_limit(resident, rss_max_bytes) {
                continue;
            }
            let result = node
                .maintenance_work()
                .clone()
                .spawn(move || {
                    let store = node.store();
                    tracing::warn!(
                        resident_bytes = resident.unwrap_or(0),
                        limit_bytes = rss_max_bytes,
                        "resident memory over watchdog limit; reclaiming"
                    );
                    let _ = store.expire_read_pins(node.read_lease_ms());
                    let _ = store.gc_tick(512);
                    watchdog_compact(&store, quiesce);
                })
                .await;
            if let Err(error) = result {
                tracing::error!(%error, "memory watchdog stopped");
                break;
            }
        }
    })
}

/// The watchdog's compaction: any growth qualifies (memory pressure is the
/// trigger), but quiescence still applies, so it never rewrites the file
/// while commits are flowing.
pub(crate) fn watchdog_compact(
    store: &rushdb_storage::Store,
    quiesce: std::time::Duration,
) -> Option<rushdb_storage::CompactOutcome> {
    match store.compact_if_needed(quiesce, 100) {
        Ok(outcome) => {
            tracing::debug!(?outcome, "watchdog compaction");
            Some(outcome)
        }
        Err(error) => {
            tracing::debug!(%error, "watchdog compaction deferred");
            None
        }
    }
}

/// Continuously archive committed WAL segments into `dir`. Each tick
/// writes the range since the durable watermark as one append-only segment.
pub(crate) fn spawn_wal_archiver(
    node: std::sync::Weak<Node>,
    period: std::time::Duration,
    dir: std::path::PathBuf,
    retention: std::time::Duration,
    retention_bytes: Option<u64>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(period);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let Some(node) = node.upgrade() else {
                break;
            };
            let dir = dir.clone();
            let archive_node = std::sync::Arc::clone(&node);
            let result = node
                .maintenance_work()
                .clone()
                .spawn(move || {
                    let archived = archive_node.archive_wal(&dir)?;
                    // Archive retention, after the new segment landed.
                    let pruned = crate::archive::prune_archive(
                        &dir,
                        crate::retention::unix_ms(),
                        retention,
                        retention_bytes,
                    )?;
                    if pruned.forced > 0 {
                        tracing::warn!(
                            segments = pruned.forced,
                            "WAL archive byte cap removed segments inside the retention window"
                        );
                    }
                    Ok::<_, String>(archived)
                })
                .await;
            match result {
                Ok(Ok(Some(lsn))) => {
                    // The archive watermark is durable before this acknowledgement.
                    // Retention must therefore keep command metadata through the
                    // covered LSN until an operator releases the consumer.
                    match node.acknowledge_retention_consumer("wal-archive", lsn) {
                        Ok(()) => tracing::debug!(lsn, "archived WAL segment"),
                        // The archive has a gap: PITR across it is impossible
                        // until a new base backup is taken.
                        Err(error) => {
                            tracing::error!(%error, "WAL archive fell behind the retention cap");
                        }
                    }
                }
                Ok(Ok(None)) => {}
                Ok(Err(error)) => tracing::warn!(%error, "WAL archiving failed"),
                Err(error) => {
                    tracing::error!(%error, "WAL archiver stopped");
                    break;
                }
            }
        }
    })
}

/// Resolve when the process receives SIGTERM or SIGINT, so the server can shut
/// down cleanly (drop the store, close redb) instead of dying mid-write and
/// forcing an O(file) repair on the next open.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let Ok(mut term) = signal(SignalKind::terminate()) else {
            let _ = tokio::signal::ctrl_c().await;
            return;
        };
        let Ok(mut interrupt) = signal(SignalKind::interrupt()) else {
            let _ = tokio::signal::ctrl_c().await;
            return;
        };
        tokio::select! {
            _ = term.recv() => {}
            _ = interrupt.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// Refresh the cached durable byte accounting off the request path. The scan is
/// O(keyspace) — on a multi-GB store it takes tens of seconds — so it must never
/// run during a `/metrics` scrape. `/metrics` serves the last snapshot.
pub(crate) fn spawn_storage_stats_worker(
    node: std::sync::Weak<Node>,
    period: std::time::Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // First refresh one period after start, not at boot: startup already
        // pays the open/replay/rebuild cost and must not add a storage walk.
        let mut interval = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let Some(node) = node.upgrade() else {
                break;
            };
            // The scan is CPU/IO heavy and runs in a blocking pool; a scrape
            // always reads the previously cached snapshot immediately.
            let result = node
                .maintenance_work()
                .clone()
                .spawn(move || {
                    // Durable byte accounting. (The per-tenant record-count scan is
                    // not scheduled: it walks every key in the tenant keyspace,
                    // which is prohibitively expensive on a large shared store.)
                    node.refresh_storage_stats(period);
                })
                .await;
            if result.is_err() {
                tracing::error!("storage-stats worker stopped");
                break;
            }
        }
    })
}

/// Text applier: every `period`, applies pending changes of every
/// tenant's text indexes. Idempotent; a tenant without pending changes never
/// commits, so it converges freshness without a
/// per-mutation cost.
/// Declared-index builder. Woken when an index is declared or dropped (and
/// every `fallback` period, which also resumes builds pending at startup),
/// it advances builds in bounded steps back to back until none is pending:
/// a build runs at the speed of the work, not of a maintenance tick. Each
/// step commits at most one batch, so writers interleave between steps.
pub(crate) fn spawn_index_builder(
    node: std::sync::Weak<Node>,
    fallback: std::time::Duration,
) -> tokio::task::JoinHandle<()> {
    let wake = node.upgrade().map(|node| node.index_build_wake());
    tokio::spawn(async move {
        let Some(wake) = wake else { return };
        let mut interval = tokio::time::interval(fallback);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                () = wake.notified() => {}
                _ = interval.tick() => {}
            }
            loop {
                let Some(node) = node.upgrade() else { return };
                let result = node
                    .maintenance_work()
                    .clone()
                    .spawn(move || node.step_index_builds(INDEX_BUILD_STEP))
                    .await;
                match result {
                    Ok(0) => break,
                    Ok(_) => tokio::task::yield_now().await,
                    Err(_) => {
                        tracing::error!("index builder stopped");
                        return;
                    }
                }
            }
        }
    })
}

/// Longest uninterrupted stretch of index building before the builder
/// yields (it resumes immediately while work remains).
const INDEX_BUILD_STEP: std::time::Duration = std::time::Duration::from_millis(250);

pub(crate) fn spawn_index_applier(
    node: std::sync::Weak<Node>,
    period: std::time::Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(period);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let Some(node) = node.upgrade() else {
                break;
            };
            // Projection commits are blocking store writes; keep them off the
            // async runtime's worker threads.
            let result = node
                .maintenance_work()
                .clone()
                .spawn(move || {
                    node.apply_text_indexes(period);
                    node.apply_structural_indexes(period);
                })
                .await;
            if result.is_err() {
                tracing::error!("text applier stopped");
                break;
            }
        }
    })
}

async fn health(State(state): State<AppState>) -> Response {
    // Reflect the gated LanguageSet so /health agrees with describe.capabilities:
    // only enabled statement languages are advertised (searchquery is always on).
    let mut languages: Vec<String> = state
        .cfg
        .languages
        .advertised()
        .iter()
        .map(|l| l.as_str().to_string())
        .collect();
    languages.push("searchQuery".to_string());
    languages.sort();
    let (state, http_status) = if state.cfg.maintenance {
        ("maintenance", StatusCode::SERVICE_UNAVAILABLE)
    } else {
        ("ok", StatusCode::OK)
    };
    json(
        http_status,
        &serde_json::json!({
            "status": state,
            "engine": "rushdb-lmpg",
            "protocolVersion": rushdb_protocol::PROTOCOL_VERSION,
            "queryLanguages": languages,
        }),
    )
}

/// Readiness probe (distinct from `/health` liveness): reports 200 only when
/// the store is serving and the configured data directory (if any) is still
/// writable, so orchestration can stop routing traffic before a node that can
/// no longer persist commits. In-memory nodes are ready once the store is open.
async fn ready(State(state): State<AppState>) -> Response {
    if state.cfg.maintenance {
        return json(
            StatusCode::SERVICE_UNAVAILABLE,
            &serde_json::json!({
                "status": "maintenance",
                "reason": "maintenance mode",
            }),
        );
    }
    if state.node.store().maintenance_aborted() {
        return json(
            StatusCode::SERVICE_UNAVAILABLE,
            &serde_json::json!({
                "status": "not_ready",
                "reason": "store is shutting down",
            }),
        );
    }
    if let Some(error) = state.node.store().publisher_failure() {
        return json(
            StatusCode::SERVICE_UNAVAILABLE,
            &serde_json::json!({
                "status": "not_ready",
                "reason": "durable publisher failed",
                "error": error,
            }),
        );
    }
    let commit_index = state.node.store().committed_version();
    let data_dir = state.cfg.data_dir.clone();
    let mut writable = true;
    if let Some(dir) = &data_dir {
        writable = std::fs::write(dir.join(".rushdb-ready"), b"ok").is_ok();
        let _ = std::fs::remove_file(dir.join(".rushdb-ready"));
    }
    if !writable {
        return json(
            StatusCode::SERVICE_UNAVAILABLE,
            &serde_json::json!({
                "status": "not_ready",
                "reason": "data directory is not writable",
            }),
        );
    }
    if !state.node.disk_space_ok() {
        return json(
            StatusCode::SERVICE_UNAVAILABLE,
            &serde_json::json!({
                "status": "not_ready",
                "reason": "free disk below threshold",
                "diskSpaceThresholdPercent": state.node.disk_space_threshold_percent(),
            }),
        );
    }
    if state.cfg.require_index_warmup && !state.node.indexes_warmed() {
        return json(
            StatusCode::SERVICE_UNAVAILABLE,
            &serde_json::json!({
                "status": "not_ready",
                "reason": "vector indexes are still warming",
                "commitIndex": commit_index,
            }),
        );
    }
    json(
        StatusCode::OK,
        &serde_json::json!({
            "status": "ready",
            "commitIndex": commit_index,
            "dataDir": data_dir.map(|d| d.display().to_string()),
        }),
    )
}

async fn metrics(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers, "read") {
        return json(
            StatusCode::UNAUTHORIZED,
            &serde_json::json!({"error":"unauthorized"}),
        );
    }
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        {
            state
                .node
                .observe_inflight_body_bytes(state.inflight_bodies.snapshot().used);
            state.node.metrics_render()
        },
    )
        .into_response()
}

/// Streaming replication: internal node-to-node endpoint, not
/// part of the client-facing protocol surface. A follower polls this with
/// its own identity and resume LSN; see [`crate::replication`].
async fn replication_wal_tail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    if !operator_token(&state, &headers) {
        return json(
            StatusCode::UNAUTHORIZED,
            &serde_json::json!({"error":"unauthorized"}),
        );
    }
    let follower_id = params.get("follower_id").map_or("", String::as_str);
    if follower_id.is_empty() {
        return json(
            StatusCode::BAD_REQUEST,
            &serde_json::json!({"error":"follower_id is required"}),
        );
    }
    let since: u64 = params
        .get("since")
        .and_then(|value| value.parse().ok())
        .unwrap_or(1);
    match crate::replication::build_wal_tail_response(&state.node, follower_id, since) {
        Ok(response) => json(
            StatusCode::OK,
            &serde_json::to_value(&response).unwrap_or(serde_json::Value::Null),
        ),
        Err(crate::replication::WalTailError::Resync(resync)) => json(
            StatusCode::CONFLICT,
            &serde_json::json!({"error": {
                "code": "resync_required",
                "message": resync.to_string(),
                "oldestAvailableLsn": resync.oldest_available_lsn,
            }}),
        ),
        Err(crate::replication::WalTailError::Read(error)) => json(
            StatusCode::INTERNAL_SERVER_ERROR,
            &serde_json::json!({"error": error}),
        ),
    }
}

/// Streaming replication: stop following and start
/// accepting local writes. Asynchronous on the follower's side — the
/// replication task clears `is_following` itself once it actually stops
/// (see `Node::finish_following`'s doc comment for why), not this call —
/// so this endpoint requests it and waits up to a few seconds for that to
/// happen before responding, rather than returning a bare "accepted" that
/// leaves the caller unsure whether writes are actually safe yet. No
/// safeguard against a concurrently-reachable old leader (split-brain):
/// that is an operator decision this mechanism deliberately does not
/// make for them (this is not a consensus system).
/// Whether the request carries the boot (operator) token, in
/// `X-RushDB-Token` or as a bearer token.
fn operator_token(state: &AppState, headers: &HeaderMap) -> bool {
    let token = header(headers, "x-rushdb-token");
    let token = if token.is_empty() {
        header(headers, "authorization")
            .strip_prefix("Bearer ")
            .unwrap_or("")
    } else {
        token
    };
    rushdb_protocol::constant_time_eq(token.as_bytes(), state.cfg.token.as_bytes())
}

/// Body of `POST /internal/usage`.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct UsageRequest {
    #[serde(default)]
    tenants: Vec<String>,
}

/// Usage of a list of tenants (operator only, USE-1): `{"tenants": [scope,
/// ...]}` in multitenant mode, `{}` in single-tenant mode. A cross-tenant
/// read, so it takes the operator token, never a request tenant scope.
async fn tenant_usage(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !operator_token(&state, &headers) {
        return json(
            StatusCode::UNAUTHORIZED,
            &serde_json::json!({"error":"unauthorized"}),
        );
    }
    let request: UsageRequest = if body.is_empty() {
        UsageRequest {
            tenants: Vec::new(),
        }
    } else {
        match serde_json::from_slice(&body) {
            Ok(request) => request,
            Err(error) => {
                return json(
                    StatusCode::BAD_REQUEST,
                    &serde_json::json!({ "error": error.to_string() }),
                )
            }
        }
    };
    let node = Arc::clone(&state.node);
    match tokio::task::spawn_blocking(move || node.tenant_usage(&request.tenants)).await {
        Ok(Ok(usage)) => json(StatusCode::OK, &usage),
        Ok(Err(crate::node::UsageError::Invalid(error))) => json(
            StatusCode::BAD_REQUEST,
            &serde_json::json!({ "error": error }),
        ),
        Ok(Err(crate::node::UsageError::Store(error))) => json(
            StatusCode::INTERNAL_SERVER_ERROR,
            &serde_json::json!({ "error": error }),
        ),
        Err(error) => json(
            StatusCode::INTERNAL_SERVER_ERROR,
            &serde_json::json!({ "error": error.to_string() }),
        ),
    }
}

/// Online physical base backup of the running server (operator only): the
/// store is checkpointed and copied into `out` on the server's own
/// filesystem while writes continue. `out` must not exist yet.
async fn backup_base_online(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    if !operator_token(&state, &headers) {
        return json(
            StatusCode::UNAUTHORIZED,
            &serde_json::json!({"error":"unauthorized"}),
        );
    }
    let Some(out) = params.get("out").filter(|out| !out.is_empty()) else {
        return json(
            StatusCode::BAD_REQUEST,
            &serde_json::json!({"error":"out is required"}),
        );
    };
    let out = std::path::PathBuf::from(out);
    let node = Arc::clone(&state.node);
    let result = node
        .maintenance_work()
        .clone()
        .spawn(move || node.backup_base_online(&out))
        .await;
    match result {
        Ok(Ok(manifest)) => json(
            StatusCode::OK,
            &serde_json::json!({
                "commitIndex": manifest.commit_index,
                "files": manifest.files.len(),
            }),
        ),
        Ok(Err(error)) => json(StatusCode::CONFLICT, &serde_json::json!({ "error": error })),
        Err(error) => json(
            StatusCode::INTERNAL_SERVER_ERROR,
            &serde_json::json!({ "error": error.to_string() }),
        ),
    }
}

async fn replication_promote(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !operator_token(&state, &headers) {
        return json(
            StatusCode::UNAUTHORIZED,
            &serde_json::json!({"error":"unauthorized"}),
        );
    }
    if !state.node.is_following() {
        return json(
            StatusCode::OK,
            &serde_json::json!({"promoted": true, "already_writable": true}),
        );
    }
    state.node.request_promotion();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while state.node.is_following() {
        if std::time::Instant::now() >= deadline {
            return json(
                StatusCode::SERVICE_UNAVAILABLE,
                &serde_json::json!({
                    "error": "promotion requested but the replication task has not stopped yet"
                }),
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    // A restart must not follow again: drop the stored replica role.
    if let Some(dir) = state.cfg.data_dir.as_deref() {
        if let Err(error) = crate::config::persist_without_roles(dir, &state.cfg, false) {
            tracing::error!(%error, "promoted, but config.yml still names the leader");
            return json(
                StatusCode::OK,
                &serde_json::json!({ "promoted": true, "persisted": false, "error": error }),
            );
        }
    }
    json(
        StatusCode::OK,
        &serde_json::json!({"promoted": true, "persisted": true}),
    )
}

#[expect(
    clippy::too_many_lines,
    reason = "single request pipeline: auth, scope checks, slot acquisition, blocking execution, slow-call logging, and response mapping read best as one linear flow"
)]
async fn execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Body,
) -> Response {
    // C7 (cloud-capacity F8): authenticate from the headers before reading a
    // single body byte, so an unauthenticated caller cannot make the server
    // buffer up to the body limit. Fine-grained (read/write) authorization
    // needs the parsed operation and runs after the body is read.
    if !authenticated(&state, &headers) {
        return json(
            StatusCode::UNAUTHORIZED,
            &serde_json::json!({"error":"unauthorized"}),
        );
    }
    let scope = if state.node.multitenant() {
        let value = header(&headers, "x-rushdb-tenant").trim();
        if value.is_empty() {
            return coded_error(&rushdb_protocol::ProtocolError::new(
                rushdb_protocol::ErrorCode::MissingScope,
                "X-RushDB-Tenant is required in multitenant mode",
            ));
        }
        if value.len() > 1024 {
            return coded_error(&rushdb_protocol::ProtocolError::new(
                rushdb_protocol::ErrorCode::InvalidRequest,
                "X-RushDB-Tenant exceeds 1024 bytes",
            ));
        }
        Some(value.to_string())
    } else {
        None
    };
    let Ok(_admitted) = Arc::clone(&state.inflight_requests).try_acquire_owned() else {
        return coded_error(&rushdb_protocol::ProtocolError::new(
            rushdb_protocol::ErrorCode::Unavailable,
            "server at request capacity; retry shortly",
        ));
    };
    // Reserve the declared body size from the node-wide in-flight budget
    // before reading it; a body without a declared length reserves the limit.
    let max_body = state.cfg.max_request_bytes;
    let declared = headers
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok());
    if declared.is_some_and(|len| len > max_body) {
        return payload_too_large(max_body);
    }
    let Ok(_body_reservation) = state
        .inflight_bodies
        .try_reserve(declared.unwrap_or(max_body))
    else {
        return coded_error(&rushdb_protocol::ProtocolError::new(
            rushdb_protocol::ErrorCode::ResourceExhausted,
            "too many request bytes in flight; retry shortly",
        ));
    };
    let Ok(payload) = axum::body::to_bytes(body, max_body).await else {
        return payload_too_large(max_body);
    };
    let (permission, op_kind) = request_route(&payload);
    if !authorized(&state, &headers, permission) {
        return json(
            StatusCode::UNAUTHORIZED,
            &serde_json::json!({"error":"unauthorized"}),
        );
    }

    // A bulk request waits for its tenant's share before it takes a worker.
    let _bulk_permit = if op_kind.starts_with("ingest") {
        state
            .node
            .bulk_share()
            .acquire(scope.as_deref().unwrap_or_default())
            .await
    } else {
        None
    };
    let node = state.node.clone();
    let timeout_ms = state.cfg.request_timeout_ms;
    // Keep a stable label before the blocking task moves the request scope.
    let scope_label = scope.clone().unwrap_or_else(|| "-".to_string());
    let started = std::time::Instant::now();
    // A client that goes away drops this handler's future, and with it the
    // guard, which cancels the work it started: nobody waits for the answer.
    let disconnected = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let _cancel_on_disconnect = CancelOnDrop(Arc::clone(&disconnected));
    let result = tokio::task::spawn_blocking(move || {
        // Deadline-derived cancellation: long-running queries abort with a
        // structured Cancelled error instead of running to completion.
        let is_cancelled = move || {
            disconnected.load(std::sync::atomic::Ordering::Relaxed)
                || (timeout_ms > 0
                    && u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
                        >= timeout_ms)
        };
        let tenant = node.schedule_key(scope.as_deref());
        let node_for_job = Arc::clone(&node);
        node.schedule_for_tenant(tenant, move || {
            crate::protocol_http::execute_cancellable(
                &node_for_job,
                scope,
                &payload,
                &is_cancelled,
                (timeout_ms > 0).then_some(timeout_ms),
            )
        })
        .map_err(|error| {
            ApiError::Protocol(
                StatusCode::SERVICE_UNAVAILABLE.as_u16(),
                crate::protocol::scheduler_error_to_protocol_error(error),
            )
        })
        .and_then(std::convert::identity)
    })
    .await;
    let elapsed = started.elapsed();
    state.node.observe(op_kind, elapsed.as_secs_f64());
    // E4 stall observability: anything at or above the threshold gets a
    // warn line (op kind, scope, elapsed, rows touched, outcome) so benchmark
    // operators can tell slow from stuck without a second measurement channel.
    let elapsed_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    let slow_call_threshold_ms = state.node.slow_call_threshold_ms();
    if slow_call_threshold_ms > 0 && elapsed_ms >= slow_call_threshold_ms {
        let (ok, rows) = match &result {
            Ok(Ok(value)) => (true, response_row_count(value)),
            _ => (false, 0),
        };
        tracing::warn!(
            op = op_kind,
            scope = scope_label.as_str(),
            elapsed_ms,
            rows,
            ok,
            "slow protocol call"
        );
    }
    match result {
        Ok(Ok(value)) => bounded_response(StatusCode::OK, &value, state.cfg.max_request_bytes),
        Ok(Err(ApiError::Protocol(status, error))) => protocol_error(status_code(status), &error),
        // SEC-5: engine/scheduler faults outside the protocol taxonomy are
        // internal. Log the detail and return a stable generic client message.
        Ok(Err(ApiError::Engine(message))) => {
            tracing::error!(detail = %message, "suppressing internal engine error detail");
            internal_error_response()
        }
        Err(error) => {
            tracing::error!(detail = %error, "suppressing internal request error detail");
            internal_error_response()
        }
    }
}

/// Rows a successful response reported (records/rows arrays), for the
/// slow-call log. Bounded so a pathological response cannot inflate the log.
fn response_row_count(value: &serde_json::Value) -> u64 {
    const MAX: u64 = 1_000_000;
    value
        .get("response")
        .and_then(|response| response.get("records").or_else(|| response.get("rows")))
        .and_then(|rows| rows.as_array())
        .map_or(0, |rows| (rows.len() as u64).min(MAX))
}

/// P11: serialize a response value and reject it with an explicit error if it
/// exceeds the output budget. A successful response is NEVER silently
/// truncated; an oversized result returns 413 with a clear message.
fn bounded_response(status: StatusCode, value: &serde_json::Value, max_bytes: usize) -> Response {
    let encoded = value.to_string();
    if encoded.len() > max_bytes {
        return protocol_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            &rushdb_protocol::ProtocolError::new(
                rushdb_protocol::ErrorCode::ExecutionFailed,
                format!("response exceeds the {max_bytes} byte output budget"),
            ),
        );
    }
    (
        status,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        encoded,
    )
        .into_response()
}

async fn not_found() -> Response {
    json(
        StatusCode::NOT_FOUND,
        &serde_json::json!({"error":"route not found"}),
    )
}

/// Whether the request carries a valid credential at all — the boot token, or
/// a verified capability for this tenant — independent of the operation's
/// permission. Checked before the body is read.
fn authenticated(state: &AppState, headers: &HeaderMap) -> bool {
    let token = presented_token(headers);
    if rushdb_protocol::constant_time_eq(token.as_bytes(), state.cfg.token.as_bytes()) {
        return true;
    }
    state
        .cfg
        .capability_secret
        .as_ref()
        .and_then(|secret| crate::capability::verify(secret, token, unix_now()).ok())
        .is_some_and(|capability| capability_tenant_matches(state, headers, &capability))
}

fn payload_too_large(limit: usize) -> Response {
    protocol_error(
        StatusCode::PAYLOAD_TOO_LARGE,
        &rushdb_protocol::ProtocolError::new(
            rushdb_protocol::ErrorCode::InvalidRequest,
            format!("request body exceeds the {limit}-byte limit"),
        ),
    )
}

fn presented_token(headers: &HeaderMap) -> &str {
    let token = header(headers, "x-rushdb-token");
    if token.is_empty() {
        header(headers, "authorization")
            .strip_prefix("Bearer ")
            .unwrap_or("")
    } else {
        token
    }
}

fn capability_tenant_matches(
    state: &AppState,
    headers: &HeaderMap,
    capability: &crate::capability::Capability,
) -> bool {
    if state.node.multitenant() {
        capability.tenant.as_deref() == Some(header(headers, "x-rushdb-tenant").trim())
    } else {
        capability.tenant.is_none()
    }
}

fn authorized(state: &AppState, headers: &HeaderMap, permission: &str) -> bool {
    let token = presented_token(headers);
    if rushdb_protocol::constant_time_eq(token.as_bytes(), state.cfg.token.as_bytes()) {
        return true;
    }
    // Operator operations (the tenant's own policy) never accept a
    // capability token, whatever its scope.
    if permission == "operator" {
        return false;
    }
    state
        .cfg
        .capability_secret
        .as_ref()
        .and_then(|secret| crate::capability::verify(secret, token, unix_now()).ok())
        .is_some_and(|capability| {
            capability_tenant_matches(state, headers, &capability)
                && crate::capability::allows_scope(&capability.scope, permission)
        })
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> &'a str {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
}

/// Parse the envelope once for auth + observability: the required permission
/// (fail-closed `write` when unparseable) and the fine-grained operation
/// kind for metrics/slow-call labels (`invalid` when unparseable).
fn request_route(body: &[u8]) -> (&'static str, &'static str) {
    match serde_json::from_slice::<rushdb_protocol::RequestEnvelope>(body) {
        Ok(envelope) => (
            crate::protocol::operation_permission(&envelope.operation),
            crate::protocol::operation_kind(&envelope.operation),
        ),
        Err(_) => ("write", "invalid"),
    }
}

/// A protocol error with the HTTP status its code maps to on every route.
fn coded_error(error: &rushdb_protocol::ProtocolError) -> Response {
    protocol_error(
        status_code(crate::protocol_http::http_status(error.code)),
        error,
    )
}

fn protocol_error(status: StatusCode, error: &rushdb_protocol::ProtocolError) -> Response {
    json(
        status,
        &serde_json::json!({"protocolVersion":rushdb_protocol::PROTOCOL_VERSION,"error":error}),
    )
}

/// Sets its flag when dropped. Held by a request's async handler, so a
/// client disconnect (which drops the handler future) cancels the blocking
/// work the handler started.
struct CancelOnDrop(Arc<std::sync::atomic::AtomicBool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Stable client-facing body for an internal fault whose detail was logged
/// server-side.
fn internal_error_response() -> Response {
    protocol_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        &rushdb_protocol::ProtocolError::new(
            rushdb_protocol::ErrorCode::ExecutionFailed,
            "internal execution error",
        ),
    )
}

fn json(status: StatusCode, value: &serde_json::Value) -> Response {
    (
        status,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        value.to_string(),
    )
        .into_response()
}

fn status_code(value: u16) -> StatusCode {
    StatusCode::from_u16(value).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Run `on_drop` when the returned guard is dropped (every exit of a
/// maintenance tick, including early returns, records its duration).
fn scopeguard_tick(on_drop: impl FnOnce()) -> impl Drop {
    struct Guard<F: FnOnce()>(Option<F>);
    impl<F: FnOnce()> Drop for Guard<F> {
        fn drop(&mut self) {
            if let Some(on_drop) = self.0.take() {
                on_drop();
            }
        }
    }
    Guard(Some(on_drop))
}

#[cfg(test)]
mod p11_tests {
    use super::*;

    #[test]
    fn external_shutdown_stops_http_and_maintenance() {
        use std::io::{Read as _, Write as _};
        use std::time::Duration;

        let mut cfg = ServerConfig {
            port: 0,
            ..ServerConfig::default()
        };
        let node = Arc::new(Node::bootstrap(&mut cfg).unwrap());
        let (mut router, port) = bind_router(cfg, Arc::clone(&node)).unwrap();
        let shutdown = router.shutdown_handle();
        let (finished, outcome) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || finished.send(router.serve_forever()).unwrap());
        let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        client
            .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut reply = String::new();
        client.read_to_string(&mut reply).unwrap();
        assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
        shutdown();
        outcome
            .recv_timeout(Duration::from_secs(8))
            .unwrap()
            .unwrap();
        worker.join().unwrap();
        assert!(node.store().maintenance_aborted());
        assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn gc_worker_keeps_protected_wal_until_consumer_release() {
        let data_dir = rushdb_storage::TempPath::dir("http-retention");
        let mut cfg = crate::ServerConfig {
            data_dir: Some(data_dir.to_path_buf()),
            ..crate::ServerConfig::default()
        };
        let node = Arc::new(Node::bootstrap(&mut cfg).unwrap());
        for index in 0..6 {
            node.scope()
                .import_json_document("Retention", &serde_json::json!({"index": index}))
                .unwrap();
        }
        // Retention's signal is the number of commits it retired (the
        // cursor that clamps WAL truncation); KeepNone wants all of them,
        // the consumer holds the floor at `protected`.
        let retired = || node.store().maintenance_counters().0;
        let protected = node.committed_version().saturating_sub(2);
        node.acknowledge_retention_consumer("test-protected", protected)
            .unwrap();
        let worker = spawn_gc_worker(
            Arc::downgrade(&node),
            std::time::Duration::from_millis(1),
            512,
            // KeepNone is the soft target consumers hold back (a byte or
            // commit cap would be hard).
            RetentionPolicy::KeepNone,
            0,
            None,
        );
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        while retired() < protected - 1 {
            assert!(
                tokio::time::Instant::now() < deadline,
                "worker did not retire the unprotected commits"
            );
            tokio::task::yield_now().await;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(retired(), protected - 1, "the consumer holds the floor");

        assert!(node.release_retention_consumer("test-protected"));
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        while retired() < node.committed_version() - 1 {
            assert!(
                tokio::time::Instant::now() < deadline,
                "retention did not resume after release"
            );
            tokio::task::yield_now().await;
        }
        worker.abort();
        let _ = worker.await;
        drop(node);
        std::fs::remove_dir_all(data_dir).unwrap();
    }

    /// A long maintenance tick names the step that held the writer
    ///: every step of a tick is timed and exported.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn every_maintenance_step_is_timed() {
        let data_dir = rushdb_storage::TempPath::dir("http-maintenance-steps");
        let mut cfg = crate::ServerConfig {
            data_dir: Some(data_dir.to_path_buf()),
            ..crate::ServerConfig::default()
        };
        let node = Arc::new(Node::bootstrap(&mut cfg).unwrap());
        node.scope()
            .import_json_document("Doc", &serde_json::json!({"n": 1}))
            .unwrap();
        let worker = spawn_gc_worker(
            Arc::downgrade(&node),
            std::time::Duration::from_millis(1),
            512,
            RetentionPolicy::KeepNone,
            0,
            None,
        );
        let steps = [
            "read_pins",
            "vector_segments",
            "housekeeping",
            "history_gc",
            "retention",
            "receipts",
            "vector_slots",
            "checkpoint",
        ];
        let observed = |step: &str| {
            let prefix = format!("rushdb_maintenance_step_seconds_count{{step=\"{step}\"}} ");
            node.metrics_render()
                .lines()
                .find_map(|line| line.strip_prefix(&prefix).map(str::to_owned))
                .and_then(|value| value.trim().parse::<f64>().ok())
                .unwrap_or(0.0)
        };
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        while steps.iter().any(|step| observed(step) < 1.0) {
            assert!(
                tokio::time::Instant::now() < deadline,
                "steps never observed: {:?}",
                steps
                    .iter()
                    .filter(|step| observed(step) < 1.0)
                    .collect::<Vec<_>>()
            );
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        worker.abort();
        let _ = worker.await;
        // SCALE-2: the import published and the tick checkpointed, each
        // under its own writer holder.
        let rendered = node.metrics_render();
        for holder in ["publish", "checkpoint"] {
            let prefix = format!("rushdb_writer_hold_seconds_count{{holder=\"{holder}\"}} ");
            let count = rendered
                .lines()
                .find_map(|line| line.strip_prefix(&prefix))
                .and_then(|value| value.trim().parse::<f64>().ok())
                .unwrap_or(0.0);
            assert!(count >= 1.0, "no {holder} writer hold in:\n{rendered}");
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn lagging_consumer_gets_resync_and_disk_stays_capped() {
        let data_dir = rushdb_storage::TempPath::dir("http-resync");
        let mut cfg = crate::ServerConfig {
            data_dir: Some(data_dir.to_path_buf()),
            ..crate::ServerConfig::default()
        };
        let node = Arc::new(Node::bootstrap(&mut cfg).unwrap());
        // A consumer that stalls at the very beginning of history.
        node.acknowledge_retention_consumer("stalled-replica", 1)
            .unwrap();
        for index in 0..40 {
            node.scope()
                .import_json_document("Retention", &serde_json::json!({"index": index}))
                .unwrap();
        }
        // Keep 10 commits: a hard cap the stalled consumer cannot hold back.
        let worker = spawn_gc_worker(
            Arc::downgrade(&node),
            std::time::Duration::from_millis(1),
            512,
            RetentionPolicy::Commits(10),
            0,
            None,
        );
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let lost = node.retention_consumer_lost("stalled-replica");
            let retired = node.store().maintenance_counters().0;
            let cap_floor = node.committed_version() - 10;
            if lost && retired >= cap_floor - 1 {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "cap not enforced: lost={lost}, retired {retired} below {cap_floor}"
            );
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        worker.abort();
        let _ = worker.await;
        // The consumer no longer protects anything. Aborting the worker does
        // not stop a tick already running on a blocking thread, whose read
        // view may still be pinned for a moment: wait for that pin to drop
        // instead of sampling once.
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        while node.protected_retention_floor().is_some() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "still protected: {:?}",
                node.protected_retention_floor()
            );
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        // Its stale acknowledgement is refused with the oldest retained LSN.
        let refused = node
            .acknowledge_retention_consumer("stalled-replica", 2)
            .unwrap_err();
        assert!(refused.oldest_available_lsn > 2);
        // Resyncing at the oldest retained LSN is accepted.
        node.acknowledge_retention_consumer("stalled-replica", refused.oldest_available_lsn)
            .unwrap();
        drop(node);
        std::fs::remove_dir_all(data_dir).ok();
    }

    /// The default policy (2026-10-10, PostgreSQL rule): with no consumer and
    /// no keep window, retention advances to the published commit (so every
    /// closed WAL segment becomes reclaimable at the next checkpoint) even
    /// though the 100K-commit hard cap would allow the history to stay.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn with_no_reader_the_default_policy_reclaims_the_wal() {
        let data_dir = rushdb_storage::TempPath::dir("http-keep-none");
        let mut cfg = crate::ServerConfig {
            data_dir: Some(data_dir.to_path_buf()),
            ..crate::ServerConfig::default()
        };
        let node = Arc::new(Node::bootstrap(&mut cfg).unwrap());
        for index in 0..40 {
            node.scope()
                .import_json_document("Retention", &serde_json::json!({"index": index}))
                .unwrap();
        }
        let defaults = crate::retention::MaintenanceConfig::default();
        assert_eq!(defaults.keep_commits, 0);
        let worker = spawn_gc_worker(
            Arc::downgrade(&node),
            std::time::Duration::from_millis(1),
            512,
            defaults.retention,
            defaults.keep_commits,
            None,
        );
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let retired = node.store().maintenance_counters().0;
            if retired + 1 >= node.committed_version() {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "history kept with no reader: retired {retired} of {}",
                node.committed_version()
            );
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        worker.abort();
        let _ = worker.await;
        drop(node);
        std::fs::remove_dir_all(data_dir).ok();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn gc_worker_zero_floor_protects_wal() {
        let data_dir = rushdb_storage::TempPath::dir("http-retention-zero");
        let mut cfg = crate::ServerConfig {
            data_dir: Some(data_dir.to_path_buf()),
            ..crate::ServerConfig::default()
        };
        let node = Arc::new(Node::bootstrap(&mut cfg).unwrap());
        for index in 0..4 {
            node.scope()
                .import_json_document("Retention", &serde_json::json!({"index": index}))
                .unwrap();
        }
        node.acknowledge_retention_consumer("test-protected-zero", 0)
            .unwrap();
        let worker = spawn_gc_worker(
            Arc::downgrade(&node),
            std::time::Duration::from_millis(1),
            512,
            // KeepNone is the soft target consumers hold back (a byte or
            // commit cap would be hard).
            RetentionPolicy::KeepNone,
            0,
            None,
        );
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        assert_eq!(
            node.store().maintenance_counters().0,
            0,
            "a consumer at 0 holds every commit"
        );
        worker.abort();
        let _ = worker.await;
        drop(node);
        std::fs::remove_dir_all(data_dir).unwrap();
    }

    #[test]
    fn bounded_response_rejects_oversized_and_accepts_small() {
        // A small value passes through as 200.
        let small = bounded_response(
            StatusCode::OK,
            &serde_json::json!({"ok": true}),
            64 * 1024 * 1024,
        );
        assert_eq!(small.status(), StatusCode::OK);

        // A value larger than the output budget is rejected with 413 and an
        // explicit message (never a truncated 200). Use a tiny budget override
        // by constructing a large value against the real const is impractical,
        // so assert the guard is wired by checking the constant is enforced
        // through the same serialization path (unit-level: build a value whose
        // serialization exceeds the cap and confirm 413).
        // Construct a value larger than the budget cheaply.
        let big = serde_json::json!({
            "blob": "x".repeat(64 * 1024 * 1024 + 1),
        });
        let rejected = bounded_response(StatusCode::OK, &big, 64 * 1024 * 1024);
        assert_eq!(rejected.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[test]
    fn request_route_labels_operations_and_fails_closed() {
        // E4: one parse feeds auth (permission) and observability (op kind).
        let (permission, kind) =
            request_route(br#"{"protocolVersion":1,"operation":{"kind":"purgeTenant"}}"#);
        assert_eq!((permission, kind), ("write", "purgeTenant"));
        let (permission, kind) = request_route(
            br#"{"protocolVersion":1,"operation":{"kind":"describe","describe":{"kind":"buildInfo"}}}"#,
        );
        assert_eq!((permission, kind), ("read", "describe.buildInfo"));
        // Unparseable bodies fail closed on permission and are still
        // counted under an explicit label (never silently dropped).
        let (permission, kind) = request_route(b"not json");
        assert_eq!((permission, kind), ("write", "invalid"));
    }

    #[test]
    fn watchdog_compaction_waits_for_quiescence() {
        let data_dir = rushdb_storage::TempPath::dir("http-watchdog");
        let mut cfg = crate::ServerConfig {
            data_dir: Some(data_dir.to_path_buf()),
            ..crate::ServerConfig::default()
        };
        let node = Node::bootstrap(&mut cfg).unwrap();
        node.scope()
            .create_record(
                "Doc",
                &serde_json::json!({ "n": 1 }),
                &[],
                &serde_json::json!({}),
            )
            .unwrap();
        let store = node.store();
        assert_eq!(
            watchdog_compact(&store, std::time::Duration::from_secs(600)),
            Some(rushdb_storage::CompactOutcome::CommitsRecent),
            "commits just flowed: the watchdog must not rewrite the file"
        );
        assert!(matches!(
            watchdog_compact(&store, std::time::Duration::ZERO),
            Some(rushdb_storage::CompactOutcome::Compacted(_))
        ));
        drop(store);
        drop(node);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn read_lease_outlasts_transactions_and_request_timeouts() {
        let mut cfg = crate::ServerConfig::default();
        let node = Node::bootstrap(&mut cfg).unwrap();
        assert_eq!(node.read_lease_ms(), 65_000, "one-minute floor plus grace");
        let mut cfg = crate::ServerConfig {
            request_timeout_ms: 300_000,
            ..crate::ServerConfig::default()
        };
        cfg.transactions.max_age_seconds = 600;
        cfg.transactions.default_ttl_seconds = 600;
        let node = Node::bootstrap(&mut cfg).unwrap();
        assert_eq!(node.read_lease_ms(), 605_000);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn declared_index_builds_without_waiting_for_a_tick() {
        let mut cfg = crate::ServerConfig::default();
        let node = Arc::new(Node::bootstrap(&mut cfg).unwrap());
        for n in 0..3_000 {
            node.scope()
                .create_record(
                    "Doc",
                    &serde_json::json!({ "n": n }),
                    &[],
                    &serde_json::json!({}),
                )
                .unwrap();
        }
        // The fallback tick is far away: once the startup pass (nothing
        // pending) is done, only the wake-up can start the build.
        let builder =
            spawn_index_builder(Arc::downgrade(&node), std::time::Duration::from_secs(3_600));
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        node.scope()
            .create_index(&rushdb_protocol::IndexDefinition {
                name: "by_n".into(),
                label: "Doc".into(),
                property: "n".into(),
                kind: "ordered".into(),
                consistency: None,
                dimensions: None,
            })
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let view = node.store().read_view().unwrap();
            let state = node.scope().list_indexes_in_view(&view).unwrap()[0]["state"].clone();
            if state == "ready" {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "index still {state}");
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        builder.abort();
    }

    #[test]
    fn early_refusals_take_the_status_of_their_code() {
        use rushdb_protocol::{ErrorCode, ProtocolError};
        for (code, status) in [
            (ErrorCode::MissingScope, 403),
            (ErrorCode::ResourceExhausted, 429),
            (ErrorCode::Unavailable, 503),
            (ErrorCode::InvalidRequest, 400),
        ] {
            let response = coded_error(&ProtocolError::new(code, "x"));
            assert_eq!(response.status().as_u16(), status, "{code:?}");
        }
    }
}
