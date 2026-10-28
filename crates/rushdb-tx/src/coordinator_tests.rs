use super::*;
use rushdb_lmpg::FieldChange;
use rushdb_storage::resources::StorageResources;
use serde_json::json;

fn store(pool: usize) -> Arc<Store> {
    let store = Arc::new(
        Store::new_with_resources(StorageResources {
            managed_limit_bytes: pool + 128 * 1024,
            page_cache_bytes: 64 * 1024,
            transaction_pool_bytes: pool,
            durability: rushdb_storage::resources::Durability::Local,
            shard_count: 1,
            experimental_multi_shard: false,
            max_group_count: 0,
            max_group_bytes: 0,
            max_group_delay_ms: 0,
            logical_cache_bytes: 0,
            phase_budget_bytes: 0,
            query_memory_max_bytes: 0,
            lock_acquisition_timeout_ms: 0,
            log_segment_bytes: 64 * 1024 * 1024,
            max_expanded_commit_bytes: 512 * 1024 * 1024,
            ..StorageResources::default()
        })
        .unwrap(),
    );
    // Match production engine bootstrap: coordinators do not replace a
    // caller-installed deterministic state-machine expander.
    drop(Lmpg::new(store.clone(), TenantId::from_ordinal(0)));
    store
}
fn limits() -> TransactionLimits {
    TransactionLimits {
        max_active: 2,
        max_operations: 2,
        max_staged_bytes: 64 * 1024,
        max_expired_ids: 1,
    }
}
fn op(size: usize) -> BatchOp {
    BatchOp::CreateRecord {
        label: "Doc".into(),
        explicit_id: None,
        fields: vec![("text".into(), FieldChange::Set(json!("x".repeat(size))))],
    }
}
fn coord(store: &Arc<Store>, tenant: u64, limits: TransactionLimits) -> TxCoordinator {
    TxCoordinator::with_limits(
        store.clone(),
        TenantId::from_ordinal(tenant),
        Duration::from_secs(30),
        None,
        limits,
    )
    .unwrap()
}

#[test]
fn requested_ttl_is_capped_by_the_coordinator_maximum() {
    let store = store(64 * 1024);
    let coordinator = TxCoordinator::with_limits_and_max_ttl(
        store,
        TenantId::from_ordinal(1),
        Duration::from_millis(10),
        Duration::from_millis(20),
        None,
        limits(),
        0,
    )
    .unwrap();

    let tx = coordinator.begin_with(Some(3_600_000), false).unwrap();
    std::thread::sleep(Duration::from_millis(35));
    assert_eq!(
        coordinator.stage(&tx, op(1)).unwrap_err(),
        "transaction expired"
    );
}

#[test]
fn tenants_share_staging_pool_and_failed_stage_is_atomic() {
    let store = store(64 * 1024);
    let a = coord(&store, 1, limits());
    let b = coord(&store, 2, limits());
    let pool = store.transaction_budget();
    let metadata = pool.snapshot().used;
    let first = a.begin().unwrap();
    let second = b.begin().unwrap();
    a.stage(&first, op(40 * 1024)).unwrap();
    let before = pool.snapshot().used;
    assert!(b
        .stage(&second, op(40 * 1024))
        .unwrap_err()
        .contains("resource exhausted"));
    assert_eq!(pool.snapshot().used, before);
    assert!(b.state.lock().unwrap().active[&second].ops.is_empty());
    let outcome = a.commit(&first);
    assert!(matches!(outcome, TxOutcome::Committed(_)), "{outcome:?}");
    b.stage(&second, op(10)).unwrap();
    b.rollback(&second).unwrap();
    assert_eq!(pool.snapshot().used, metadata);
    drop(a);
    drop(b);
    assert_eq!(pool.snapshot().used, 0);
}

