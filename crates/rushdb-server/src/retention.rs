//! Growth/memory maintenance policy, with Neo4j-style knobs. The canonical surface is
//! [`crate::resource_config::ResourceConfig::maintenance`] (config.yml /
//! `resources` section / environment via clap).

/// Retention policy for per-commit metadata (command-log rows and stored
/// outcome payloads). Commit-count based because the logical log has no
/// per-record timestamp; `keep_all` / `keep_none` mirror Neo4j's
/// `db.tx_log.rotation.retention_policy`.
///
/// **This governs commit metadata / WAL only, not MVCC history.** Superseded
/// key versions are retained for the *oldest live reader* and reclaimed
/// immediately when no reader is pinned; there is no configured history window.
/// The WAL/outcome window is legitimate because it feeds PITR, replication and
/// idempotent retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetentionPolicy {
    /// Retain all commit metadata (no pruning).
    KeepAll,
    /// Retain only the current commit's metadata.
    KeepNone,
    /// Retain the most recent `N` commits.
    Commits(u64),
}

impl RetentionPolicy {
    /// Parse `keep_all`, `keep_none`, or `N commits` (the unit is optional and
    /// singular/plural tolerated). Byte/time policies are rejected explicitly
    /// because the logical log has no per-row size or timestamp index here.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message when the policy is empty, malformed,
    /// uses a non-numeric count, or names an unsupported unit (for example a
    /// byte- or time-based retention policy).
    pub fn parse(raw: &str) -> Result<Self, String> {
        let text = raw.trim().to_ascii_lowercase();
        if text.is_empty() || text == "keep_all" {
            return Ok(Self::KeepAll);
        }
        if text == "keep_none" {
            return Ok(Self::KeepNone);
        }
        let mut parts = text.split_whitespace();
        let number = parts
            .next()
            .ok_or_else(|| "retention policy is empty".to_string())?;
        let unit = parts.next().unwrap_or("commits");
        if parts.next().is_some() {
            return Err(format!("invalid retention policy `{raw}`"));
        }
        let count: u64 = number
            .parse()
            .map_err(|_| format!("retention count `{number}` is not an integer"))?;
        match unit.trim_end_matches('s') {
            "commit" => Ok(Self::Commits(count)),
            other => Err(format!(
                "unsupported retention unit `{other}`; use `N commits`, `keep_all`, or `keep_none`"
            )),
        }
    }

    /// Lowest LSN that must be retained. `0` means "prune nothing".
    #[must_use]
    pub fn floor(&self, published: u64) -> u64 {
        match self {
            Self::KeepAll => 0,
            Self::KeepNone => published,
            Self::Commits(count) => published.saturating_sub(*count),
        }
    }

    /// The retention floor for one maintenance tick: commits
    /// below it lose their metadata and their WAL segments become
    /// reclaimable.
    ///
    /// The commit-count policy and the byte target are **hard caps**:
    /// consumers (replicas, backups, archivers, cursors) only protect
    /// history above them. A consumer that falls below is lost and must
    /// resync ([`ConsumerResyncRequired`]). Below the caps the floor follows
    /// the PostgreSQL rule (2026-10-10): the log keeps only what a consumer
    /// or open reader pins, plus an optional `keep` window of recent commits
    /// (default 0, like `wal_keep_size`). `KeepAll` sets no cap and prunes
    /// nothing.
    #[must_use]
    pub fn capped_floor(
        &self,
        published: u64,
        keep: u64,
        byte_floor: Option<u64>,
        protected_floor: Option<u64>,
    ) -> u64 {
        let wanted = published.saturating_sub(keep);
        let (hard, soft) = match self {
            Self::KeepAll => (0, 0),
            Self::KeepNone => (0, wanted),
            Self::Commits(_) => (self.floor(published), wanted),
        };
        let hard = hard.max(byte_floor.unwrap_or(0));
        let desired = soft.max(hard);
        protected_floor.map_or(desired, |protected| desired.min(protected).max(hard))
    }
}

