use crate::staging::StagedOp;
use rushdb_lmpg::reader::keys_ext as kx;
use rushdb_lmpg::{BatchOp, Lmpg};
use rushdb_storage::{
    resources::{ByteBudget, ByteReservation},
    OutcomeKind, OverlayView, Store, TxAccumulator,
};
use rushdb_types::ids::{OwnerKind, TenantId};

use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const TRANSACTION_METADATA_ALLOWANCE: usize = 1024;

/// Resource caps enforced by [`TxCoordinator`]. All fields are counts or byte
/// budgets that must be positive for the coordinator to be constructed.
#[derive(Debug, Clone, Copy)]
pub struct TransactionLimits {
    /// Maximum number of concurrently active transactions.
    pub max_active: usize,
    /// Maximum number of staged operations per transaction.
    pub max_operations: usize,
    /// Encoded operation bytes, excluding separately charged metadata allowances.
    pub max_staged_bytes: usize,
    /// Bounded recent-expiry IDs; older evicted IDs become unknown/retryable.
    pub max_expired_ids: usize,
}
impl Default for TransactionLimits {
    fn default() -> Self {
        Self {
            max_active: 128,
            max_operations: 10_000,
            max_staged_bytes: 8 * 1024 * 1024,
            max_expired_ids: 128,
        }
    }
}

impl TransactionLimits {
    /// Validate that the limits are non-zero and that `ttl` is representable as
    /// an instant, including the derived metadata allowance.
    ///
    /// # Errors
    ///
    /// Returns a message when any size limit is zero, when `ttl` is zero or
    /// overflows [`Instant`], or when the metadata allowance overflows.
    pub fn validate(&self, ttl: Duration) -> Result<(), String> {
        if self.max_active == 0 || self.max_operations == 0 || self.max_staged_bytes == 0 {
            return Err(
                "invalid transaction limits: active, operations, and bytes must be positive".into(),
            );
        }
        if ttl.is_zero() || Instant::now().checked_add(ttl).is_none() {
            return Err("invalid transaction TTL".into());
        }
        self.metadata_allowance()?;
        Ok(())
    }

    /// Compute the byte allowance reserved for expiry tombstones and
    /// transaction metadata.
    ///
    /// # Errors
    ///
    /// Returns a message if the allowance computation overflows `usize`.
    pub fn metadata_allowance(&self) -> Result<usize, String> {
        self.max_expired_ids
            .checked_mul(128)
            .and_then(|n| n.checked_add(1024))
            .ok_or_else(|| "invalid transaction limits: metadata size overflow".into())
    }
}

struct ActiveTx {
    expires_at: Instant,
    staged_bytes: usize,
    /// Keep the transaction's base snapshot protected from MVCC reclamation
    /// for the full cross-request transaction lifetime. Shared with the
    /// accumulator so every transaction read resolves at this snapshot.
    _read_view: Arc<rushdb_storage::ReadView>,
    /// Policy generation captured at BEGIN; commit revalidates against the
    /// live generation and aborts with `authorization_changed` on rotation.
    policy_generation: Option<u64>,
    ops: Vec<StagedOp>,
    /// Read-your-writes accumulator: overlay buffer + dependency set + the
    /// writes staged so far. Commit replays this atomically.
    acc: TxAccumulator,
    /// When true, an expired transaction is committed (autocommit) instead of
    /// rolled back when its TTL lapses.
    autocommit: bool,
    _metadata: Option<ByteReservation>,
}

/// Terminal outcome of committing a coordinated transaction.
#[derive(Debug, Clone, PartialEq)]
pub enum TxOutcome {
    /// The transaction was durably committed at the given commit index.
    Committed(u64),
    /// The transaction was rolled back or rejected, with a reason message.
    Aborted(String),
    /// The transaction lost a first-committer-wins race at commit. The client
    /// may safely retry the whole transaction.
    ConflictRetryable(String),
    /// The transaction TTL lapsed before it could commit.
    Expired,
    /// No durable outcome is known yet; the caller may safely retry.
    UnknownRetryable,
}

/// Where a transaction id stands (see [`TxCoordinator::status`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxStatus {
    /// Open: operations on it are accepted.
    Active,
    /// Its TTL lapsed; it is remembered as expired.
    Expired,
    /// Never begun here, already settled, or forgotten.
    Unknown,
}

