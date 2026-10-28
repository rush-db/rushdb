//! Command admission, ordered publication, and transaction commit.
//! The store owns the health fence and publication locks across these phases.

use super::{
    log_key, mpsc, resources, wal, Arc, BTreeMap, BTreeSet, CommitBatch, CommitOutcome,
    CommitPhase, DependencySet, DeserializeOwned, Op, Ordering, OutcomeKind, OutcomeStatus,
    OverlayView, PendingCommit, PreparedCommand, RefCell, SequencerState, Serialize, StorageError,
    Store, TenantId, TxAccumulator, Txn, TxnBase, Uuid, LAST_APPLIED_KEY, SERIALIZABLE_CONFLICT,
};

impl Store {
    /// Data-map guard for a transaction base. `None` when the transaction
    /// carries a pinned snapshot (the snapshot owns the read, and taking this
    /// guard too would self-deadlock re-reading through the memory snapshot
    /// while holding `data.read()`, since std `RwLock` is not reentrant);
    /// otherwise the live committed data guard.
    fn txn_data(
        &self,
        snapshot: bool,
    ) -> Result<Option<std::sync::RwLockReadGuard<'_, BTreeMap<Vec<u8>, Vec<u8>>>>, StorageError>
    {
        if snapshot {
            Ok(None)
        } else {
            Ok(Some(self.health.guard(self.data.read())?))
        }
    }

    // Shared by every commit path (autocommit, serializable, explicit tx): reject an expanded
    // op-set (plus command) over `max_expanded_commit_bytes` before a slot is reserved, as a
    // retryable resource error rather than the store-fencing a durability failure would cause.
    fn reject_oversized_commit(
        &self,
        ops: &[Op],
        command_payload_len: usize,
    ) -> Result<(), StorageError> {
        let expanded_bytes = wal::op_set_encoded_len(ops).saturating_add(command_payload_len);
        if expanded_bytes > self.resource_config.max_expanded_commit_bytes {
            return Err(StorageError::ResourceExhausted(format!(
                "expanded mutation batch exceeds {} bytes (got {expanded_bytes})",
                self.resource_config.max_expanded_commit_bytes
            )));
        }
        Ok(())
    }

    /// Deterministic expansion + commit bookkeeping for `batch` at commit index
    /// `ci`, against whatever base `txn` is positioned at (live committed
    /// state, a pinned snapshot, or a sequencer overlay). Returns the drained
    /// op-set exactly as the forward publish path persists it. This is the one
    /// home for expansion: the live commit path and the sequencer overlay path
    /// funnel through it, so their op-sets cannot drift.
    ///
    /// # Errors
    /// Returns [`StorageError::CorruptLog`] when no expander is installed and
    /// [`StorageError::Builder`] when expansion fails.
    fn expand_commit_ops(
        &self,
        ci: u64,
        batch: &CommitBatch,
        payload_bytes: &[u8],
        mut txn: Txn<'_>,
    ) -> Result<Vec<Op>, StorageError> {
        let expander = self.health.guard(self.expander.read())?.clone();
        let Some(expand) = expander.as_ref() else {
            return Err(StorageError::CorruptLog);
        };
        expand(ci, batch, &mut txn).map_err(StorageError::Builder)?;
        // A durable store derives the journal from the WAL instead;
        // an in-memory store has no WAL, so it keeps this row (test-mode only).
        if !self.is_durable() {
            let command_payload = rushdb_types::command_codec::encode(batch);
            txn.put(log_key(ci), wal::encode_wal_record(0, ci, &command_payload));
        }
        self.put_receipt(
            &mut txn,
            batch.commit_key,
            OutcomeStatus::Committed,
            ci,
            Some(payload_bytes),
        );
        txn.set(LAST_APPLIED_KEY.to_vec(), wal::encode_last_applied(0, ci));
        Ok(txn.drain())
    }

    /// Sequencer expansion primitive: the deterministic ordered op-set for
    /// `batch` at commit index `ci` computed against `overlay` — a committed
    /// base plus lower-index *pending* ops — instead of the live committed
    /// state. Deterministic and side-effect free: takes no admission, shard
    /// or publish lock, and publishes nothing.
    ///
    /// When `overlay` carries the expanded output of commits `< ci`, the
    /// result is byte-identical to the serialized forward path publishing
    /// those commits first (`sequencer_tests` asserts the equivalence). The
    /// sequencer calls this to expand without holding the shard lock
    /// across durability.
    ///
    /// # Errors
    /// Returns the same errors as forward expansion: a base read error, or
    /// [`StorageError::CorruptLog`]/[`StorageError::Builder`] when no
    /// expander is installed or expansion fails.
    pub fn expand_against_overlay<P>(
        &self,
        ci: u64,
        batch: &CommitBatch,
        payload: &P,
        overlay: Arc<OverlayView>,
    ) -> Result<Vec<Op>, StorageError>
    where
        P: Serialize,
    {
        self.check_health()?;
        let payload_bytes =
            serde_json::to_vec(payload).map_err(|_| StorageError::OutcomePayload)?;
        let txn = Txn {
            base: TxnBase::Overlay(overlay),
            buffer: BTreeMap::new(),
            purge_keys: BTreeSet::default(),
            set_keys: BTreeSet::default(),
            deps: None,
        };
        self.expand_commit_ops(ci, batch, &payload_bytes, txn)
    }

    /// Autocommit path: admission builds and validates a canonical
    /// `CommitBatch` inside `build`, storage publishes everything atomically.
    /// Retrying with the same commit key discovers the durable outcome.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the publisher has failed, a precondition is
    /// violated, expansion fails, or durable publication fails.
    pub fn commit<P, F>(&self, commit_key: Uuid, build: F) -> Result<CommitOutcome<P>, StorageError>
    where
        P: Serialize + DeserializeOwned,
        F: FnOnce(u64, &mut Txn) -> Result<(CommitBatch, P), String>,
    {
        self.check_health()?;
        self.commit_sharded(0, None, commit_key, None, build)
    }