/// A consumer acknowledged a commit that retention already reclaimed: it has
/// to rebuild from a fresh base (backup restore, snapshot export) and resume
/// at or after `oldest_available_lsn`. Protocol error code
/// `resync_required`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("resync required: retained history starts at LSN {oldest_available_lsn}")]
pub struct ConsumerResyncRequired {
    /// The oldest commit still retained.
    pub oldest_available_lsn: u64,
}

/// One consumer's retention claim: the lowest commit it still needs and when
/// it last acknowledged (unix milliseconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsumerLease {
    /// Lowest commit the consumer still needs.
    pub floor: u64,
    /// When the consumer last acknowledged (unix milliseconds).
    pub acknowledged_at_ms: u64,
}

/// A bounded registry of retention consumers. Each consumer acknowledges the
/// lowest commit it still needs; the minimum acknowledgement protects all
/// consumers while allowing released consumers to stop pinning history.
///
/// Claims are **leases**. A consumer that stops acknowledging (a crashed
/// backup job, a decommissioned replica, a disabled archiver) is expired by
/// [`Self::expire`] instead of pinning the WAL forever (C5, cloud-capacity
/// F2/F3).
#[derive(Debug, Default, Clone)]
pub struct RetentionConsumers {
    floors: std::collections::BTreeMap<String, ConsumerLease>,
    /// Lease length in milliseconds; `0` disables expiry.
    lease_ms: u64,
    /// Oldest commit still retained: acknowledgements below it are refused.
    oldest_available: u64,
    /// Consumers dropped because retention passed them (until they resync).
    lost: std::collections::BTreeSet<String>,
}

impl RetentionConsumers {
    /// An empty registry whose claims expire after `lease` without renewal.
    #[must_use]
    pub fn with_lease(lease: std::time::Duration) -> Self {
        Self {
            floors: std::collections::BTreeMap::new(),
            lease_ms: u64::try_from(lease.as_millis()).unwrap_or(u64::MAX),
            oldest_available: 0,
            lost: std::collections::BTreeSet::new(),
        }
    }

    /// Register or advance a consumer acknowledgement monotonically and renew
    /// its lease.
    ///
    /// # Errors
    /// [`ConsumerResyncRequired`] when `floor` is below the oldest retained
    /// commit: the history it asks for is gone, so the claim is refused and
    /// the consumer is marked lost.
    pub fn acknowledge(
        &mut self,
        consumer: impl Into<String>,
        floor: u64,
        now_ms: u64,
    ) -> Result<(), ConsumerResyncRequired> {
        let consumer = consumer.into();
        if floor < self.oldest_available {
            self.floors.remove(&consumer);
            self.lost.insert(consumer);
            return Err(ConsumerResyncRequired {
                oldest_available_lsn: self.oldest_available,
            });
        }
        self.lost.remove(&consumer);
        self.floors
            .entry(consumer)
            .and_modify(|lease| {
                lease.floor = lease.floor.max(floor);
                lease.acknowledged_at_ms = lease.acknowledged_at_ms.max(now_ms);
            })
            .or_insert(ConsumerLease {
                floor,
                acknowledged_at_ms: now_ms,
            });
        Ok(())
    }

    /// Retention reclaimed everything below `floor`: refuse acknowledgements
    /// under it from now on, and drop (as lost) every consumer whose claim
    /// it passed. Returns the newly lost consumers.
    pub fn enforce_floor(&mut self, floor: u64) -> Vec<String> {
        self.oldest_available = self.oldest_available.max(floor);
        let oldest = self.oldest_available;
        let lost: Vec<String> = self
            .floors
            .iter()
            .filter(|(_, lease)| lease.floor < oldest)
            .map(|(name, _)| name.clone())
            .collect();
        for name in &lost {
            self.floors.remove(name);
            self.lost.insert(name.clone());
        }
        lost
    }

    /// Whether `consumer` was dropped by retention and has not resynced.
    #[must_use]
    pub fn is_lost(&self, consumer: &str) -> bool {
        self.lost.contains(consumer)
    }