/// Tenancy Phase 3: a positive `max_staged_bytes_for_tenant` gives each
/// coordinator its own child of the shared pool, so one tenant's heavy
/// staging is rejected by its own cap — not the shared pool the test above
/// shows a noisy tenant can otherwise exhaust — without affecting a second
/// tenant's concurrent admission.
#[test]
fn per_tenant_staging_cap_isolates_tenants_sharing_one_pool() {
    let store = store(256 * 1024);
    let per_tenant_cap = 40 * 1024;
    // A per-transaction limit well above the per-tenant pool cap, so this
    // test exercises the ByteBudget child, not `TransactionLimits`'s own
    // independent per-transaction byte cap.
    let generous_limits = TransactionLimits {
        max_staged_bytes: 1024 * 1024,
        ..limits()
    };
    let a = TxCoordinator::with_limits_and_max_ttl(
        store.clone(),
        TenantId::from_ordinal(1),
        Duration::from_secs(30),
        Duration::from_secs(30),
        None,
        generous_limits,
        per_tenant_cap,
    )
    .unwrap();
    let b = TxCoordinator::with_limits_and_max_ttl(
        store,
        TenantId::from_ordinal(2),
        Duration::from_secs(30),
        Duration::from_secs(30),
        None,
        generous_limits,
        per_tenant_cap,
    )
    .unwrap();

    let tx_a = a.begin().unwrap();
    a.stage(&tx_a, op(35 * 1024)).unwrap();
    // A second same-sized op exceeds tenant-a's own 40KiB cap, even though
    // the 256KiB store-wide pool still has plenty of room left.
    let err = a.stage(&tx_a, op(35 * 1024)).unwrap_err();
    assert!(err.contains("resource exhausted"), "{err}");
    assert!(
        err.contains("tenant-transaction-staging"),
        "rejection must come from the per-tenant child, not the shared pool: {err}"
    );

    // tenant-b draws from its own child budget and is unaffected by
    // tenant-a's exhaustion.
    let tx_b = b.begin().unwrap();
    b.stage(&tx_b, op(35 * 1024)).unwrap();
}

#[test]
fn active_operation_and_byte_limits_do_not_drop_prior_work() {
    let store = store(64 * 1024);
    let coordinator = coord(
        &store,
        1,
        TransactionLimits {
            max_active: 1,
            max_operations: 1,
            ..limits()
        },
    );
    let tx = coordinator.begin().unwrap();
    assert!(coordinator.begin().is_err());
    assert!(coordinator.stage(&tx, op(70 * 1024)).is_err());
    assert!(coordinator.state.lock().unwrap().active[&tx].ops.is_empty());
    coordinator.stage(&tx, op(1)).unwrap();
    let used = store.transaction_budget().snapshot().used;
    assert!(coordinator.stage(&tx, op(2)).is_err());
    assert_eq!(store.transaction_budget().snapshot().used, used);
    assert_eq!(coordinator.state.lock().unwrap().active[&tx].ops.len(), 1);
    let outcome = coordinator.commit(&tx);
    assert!(matches!(outcome, TxOutcome::Committed(_)), "{outcome:?}");
}

#[test]
fn expiry_releases_payloads_and_retains_only_bounded_recent_ids() {
    let store = store(64 * 1024);
    let coordinator = coord(&store, 1, limits());
    let pool = store.transaction_budget();
    let baseline = pool.snapshot().used;
    let first = coordinator.begin().unwrap();
    coordinator.stage(&first, op(30 * 1024)).unwrap();
    coordinator
        .state
        .lock()
        .unwrap()
        .active
        .get_mut(&first)
        .unwrap()
        .expires_at = Instant::now();
    assert_eq!(coordinator.reap_expired(), 1);
    assert_eq!(coordinator.active_count(), 0);
    assert_eq!(
        pool.snapshot().used,
        baseline,
        "expired tx payload/metadata charges are released on settle"
    );
    assert_eq!(coordinator.commit(&first), TxOutcome::Expired);
    let second = coordinator.begin().unwrap();
    coordinator
        .state
        .lock()
        .unwrap()
        .active
        .get_mut(&second)
        .unwrap()
        .expires_at = Instant::now();
    coordinator.reap_expired();
    assert_eq!(coordinator.commit(&first), TxOutcome::UnknownRetryable);
    assert_eq!(coordinator.commit(&second), TxOutcome::Expired);
    assert_eq!(
        pool.snapshot().used,
        baseline,
        "expired tx charges are released on settle"
    );
    drop(coordinator);
    assert_eq!(pool.snapshot().used, 0);
}

#[test]
fn active_transaction_holds_and_releases_snapshot_retention_pin() {
    let store = store(64 * 1024);
    let coordinator = coord(&store, 1, limits());
    assert_eq!(store.retention_summary(), (None, 0));

    let tx = coordinator.begin().unwrap();
    assert_eq!(store.retention_summary(), (Some(0), 1));

    coordinator.rollback(&tx).unwrap();
    assert_eq!(store.retention_summary(), (None, 0));
}