struct State {
    active: BTreeMap<String, ActiveTx>,
    expired: VecDeque<String>,
    last_activity: Instant,
}
impl Default for State {
    fn default() -> Self {
        Self {
            active: BTreeMap::new(),
            expired: VecDeque::new(),
            last_activity: Instant::now(),
        }
    }
}
impl State {
    /// Move every expired transaction out of the active map (bounded by the
    /// tombstone limit). The caller settles each one (commit if autocommit,
    /// otherwise discard) after releasing the state lock.
    fn expire(&mut self, now: Instant, limit: usize) -> Vec<(String, ActiveTx)> {
        let expired_ids: Vec<String> = self
            .active
            .iter()
            .filter(|(_, tx)| now >= tx.expires_at)
            .map(|(id, _)| id.clone())
            .collect();
        let mut out = Vec::new();
        for id in expired_ids {
            if let Some(tx) = self.active.remove(&id) {
                if limit > 0 {
                    while self.expired.len() >= limit {
                        self.expired.pop_front();
                    }
                    self.expired.push_back(id.clone());
                }
                out.push((id, tx));
            }
        }
        if !out.is_empty() {
            self.last_activity = now;
        }
        out
    }
    fn was_expired(&self, id: &str) -> bool {
        self.expired.iter().any(|known| known == id)
    }
    fn forget(&mut self, id: &str) {
        self.active.remove(id);
        self.expired.retain(|known| known != id);
    }
}

/// Cross-request staging draws from the store-wide transaction pool by
/// default; a caller that wants one tenant's staging unable to starve
/// another's constructs this with a positive `max_staged_bytes_for_tenant`
/// (tenancy Pillar 4), which charges this coordinator's reservations
/// against a child budget capped independently of its siblings — still
/// counted against the shared pool through the existing parent-chain
/// accounting, just unable to consume all of it alone. Decoded mutation
/// planning/publication is not yet charged by this layer.
pub struct TxCoordinator {
    store: Arc<Store>,
    tenant: TenantId,
    ttl: Duration,
    max_ttl: Duration,
    limits: TransactionLimits,
    state: Mutex<State>,
    budget: ByteBudget,
    authorizer: Option<Arc<dyn rushdb_lmpg::Authorizer>>,
    // Metadata/container allowance remains charged even while state is empty.
    _metadata: ByteReservation,
}

impl TxCoordinator {
    /// Construct a coordinator with default limits and no authorizer.
    ///
    /// # Errors
    ///
    /// Returns a message when the default limits or `ttl` are invalid, or when
    /// the metadata reservation cannot be satisfied.
    pub fn new(store: Arc<Store>, tenant: TenantId, ttl: Duration) -> Result<Self, String> {
        Self::new_with_authorizer(store, tenant, ttl, None)
    }

    /// Construct a coordinator with default limits and an optional authorizer.
    ///
    /// # Errors
    ///
    /// Returns a message when the default limits or `ttl` are invalid, or when
    /// the metadata reservation cannot be satisfied.
    pub fn new_with_authorizer(
        store: Arc<Store>,
        tenant: TenantId,
        ttl: Duration,
        authorizer: Option<Arc<dyn rushdb_lmpg::Authorizer>>,
    ) -> Result<Self, String> {
        Self::with_limits(store, tenant, ttl, authorizer, TransactionLimits::default())
    }

    /// Construct a coordinator with explicit limits and no separate maximum TTL.
    ///
    /// # Errors
    ///
    /// Returns a message when `limits` or `ttl` are invalid, or when the
    /// metadata reservation cannot be satisfied.
    pub fn with_limits(
        store: Arc<Store>,
        tenant: TenantId,
        ttl: Duration,
        authorizer: Option<Arc<dyn rushdb_lmpg::Authorizer>>,
        limits: TransactionLimits,
    ) -> Result<Self, String> {
        Self::with_limits_and_max_ttl(store, tenant, ttl, ttl, authorizer, limits, 0)
    }