    /// Consumers currently lost (a metric).
    #[must_use]
    pub fn lost_len(&self) -> usize {
        self.lost.len()
    }

    /// Remove a consumer that no longer requires retained history.
    pub fn release(&mut self, consumer: &str) -> bool {
        self.floors.remove(consumer).is_some()
    }

    /// Drop every consumer whose lease is older than the registry's lease at
    /// `now_ms` and return their names. A zero lease disables expiry.
    pub fn expire(&mut self, now_ms: u64) -> Vec<String> {
        let lease_ms = self.lease_ms;
        if lease_ms == 0 {
            return Vec::new();
        }
        let expired: Vec<String> = self
            .floors
            .iter()
            .filter(|(_, lease)| now_ms.saturating_sub(lease.acknowledged_at_ms) > lease_ms)
            .map(|(name, _)| name.clone())
            .collect();
        for name in &expired {
            self.floors.remove(name);
        }
        expired
    }

    /// Lowest acknowledged commit, or `None` when there are no consumers.
    #[must_use]
    pub fn protected_floor(&self) -> Option<u64> {
        self.floors.values().map(|lease| lease.floor).min()
    }

    /// Number of active consumers, useful for retention metrics.
    #[must_use]
    pub fn len(&self) -> usize {
        self.floors.len()
    }

    /// Whether no consumers are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.floors.is_empty()
    }

    /// Replace the registry from its durable representation during boot.
    pub fn replace(&mut self, floors: std::collections::BTreeMap<String, ConsumerLease>) {
        self.floors = floors;
    }

    /// Copy the leases for durable persistence.
    #[must_use]
    pub fn snapshot(&self) -> std::collections::BTreeMap<String, ConsumerLease> {
        self.floors.clone()
    }
}

/// Current unix time in milliseconds (lease clock).
#[must_use]
pub fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Maintenance knobs read once at startup.
#[derive(Debug, Clone)]
pub struct MaintenanceConfig {
    /// Retention policy for per-commit metadata: the hard commit cap.
    pub retention: RetentionPolicy,
    /// Commits kept even when no consumer or reader pins them
    /// (`RUSHDB_TX_LOG_KEEP_COMMITS`, default 0).
    pub keep_commits: u64,
    /// Optional byte target for retained command-log rows
    /// (`RUSHDB_TX_LOG_RETENTION_BYTES`; none = off). Applied in addition to
    /// the commit floor.
    pub retention_bytes: Option<u64>,
    /// Idle-window compaction period (`RUSHDB_COMPACT_INTERVAL_SECONDS`; none =
    /// disabled). Acts as the minimum interval between compactions.
    pub compact_interval: Option<std::time::Duration>,
    /// Minimum quiet period after the last commit before compaction may run
    /// (`RUSHDB_COMPACT_QUIESCE_SECONDS`, default 10s). Prevents compaction
    /// from rewriting the file while commits are flowing.
    pub compact_quiesce: std::time::Duration,
    /// Compact only when the file has grown by at least this percent since the
    /// last compaction (`RUSHDB_COMPACT_GROWTH_PERCENT`, default 150).
    pub compact_growth_percent: u32,
    /// RSS watchdog threshold in bytes (`RUSHDB_RSS_MAX_BYTES`; none =
    /// disabled). Policy, not a hard allocation cap.
    pub rss_max_bytes: Option<u64>,
    /// Directory for continuous logical-WAL archiving (`RUSHDB_WAL_ARCHIVE_DIR`;
    /// none = off). A worker writes append-only segment files plus an
    /// `archive.state` watermark there.
    pub wal_archive_dir: Option<std::path::PathBuf>,
    /// Archive retention window (`RUSHDB_WAL_ARCHIVE_RETENTION_SECONDS`).
    pub wal_archive_retention: std::time::Duration,
    /// Hard archive byte cap (`RUSHDB_WAL_ARCHIVE_RETENTION_BYTES`; none =
    /// off).
    pub wal_archive_retention_bytes: Option<u64>,
    /// Streaming replication follower settings (`RUSHDB_REPLICA_OF`; none =
    /// not a follower). See [`ReplicaConfig`].
    pub replica: Option<ReplicaConfig>,
}