    /// Tenant-aware autocommit. Outer shards preserve per-tenant replication
    /// ordering; the local logical writer serializes planning and expansion.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the publisher has failed, a precondition is
    /// violated, expansion fails, or durable publication fails.
    pub fn commit_for_tenant<P, F>(
        &self,
        tenant: &TenantId,
        commit_key: Uuid,
        build: F,
    ) -> Result<CommitOutcome<P>, StorageError>
    where
        P: Serialize + DeserializeOwned,
        F: FnOnce(u64, &mut Txn) -> Result<(CommitBatch, P), String>,
    {
        self.check_health()?;
        let shard = Store::shard_for(Some(tenant));
        self.commit_sharded(shard, Some(tenant), commit_key, None, build)
    }

    /// Tenant-aware autocommit of a batch planned at an earlier snapshot: it
    /// commits only if no commit after `snapshot_ci` wrote a key `deps`
    /// covers (published or still in flight); otherwise it is
    /// [`CommitOutcome::Conflicted`] with a `serializable_conflict` reason.
    ///
    /// # Errors
    /// As [`Store::commit_for_tenant`], plus
    /// [`StorageError::ConflictHistoryUnavailable`] when the conflict window
    /// no longer holds a commit the check needs (retryable).
    pub fn commit_for_tenant_since<P, F>(
        &self,
        tenant: &TenantId,
        commit_key: Uuid,
        snapshot_ci: u64,
        deps: &DependencySet,
        build: F,
    ) -> Result<CommitOutcome<P>, StorageError>
    where
        P: Serialize + DeserializeOwned,
        F: FnOnce(u64, &mut Txn) -> Result<(CommitBatch, P), String>,
    {
        self.check_health()?;
        let shard = Store::shard_for(Some(tenant));
        self.commit_sharded(
            shard,
            Some(tenant),
            commit_key,
            Some((snapshot_ci, deps)),
            build,
        )
    }