    /// Construct a coordinator with a default transaction TTL and an enforced
    /// upper bound for caller-provided TTL overrides.
    ///
    /// `max_staged_bytes_for_tenant` caps this coordinator's own share of the
    /// store-wide transaction pool (0 = share it directly, uncapped beyond
    /// the pool's own limit — today's behavior).
    ///
    /// # Errors
    ///
    /// Returns a message when `limits`, `ttl`, or `max_ttl` are invalid, when
    /// `max_staged_bytes_for_tenant` exceeds the store-wide pool, or when the
    /// metadata reservation cannot be satisfied.
    pub fn with_limits_and_max_ttl(
        store: Arc<Store>,
        tenant: TenantId,
        ttl: Duration,
        max_ttl: Duration,
        authorizer: Option<Arc<dyn rushdb_lmpg::Authorizer>>,
        limits: TransactionLimits,
        max_staged_bytes_for_tenant: usize,
    ) -> Result<Self, String> {
        limits.validate(ttl)?;
        if max_ttl.is_zero() || max_ttl < ttl {
            return Err(
                "transaction maximum TTL must be positive and at least the default TTL".into(),
            );
        }
        let allowance = limits.metadata_allowance()?;
        let root = store.transaction_budget();
        let budget = if max_staged_bytes_for_tenant == 0 {
            root
        } else {
            root.child("tenant-transaction-staging", max_staged_bytes_for_tenant)
                .map_err(|e| format!("invalid per-tenant transaction byte limit: {e}"))?
        };
        let metadata = budget
            .try_reserve(allowance)
            .map_err(|e| format!("resource exhausted: {e}"))?;
        Ok(Self {
            store,
            tenant,
            ttl,
            max_ttl,
            limits,
            state: Mutex::new(State::default()),
            budget,
            authorizer,
            _metadata: metadata,
        })
    }
    fn lmpg(&self) -> Lmpg {
        Lmpg::new_with_authorizer(self.store.clone(), self.tenant, self.authorizer.clone())
    }

    /// Build a read overlay for `tx_id` reflecting every write staged so far.
    /// Statement planning reads through this so a later statement sees an
    /// earlier statement's writes (cross-statement read-your-writes, T-5b).
    ///
    /// # Errors
    /// Returns a message when the transaction is unknown or the base snapshot
    /// cannot be opened.
    pub fn overlay_view(&self, tx_id: &str) -> Result<Arc<OverlayView>, String> {
        let state = self.lock_state();
        let tx = state
            .active
            .get(tx_id)
            .ok_or_else(|| "unknown transaction id".to_string())?;
        let base = match &tx.acc.base_view {
            Some(view) => Arc::clone(view),
            None => Arc::new(self.store.read_view().map_err(|error| error.to_string())?),
        };
        Ok(Arc::new(OverlayView::with_buffer(
            base,
            tx.acc.buffer.clone(),
        )))
    }

    /// Lock the coordinator state, recovering from poisoning rather than
    /// panicking. One panicking transaction must not cascade into every later
    /// transaction sharing this coordinator.
    fn lock_state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Drop expired payloads. Also called by begin/stage/commit. A node-wide
    /// scheduled reaper is still required to reclaim completely idle scopes.
    /// Autocommit transactions are settled (committed) here, not rolled back.
    ///
    /// A poisoned state mutex is recovered, not propagated.
    pub fn reap_expired(&self) -> usize {
        let expired = self
            .lock_state()
            .expire(Instant::now(), self.limits.max_expired_ids);
        let count = expired.len();
        self.settle(expired);
        count
    }

    /// Expire transactions while suppressing autocommit publication. Used by
    /// maintenance mode, where background expiry must release state without
    /// creating a new write commit.
    ///
    /// A poisoned state mutex is recovered, not propagated.
    pub fn reap_expired_without_autocommit(&self) -> usize {
        let expired = self
            .lock_state()
            .expire(Instant::now(), self.limits.max_expired_ids);
        let count = expired.len();
        self.settle_with_autocommit(expired, false);
        count
    }
    /// Where `tx_id` stands, so a caller can classify a failed operation on
    /// it without reading the error text.
    ///
    /// A poisoned state mutex is recovered, not propagated.
    pub fn status(&self, tx_id: &str) -> TxStatus {
        let state = self.lock_state();
        if state.active.contains_key(tx_id) {
            TxStatus::Active
        } else if state.was_expired(tx_id) {
            TxStatus::Expired
        } else {
            TxStatus::Unknown
        }
    }

    /// Number of transactions currently active (not yet committed, rolled
    /// back, or expired).
    ///
    /// A poisoned state mutex is recovered, not propagated.
    pub fn active_count(&self) -> usize {
        self.lock_state().active.len()
    }