/// Streaming replication follower settings: which leader to
/// poll, the identity this node reports (so the leader can protect WAL
/// retention per follower), and the bearer token to present.
#[derive(Debug, Clone)]
pub struct ReplicaConfig {
    /// The leader's base URL, e.g. `http://leader:3000`.
    pub leader_url: String,
    /// Stable identity reported to the leader's replication endpoint.
    pub follower_id: String,
    /// Bearer token presented to the leader.
    pub token: String,
}

impl Default for MaintenanceConfig {
    fn default() -> Self {
        Self {
            retention: RetentionPolicy::Commits(100_000),
            keep_commits: 0,
            retention_bytes: None,
            compact_interval: Some(std::time::Duration::from_secs(300)),
            compact_quiesce: std::time::Duration::from_secs(10),
            compact_growth_percent: 150,
            rss_max_bytes: None,
            wal_archive_dir: None,
            wal_archive_retention: std::time::Duration::from_hours(7 * 24),
            wal_archive_retention_bytes: None,
            replica: None,
        }
    }
}

/// Memory watchdog decision: act when resident memory exceeds the configured
/// limit. `resident` is `None` when the allocator exposes no counters.
#[must_use]
pub fn over_rss_limit(resident: Option<u64>, limit: u64) -> bool {
    limit > 0 && resident.is_some_and(|rss| rss > limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_policy_parses_commits_and_extremes() {
        assert_eq!(
            RetentionPolicy::parse("").unwrap(),
            RetentionPolicy::KeepAll
        );
        assert_eq!(
            RetentionPolicy::parse("keep_all").unwrap(),
            RetentionPolicy::KeepAll
        );
        assert_eq!(
            RetentionPolicy::parse("keep_none").unwrap(),
            RetentionPolicy::KeepNone
        );
        assert_eq!(
            RetentionPolicy::parse("500 commits").unwrap(),
            RetentionPolicy::Commits(500)
        );
        assert_eq!(
            RetentionPolicy::parse("500 commit").unwrap(),
            RetentionPolicy::Commits(500)
        );
        assert!(RetentionPolicy::parse("2 days").is_err());
        assert!(RetentionPolicy::parse("1GiB").is_err());
        assert!(RetentionPolicy::parse("not a number commits").is_err());
    }

    #[test]
    fn retention_floor_is_monotonic_and_bounded() {
        let published = 1000u64;
        assert_eq!(RetentionPolicy::KeepAll.floor(published), 0);
        assert_eq!(RetentionPolicy::KeepNone.floor(published), published);
        assert_eq!(RetentionPolicy::Commits(200).floor(published), 800);
        assert_eq!(RetentionPolicy::Commits(5000).floor(published), 0);
        assert_eq!(RetentionPolicy::Commits(0).floor(published), published);
    }

    #[test]
    fn commit_and_byte_caps_are_hard_and_consumers_hold_only_above_them() {
        // A consumer far behind cannot hold the commit cap back.
        assert_eq!(
            RetentionPolicy::Commits(200).capped_floor(1000, 0, None, Some(10)),
            800
        );
        // Nor the byte cap.
        assert_eq!(
            RetentionPolicy::KeepAll.capped_floor(1000, 0, Some(600), Some(10)),
            600
        );
        // KeepNone is soft: consumers hold it, but never below a byte cap.
        assert_eq!(
            RetentionPolicy::KeepNone.capped_floor(1000, 0, None, Some(950)),
            950
        );
        assert_eq!(
            RetentionPolicy::KeepNone.capped_floor(1000, 0, Some(970), Some(950)),
            970
        );
        assert_eq!(
            RetentionPolicy::KeepNone.capped_floor(1000, 0, None, None),
            1000
        );
        assert_eq!(
            RetentionPolicy::KeepAll.capped_floor(1000, 0, None, Some(5)),
            0
        );
    }

    #[test]
    fn with_no_reader_the_log_keeps_only_the_keep_window() {
        // No consumer, no keep window: everything below the published commit
        // is reclaimable, even though the commit cap would allow 200 more.
        assert_eq!(
            RetentionPolicy::Commits(200).capped_floor(1000, 0, None, None),
            1000
        );
        // An explicit keep window is retained without any consumer...
        assert_eq!(
            RetentionPolicy::Commits(200).capped_floor(1000, 50, None, None),
            950
        );
        // ...but never past the hard cap.
        assert_eq!(
            RetentionPolicy::Commits(200).capped_floor(1000, 500, None, None),
            800
        );
        // A consumer holds the floor back within the cap.
        assert_eq!(
            RetentionPolicy::Commits(200).capped_floor(1000, 0, None, Some(900)),
            900
        );
    }

    #[test]
    fn consumers_passed_by_the_floor_are_lost_until_they_resync() {
        let mut consumers = RetentionConsumers::default();
        consumers.acknowledge("replica", 100, 0).unwrap();
        consumers.acknowledge("backup", 900, 0).unwrap();
        assert_eq!(consumers.enforce_floor(500), vec!["replica".to_string()]);
        assert!(consumers.is_lost("replica"));
        assert_eq!(consumers.protected_floor(), Some(900));
        // Its next acknowledgement below the floor is refused…
        assert_eq!(
            consumers.acknowledge("replica", 101, 0),
            Err(ConsumerResyncRequired {
                oldest_available_lsn: 500
            })
        );
        // …and a resynced one at or above it is accepted again.
        consumers.acknowledge("replica", 500, 0).unwrap();
        assert!(!consumers.is_lost("replica"));
        assert_eq!(consumers.protected_floor(), Some(500));
        assert_eq!(consumers.lost_len(), 0);
    }

    #[test]
    fn consumer_acknowledgements_are_monotonic_and_releaseable() {
        let mut consumers = RetentionConsumers::default();
        consumers.acknowledge("backup", 40, 0).unwrap();
        consumers.acknowledge("replica", 20, 0).unwrap();
        consumers.acknowledge("backup", 10, 0).unwrap();
        assert_eq!(consumers.protected_floor(), Some(20));
        consumers.acknowledge("replica", 50, 0).unwrap();
        assert_eq!(consumers.protected_floor(), Some(40));
        assert!(consumers.release("backup"));
        assert_eq!(consumers.protected_floor(), Some(50));
        assert!(!consumers.release("missing"));
    }

    #[test]
    fn unrenewed_consumer_leases_expire_and_stop_pinning_history() {
        // C5: a consumer that stops acknowledging must not pin the WAL forever.
        let mut consumers = RetentionConsumers::with_lease(std::time::Duration::from_secs(10));
        consumers.acknowledge("backup", 5, 1_000).unwrap();
        consumers.acknowledge("replica", 90, 1_000).unwrap();
        // The replica keeps renewing; the backup job died.
        consumers.acknowledge("replica", 95, 9_000).unwrap();
        assert!(consumers.expire(10_000).is_empty(), "within the lease");
        assert_eq!(consumers.expire(11_001), vec!["backup".to_string()]);
        assert_eq!(consumers.protected_floor(), Some(95));
        // A zero lease never expires.
        let mut forever = RetentionConsumers::default();
        forever.acknowledge("archive", 1, 0).unwrap();
        assert!(forever.expire(u64::MAX).is_empty());
    }

    #[test]
    fn watchdog_acts_only_above_a_positive_limit() {
        assert!(!over_rss_limit(Some(100), 0));
        assert!(!over_rss_limit(None, 100));
        assert!(!over_rss_limit(Some(100), 100));
        assert!(over_rss_limit(Some(101), 100));
    }
}