    /// The first commit after `snapshot_ci` that wrote a key `deps` covers:
    /// published commits from the conflict window, in-flight ones from the
    /// admission buffer. The caller holds admission (so no commit is
    /// assigned meanwhile) and has drained publication.
    fn first_conflict_since(
        &self,
        admission: &SequencerState,
        snapshot_ci: u64,
        deps: &DependencySet,
    ) -> Result<Option<u64>, StorageError> {
        let published = self.published.load(Ordering::SeqCst);
        let window = self.health.guard(self.conflicts.lock())?;
        for probe in (snapshot_ci + 1)..=published {
            let key_set = window
                .keys(probe)
                .ok_or(StorageError::ConflictHistoryUnavailable {
                    commit_index: probe,
                })?;
            if key_set.iter().any(|key| deps.intersects(key)) {
                return Ok(Some(probe));
            }
        }
        drop(window);
        Ok(admission
            .buffer
            .iter()
            .filter(|(key, (ci, _))| *ci > snapshot_ci && deps.intersects(key))
            .map(|(_, (ci, _))| *ci)
            .min())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the commit state machine keeps admission, expansion, slot reservation, and publication together so the ordering contract is auditable in one place"
    )]
    pub(super) fn commit_sharded<P, F>(
        &self,
        shard: usize,
        routed_tenant: Option<&TenantId>,
        commit_key: Uuid,
        since: Option<(u64, &DependencySet)>,
        build: F,
    ) -> Result<CommitOutcome<P>, StorageError>
    where
        P: Serialize + DeserializeOwned,
        F: FnOnce(u64, &mut Txn) -> Result<(CommitBatch, P), String>,
    {
        self.check_health()?;
        let shard_guard = self.health.guard(self.shards[shard].lock())?;
        // Replication re-enters through apply_replicated_batch; do not hold
        // this mutex across the replication call. Local writes hold it through
        // planning + expansion + enqueue, then release it before the durability
        // wait so concurrent commits can coalesce; every expansion reads
        // the preceding logical state from the pending overlay instead.
        let replicator = self.replicator();
        let admission_started = std::time::Instant::now();
        let mut admission = if replicator.is_none() {
            Some(self.acquire_admission()?)
        } else {
            None
        };
        self.observe_phase(CommitPhase::AdmissionWait, admission_started);
        self.check_publisher()?;
        if let Some((status, prev_ci)) = self.lookup_outcome_checked(commit_key)? {
            return match status {
                OutcomeStatus::Aborted => Ok(CommitOutcome::Conflicted(
                    "previously aborted under this commit key".to_string(),
                )),
                OutcomeStatus::Committed => Ok(CommitOutcome::Committed {
                    commit_index: prev_ci,
                    payload: self.committed_payload(commit_key)?,
                }),
            };
        }

        // Admission builds + validates against a stable read view. Aborts do
        // NOT consume a commit-index slot, keeping the log dense and replay
        // position-indexed. `ci` is only reserved after the build succeeds.
        // Refresh the pending overlay, then plan and expand against it so the
        // command sees every lower-index commit whether or not it is published
        // yet (the property the shard lock used to guarantee). Phase A holds
        // admission across publication, so the overlay is empty here and this
        // is behavior-preserving; Phase C releases it before the ack wait.
        if let Some(state) = admission.as_mut() {
            self.trim_sequencer(state);
        }
        let sequencer_overlay = match admission.as_ref() {
            Some(state) => Some(self.sequencer_overlay(state)?),
            None => None,
        };
        let exists_in_view = |key: &[u8]| -> Result<bool, StorageError> {
            match &sequencer_overlay {
                Some(overlay) => Ok(overlay.get(key)?.is_some()),
                None => Ok(self.try_get(key)?.is_some()),
            }
        };
        let provisional = self.next_commit_index.load(Ordering::SeqCst);
        let build_deps = RefCell::new(DependencySet::default());
        let (mut batch, payload) = {
            // When an overlay is present it is authoritative, so the base reads
            // go through it and never touch the live map. Acquiring
            // `data.read()` here and then reading through the overlay would
            // re-enter the same in-memory RwLock while a publisher waits on the
            // write side — a self-deadlock.
            let guard = if sequencer_overlay.is_none() {
                Some(self.health.guard(self.data.read())?)
            } else {
                None
            };
            let native = if sequencer_overlay.is_none() {
                self.native_transaction_view()?
            } else {
                None
            };
            let base = TxnBase::resolve(sequencer_overlay.clone(), None, native, guard.as_deref());
            let mut txn = Txn {
                base,
                buffer: BTreeMap::new(),
                purge_keys: BTreeSet::default(),
                set_keys: BTreeSet::default(),
                deps: Some(&build_deps),
            };
            let build_started = std::time::Instant::now();
            let build_result = build(provisional, &mut txn);
            self.observe_phase(CommitPhase::Build, build_started);
            match build_result {
                Ok(outcome) => outcome,
                Err(reason) => {
                    drop(txn);
                    if admission.is_some() {
                        self.record_aborted_outcome_admitted(commit_key)?;
                    } else {
                        self.record_aborted_outcome(commit_key)?;
                    }
                    return Ok(CommitOutcome::Conflicted(reason));
                }
            }
        };
        // The storage boundary owns idempotency. Planners cannot accidentally
        // emit a nil or mismatched key into the replicated command log.
        batch.commit_key = commit_key;
        if routed_tenant.is_some_and(|tenant| batch.tenant != *tenant) {
            return Err(StorageError::Builder(
                "commit batch tenant does not match the routed tenant".into(),
            ));
        }

        // When a replication seam is installed, admission still validates
        // locally (leader planning), then the batch is routed through the
        // leader. `apply_replicated_batch` on this node publishes the outcome
        // durably, and the returned commit index + payload reproduce the
        // caller's response. Aborts stay local and durable.
        if let Some(replicator) = replicator {
            for pc in &batch.preconditions {
                let exists = self.try_get(&pc.key)?.is_some();
                let violated =
                    exists != matches!(pc.kind, rushdb_types::command::PreconditionKind::Exists);
                if violated {
                    self.record_aborted_outcome(commit_key)?;
                    return Err(StorageError::Precondition {
                        path: rushdb_types::keys::describe_key(&pc.key),
                    });
                }
            }
            let payload_bytes =
                serde_json::to_vec(&payload).map_err(|_| StorageError::OutcomePayload)?;
            replicator
                .replicate(&batch, &payload_bytes)
                .map_err(StorageError::Builder)?;
            return match self.lookup_outcome_checked(commit_key)? {
                Some((OutcomeStatus::Committed, ci)) => Ok(CommitOutcome::Committed {
                    commit_index: ci,
                    payload,
                }),
                _ => Err(StorageError::CorruptLog),
            };
        }

        if let (Some((snapshot_ci, deps)), Some(state)) = (since, admission.as_ref()) {
            {
                let _publish = self.health.guard(self.publish_lock.lock())?;
                self.drain_and_publish();
            }
            if let Some(probe) = self.first_conflict_since(state, snapshot_ci, deps)? {
                self.record_aborted_outcome_admitted(commit_key)?;
                return Ok(CommitOutcome::Conflicted(format!(
                    "{SERIALIZABLE_CONFLICT} on write at ci {probe}"
                )));
            }
        }

        for pc in &batch.preconditions {
            let exists = exists_in_view(&pc.key)?;
            let violated =
                exists != matches!(pc.kind, rushdb_types::command::PreconditionKind::Exists);
            if violated {
                // Terminal and durable: a retry under the same commit key
                // must discover `aborted`, never run again or report success.
                self.record_aborted_outcome_admitted(commit_key)?;
                return Err(StorageError::Precondition {
                    path: rushdb_types::keys::describe_key(&pc.key),
                });
            }
        }

        let provisional = self.next_commit_index.load(Ordering::SeqCst);
        batch.start_index = provisional.saturating_sub(1);

        // Deterministic expansion + log/outcome bookkeeping. The expansion
        // runs BEFORE the slot is reserved so an expansion failure does not
        // consume a commit index (the log stays dense). The reservation is a
        // CAS from the expansion's `provisional`; if another admission
        // reserved in between, the expansion is retried at the current value.
        let payload_bytes =
            serde_json::to_vec(&payload).map_err(|_| StorageError::OutcomePayload)?;
        let expand_at = |ci: u64| -> Result<(Vec<Op>, DependencySet), StorageError> {
            let expand_deps = RefCell::new(DependencySet::default());
            // As in the build block: with an overlay the live map is never
            // read, so do not take its read guard (which the overlay's base
            // read would re-enter).
            let guard = if sequencer_overlay.is_none() {
                Some(self.health.guard(self.data.read())?)
            } else {
                None
            };
            let native = if sequencer_overlay.is_none() {
                self.native_transaction_view()?
            } else {
                None
            };
            let base = TxnBase::resolve(sequencer_overlay.clone(), None, native, guard.as_deref());
            let wtxn = Txn {
                base,
                buffer: BTreeMap::new(),
                purge_keys: BTreeSet::default(),
                set_keys: BTreeSet::default(),
                // Record expansion reads so `prepared.reads` is non-empty and
                // admission is held through publication. Otherwise a
                // read-dependent expander (e.g. a counter) releases admission
                // early and concurrent commits overwrite each other.
                deps: Some(&expand_deps),
            };
            let ops = self.expand_commit_ops(ci, &batch, &payload_bytes, wtxn)?;
            Ok((ops, expand_deps.into_inner()))
        };

        // Computed once, up front, so the size check and the WAL/outcome writers never re-serialize it.
        let command_payload = rushdb_types::command_codec::encode(&batch);

        // Reserve the slot with a CAS loop: expansion runs *before* reservation so a failed
        // expansion consumes no commit index (the log stays dense); if another admission
        // reserved in between, re-read and re-expand at the new value — never `fetch_add` a
        // slot before expansion, which would leak a hole on a failed retry. The size check
        // runs inside the loop, before reservation.
        let expand_started = std::time::Instant::now();
        let (ci, ops, expand_deps) = loop {
            let ci = self.next_commit_index.load(Ordering::SeqCst);
            let (ops, deps) = expand_at(ci)?;
            self.reject_oversized_commit(&ops, command_payload.len())?;
            if self
                .next_commit_index
                .compare_exchange(ci, ci + 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                break (ci, ops, deps);
            }
        };
        self.observe_phase(CommitPhase::Expand, expand_started);

        let shard =
            resources::physical_shard_for(Some(&batch.tenant), self.resource_config.shard_count);
        let mut prepared = PreparedCommand::from_ops(ci, &ops);
        let mut build_deps = build_deps.into_inner();
        build_deps.extend(expand_deps);
        prepared.reads = build_deps;
        // Planned and expanded against the sequencer overlay under admission
        // (the replicated path returned above), so every predecessor's writes
        // were visible to this command's reads.
        prepared.sequenced = sequencer_overlay.is_some();
        self.enqueue_and_publish_prepared(
            ci,
            shard,
            ops,
            command_payload,
            Some(prepared),
            admission,
            Some(shard_guard),
        )?;
        Ok(CommitOutcome::Committed {
            commit_index: ci,
            payload,
        })
    }

    /// Enqueue a fully-admitted op-set and wait until it is durably
    /// persisted + applied. The committing thread attempts to become the
    /// group-commit publisher; if another thread already holds the publish
    /// lock, that publisher drains everything currently pending (including
    /// this entry) and acks it.
    pub(super) fn enqueue_and_publish(
        &self,
        ci: u64,
        shard: usize,
        ops: Vec<Op>,
        command_payload: Vec<u8>,
    ) -> Result<(), StorageError> {
        self.check_health()?;
        self.enqueue_and_publish_prepared(ci, shard, ops, command_payload, None, None, None)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "one ordered enqueue carries the reserved index, shard, ops, originating command, prepared metadata, and the two serialization guards it releases before the durability wait"
    )]
    pub(super) fn enqueue_and_publish_prepared(
        &self,
        ci: u64,
        shard: usize,
        ops: Vec<Op>,
        command_payload: Vec<u8>,
        prepared: Option<PreparedCommand>,
        mut admission: Option<std::sync::MutexGuard<'_, SequencerState>>,
        shard_guard: Option<std::sync::MutexGuard<'_, ()>>,
    ) -> Result<(), StorageError> {
        let started = std::time::Instant::now();
        self.check_health()?;
        if self.maintenance_aborted() {
            return Err(StorageError::Durability(
                "store is shutting down; commit was not enqueued".into(),
            ));
        }
        let (ack_tx, ack_rx) = mpsc::channel();
        {
            let mut pending = self.health.guard(self.pending.lock())?;
            self.check_publisher()?;
            // Record the ordered commit in the pending overlay *before* it can
            // be published, so the very next writer to acquire admission sees
            // it. Recording under `pending` means an aborted enqueue cannot
            // leave a phantom overlay entry.
            if let Some(state) = admission.as_mut() {
                state.record(ci, &ops);
            }
            pending.insert(
                ci,
                PendingCommit {
                    ci,
                    shard,
                    prepared: prepared.unwrap_or_else(|| PreparedCommand::from_ops(ci, &ops)),
                    ops,
                    command_payload,
                    ack: ack_tx,
                },
            );
        }
        // The ordered entry is now visible in `pending`, so the next writer may
        // sequence. Release the serialization locks *before* waiting on
        // durability: concurrent commits then accumulate in `pending` and
        // `drain_and_publish` coalesces them into one fsync. Their expansions
        // already read this commit's ops from the pending overlay, so releasing
        // here cannot expose them to stale state.
        drop(admission);
        drop(shard_guard);
        self.pending_cv.notify_all();

        loop {
            // Try to become the publisher — only until this commit is
            // published; later commits belong to their own committers.
            if let Ok(guard) = self.publish_lock.try_lock() {
                self.drain_and_publish_until(Some(ci));
                drop(guard);
            }
            match ack_rx.try_recv() {
                Ok(outcome) => {
                    if outcome.is_ok() {
                        self.observe_phase(CommitPhase::Durable, started);
                    }
                    return outcome;
                }
                Err(mpsc::TryRecvError::Disconnected) => return Err(StorageError::CorruptLog),
                Err(mpsc::TryRecvError::Empty) => {}
            }
            // Wait for publish progress (a lower ci may still be admitted).
            let mut pending = self.health.guard(self.pending.lock())?;
            match ack_rx.try_recv() {
                Ok(outcome) => {
                    if outcome.is_ok() {
                        self.observe_phase(CommitPhase::Durable, started);
                    }
                    return outcome;
                }
                Err(mpsc::TryRecvError::Disconnected) => return Err(StorageError::CorruptLog),
                Err(mpsc::TryRecvError::Empty) => {}
            }
            self.check_publisher()?;
            // A shutdown must not leave a committer waiting forever for a
            // publisher that will never run. Atomically dequeue our own entry
            // under `pending`: if it is still queued, no publisher can have
            // published it, so a terminal error is accurate. If a publisher
            // already took it, the commit is in flight — keep waiting for its
            // ack (or the dropped-sender `Disconnected` error) so we report the
            // true outcome instead of a spurious failure.
            if self.maintenance_aborted() && pending.remove(&ci).is_some() {
                return Err(StorageError::Durability(
                    "store is shutting down before the commit was published".into(),
                ));
            }
            // A read-view/maintenance operation may hold publish_lock without
            // becoming a publisher. Retry after a bounded wait in that case;
            // a notification must not be the only route to terminal progress.
            pending = self
                .health
                .guard(
                    self.pending_cv
                        .wait_timeout(pending, std::time::Duration::from_millis(10)),
                )?
                .0;
            drop(pending);
        }
    }

    /// Serializable-optimistic entry point (document 07): `deps` records
    /// point/range reads performed at `snapshot_ci`; validation rejects if
    /// any committed mutation between then and now writes into a recorded
    /// range. Runs while the publish lock is held so validation and
    /// publication are atomic; a loser records its terminal-aborted outcome.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the publisher has failed, a precondition is
    /// violated, a conflict summary is missing, the snapshot is in the future,
    /// or publication fails. A poisoned admission, publish, or data lock
    /// permanently fences the store and returns [`StorageError::Unhealthy`].
    ///
    /// # Panics
    /// Panics only if `batch` fails to serialize (never, for a well-formed
    /// `CommitBatch`).
    pub fn commit_serializable<P, F>(
        &self,
        commit_key: Uuid,
        snapshot_ci: u64,
        deps: DependencySet,
        build: F,
    ) -> Result<CommitOutcome<P>, StorageError>
    where
        P: Serialize + DeserializeOwned,
        F: FnOnce(&mut Txn) -> Result<(CommitBatch, P), String>,
    {
        self.check_health()?;
        let mut admission = self.acquire_admission()?;
        self.check_publisher()?;
        if let Some((status, prev_ci)) = self.lookup_outcome_checked(commit_key)? {
            return match status {
                OutcomeStatus::Aborted => Ok(CommitOutcome::Conflicted(
                    "previously aborted under this commit key".to_string(),
                )),
                OutcomeStatus::Committed => Ok(CommitOutcome::Committed {
                    commit_index: prev_ci,
                    payload: self.committed_payload(commit_key)?,
                }),
            };
        }

        if snapshot_ci > self.committed_version() {
            return Err(StorageError::FutureSnapshot {
                requested: snapshot_ci,
                visible: self.committed_version(),
            });
        }
        // Refresh the pending overlay and plan/expand against it so this commit
        // sees every lower-index commit, published or not. In-flight writes are
        // additionally checked against `deps` below because they are not yet in
        // the conflict window.
        self.trim_sequencer(&mut admission);
        let sequencer_overlay = self.sequencer_overlay(&admission)?;
        // This entry point preserves the existing staging-at-commit contract;
        // supplied dependencies describe the caller's earlier reads. It does
        // not itself implement begin-time transactional reads.
        let dep_recorder = RefCell::new(deps);
        let (mut batch, payload) = {
            // The sequencer overlay is authoritative, so base reads go through
            // it and never touch the live map; taking its read guard would let
            // the overlay's base read re-enter the same RwLock (self-deadlock).
            let mut txn = Txn {
                base: TxnBase::Overlay(Arc::clone(&sequencer_overlay)),
                buffer: BTreeMap::new(),
                purge_keys: BTreeSet::default(),
                set_keys: BTreeSet::default(),
                deps: Some(&dep_recorder),
            };
            match build(&mut txn) {
                Ok(outcome) => outcome,
                Err(reason) => {
                    drop(txn);
                    self.record_aborted_outcome_admitted(commit_key)?;
                    return Ok(CommitOutcome::Conflicted(reason));
                }
            }
        };
        let deps = dep_recorder.into_inner();
        batch.commit_key = commit_key;

        // Admission excludes concurrent logical writers through publication.
        // Select a tentative CI; reserve it only after successful expansion.
        let ci = {
            let _guard = self.health.guard(self.publish_lock.lock())?;
            // Publish any pending commits so the committed log is complete.
            self.drain_and_publish();

            // Never replay an old command against today's state to infer what
            // it once wrote. Missing legacy summaries are an explicit retry,
            // not evidence of the absence of a conflict.
            // In-flight commits (assigned but not yet published) are not in
            // the conflict window; `first_conflict_since` consults the pending
            // overlay too, so a dependency cannot be missed against an
            // unpublished writer.
            if let Some(probe) = self.first_conflict_since(&admission, snapshot_ci, &deps)? {
                // An aborted tx consumes no commit index; the terminal aborted
                // outcome is keyed by the commit key, so the contiguous
                // publisher never has to fill a hole for it.
                self.record_outcome_locked(commit_key, OutcomeStatus::Aborted, 0)?;
                return Ok(CommitOutcome::Conflicted(format!(
                    "{SERIALIZABLE_CONFLICT} on write at ci {probe}"
                )));
            }

            for pc in &batch.preconditions {
                let exists = sequencer_overlay.get(&pc.key)?.is_some();
                let violated =
                    exists != matches!(pc.kind, rushdb_types::command::PreconditionKind::Exists);
                if violated {
                    self.record_outcome_locked(commit_key, OutcomeStatus::Aborted, 0)?;
                    return Err(StorageError::Precondition {
                        path: rushdb_types::keys::describe_key(&pc.key),
                    });
                }
            }

            // The candidate slot is derived from the published watermark; the
            // actual reservation happens after expansion succeeds so an
            // expansion failure never consumes a commit index.
            self.published.load(Ordering::SeqCst) + 1
        };

        // Feeds the WAL's embedded command on a durable store, or
        // the in-memory command-journal row otherwise (no WAL to derive it from).
        let command_payload = rushdb_types::command_codec::encode(&batch);
        let payload_bytes =
            serde_json::to_vec(&payload).map_err(|_| StorageError::OutcomePayload)?;
        let expand_at = |ci: u64| -> Result<Vec<Op>, StorageError> {
            // The overlay is authoritative; skip the live map's read guard.
            let wtxn = Txn {
                base: TxnBase::Overlay(Arc::clone(&sequencer_overlay)),
                buffer: BTreeMap::new(),
                purge_keys: BTreeSet::default(),
                set_keys: BTreeSet::default(),
                deps: None,
            };
            self.expand_commit_ops(ci, &batch, &payload_bytes, wtxn)
        };

        // Admission is held for the whole protocol, so the prospective `ci`
        // cannot move. Expand first (a failure must not consume a slot), then
        // reserve with a plain store; the previous CAS/`fetch_add` fallback
        // could leak a reserved index when the re-expansion failed.
        let ops = expand_at(ci)?;
        self.reject_oversized_commit(&ops, command_payload.len())?;
        admission.record(ci, &ops);
        self.next_commit_index.store(ci + 1, Ordering::SeqCst);
        let shard =
            resources::physical_shard_for(Some(&batch.tenant), self.resource_config.shard_count);
        self.enqueue_and_publish(ci, shard, ops, command_payload)?;
        Ok(CommitOutcome::Committed {
            commit_index: ci,
            payload,
        })
    }

    /// Run one statement's planning + expansion against a transaction's
    /// accumulated overlay (read-your-writes), appending the produced writes to
    /// the transaction. Nothing is committed or visible outside the tx.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the publisher has failed, no expander is
    /// installed, or the supplied `build`/expansion closure fails.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn tx_apply<P, F>(
        &self,
        tx: &mut TxAccumulator,
        commit_key: Uuid,
        build: F,
    ) -> Result<P, StorageError>
    where
        P: Serialize + DeserializeOwned,
        F: FnOnce(u64, &mut Txn) -> Result<(CommitBatch, P), String>,
    {
        self.check_health()?;
        self.check_publisher()?;
        let ci = self.next_commit_index.load(Ordering::SeqCst);
        let payload = {
            // Statement planning reads the transaction's pinned snapshot, not
            // current committed state, so read-your-writes is stable and
            // later commits cannot leak into the plan.
            let snapshot = tx.base_view.is_some();
            let guard = self.txn_data(snapshot)?;
            let native = if snapshot {
                None
            } else {
                self.native_transaction_view()?
            };
            let base = TxnBase::resolve(None, tx.base_view.clone(), native, guard.as_deref());
            // Both statement planning and deterministic expansion can branch
            // on committed state. Preserve those reads in the accumulator so
            // tx_commit can reject a stale read-dependent transaction even
            // when the branch did not emit the key it inspected.
            let dep_recorder = RefCell::new(tx.deps.clone());
            let mut txn = Txn {
                base,
                buffer: tx.buffer.clone(),
                purge_keys: BTreeSet::default(),
                set_keys: BTreeSet::default(),
                deps: Some(&dep_recorder),
            };
            let (batch, payload) = build(ci, &mut txn).map_err(StorageError::Builder)?;
            let mut batch = batch;
            batch.commit_key = commit_key;
            let expander = self.health.guard(self.expander.read())?.clone();
            let Some(expand) = expander.as_ref() else {
                return Err(StorageError::CorruptLog);
            };
            expand(ci, &batch, &mut txn).map_err(StorageError::Builder)?;
            tx.buffer = txn.buffer;
            tx.mutations.append(&mut batch.mutations);
            tx.preconditions.append(&mut batch.preconditions);
            tx.deps = dep_recorder.into_inner();
            payload
        };
        Ok(payload)
    }

    /// Run a read inside a transaction: the closure sees committed state merged
    /// with the transaction's overlay (read-your-writes) and records its reads
    /// into the transaction's dependency set. Use `get_tracked`/range-tracked
    /// reads so the conflict check at commit covers what this read observed.
    ///
    /// # Errors
    /// Returns [`StorageError`] if a backend read fails or the closure returns
    /// an error.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn tx_read<P, F>(&self, tx: &mut TxAccumulator, f: F) -> Result<P, StorageError>
    where
        F: FnOnce(&mut Txn) -> Result<P, String>,
    {
        self.check_health()?;
        // Reads resolve at the transaction's pinned snapshot.
        let snapshot = tx.base_view.is_some();
        let guard = self.txn_data(snapshot)?;
        let native = if snapshot {
            None
        } else {
            self.native_transaction_view()?
        };
        let base = TxnBase::resolve(None, tx.base_view.clone(), native, guard.as_deref());
        let dep_recorder = RefCell::new(tx.deps.clone());
        let mut txn = Txn {
            base,
            buffer: tx.buffer.clone(),
            purge_keys: BTreeSet::default(),
            set_keys: BTreeSet::default(),
            deps: Some(&dep_recorder),
        };
        let result = f(&mut txn).map_err(StorageError::Builder);
        tx.deps = dep_recorder.into_inner();
        result
    }

    /// Stage a pre-built commit batch (e.g. a statement-language mutation delta)
    /// into a transaction: the batch's writes expand against the transaction's
    /// overlay and append to its accumulated mutations. Nothing is committed or
    /// visible outside the tx.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the publisher has failed, no expander is
    /// installed, or expansion fails.
    ///
    /// Lock poisoning fences the store until reopen.
    pub fn tx_apply_batch(
        &self,
        tx: &mut TxAccumulator,
        commit_key: Uuid,
        batch: &CommitBatch,
    ) -> Result<(), StorageError> {
        self.check_health()?;
        self.check_publisher()?;
        let mut batch = batch.clone();
        batch.commit_key = commit_key;
        let ci = self.next_commit_index.load(Ordering::SeqCst);
        // Batch expansion reads the transaction's pinned snapshot.
        let snapshot = tx.base_view.is_some();
        let guard = self.txn_data(snapshot)?;
        let native = if snapshot {
            None
        } else {
            self.native_transaction_view()?
        };
        let base = TxnBase::resolve(None, tx.base_view.clone(), native, guard.as_deref());
        // A batch expander may read a key to decide which derived rows to
        // produce. Track that dependency just like tx_apply tracks planner
        // and expansion reads; otherwise stale read-dependent commits can
        // pass conflict validation when their write-set is disjoint.
        let dep_recorder = RefCell::new(tx.deps.clone());
        let mut txn = Txn {
            base,
            buffer: tx.buffer.clone(),
            purge_keys: BTreeSet::default(),
            set_keys: BTreeSet::default(),
            deps: Some(&dep_recorder),
        };
        let expander = self.health.guard(self.expander.read())?.clone();
        let Some(expand) = expander.as_ref() else {
            return Err(StorageError::CorruptLog);
        };
        expand(ci, &batch, &mut txn).map_err(StorageError::Builder)?;
        tx.buffer = txn.buffer;
        tx.mutations.append(&mut batch.mutations);
        tx.preconditions.append(&mut batch.preconditions);
        tx.deps = dep_recorder.into_inner();
        Ok(())
    }

    /// Atomically commit a transaction's accumulated writes. Conflict rule: any
    /// committed mutation since `base_ci` that wrote a key the transaction read
    /// (public read-set) or writes (write-set, including counters) aborts it —
    /// this catches read-write anomalies and ordinal/counter clashes without the
    /// planner tracking its internal reads. Success publishes the writes as one
    /// atomic commit; conflict aborts the transaction, which may retry or roll back.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the publisher has failed, the base snapshot
    /// is in the future, a precondition fails, a conflict is detected, or
    /// publication fails.
    pub fn tx_commit(&self, commit_key: Uuid, tx: &TxAccumulator) -> Result<u64, StorageError> {
        self.check_health()?;
        self.tx_commit_impl(commit_key, tx, None)
    }

    /// Commit a cross-request transaction with an optional revalidator that runs inside
    /// the publish lock (atomic with conflict detection), reading keys via the injected
    /// reader — never by opening a new store view, which would nest view locks under the
    /// publish lock — and returning `Err` to abort the commit and record a terminal outcome.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the publisher has failed, the base snapshot is in the future,
    /// a precondition fails, the revalidator rejects, a conflict is detected, or publication fails.
    pub fn tx_commit_revalidated(
        &self,
        commit_key: Uuid,
        tx: &TxAccumulator,
        revalidate: impl FnOnce(&dyn Fn(&[u8]) -> Result<Option<Vec<u8>>, StorageError>) -> Result<(), String>
            + 'static,
    ) -> Result<u64, StorageError> {
        self.check_health()?;
        let revalidate: Box<
            dyn FnOnce(
                &dyn Fn(&[u8]) -> Result<Option<Vec<u8>>, StorageError>,
            ) -> Result<(), String>,
        > = Box::new(revalidate);
        self.tx_commit_impl(commit_key, tx, Some(revalidate))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "transaction commit keeps conflict probing, revalidation, index reservation, and publication in one linear protocol for auditability"
    )]
    pub(super) fn tx_commit_impl(
        &self,
        commit_key: Uuid,
        tx: &TxAccumulator,
        revalidate: Option<
            Box<
                dyn FnOnce(
                    &dyn Fn(&[u8]) -> Result<Option<Vec<u8>>, StorageError>,
                ) -> Result<(), String>,
            >,
        >,
    ) -> Result<u64, StorageError> {
        self.check_health()?;
        // A replicated commit is assigned and published by the replication
        // path. Reserving a local slot first would advance
        // `next_commit_index`; a loopback replicator would then apply at the
        // following index and leave the reserved slot permanently missing.
        // Keep the selected replicator for the call below so the admission
        // decision and the publication path agree.
        let replicator = self.replicator();
        // One writer-admission protocol for the whole commit: discovery,
        // validation, expansion, CI reservation, publication and ack. Without
        // it, two transactions can validate against the same published
        // watermark and both reserve, missing interleaved write/read conflicts
        // (a lost update). A replicator owns ordering, so admission is skipped
        // there exactly as in the autocommit path (holding it across the
        // synchronous replication call would deadlock the state-machine apply).
        let mut admission = if replicator.is_none() {
            Some(self.acquire_admission()?)
        } else {
            None
        };
        self.check_publisher()?;
        if let Some((OutcomeStatus::Committed, ci)) = self.lookup_outcome_checked(commit_key)? {
            return Ok(ci);
        }
        if matches!(
            self.try_outcome_for(commit_key)?,
            Some(OutcomeKind::Aborted)
        ) {
            return Err(StorageError::Builder(
                "commit key has a terminal aborted outcome".into(),
            ));
        }
        if tx.base_ci > self.committed_version() {
            return Err(StorageError::FutureSnapshot {
                requested: tx.base_ci,
                visible: self.committed_version(),
            });
        }
        // This path expands against the transaction's pinned snapshot, not the
        // live overlay: a cross-request transaction must not observe commits
        // after its snapshot. `drain_and_publish` below publishes every
        // assigned-but-unpublished commit before validation, so the durable
        // conflict summaries already cover them; the pending buffer is trimmed
        // here and consulted below as a defensive second source.
        if let Some(state) = admission.as_mut() {
            self.trim_sequencer(state);
        }

        // Conflict check + CI selection under the publish lock. The guard is
        // scoped to this block: `enqueue_and_publish` must be able to become
        // the publisher afterwards.
        let ci = {
            let _guard = self.health.guard(self.publish_lock.lock())?;
            self.drain_and_publish();
            // The slot is reserved only after every abort check passes. An
            // aborted transaction must NOT consume a commit index: it records
            // a terminal aborted outcome keyed by its commit key, and the
            // contiguous publisher never has to fill a hole for it.
            let published = self.published.load(Ordering::SeqCst);

            // Authorization revalidation runs before the generic read/write
            // conflict scan so a rotated or malformed policy surfaces as an
            // explicit `authorization_changed`/`invalid authorization policy`
            // abort rather than being masked by the policy key's read-write
            // conflict. It still runs under the publish lock, atomic with
            // conflict detection and publication.
            if let Some(revalidate) = revalidate {
                let read =
                    |key: &[u8]| -> Result<Option<Vec<u8>>, StorageError> { self.try_get(key) };
                if let Err(reason) = revalidate(&read) {
                    self.record_outcome_locked(commit_key, OutcomeStatus::Aborted, 0)?;
                    return Err(StorageError::Builder(reason));
                }
            }

            // Read-set (public reads) + write-set (buffer, incl. counters) check.
            // Only published commits since the tx's snapshot can conflict; an
            // in-flight reserved slot is not visible yet and is serialized by
            // the publisher before this commit.
            let mut conflicted: Option<u64> = None;
            let window = self.health.guard(self.conflicts.lock())?;
            for probe in (tx.base_ci + 1)..=published {
                let key_set =
                    window
                        .keys(probe)
                        .ok_or(StorageError::ConflictHistoryUnavailable {
                            commit_index: probe,
                        })?;
                for k in key_set.iter() {
                    if tx.deps.intersects_key(k) || tx.buffer.contains_key(k) {
                        conflicted = Some(probe);
                        break;
                    }
                }
                if conflicted.is_some() {
                    break;
                }
            }
            drop(window);
            // Defensive: consult the pending overlay for any commit that
            // `drain_and_publish` did not publish (for example a mid-assignment
            // gap), so an unpublished writer cannot be missed.
            if let Some(state) = admission.as_ref() {
                for (key, (ci, _)) in &state.buffer {
                    if *ci > tx.base_ci
                        && (tx.deps.intersects_key(key) || tx.buffer.contains_key(key))
                    {
                        conflicted = Some(*ci);
                        break;
                    }
                }
            }
            if let Some(probe) = conflicted {
                self.record_outcome_locked(commit_key, OutcomeStatus::Aborted, 0)?;
                return Err(StorageError::Builder(format!(
                    "transaction_conflict on write at ci {probe}"
                )));
            }
            for pc in &tx.preconditions {
                // Preconditions from statements in one transaction must see
                // the transaction's own write overlay. Relationship batches,
                // for example, require endpoints created earlier in this same
                // transaction to exist before the final commit is published.
                let exists = match pc.kind {
                    // An `Absent` guard belongs to the create operation that
                    // also stages this key. Check committed state so that the
                    // operation does not conflict with its own overlay.
                    rushdb_types::command::PreconditionKind::Absent => {
                        self.try_get(&pc.key)?.is_some()
                    }
                    // An `Exists` guard holds when the key is committed or
                    // this transaction wrote it: relationship batches need
                    // endpoints created earlier in the same transaction, and
                    // a delete guards a key its own write removes.
                    rushdb_types::command::PreconditionKind::Exists => {
                        matches!(tx.buffer.get(&pc.key), Some(Some(_)))
                            || self.try_get(&pc.key)?.is_some()
                    }
                };
                let violated =
                    exists != matches!(pc.kind, rushdb_types::command::PreconditionKind::Exists);
                if violated {
                    self.record_outcome_locked(commit_key, OutcomeStatus::Aborted, 0)?;
                    return Err(StorageError::Precondition {
                        path: rushdb_types::keys::describe_key(&pc.key),
                    });
                }
            }
            // Prospective slot only. The slot is reserved after expansion
            // succeeds so a fallible expansion can never consume (and strand) a
            // commit index. Admission is held for the local path, so no other
            // local writer can change the counter in between; a replicator owns
            // index assignment and must observe the current slot unchanged.
            self.next_commit_index.load(Ordering::SeqCst)
        };

        let batch = CommitBatch {
            command_version: rushdb_types::command::COMMAND_VERSION,
            tenant: tx.tenant,
            transaction_id: String::new(),
            commit_key,
            start_index: ci.saturating_sub(1),
            isolation: rushdb_types::command::IsolationMode::Snapshot,
            preconditions: tx.preconditions.clone(),
            mutations: tx.mutations.clone(),
        };

        // When a replication seam is installed, the tx commits by replicating
        // the batch (like every other commit) instead of publishing locally.
        if let Some(replicator) = replicator {
            let payload_bytes: Vec<u8> = b"{}".to_vec();
            replicator
                .replicate(&batch, &payload_bytes)
                .map_err(StorageError::Builder)?;
            return match self.lookup_outcome_checked(commit_key)? {
                Some((OutcomeStatus::Committed, replicated_ci)) => Ok(replicated_ci),
                _ => Err(StorageError::CorruptLog),
            };
        }

        // Feeds the WAL's embedded command on a durable store, or
        // the in-memory command-journal row otherwise (no WAL to derive it from).
        let command_payload = rushdb_types::command_codec::encode(&batch);
        let ops = {
            // Deterministic expansion reads the transaction's snapshot so it
            // matches the state the transaction observed.
            let snapshot = tx.base_view.is_some();
            let guard = self.txn_data(snapshot)?;
            let native = if snapshot {
                None
            } else {
                self.native_transaction_view()?
            };
            let base = TxnBase::resolve(None, tx.base_view.clone(), native, guard.as_deref());
            let mut wtxn = Txn {
                base,
                buffer: BTreeMap::new(),
                purge_keys: BTreeSet::default(),
                set_keys: BTreeSet::default(),
                deps: None,
            };
            let expander = self.health.guard(self.expander.read())?.clone();
            let Some(expand) = expander.as_ref() else {
                return Err(StorageError::CorruptLog);
            };
            expand(ci, &batch, &mut wtxn).map_err(StorageError::Builder)?;
            if !self.is_durable() {
                wtxn.put(log_key(ci), wal::encode_wal_record(0, ci, &command_payload));
            }
            // Interactive transactions return no stored response body.
            self.put_receipt(&mut wtxn, commit_key, OutcomeStatus::Committed, ci, None);
            wtxn.set(LAST_APPLIED_KEY.to_vec(), wal::encode_last_applied(0, ci));
            wtxn.drain()
        };

        self.reject_oversized_commit(&ops, command_payload.len())?;
        // Expansion succeeded: reserve the slot. Admission is held until this
        // function returns, so the prospective `ci` is still current.
        self.next_commit_index.store(ci + 1, Ordering::SeqCst);
        let shard =
            resources::physical_shard_for(Some(&batch.tenant), self.resource_config.shard_count);
        self.enqueue_and_publish(ci, shard, ops, command_payload)?;
        Ok(ci)
    }
}