    /// Settle expired transactions outside the state lock: autocommit ones are
    /// committed atomically (best-effort; a conflict discards them), the rest
    /// are discarded. Dropping the `ActiveTx` releases its metadata charge.
    fn settle(&self, expired: Vec<(String, ActiveTx)>) {
        self.settle_with_autocommit(expired, true);
    }

    fn settle_with_autocommit(&self, expired: Vec<(String, ActiveTx)>, allow_autocommit: bool) {
        for (id, tx) in expired {
            if allow_autocommit && tx.autocommit && !tx.acc.mutations.is_empty() {
                let captured_generation = tx.policy_generation;
                let authorizer = self.authorizer.clone();
                let tenant = self.tenant;
                let _ =
                    self.store
                        .tx_commit_revalidated(commit_key_of(&id), &tx.acc, move |read| {
                            if authorizer.is_none() {
                                return Ok(());
                            }
                            let Some(captured) = captured_generation else {
                                return Ok(());
                            };
                            let live =
                                rushdb_authz::load_policy_from_reader(read, tenant, u64::MAX)
                                    .map_err(|e| e.to_string())?
                                    .map_or(0, |active| active.generation);
                            if live != captured {
                                return Err(
                                    "authorization_changed: policy generation rotated".into()
                                );
                            }
                            Ok(())
                        });
            }
            drop(tx);
        }
    }

    /// Registry owners may evict an idle coordinator after its TTL if no
    /// external Arc users remain. Expiry tombstones then become unknown IDs.
    ///
    /// A poisoned state mutex is recovered, not propagated.
    pub fn idle_ttl_elapsed(&self) -> bool {
        let state = self.lock_state();
        state.active.is_empty() && state.last_activity.elapsed() >= self.ttl
    }

    /// Begin a transaction with the coordinator's default TTL and rollback
    /// expiry semantics.
    ///
    /// # Errors
    ///
    /// Returns a message when the policy generation cannot be read, the active
    /// transaction limit is reached, or the transaction metadata reservation
    /// cannot be satisfied.
    ///
    /// A poisoned state mutex is recovered, not propagated.
    pub fn begin(&self) -> Result<String, String> {
        self.begin_with(None, false)
    }

    /// Begin with an optional per-transaction TTL override and an autocommit
    /// flag: when `autocommit` is true, staged writes are COMMITTED (best
    /// effort) when the transaction expires instead of being rolled back.
    ///
    /// # Errors
    ///
    /// Returns a message when the TTL is zero or not representable, when the
    /// policy generation cannot be read, when the active transaction limit is
    /// reached, or when the transaction metadata reservation cannot be
    /// satisfied.
    ///
    /// A poisoned state mutex is recovered, not propagated.
    pub fn begin_with(&self, ttl_ms: Option<u64>, autocommit: bool) -> Result<String, String> {
        let now = Instant::now();
        let ttl = ttl_ms
            .map_or(self.ttl, Duration::from_millis)
            .min(self.max_ttl);
        if ttl.is_zero() {
            return Err("invalid transaction TTL".into());
        }
        let expires_at = now.checked_add(ttl).ok_or("invalid transaction TTL")?;
        // Capture the policy generation the transaction starts under so
        // commit can revalidate it (default-deny on a malformed/missing
        // policy surfaces here as a begin error).
        let policy_generation = match &self.authorizer {
            Some(a) => {
                let g = a.current_generation()?;
                Some(g)
            }
            None => None,
        };
        let mut state = self.lock_state();
        let expired = state.expire(now, self.limits.max_expired_ids);
        drop(state);
        self.settle(expired);
        let mut state = self.lock_state();
        if state.active.len() >= self.limits.max_active {
            return Err("resource exhausted: active transaction limit".into());
        }
        let metadata = self
            .budget
            .try_reserve(TRANSACTION_METADATA_ALLOWANCE)
            .map_err(|e| format!("resource exhausted: {e}"))?;
        let id = uuid::Uuid::now_v7().simple().to_string();
        let base_ci = self.store.committed_version();
        let read_view = Arc::new(
            self.store
                .read_view()
                .map_err(|error| format!("transaction snapshot unavailable: {error}"))?,
        );
        let mut acc = TxAccumulator::new(self.tenant, base_ci);
        acc.base_view = Some(Arc::clone(&read_view));
        state.last_activity = now;
        state.active.insert(
            id.clone(),
            ActiveTx {
                expires_at,
                staged_bytes: 0,
                _read_view: read_view,
                policy_generation,
                ops: Vec::new(),
                acc,
                autocommit,
                _metadata: Some(metadata),
            },
        );
        Ok(id)
    }

