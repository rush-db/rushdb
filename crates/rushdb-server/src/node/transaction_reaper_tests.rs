use super::metrics::free_percent_ok;
use super::*;

#[test]
fn bounded_json_response_rejects_before_retaining_row() {
    let mut values = Vec::new();
    let mut bytes = 0;
    let error = append_bounded_json_response(
        &mut values,
        &mut bytes,
        serde_json::json!({"payload": "large"}),
        1,
    )
    .expect_err("response byte budget");
    assert!(error.contains("response exceeds configured byte limit"));
    assert!(values.is_empty());
    assert_eq!(bytes, 0);
}

#[test]
fn expired_cursor_releases_resident_bytes() {
    let mut cfg = crate::ServerConfig::default();
    let node = Node::bootstrap(&mut cfg).expect("default node bootstrap");
    let view = Arc::new(node.store.read_view().expect("read view"));
    let cursor = ResultCursor {
        tenant: node.scope().tenant(),
        policy_generation: 0,
        retention_consumer: "cursor:expired".into(),
        view,
        search: SearchRequest::default(),
        next_offset: 0,
        total: 1,
        expires_at: Instant::now()
            .checked_sub(Duration::from_secs(1))
            .expect("current instant is after one second"),
        _reservation: None,
    };
    let resident = cursor_resident_bytes(&cursor);
    let retention_floor = cursor.view.commit_index();
    node.retain_ephemeral_cursor("cursor:expired".into(), retention_floor)
        .unwrap();
    assert_eq!(node.protected_retention_floor(), Some(retention_floor));
    let cache_budget = node.store.cache_budget();
    let cache_used_before = cache_budget.as_ref().map(|budget| budget.snapshot().used);
    let reservation = cache_budget
        .as_ref()
        .map(|budget| budget.try_reserve(resident).unwrap());
    let cursor = ResultCursor {
        _reservation: reservation,
        ..cursor
    };
    node.cursors
        .lock()
        .unwrap()
        .insert("expired".to_string(), cursor);
    *node.cursor_bytes.lock().unwrap() = resident;

    assert_eq!(node.reap_expired_cursors(), 1);
    assert!(node.cursors.lock().unwrap().is_empty());
    assert_eq!(*node.cursor_bytes.lock().unwrap(), 0);
    assert_eq!(node.protected_retention_floor(), None);
    if let (Some(before), Some(budget)) = (cache_used_before, cache_budget) {
        assert_eq!(budget.snapshot().used, before);
    }
}

#[test]
fn invalid_boot_limits_fail_before_creating_a_directory() {
    let root = rushdb_storage::TempPath::dir("invalid-resource");
    let dir = root.join("data");
    let mut cfg = crate::ServerConfig {
        data_dir: Some(dir.clone()),
        max_request_bytes: 64 * 1024 * 1024,
        listen_addr: "127.0.0.1".into(),
        tls_cert: None,
        tls_key: None,
        ..Default::default()
    };
    cfg.resources.storage_cache = crate::resource_config::ByteSize(1);
    assert!(Node::bootstrap(&mut cfg).is_err());
    assert!(!dir.exists());
}