#[test]
fn compact_encoding_discards_input_spare_capacity_and_checks_depth() {
    let store = store(64 * 1024);
    let coordinator = coord(&store, 1, limits());
    let tx = coordinator.begin().unwrap();
    let baseline = store.transaction_budget().snapshot().used;
    let mut text = String::with_capacity(1024 * 1024);
    text.push('x');
    let operation = BatchOp::CreateRecord {
        label: "Doc".into(),
        explicit_id: None,
        fields: vec![(
            "object".into(),
            FieldChange::Set(serde_json::Value::String(text)),
        )],
    };
    coordinator.stage(&tx, operation).unwrap();
    assert!(store.transaction_budget().snapshot().used - baseline < 1024);
    let mut nested = json!(0);
    for _ in 0..66 {
        nested = json!([nested]);
    }
    let too_deep = BatchOp::CreateRecord {
        label: "Doc".into(),
        explicit_id: None,
        fields: vec![("deep".into(), FieldChange::Set(nested))],
    };
    assert!(coordinator
        .stage(&tx, too_deep)
        .unwrap_err()
        .contains("depth"));
    assert_eq!(coordinator.state.lock().unwrap().active[&tx].ops.len(), 1);
}

#[test]
fn coordinator_metadata_admission_is_fallible_and_released_on_drop() {
    let store = store(1024);
    let tiny = TransactionLimits {
        max_expired_ids: 0,
        ..limits()
    };
    let coordinator = coord(&store, 1, tiny);
    assert!(coordinator.begin().is_err());
    assert!(TxCoordinator::with_limits(
        store.clone(),
        TenantId::from_ordinal(2),
        Duration::from_secs(1),
        None,
        tiny
    )
    .is_err());
    assert_eq!(store.transaction_budget().snapshot().used, 1024);
    drop(coordinator);
    assert_eq!(store.transaction_budget().snapshot().used, 0);
}

#[test]
fn encoded_payload_charge_survives_until_replicated_acknowledgement() {
    struct ObserveAck {
        store: std::sync::Weak<Store>,
        observed: Arc<std::sync::atomic::AtomicUsize>,
    }
    impl rushdb_storage::Replicator for ObserveAck {
        fn replicate(
            &self,
            batch: &rushdb_types::command::CommitBatch,
            payload: &[u8],
        ) -> Result<(), String> {
            let store = self.store.upgrade().unwrap();
            store
                .apply_replicated_batch_with_payload(batch, Some(payload))
                .map_err(|e| e.to_string())?;
            self.observed.store(
                store.transaction_budget().snapshot().used,
                std::sync::atomic::Ordering::SeqCst,
            );
            Ok(())
        }
    }
    let store = store(64 * 1024);
    let observed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    store.set_replicator(Some(Arc::new(ObserveAck {
        store: Arc::downgrade(&store),
        observed: observed.clone(),
    })));
    let coordinator = coord(&store, 1, limits());
    let baseline = store.transaction_budget().snapshot().used;
    let tx = coordinator.begin().unwrap();
    coordinator.stage(&tx, op(1024)).unwrap();
    let staged = store.transaction_budget().snapshot().used;
    let outcome = coordinator.commit(&tx);
    assert!(matches!(outcome, TxOutcome::Committed(_)), "{outcome:?}");
    assert_eq!(observed.load(std::sync::atomic::Ordering::SeqCst), staged);
    assert_eq!(store.transaction_budget().snapshot().used, baseline);
}

#[test]
fn durable_retry_discovery_does_not_require_free_staging_capacity() {
    let store = store(64 * 1024);
    let coordinator = coord(&store, 1, limits());
    let tx = coordinator.begin().unwrap();
    coordinator.stage(&tx, op(10)).unwrap();
    let outcome = coordinator.commit(&tx);
    assert!(matches!(outcome, TxOutcome::Committed(_)));
    drop(coordinator);
    let pool = store.transaction_budget();
    let _full = pool.try_reserve(pool.snapshot().limit).unwrap();
    assert_eq!(TxCoordinator::discover(&store, &tx).unwrap(), Some(outcome));
}

#[test]
fn immediate_expiry_is_reported_even_without_expiry_id_retention() {
    let store = store(64 * 1024);
    let coordinator = coord(
        &store,
        1,
        TransactionLimits {
            max_expired_ids: 0,
            ..limits()
        },
    );
    let baseline = store.transaction_budget().snapshot().used;
    let tx = coordinator.begin().unwrap();
    coordinator
        .state
        .lock()
        .unwrap()
        .active
        .get_mut(&tx)
        .unwrap()
        .expires_at = Instant::now();
    assert_eq!(coordinator.commit(&tx), TxOutcome::Expired);
    assert_eq!(store.transaction_budget().snapshot().used, baseline);
    let tx = coordinator.begin().unwrap();
    coordinator
        .state
        .lock()
        .unwrap()
        .active
        .get_mut(&tx)
        .unwrap()
        .expires_at = Instant::now();
    assert!(coordinator
        .stage(&tx, op(1))
        .unwrap_err()
        .contains("expired"));
    assert_eq!(store.transaction_budget().snapshot().used, baseline);
}