    /// Stage one operation into an active transaction, byte-charging the
    /// encoded payload and applying it to the transaction's read-your-writes
    /// overlay.
    ///
    /// # Errors
    ///
    /// Returns a message when `tx_id` is unknown or expired, when the
    /// operation or byte limits are exceeded, when staging cannot reserve
    /// memory, or when the operation cannot be planned on the overlay.
    ///
    /// A poisoned state mutex is recovered, not propagated.
    pub fn stage(&self, tx_id: &str, op: BatchOp) -> Result<(), String> {
        let mut state = self.lock_state();
        let now = Instant::now();
        let expired = state
            .active
            .get(tx_id)
            .is_some_and(|tx| now >= tx.expires_at);
        let settled = state.expire(now, self.limits.max_expired_ids);
        drop(state);
        self.settle(settled);
        let mut state = self.lock_state();
        if expired || state.was_expired(tx_id) {
            return Err("transaction expired".into());
        }
        let tx = state
            .active
            .get_mut(tx_id)
            .ok_or("unknown transaction id")?;
        if tx.ops.len() >= self.limits.max_operations {
            return Err("resource exhausted: transaction operation limit".into());
        }
        let staged = StagedOp::encode(
            &op,
            self.limits.max_staged_bytes - tx.staged_bytes,
            &self.budget,
        )?;
        if Instant::now() >= tx.expires_at {
            state.expire(Instant::now(), self.limits.max_expired_ids);
            return Err("transaction expired".into());
        }
        tx.ops
            .try_reserve(1)
            .map_err(|e| format!("resource exhausted: {e}"))?;
        tx.staged_bytes += staged.len();
        tx.ops.push(staged);
        // Read-your-writes: apply the operation to the transaction overlay now
        // (planning reads committed state merged with earlier staged writes),
        // so later stages and commits see this operation's effects.
        let lmpg = self.lmpg();
        Self::apply_batch_op(&lmpg, op, &mut tx.acc).map_err(|e| e.to_reason())?;
        Ok(())
    }

    /// Stage every operation of a pre-built batch (e.g. a statement-language
    /// mutation delta) into the transaction, byte-charging each op.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by [`stage`](Self::stage); operations
    /// staged before the failure remain staged.
    pub fn stage_batch(&self, tx_id: &str, batch: rushdb_lmpg::BatchTx) -> Result<(), String> {
        for op in batch.ops {
            self.stage(tx_id, op)?;
        }
        Ok(())
    }

    fn apply_batch_op(
        lmpg: &Lmpg,
        op: BatchOp,
        acc: &mut TxAccumulator,
    ) -> Result<(), rushdb_lmpg::LmpgError> {
        use rushdb_lmpg::BatchOp as B;
        match op {
            B::CreateRecord {
                label,
                explicit_id,
                fields,
            } => {
                lmpg.create_record_tx(&label, explicit_id, fields, Some(acc))?;
            }
            B::UpdateFields {
                owner,
                owner_ord,
                owner_public_id,
                fields,
            } => {
                debug_assert_eq!(owner, rushdb_types::ids::OwnerKind::Record);
                lmpg.update_fields_tx(
                    rushdb_lmpg::RecordRef {
                        record_id: owner_public_id,
                        ordinal: owner_ord,
                    },
                    fields,
                    Some(acc),
                )?;
            }
            B::RetypeRecord {
                record,
                from_label,
                to_label,
            } => {
                lmpg.retype_record_tx(record, &from_label, &to_label, Some(acc))?;
            }
            B::DeleteRecord { record } => {
                lmpg.delete_record_tx(record, Some(acc))?;
            }
            B::DeleteRelationship { rel } => {
                lmpg.delete_relationship_tx(rel, Some(acc))?;
            }
            B::CreateRelationship {
                rel_type,
                source,
                target,
                fields,
            } => {
                lmpg.create_relationship_tx(&rel_type, source, target, fields, Some(acc))?;
            }
            other @ B::CreateRelationshipById { .. } => {
                return Err(rushdb_lmpg::LmpgError::Conflict(format!(
                    "transaction stage does not support operation {other:?} yet"
                )))
            }
        }
        Ok(())
    }