#[test]
fn restart_applies_saved_cache_before_store_open_and_enforces_tx_limits() {
    use crate::resource_config::ByteSize;
    let dir = rushdb_storage::TempPath::dir("resource-restart");
    let mut cfg = crate::ServerConfig {
        data_dir: Some(dir.to_path_buf()),
        max_request_bytes: 64 * 1024 * 1024,
        listen_addr: "127.0.0.1".into(),
        tls_cert: None,
        tls_key: None,
        ..Default::default()
    };
    cfg.resources.managed_limit = ByteSize(2 * 1024 * 1024);
    cfg.resources.storage_cache = ByteSize(64 * 1024);
    cfg.resources.transaction_pool = ByteSize(64 * 1024);
    cfg.transactions.max_active = 1;
    cfg.transactions.max_operations = 1;
    cfg.transactions.max_staged_bytes = ByteSize(1024);
    cfg.transactions.max_expired_ids = 0;
    let node = Node::bootstrap(&mut cfg).unwrap();
    assert_eq!(node.store.resource_config().page_cache_bytes, 64 * 1024);
    assert_eq!(node.store.resource_budget().snapshot().used, 64 * 1024);
    drop(node);

    let mut restart = crate::ServerConfig {
        data_dir: Some(dir.to_path_buf()),
        max_request_bytes: 64 * 1024 * 1024,
        listen_addr: "127.0.0.1".into(),
        tls_cert: None,
        tls_key: None,
        ..Default::default()
    };
    let node = Node::bootstrap(&mut restart).unwrap();
    assert_eq!(node.store.resource_config().page_cache_bytes, 64 * 1024);
    assert_eq!(node.store.transaction_budget().snapshot().limit, 64 * 1024);
    let tx = node.scope().tx_begin().unwrap();
    assert!(node
        .scope()
        .tx_begin()
        .unwrap_err()
        .contains("active transaction limit"));
    let mutation = rushdb_protocol::TransactionMutation::CreateRecord {
        label: "Doc".into(),
        id: None,
        properties: serde_json::json!({"payload": "x"}),
        options: serde_json::json!({}),
    };
    node.scope().tx_stage(&tx, &mutation).unwrap();
    assert!(node
        .scope()
        .tx_stage(&tx, &mutation)
        .unwrap_err()
        .contains("operation limit"));
    let outcome = node.scope().tx_commit(&tx);
    assert!(
        matches!(outcome, rushdb_tx::TxOutcome::Committed(_)),
        "{outcome:?}"
    );
    // Cold registry outcome recovery must not allocate a new coordinator,
    // even when the shared staging pool is fully reserved.
    node.residency.remove(node.scope().tenant());
    assert_eq!(node.scope().try_tx_commit(&tx).unwrap(), outcome);
    let pool = node.store.transaction_budget();
    let _full = pool.try_reserve(pool.snapshot().limit).unwrap();
    assert_eq!(node.scope().tx_commit(&tx), outcome);
    assert!(
        node.residency.existing_tx(node.scope().tenant()).is_none(),
        "outcome recovery allocated no coordinator"
    );
    drop(node);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn metrics_expose_resource_budget_gauges() {
    // P01: exhaustion must have a stable observable signal. The rendered
    // Prometheus exposition must include managed + transaction budget
    // used/peak/limit gauges.
    let node = Node::bootstrap(&mut crate::ServerConfig::default()).unwrap();
    let rendered = node.metrics_render();
    for metric in [
        "rushdb_budget_used_bytes",
        "rushdb_budget_peak_bytes",
        "rushdb_budget_limit_bytes",
    ] {
        assert!(
            rendered.contains(metric),
            "missing {metric} in metrics:\n{rendered}"
        );
    }
    for pool in ["managed", "transaction"] {
        assert!(
            rendered.contains(&format!("pool=\"{pool}\"")),
            "missing pool={pool} in metrics:\n{rendered}"
        );
    }
    drop(node);
}

#[test]
fn metrics_render_without_scope_serves_allocator_gauges() {
    // Regression: scrape paths (`GET /metrics`) carry no tenant scope,
    // and tenant resolution panics without one. Rendering in multitenant
    // mode must skip the scope-bound probes instead of crashing, and must
    // expose allocator introspection alongside the budget gauges.
    let mut cfg = crate::ServerConfig {
        multitenant: true,
        ..Default::default()
    };
    let node = Node::bootstrap(&mut cfg).unwrap();
    // RM-1: there is no ambient scope to assert the absence of anymore — a
    // scrape path never had a `RequestScope` to begin with, which is the
    // point. The regression this guards (metrics must not crash without a
    // tenant) is still covered by the render call below.
    let rendered = node.metrics_render();
    for metric in [
        "rushdb_budget_used_bytes",
        "rushdb_allocator_allocated_bytes",
        "rushdb_allocator_active_bytes",
        "rushdb_allocator_metadata_bytes",
        "rushdb_allocator_resident_bytes",
        "rushdb_allocator_retained_bytes",
        "rushdb_storage_file_bytes",
        "rushdb_commit_log_rows_pruned",
        "rushdb_receipts_swept",
        "rushdb_storage_compactions_total",
        "rushdb_scheduler_inflight",
        "rushdb_scheduler_capacity",
    ] {
        assert!(
            rendered.contains(metric),
            "missing {metric} in metrics:\n{rendered}"
        );
    }
    drop(node);
}

#[test]
fn config_surface_reaches_metrics_build_info_and_observability() {
    let mut cfg = crate::ServerConfig::default();
    cfg.resources.metrics_prefix = Some("x_".into());
    cfg.resources.metrics_filter = Some("budget".into());
    cfg.resources.advertised_address = Some("db.internal:1234".into());
    cfg.resources.logs_query_threshold_ms = 250;
    cfg.resources.disk_space_threshold_percent = 90;
    cfg.resources.threads = 3;
    let node = Node::bootstrap(&mut cfg).unwrap();
    assert_eq!(node.slow_call_threshold_ms(), 250);
    assert_eq!(node.disk_space_threshold_percent(), 90);
    let info = node.build_info();
    assert_eq!(info.advertised_address.as_deref(), Some("db.internal:1234"));
    let rendered = node.metrics_render();
    assert!(
        rendered.contains("x_rushdb_budget_used_bytes"),
        "prefix not applied:\n{rendered}"
    );
    assert!(
        !rendered.contains("x_rushdb_requests_total"),
        "filter should drop non-matching families:\n{rendered}"
    );
    drop(node);
}

#[test]
fn disk_free_guard_percentage_decision() {
    assert!(free_percent_ok(100, 90, 90));
    assert!(!free_percent_ok(100, 89, 90));
    assert!(free_percent_ok(100, 0, 0));
    assert!(free_percent_ok(0, 0, 90), "unknown total is treated as ok");
    // A disabled guard (0%) is always ready regardless of the filesystem.
    let node = Node::bootstrap(&mut crate::ServerConfig::default()).unwrap();
    assert!(node.disk_space_ok());
    drop(node);
}

#[test]
fn invalid_saved_limits_do_not_create_database_files() {
    let dir = rushdb_storage::TempPath::dir("resource-preflight");
    std::fs::create_dir_all(&dir).unwrap();
    let mut cfg = crate::ServerConfig {
        data_dir: Some(dir.to_path_buf()),
        max_request_bytes: 64 * 1024 * 1024,
        listen_addr: "127.0.0.1".into(),
        tls_cert: None,
        tls_key: None,
        ..Default::default()
    };
    cfg.transactions.max_age_seconds = 0;
    crate::config::write_config(&dir, &cfg).unwrap();
    let before = std::fs::read(dir.join(crate::config::CONFIG_FILE_NAME)).unwrap();
    let mut requested = crate::ServerConfig {
        data_dir: Some(dir.to_path_buf()),
        max_request_bytes: 64 * 1024 * 1024,
        listen_addr: "127.0.0.1".into(),
        tls_cert: None,
        tls_key: None,
        ..Default::default()
    };
    assert!(Node::bootstrap(&mut requested).is_err());
    assert!(!dir.join("rushdb.redb").exists());
    assert_eq!(
        std::fs::read(dir.join(crate::config::CONFIG_FILE_NAME)).unwrap(),
        before
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_transaction_payloads_and_unreferenced_coordinators_are_reclaimed() {
    let mut cfg = crate::ServerConfig::default();
    // Residency evicts an idle tenant at once; the coordinator's own
    // TTL still governs its transactions.
    cfg.resources.tenant_idle_seconds = 0;
    let node = Arc::new(Node::bootstrap(&mut cfg).unwrap());
    let tenant = node.scope().tenant();
    let store = node.store.clone();
    let coordinator = node
        .residency
        .tx(tenant, || {
            rushdb_tx::TxCoordinator::with_limits(
                store,
                tenant,
                std::time::Duration::from_millis(100),
                None,
                rushdb_tx::TransactionLimits {
                    max_expired_ids: 1,
                    ..Default::default()
                },
            )
        })
        .unwrap();
    let pool = node.store.transaction_budget();
    let tx = coordinator.begin().unwrap();
    coordinator
        .stage(
            &tx,
            rushdb_lmpg::BatchOp::CreateRecord {
                label: "Doc".into(),
                explicit_id: None,
                fields: vec![(
                    "payload".into(),
                    rushdb_lmpg::FieldChange::Set(serde_json::json!("x".repeat(4096))),
                )],
            },
        )
        .unwrap();
    let staged = pool.snapshot().used;
    let reaper = crate::http::spawn_transaction_reaper(
        Arc::downgrade(&node),
        std::time::Duration::from_millis(10),
    );
    // `active_count` drops to zero as soon as the reaper detaches the
    // transaction, which is slightly before the detached `ActiveTx` is dropped
    // and its budget released. Wait on the observable release so the assertion
    // cannot race the settle step under load.
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while pool.snapshot().used >= staged {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("background reaper did not release idle transaction payloads");
    assert_eq!(coordinator.active_count(), 0);
    assert_eq!(coordinator.commit(&tx), rushdb_tx::TxOutcome::Expired);
    assert_eq!(node.residency.len(), 1, "external Arc prevents eviction");
    drop(coordinator);
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while pool.snapshot().used != 0 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("idle coordinator was not evicted");
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while node.residency.len() != 0 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("idle tenant was not evicted");
    drop(node);
    tokio::time::timeout(std::time::Duration::from_secs(1), reaper)
        .await
        .expect("reaper retained the node")
        .unwrap();
}

#[test]
fn active_tenants_exhausting_budget_get_retryable_503() {
    let mut cfg = crate::ServerConfig {
        multitenant: true,
        ..Default::default()
    };
    // Room for exactly two resident tenants.
    cfg.resources.tenant_resident_bytes =
        crate::resource_config::ByteSize(2 * super::residency::ENTRY_OVERHEAD_BYTES);
    let node = Node::bootstrap(&mut cfg).unwrap();
    let first = RequestScope::new(&node, Some("capacity-a".into()));
    let second = RequestScope::new(&node, Some("capacity-b".into()));
    let held_a = first.tx_coordinator().unwrap();
    let held_b = second.tx_coordinator().unwrap();
    let third = RequestScope::new(&node, Some("capacity-c".into()));
    let Err(error) = third.tx_coordinator() else {
        panic!("a third active tenant must not be admitted")
    };
    assert!(error.contains("tenant_capacity"), "{error}");
    assert_eq!(
        crate::protocol::execution_failed(&error).code,
        rushdb_protocol::ErrorCode::Unavailable,
        "a load signal (HTTP 503), not a failure"
    );
    // Once one tenant goes idle, its entry yields to the newcomer.
    drop(held_b);
    assert!(third.tx_coordinator().is_ok());
    assert!(node.residency.existing_tx(first.tenant()).is_some());
    drop(held_a);
}

#[test]
fn purged_tenant_metric_series_are_removed() {
    let mut cfg = crate::ServerConfig {
        multitenant: true,
        ..Default::default()
    };
    cfg.resources.metrics_tenant_limit = 100;
    let node = Node::bootstrap(&mut cfg).unwrap();
    let a = RequestScope::new(&node, Some("metrics-a".into()));
    let b = RequestScope::new(&node, Some("metrics-b".into()));
    for scope in [&a, &b] {
        scope
            .create_record(
                "Doc",
                &serde_json::json!({ "n": 1 }),
                &[],
                &serde_json::json!({}),
            )
            .unwrap();
    }
    let series = |tenant: TenantId| format!("rushdb_records_stored{{tenant=\"{}\"}} 1", tenant.0);
    node.refresh_records_by_tenant(std::time::Duration::ZERO);
    let rendered = node.metrics_render();
    assert!(rendered.contains(&series(a.tenant())), "{rendered}");
    assert!(rendered.contains(&series(b.tenant())));
    assert!(rendered.contains("rushdb_records_stored{tenant=\"all\"} 2"));
    b.purge_tenant_with_cancel(&|| false).unwrap();
    node.refresh_records_by_tenant(std::time::Duration::ZERO);
    let rendered = node.metrics_render();
    assert!(rendered.contains(&series(a.tenant())));
    assert!(
        !rendered.contains(&format!("tenant=\"{}\"", b.tenant().0)),
        "a purged tenant's series is removed"
    );
}

#[test]
fn per_tenant_metric_series_are_off_by_default() {
    let node = Node::bootstrap(&mut crate::ServerConfig {
        multitenant: true,
        ..Default::default()
    })
    .unwrap();
    let a = RequestScope::new(&node, Some("metrics-default".into()));
    a.create_record(
        "Doc",
        &serde_json::json!({ "n": 1 }),
        &[],
        &serde_json::json!({}),
    )
    .unwrap();
    node.refresh_records_by_tenant(std::time::Duration::ZERO);
    let rendered = node.metrics_render();
    assert!(
        rendered.contains("rushdb_records_stored{tenant=\"all\"} 1"),
        "{rendered}"
    );
    assert!(!rendered.contains(&format!("tenant=\"{}\"", a.tenant().0)));
}