#[test]
fn invalid_limits_and_ttl_do_not_reserve_metadata() {
    let store = store(64 * 1024);
    for invalid in [
        TransactionLimits {
            max_active: 0,
            ..limits()
        },
        TransactionLimits {
            max_operations: 0,
            ..limits()
        },
        TransactionLimits {
            max_staged_bytes: 0,
            ..limits()
        },
        TransactionLimits {
            max_expired_ids: usize::MAX,
            ..limits()
        },
    ] {
        assert!(TxCoordinator::with_limits(
            store.clone(),
            TenantId::from_ordinal(1),
            Duration::from_secs(30),
            None,
            invalid
        )
        .is_err());
    }
    assert!(TxCoordinator::with_limits(
        store.clone(),
        TenantId::from_ordinal(1),
        Duration::ZERO,
        None,
        limits()
    )
    .is_err());
    assert!(TxCoordinator::with_limits(
        store.clone(),
        TenantId::from_ordinal(1),
        Duration::MAX,
        None,
        limits()
    )
    .is_err());
    assert_eq!(store.transaction_budget().snapshot().used, 0);
}

/// Autocommit semantics: an expired transaction with `autocommit: true` is
/// COMMITTED (best effort) by the reaper instead of rolled back.
#[test]
fn expired_autocommit_transaction_is_committed_by_reaper() {
    let store = store(64 * 1024);
    let coordinator = coord(&store, 1, limits());
    let tx = coordinator.begin_with(None, true).unwrap();
    coordinator.stage(&tx, op(10)).unwrap();
    coordinator
        .state
        .lock()
        .unwrap()
        .active
        .get_mut(&tx)
        .unwrap()
        .expires_at = Instant::now();
    assert_eq!(
        coordinator.reap_expired(),
        1,
        "reaper settles the expired tx"
    );
    assert_eq!(
        coordinator.commit(&tx),
        TxOutcome::Committed(1),
        "an autocommit tx is committed on expiry (discover returns the outcome)"
    );

    let view = store.read_view().unwrap();
    assert!(
        rushdb_lmpg::reader::try_access::record_by_ord(&view, TenantId::from_ordinal(1), 0)
            .unwrap()
            .is_some(),
        "the autocommitted staged write is durable after expiry"
    );
}

/// When the client's own `commit` is what settles an expired autocommit
/// transaction, that call reports the commit, and a repeat reports it again.
#[test]
fn commit_after_autocommit_expiry_reports_the_commit_at_once() {
    let store = store(64 * 1024);
    let coordinator = coord(&store, 1, limits());
    let tx = coordinator.begin_with(None, true).unwrap();
    coordinator.stage(&tx, op(10)).unwrap();
    coordinator
        .state
        .lock()
        .unwrap()
        .active
        .get_mut(&tx)
        .unwrap()
        .expires_at = Instant::now();
    assert_eq!(coordinator.commit(&tx), TxOutcome::Committed(1));
    assert_eq!(coordinator.commit(&tx), TxOutcome::Committed(1));
}

#[test]
fn maintenance_expiry_releases_autocommit_without_publishing() {
    let store = store(64 * 1024);
    let coordinator = coord(&store, 1, limits());
    let tx = coordinator.begin_with(None, true).unwrap();
    coordinator.stage(&tx, op(10)).unwrap();
    coordinator
        .state
        .lock()
        .unwrap()
        .active
        .get_mut(&tx)
        .unwrap()
        .expires_at = Instant::now();

    assert_eq!(coordinator.reap_expired_without_autocommit(), 1);
    assert_eq!(store.committed_version(), 0);
    assert_eq!(coordinator.active_count(), 0);
}

/// The default remains rollback: an expired tx WITHOUT autocommit is discarded.
#[test]
fn expired_non_autocommit_transaction_is_discarded() {
    let store = store(64 * 1024);
    let coordinator = coord(&store, 1, limits());
    let tx = coordinator.begin().unwrap(); // autocommit = false (default)
    coordinator.stage(&tx, op(10)).unwrap();
    coordinator
        .state
        .lock()
        .unwrap()
        .active
        .get_mut(&tx)
        .unwrap()
        .expires_at = Instant::now();
    coordinator.reap_expired();
    assert_eq!(coordinator.commit(&tx), TxOutcome::Expired);

    let view = store.read_view().unwrap();
    assert!(
        rushdb_lmpg::reader::try_access::record_by_ord(&view, TenantId::from_ordinal(1), 0)
            .unwrap()
            .is_none(),
        "a non-autocommit tx is rolled back on expiry"
    );
}