    /// Read one record inside the transaction. The result merges committed
    /// state with the transaction's staged writes (read-your-writes), and every
    /// key the reconstruction touches is recorded into the transaction's
    /// dependency set so `commit` aborts on concurrent changes to those keys.
    ///
    /// # Errors
    ///
    /// Returns a message when `tx_id` is unknown or expired, when no record
    /// has `record_id`, when stored rows are corrupt, or when a read
    /// dependency exceeds the configured read limits.
    ///
    /// A poisoned state mutex is recovered, not propagated.
    pub fn get_record(
        &self,
        tx_id: &str,
        record_id: [u8; 16],
    ) -> Result<serde_json::Value, String> {
        let tenant = self.tenant;
        self.read_in_tx(tx_id, |txn| {
            let ordinal = resolve_ordinal(txn, tenant, record_id)?;
            reconstruct_record(txn, tenant, ordinal, record_id)
        })
    }

    /// The storage ordinal of the record with public id `record_id`, as the
    /// transaction sees it: committed records and records this transaction
    /// staged. The lookup is a tracked read, so `commit` aborts if a
    /// concurrent writer changes which record holds the id.
    ///
    /// # Errors
    ///
    /// Returns a message when `tx_id` is unknown or expired, when no record
    /// has `record_id`, or when the stored row is corrupt.
    ///
    /// A poisoned state mutex is recovered, not propagated.
    pub fn resolve_record(&self, tx_id: &str, record_id: [u8; 16]) -> Result<u64, String> {
        let tenant = self.tenant;
        self.read_in_tx(tx_id, |txn| resolve_ordinal(txn, tenant, record_id))
    }

    /// Run a tracked read against an active transaction's view.
    fn read_in_tx<P>(
        &self,
        tx_id: &str,
        read: impl FnOnce(&mut rushdb_storage::Txn<'_>) -> Result<P, String>,
    ) -> Result<P, String> {
        let mut state = self.lock_state();
        let now = Instant::now();
        let expired = state
            .active
            .get(tx_id)
            .is_some_and(|tx| now >= tx.expires_at);
        let settled = state.expire(now, self.limits.max_expired_ids);
        drop(state);
        self.settle(settled);
        let mut state = self.lock_state();
        if expired || state.was_expired(tx_id) {
            return Err("transaction expired".into());
        }
        let tx = state
            .active
            .get_mut(tx_id)
            .ok_or("unknown transaction id")?;
        let out = self
            .store
            .tx_read(&mut tx.acc, read)
            .map_err(|e| e.to_string())?;
        state.last_activity = now;
        Ok(out)
    }

    /// Commit an active transaction, publishing all of its staged work as one
    /// atomic batch and revalidating the authorization generation captured at
    /// [`begin`](Self::begin). Retrying with the same `tx_id` discovers the
    /// durable outcome instead of re-executing.
    ///
    /// A poisoned state mutex is recovered, not propagated.
    pub fn commit(&self, tx_id: &str) -> TxOutcome {
        let key = commit_key_of(tx_id);
        match Self::discover(&self.store, tx_id) {
            Ok(Some(outcome)) => {
                self.lock_state().forget(tx_id);
                return outcome;
            }
            Ok(None) => {}
            Err(_) => return TxOutcome::UnknownRetryable,
        }
        let taken = {
            let mut state = self.lock_state();
            let now = Instant::now();
            let expired = state
                .active
                .get(tx_id)
                .is_some_and(|tx| now >= tx.expires_at);
            let settled = state.expire(now, self.limits.max_expired_ids);
            drop(state);
            self.settle(settled);
            let mut state = self.lock_state();
            if expired || state.was_expired(tx_id) {
                drop(state);
                // Settling an autocommit transaction commits it: report that
                // durable outcome now, not on the next call.
                return match Self::discover(&self.store, tx_id) {
                    Ok(Some(outcome)) => {
                        self.lock_state().forget(tx_id);
                        outcome
                    }
                    Ok(None) => TxOutcome::Expired,
                    Err(_) => TxOutcome::UnknownRetryable,
                };
            }
            match state.active.remove(tx_id) {
                Some(tx) => {
                    state.last_activity = now;
                    tx
                }
                None => return TxOutcome::UnknownRetryable,
            }
        };
        let authorizer = self.authorizer.clone();
        let captured_generation = taken.policy_generation;
        let tenant = self.tenant;
        match self
            .store
            .tx_commit_revalidated(key, &taken.acc, move |read| {
                if authorizer.is_none() {
                    return Ok(());
                }
                let Some(captured) = captured_generation else {
                    return Ok(());
                };
                // Revalidate the generation captured at BEGIN against live policy
                // state, read through the injected reader (no nested store view).
                let live = match rushdb_authz::load_policy_from_reader(read, tenant, u64::MAX)
                    .map_err(|e| e.to_string())?
                {
                    Some(active) => active.generation,
                    None => 0,
                };
                if live != captured {
                    return Err(
                        "authorization_changed: policy generation rotated during transaction"
                            .into(),
                    );
                }
                Ok(())
            }) {
            Ok(ci) => TxOutcome::Committed(ci),
            Err(e) => {
                let reason = e.to_string();
                if reason.contains("serializable_conflict")
                    || reason.contains("transaction_conflict")
                {
                    TxOutcome::ConflictRetryable(reason)
                } else {
                    TxOutcome::Aborted(reason)
                }
            }
        }
    }

    /// Native durable retry discovery needs no coordinator or staging quota.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`rushdb_storage::StorageError`] when the durable
    /// outcome cannot be read from the store.
    pub fn discover(
        store: &Store,
        tx_id: &str,
    ) -> Result<Option<TxOutcome>, rushdb_storage::StorageError> {
        Ok(store
            .try_outcome_for(commit_key_of(tx_id))?
            .map(|outcome| match outcome {
                OutcomeKind::Committed(ci) => TxOutcome::Committed(ci),
                OutcomeKind::Aborted => TxOutcome::Aborted("rolled back".into()),
            }))
    }

    /// Record a whole-keyspace read dependency for `tx_id`.
    ///
    /// Statement-language staging plans against a committed graph snapshot
    /// rather than the transaction's overlay, so its predicate read-set is not
    /// captured. Recording a whole-keyspace dependency conservatively orders
    /// the transaction against every concurrent writer, preventing phantoms at
    /// the cost of additional conflicts. Precise predicate/range locking is
    /// future work.
    ///
    /// # Errors
    /// Returns a message when `tx_id` is unknown.
    ///
    /// # Panics
    /// Panics if the internal state mutex is poisoned by a previous panic.
    pub fn record_whole_keyspace_dependency(&self, tx_id: &str) -> Result<(), String> {
        let mut state = self.lock_state();
        let tx = state
            .active
            .get_mut(tx_id)
            .ok_or("unknown transaction id")?;
        tx.acc.deps.record_prefix(&[]);
        Ok(())
    }

    /// Discard an active transaction and durably record an aborted outcome so a
    /// later commit with the same `tx_id` cannot report false success.
    ///
    /// # Errors
    ///
    /// Returns a message when `tx_id` is unknown or when the aborted outcome
    /// cannot be recorded.
    ///
    /// A poisoned state mutex is recovered, not propagated.
    pub fn rollback(&self, tx_id: &str) -> Result<(), String> {
        let mut state = self.lock_state();
        if state.active.remove(tx_id).is_none() {
            return Err("unknown transaction id".into());
        }
        state.last_activity = Instant::now();
        drop(state);
        self.store
            .record_aborted_outcome(commit_key_of(tx_id))
            .map_err(|e| format!("record aborted outcome: {e}"))
    }
}

/// The ordinal behind a public record id, read through the transaction so
/// staged creates resolve and the id row becomes a conflict dependency.
fn resolve_ordinal(
    txn: &mut rushdb_storage::Txn<'_>,
    tenant: TenantId,
    record_id: [u8; 16],
) -> Result<u64, String> {
    let raw = txn
        .get_tracked(&rushdb_types::keys::record_id_key(tenant, &record_id))
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!(
                "record not found: {}",
                uuid::Uuid::from_bytes(record_id).hyphenated()
            )
        })?;
    Ok(u64::from_be_bytes(
        raw.as_slice()
            .try_into()
            .map_err(|_| "corrupt record id row".to_string())?,
    ))
}

/// Rebuild one record's JSON view from the transaction's tracked reads,
/// recording every key as a conflict dependency.
fn reconstruct_record(
    txn: &mut rushdb_storage::Txn<'_>,
    tenant: TenantId,
    ordinal: u64,
    record_id: [u8; 16],
) -> Result<serde_json::Value, String> {
    // Tracked reads: the header row becomes the conflict dependency.
    let hdr_raw = txn
        .get_tracked(&rushdb_types::keys::record_hdr_key(tenant, ordinal))
        .map_err(|e| e.to_string())?
        .ok_or("record not found")?;
    let (stored_id, label) =
        rushdb_types::keys::decode_record_hdr(&hdr_raw).ok_or("corrupt record header")?;
    if stored_id != record_id {
        return Err("record identity mismatch".into());
    }
    let label = label.to_owned();
    let properties = build_properties(txn, tenant, ordinal)?;

    let mut record = serde_json::Map::new();
    record.insert(
        rushdb_types::value::ID_FIELD.into(),
        serde_json::Value::String(uuid::Uuid::from_bytes(record_id).simple().to_string()),
    );
    record.insert(
        rushdb_types::value::LABEL_FIELD.into(),
        serde_json::Value::String(label),
    );
    record.insert("properties".into(), serde_json::Value::Object(properties));
    Ok(serde_json::Value::Object(record))
}

/// Materialize a record's properties from its occurrence rows (one tracked
/// prefix scan, DC11), resolving family names through tracked reads.
fn build_properties(
    txn: &mut rushdb_storage::Txn<'_>,
    tenant: TenantId,
    ordinal: u64,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let mut by_pid: BTreeMap<u64, Vec<(bool, rushdb_types::occurrence::OccurrenceValue, Vec<u8>)>> =
        BTreeMap::new();
    for (key, raw) in txn
        .range_prefix_tracked(&rushdb_types::keys::occ_owner_prefix(
            tenant,
            OwnerKind::Record,
            ordinal,
        ))
        .map_err(|e| e.to_string())?
    {
        let (_, _, _, pid, _) =
            rushdb_types::keys::parse_occ_key(&key).ok_or("corrupt occurrence key")?;
        let occ = rushdb_types::occurrence::decode_occ_value(&raw)
            .map_err(|_| "corrupt occurrence row".to_string())?;
        by_pid
            .entry(pid)
            .or_default()
            .push((occ.array, occ.value, occ.payload.to_vec()));
    }
    let mut properties = serde_json::Map::new();
    for (pid, rows) in by_pid {
        let def_raw = txn
            .get_tracked(&kx::definition_row_key(tenant, pid))
            .map_err(|e| e.to_string())?
            .ok_or("corrupt definition")?;
        let def: rushdb_types::occurrence::PropertyDefinition =
            serde_json::from_slice(&def_raw).map_err(|e| format!("corrupt definition: {e}"))?;
        let fam_raw = txn
            .get_tracked(&kx::family_key(tenant, def.family_id))
            .map_err(|e| e.to_string())?
            .ok_or("corrupt family")?;
        let fam: rushdb_types::occurrence::PropertyFamily =
            serde_json::from_slice(&fam_raw).map_err(|e| format!("corrupt family: {e}"))?;
        let is_array = rows.iter().all(|(array, _, _)| *array);
        let empty_array = rows.len() == 1
            && matches!(
                rows[0].1,
                rushdb_types::occurrence::OccurrenceValue::EmptyArray
            );
        let values = rows
            .iter()
            .map(|(_, value, payload)| {
                rushdb_lmpg::reconstruct::decode_value(*value, payload).map_err(|e| e.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        properties.insert(
            fam.display_name,
            if empty_array {
                serde_json::Value::Array(vec![])
            } else if is_array {
                serde_json::Value::Array(values)
            } else {
                values.into_iter().next().unwrap_or(serde_json::Value::Null)
            },
        );
    }
    Ok(properties)
}

fn commit_key_of(tx_id: &str) -> uuid::Uuid {
    let mut bytes = [0u8; 16];
    let h = rushdb_types::hash::stable_hash64(tx_id.as_bytes());
    let h2 = rushdb_types::hash::stable_hash64(format!("{tx_id}:commit").as_bytes());
    bytes[..8].copy_from_slice(&h.to_be_bytes());
    bytes[8..].copy_from_slice(&h2.to_be_bytes());
    uuid::Uuid::from_bytes(bytes)
}

#[cfg(test)]
#[path = "coordinator_tests.rs"]
mod tests;
