//! Durability gates for the redb-backed storage engine.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::reader::{self, try_access};
use rushdb_lmpg::Lmpg;
use rushdb_storage::{OutcomeKind, Store};
use rushdb_types::command::CommitBatch;
use rushdb_types::ids::TenantId;
use serde_json::{json, Value};

const TENANT: TenantId = TenantId::from_ordinal(55);

fn path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "rushdb-redb-{name}-{}-{}",
        std::process::id(),
        uuid::Uuid::now_v7()
    ))
}

fn scrub_clean(store: &Store) {
    let view = store.read_view().unwrap();
    let violations = rushdb_lmpg::scrub::scrub(&view, TENANT).unwrap();
    assert!(
        violations.is_empty(),
        "invariant violations: {violations:?}"
    );
}

#[test]
fn acknowledged_commits_and_idempotency_survive_reopen() {
    let dir = path("reopen");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let first_commit_key;

    {
        let store = Arc::new(Store::open(&db).unwrap());
        let lmpg = Lmpg::new(store.clone(), TENANT);
        let first = lmpg
            .create_record("R", None, vec![("a".into(), FieldChange::Set(json!(1)))])
            .unwrap();
        first_commit_key = store
            .log_bytes()
            .first()
            .and_then(|raw| rushdb_types::command_codec::decode(raw).ok())
            .map(|batch| batch.commit_key)
            .unwrap();
        lmpg.update_fields(
            rushdb_lmpg::RecordRef {
                ordinal: first.ordinal,
                record_id: first.record_id,
            },
            vec![("a".into(), FieldChange::Set(json!(2)))],
        )
        .unwrap();
        lmpg.create_record("R", None, vec![("a".into(), FieldChange::Set(json!(3)))])
            .unwrap();
        assert_eq!(store.committed_version(), 3);
    }

    let store = Arc::new(Store::open(&db).unwrap());
    let lmpg = Lmpg::new(store.clone(), TENANT);
    assert_eq!(store.committed_version(), 3);
    assert_eq!(
        store.outcome_for(first_commit_key),
        Some(OutcomeKind::Committed(1))
    );
    let snap = lmpg.store.read_view().unwrap();
    assert_eq!(
        try_access::label_members(&snap, TENANT, "R").unwrap().len(),
        2
    );
    let first = rushdb_lmpg::reconstruct::reconstruct_record(&snap, TENANT, 0)
        .unwrap()
        .unwrap();
    assert_eq!(first.get("a"), Some(&json!(2)));
    scrub_clean(&store);
    drop(lmpg);
    drop(store);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn deletes_and_mvcc_history_survive_reopen() {
    let dir = path("delete-mvcc");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let deleted;
    let before_delete;

    {
        let store = Arc::new(Store::open(&db).unwrap());
        let lmpg = Lmpg::new(store.clone(), TENANT);
        deleted = lmpg
            .create_record("Gone", None, vec![("v".into(), FieldChange::Set(json!(1)))])
            .unwrap();
        before_delete = store.committed_version();
        lmpg.delete_record(deleted).unwrap();
        assert!(
            try_access::label_members(&lmpg.store.read_view().unwrap(), TENANT, "Gone")
                .unwrap()
                .is_empty()
        );
    }

    let store = Arc::new(Store::open(&db).unwrap());
    let lmpg = Lmpg::new(store.clone(), TENANT);
    assert!(
        try_access::label_members(&lmpg.store.read_view().unwrap(), TENANT, "Gone")
            .unwrap()
            .is_empty()
    );
    let header = rushdb_types::keys::record_id_key(TENANT, &deleted.record_id);
    // The pre-delete snapshot must still resolve the header, while the latest
    // snapshot and live lookup must observe the deletion. This is the MVCC
    // history retention contract: reading at an older commit is stable.
    assert!(store
        .read_at(before_delete)
        .unwrap()
        .get(&header)
        .unwrap()
        .is_some());
    assert!(store.read_view().unwrap().get(&header).unwrap().is_none());
    assert!(store.try_get(&header).unwrap().is_none());
    scrub_clean(&store);
    drop(lmpg);
    drop(store);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn redb_compaction_preserves_graph_state() {
    let dir = path("compact");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");

    let store = Arc::new(Store::open(&db).unwrap());
    let lmpg = Lmpg::new(store.clone(), TENANT);
    let record = lmpg.create_record("Doc", None, vec![]).unwrap();
    for value in 0..25 {
        lmpg.update_fields(
            record,
            vec![("value".into(), FieldChange::Set(json!(value)))],
        )
        .unwrap();
    }
    let compacted_at = store.compact().unwrap();
    assert_eq!(store.gc_watermark(), compacted_at);
    drop(lmpg);
    drop(store);

    let reopened = Arc::new(Store::open(&db).unwrap());
    let lmpg = Lmpg::new(reopened.clone(), TENANT);
    let value = rushdb_lmpg::reconstruct::reconstruct_record(
        &lmpg.store.read_view().unwrap(),
        TENANT,
        record.ordinal,
    )
    .unwrap()
    .unwrap();
    assert_eq!(value.get("value"), Some(&json!(24)));
    assert_eq!(reopened.gc_watermark(), compacted_at);
    scrub_clean(&reopened);
    drop(lmpg);
    drop(reopened);
    std::fs::remove_dir_all(&dir).unwrap();
}

fn two_shard_resources() -> rushdb_storage::resources::StorageResources {
    rushdb_storage::resources::StorageResources {
        shard_count: 2,
        experimental_multi_shard: true,
        ..Default::default()
    }
}

fn tenants_on_distinct_shards() -> (TenantId, TenantId) {
    // Physical shard routing hashes the tenant id; probe until two tenants
    // land on different shards so the test actually exercises the fan-out.
    let first = TenantId::from_ordinal(1000);
    let first_shard = rushdb_storage::resources::physical_shard_for(Some(&first), 2);
    let second = (1001..1001 + 64)
        .map(TenantId::from_ordinal)
        .find(|t| rushdb_storage::resources::physical_shard_for(Some(t), 2) != first_shard)
        .expect("shard space has two shards");
    (first, second)
}

#[test]
fn parallel_shard_persist_keeps_both_shards_durable() {
    const PER_TENANT: usize = 25;
    // With two physical shards, concurrent commits to tenants on
    // distinct shards persist in parallel with a dense contiguous commit
    // index, and every acknowledged commit survives a reopen.
    //
    // Scope note: cross-shard *reads* are exercised by
    // `multi_shard_reads_merge_across_shards`; this test asserts the durable
    // surface — dense index, both shard files, reopen watermark — while
    // per-shard placement is gated deterministically in
    // `writer_tests::multi_shard_persist_fans_out_to_both_backends_in_parallel`.
    let (tenant_a, tenant_b) = tenants_on_distinct_shards();
    let dir = path("shards");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    {
        let store = Arc::new(Store::open_with_resources(&db, two_shard_resources()).unwrap());
        std::thread::scope(|scope| {
            for tenant in [tenant_a, tenant_b] {
                let store = store.clone();
                scope.spawn(move || {
                    let lmpg = Lmpg::new(store, tenant);
                    for i in 0..PER_TENANT {
                        lmpg.create_record(
                            "S",
                            None,
                            vec![("n".into(), FieldChange::Set(json!(i)))],
                        )
                        .unwrap();
                    }
                });
            }
        });
        assert_eq!(store.committed_version(), (2 * PER_TENANT) as u64);
        // Both physical shard files exist: the fan-out wrote past shard 0.
        assert!(db.exists(), "shard 0 file must exist");
        assert!(
            dir.join("rushdb.redb-1").exists(),
            "shard 1 file must exist after cross-shard commits"
        );
    }
    // Reopen: the durable watermark is the max LSN across shards — nothing
    // acknowledged is lost.
    let store = Arc::new(Store::open_with_resources(&db, two_shard_resources()).unwrap());
    assert_eq!(store.committed_version(), (2 * PER_TENANT) as u64);
    drop(store);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn multi_shard_crash_log_is_returned_in_commit_order() {
    // Shards are scanned in shard order, so a naive union of their log rows is
    // grouped by shard, not by commit index. The crash-log export must sort by
    // the LSN carried in each WAL frame's header.
    let (tenant_a, tenant_b) = tenants_on_distinct_shards();
    let dir = path("shard-log-order");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let store = Arc::new(Store::open_with_resources(&db, two_shard_resources()).unwrap());
    store.set_expander(Arc::new(Box::new(|_ci, _batch, _txn| Ok(()))));

    let plan = [tenant_a, tenant_b, tenant_a, tenant_b, tenant_a];
    for (i, tenant) in plan.into_iter().enumerate() {
        let key = uuid::Uuid::now_v7();
        let batch = CommitBatch {
            command_version: rushdb_types::command::COMMAND_VERSION,
            tenant,
            transaction_id: format!("tx{i}"),
            commit_key: key,
            start_index: 0,
            isolation: rushdb_types::command::IsolationMode::ReadCommitted,
            preconditions: vec![],
            mutations: vec![],
        };
        store
            .commit_for_tenant(&tenant, key, move |_, _| Ok((batch, ())))
            .unwrap();
    }

    let log = store.try_take_log_for_crash().unwrap();
    let txids: Vec<String> = log
        .iter()
        .map(|payload| {
            rushdb_types::command_codec::decode(payload)
                .expect("log payload is an encoded commit batch")
                .transaction_id
        })
        .collect();
    assert_eq!(
        txids,
        vec!["tx0", "tx1", "tx2", "tx3", "tx4"],
        "crash log must follow commit order across shards"
    );

    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn historical_prefix_scan_reads_disk_backed_store() {
    // The RAM mirrors are not maintained for a redb-backed store, so the
    // memory candidate enumeration returned an empty scan. It must read
    // natively at the requested snapshot.
    let dir = path("hist-prefix");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let store = Arc::new(Store::open(&db).unwrap());
    let commit = |value: u8| {
        store.set_expander(Arc::new(Box::new(move |_, _, txn| {
            txn.put(b"p/a".to_vec(), vec![value]);
            Ok(())
        })));
        let key = uuid::Uuid::now_v7();
        store
            .commit(key, move |_, _| {
                Ok((
                    CommitBatch {
                        command_version: rushdb_types::command::COMMAND_VERSION,
                        tenant: TENANT,
                        transaction_id: String::new(),
                        commit_key: key,
                        start_index: 0,
                        isolation: rushdb_types::command::IsolationMode::ReadCommitted,
                        preconditions: vec![],
                        mutations: vec![],
                    },
                    (),
                ))
            })
            .unwrap();
    };
    commit(1);
    commit(2);

    let at_one = store.try_range_prefix_at(b"p/", 1).unwrap();
    assert_eq!(at_one, vec![(b"p/a".to_vec(), vec![1])]);
    let at_two = store.try_range_prefix_at(b"p/", 2).unwrap();
    assert_eq!(at_two, vec![(b"p/a".to_vec(), vec![2])]);

    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

/// W4 phase 2: a multi-shard store serves reads by merging the native per-shard
/// views (no in-memory mirror). Both the query read path (`ReadView`) and the
/// commit-admission read path (`Txn`, used by the expander) must see data on
/// shards other than shard 0, in-process and after reopen.
#[test]
fn multi_shard_reads_merge_across_shards() {
    const PER_TENANT: usize = 25;
    let (tenant_a, tenant_b) = tenants_on_distinct_shards();
    let dir = path("shard-reads");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    {
        let store = Arc::new(Store::open_with_resources(&db, two_shard_resources()).unwrap());
        for tenant in [tenant_a, tenant_b] {
            let lmpg = Lmpg::new(store.clone(), tenant);
            for i in 0..PER_TENANT {
                lmpg.create_record("S", None, vec![("n".into(), FieldChange::Set(json!(i)))])
                    .unwrap();
            }
            // In-process read merges the native shard view. The count is only
            // correct if the commit path read the tenant's own shard too.
            let snap = lmpg.store.read_view().unwrap();
            assert_eq!(
                try_access::label_members(&snap, tenant, "S").unwrap().len(),
                PER_TENANT,
                "in-process read for {tenant:?}"
            );
        }
    }
    // Reopen: reads still merge across both shard files.
    let store = Arc::new(Store::open_with_resources(&db, two_shard_resources()).unwrap());
    assert_eq!(store.committed_version(), (2 * PER_TENANT) as u64);
    for tenant in [tenant_a, tenant_b] {
        let lmpg = Lmpg::new(store.clone(), tenant);
        let snap = lmpg.store.read_view().unwrap();
        assert_eq!(
            try_access::label_members(&snap, tenant, "S").unwrap().len(),
            PER_TENANT,
            "reopened read for {tenant:?}"
        );
        for ordinal in 0..PER_TENANT as u64 {
            let record = rushdb_lmpg::reconstruct::reconstruct_record(&snap, tenant, ordinal)
                .unwrap()
                .unwrap_or_else(|| {
                    panic!("tenant {tenant:?} ordinal {ordinal} missing after reopen")
                });
            assert_eq!(record.get("n"), Some(&json!(ordinal)));
        }
    }
    drop(store);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn concurrent_commits_on_one_store_stay_contiguous() {
    const THREADS: usize = 8;
    const PER_THREAD: usize = 25;
    // The cooperative group-commit publisher admits
    // concurrent writers and every commit lands exactly once with a dense
    // index — the baseline the 1/4/16/64-writer sweep measures against.
    let store = Arc::new(Store::new_in_memory());
    std::thread::scope(|scope| {
        for _ in 0..THREADS {
            let store = store.clone();
            scope.spawn(move || {
                let lmpg = Lmpg::new(store, TENANT);
                for i in 0..PER_THREAD {
                    lmpg.create_record("C", None, vec![("n".into(), FieldChange::Set(json!(i)))])
                        .unwrap();
                }
            });
        }
    });
    assert_eq!(
        store.committed_version(),
        (THREADS * PER_THREAD) as u64,
        "every concurrent commit must publish exactly once"
    );
    let lmpg = Lmpg::new(store.clone(), TENANT);
    let snap = lmpg.store.read_view().unwrap();
    assert_eq!(
        try_access::label_members(&snap, TENANT, "C").unwrap().len(),
        THREADS * PER_THREAD
    );
    scrub_clean(&store);
}

#[test]
fn commit_metadata_retention_prunes_log_but_preserves_state_and_outcomes() {
    // Retention retires old commits (bounded, resumable cursor advance)
    // without touching canonical state or retry receipts: receipts expire by
    // time, not by commit count, so an old retry inside the window
    // still discovers its durable outcome. The command journal
    // (`log_bytes`) is derived from the WAL, which retention does not
    // truncate (that is `checkpoint`'s job, clamped by the cursor).
    let dir = path("retention");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let old_commit_key;
    {
        let store = Arc::new(Store::open(&db).unwrap());
        assert_eq!(store.retire_commit_metadata(50, 32).unwrap(), 0);
        let lmpg = Lmpg::new(store.clone(), TENANT);
        for i in 0..200 {
            lmpg.create_record("R", None, vec![("a".into(), FieldChange::Set(json!(i)))])
                .unwrap();
        }
        assert_eq!(store.committed_version(), 200);
        let log_before = store.log_bytes();
        assert_eq!(log_before.len(), 200, "one journal entry per commit");
        old_commit_key = rushdb_types::command_codec::decode(&log_before[0])
            .unwrap()
            .commit_key;
        let receipt_rows = store.storage_stats().unwrap().receipt_keys;
        assert_eq!(
            receipt_rows, 400,
            "one receipt plus its expiry row per commit"
        );

        let mut pruned = 0usize;
        loop {
            let n = store.retire_commit_metadata(50, 32).unwrap();
            pruned += n;
            if n == 0 {
                break;
            }
        }
        // Floor = 200 - 50 = 150: commits 1..=149 retired, in bounded
        // steps of at most 32.
        assert_eq!(
            pruned, 149,
            "expected to retire the 149 commits below the floor"
        );
        assert_eq!(
            store.storage_stats().unwrap().receipt_keys,
            receipt_rows,
            "receipts expire by time, not by retention"
        );
        // The command journal itself is untouched by this sweep.
        assert_eq!(store.log_bytes().len(), 200);
        // Canonical state and the committed status outcome survive.
        assert_eq!(store.committed_version(), 200);
        assert_eq!(
            store.outcome_for(old_commit_key),
            Some(OutcomeKind::Committed(1)),
            "the receipt answers retries for its whole window"
        );
        let (retired, _, _) = store.maintenance_counters();
        assert_eq!(retired, pruned as u64);
        // C4: conflict write sets live only in memory; no commit leaves a
        // durable `__conflicts__` row behind for retention to sweep.
        assert!(
            store
                .try_range_prefix(b"__conflicts__/")
                .unwrap()
                .is_empty(),
            "conflict summaries must not be persisted"
        );
    }
    // Reopen: retention deleted only derived metadata, so restart is lossless.
    let store = Arc::new(Store::open(&db).unwrap());
    assert_eq!(store.committed_version(), 200);
    let lmpg = Lmpg::new(store.clone(), TENANT);
    let snap = lmpg.store.read_view().unwrap();
    assert_eq!(
        try_access::label_members(&snap, TENANT, "R").unwrap().len(),
        200
    );
    assert_eq!(
        store.outcome_for(old_commit_key),
        Some(OutcomeKind::Committed(1))
    );
    scrub_clean(&store);
    drop(store);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn idle_compaction_is_deferred_by_readers_and_preserves_state() {
    let dir = path("compact");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    {
        let store = Arc::new(Store::open(&db).unwrap());
        let lmpg = Lmpg::new(store.clone(), TENANT);
        for i in 0..64 {
            lmpg.create_record("C", None, vec![("a".into(), FieldChange::Set(json!(i)))])
                .unwrap();
        }
        let ci = store.committed_version();
        // A protected reader defers compaction ("not now"), never errors.
        let view = store.read_view().unwrap();
        assert_eq!(store.compact_if_idle().unwrap(), None);
        assert!(
            matches!(
                store.compact(),
                Err(rushdb_storage::StorageError::ReadersActive)
            ),
            "explicit compact keeps the strict contract"
        );
        drop(view);
        assert_eq!(
            store.compact_if_idle().unwrap(),
            Some(ci),
            "idle compaction runs once readers release"
        );
    }
    // Compaction is a rewrite of derived pages; committed data is untouched.
    let store = Arc::new(Store::open(&db).unwrap());
    assert_eq!(store.committed_version(), 64);
    let lmpg = Lmpg::new(store.clone(), TENANT);
    let snap = lmpg.store.read_view().unwrap();
    assert_eq!(
        try_access::label_members(&snap, TENANT, "C").unwrap().len(),
        64
    );
    scrub_clean(&store);
    drop(store);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn retire_commit_metadata_before_uses_an_explicit_floor() {
    // Policy-driven retention: pruning below an explicit floor removes old
    // commit metadata while canonical state and committed status rows survive.
    let dir = path("retention-floor");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    {
        let store = Arc::new(Store::open(&db).unwrap());
        let lmpg = Lmpg::new(store.clone(), TENANT);
        for i in 0..120 {
            lmpg.create_record("R", None, vec![("a".into(), FieldChange::Set(json!(i)))])
                .unwrap();
        }
        let published = store.committed_version();
        assert_eq!(published, 120);
        // keep_none-equivalent floor: prune everything below the current commit.
        let mut pruned = 0usize;
        loop {
            let n = store.retire_commit_metadata_before(published, 32).unwrap();
            pruned += n;
            if n == 0 {
                break;
            }
        }
        assert!(pruned >= 100, "expected to prune >=100 rows, got {pruned}");
        assert!(store.log_bytes().len() <= 120);
        let snap = lmpg.store.read_view().unwrap();
        assert_eq!(
            try_access::label_members(&snap, TENANT, "R").unwrap().len(),
            120
        );
        // A floor above the published watermark prunes nothing.
        assert_eq!(store.retire_commit_metadata_before(1, 32).unwrap(), 0);
    }
    let store = Arc::new(Store::open(&db).unwrap());
    assert_eq!(store.committed_version(), 120);
    scrub_clean(&store);
    drop(store);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn log_byte_floor_prunes_sealed_wal_segments_after_retention_and_checkpoint() {
    // The WAL is one store-wide log (shard routing is per-record
    // metadata, not a per-shard file), so the byte-target floor now walks
    // segments instead of merging per-shard `__log__` row accounting. Force
    // small segments so a realistic commit volume rotates several, then
    // drive the floor through retention (which advances the cursor
    // `truncate_log` clamps to, A4) and a checkpoint — the only thing that
    // actually reclaims WAL segments — and confirm retained bytes shrink.
    let (tenant_a, tenant_b) = tenants_on_distinct_shards();
    let dir = path("logfloor-segments");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let config = rushdb_storage::resources::StorageResources {
        log_segment_bytes: 64 * 1024,
        ..two_shard_resources()
    };
    let store = Arc::new(Store::open_with_resources(&db, config).unwrap());
    let a = Lmpg::new(store.clone(), tenant_a);
    let b = Lmpg::new(store.clone(), tenant_b);
    let padding = "x".repeat(2000);
    for i in 0..200u64 {
        a.create_record(
            "S",
            None,
            vec![
                ("u".into(), FieldChange::Set(json!(i))),
                ("pad".into(), FieldChange::Set(json!(padding.clone()))),
            ],
        )
        .unwrap();
        b.create_record(
            "S",
            None,
            vec![
                ("u".into(), FieldChange::Set(json!(i))),
                ("pad".into(), FieldChange::Set(json!(padding.clone()))),
            ],
        )
        .unwrap();
    }
    let total = store.retained_log_bytes().unwrap();
    assert!(total > 0);

    let target = total / 2;
    let floor = store.log_byte_floor(target).unwrap();
    assert!(floor > 0, "byte floor must be positive when over target");
    loop {
        if store
            .retire_commit_metadata_before(floor, 1_000_000)
            .unwrap()
            == 0
        {
            break;
        }
    }
    store.checkpoint().unwrap();

    let after = store.retained_log_bytes().unwrap();
    assert!(
        after < total,
        "retained WAL bytes {after} did not shrink from {total}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn parallel_compaction_and_gc_fan_out_across_shards() {
    // V2: GC and redb compaction fan out to every shard file concurrently.
    // Both shards must reclaim and stay readable.
    let (tenant_a, tenant_b) = tenants_on_distinct_shards();
    let dir = path("parallel-compact");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    {
        let store = Arc::new(Store::open_with_resources(&db, two_shard_resources()).unwrap());
        let a = Lmpg::new(store.clone(), tenant_a);
        let b = Lmpg::new(store.clone(), tenant_b);
        for i in 0..100u64 {
            let ra = a
                .create_record("S", None, vec![("u".into(), FieldChange::Set(json!(i)))])
                .unwrap();
            a.update_fields(ra, vec![("u".into(), FieldChange::Set(json!(i + 1)))])
                .unwrap();
            let rb = b
                .create_record("S", None, vec![("u".into(), FieldChange::Set(json!(i)))])
                .unwrap();
            b.update_fields(rb, vec![("u".into(), FieldChange::Set(json!(i + 1)))])
                .unwrap();
        }
        let ci = store.compact().unwrap();
        assert!(ci > 0);
        assert_eq!(store.gc_watermark(), ci, "both shards advanced the floor");

        // Both tenants remain readable through the merged native views.
        let view = store.read_view().unwrap();
        assert_eq!(
            reader::try_access::label_members(&view, tenant_a, "S")
                .unwrap()
                .len(),
            100
        );
        assert_eq!(
            reader::try_access::label_members(&view, tenant_b, "S")
                .unwrap()
                .len(),
            100
        );
    }
    let reopened = Store::open_with_resources(&db, two_shard_resources()).unwrap();
    assert_eq!(reopened.gc_watermark(), reopened.committed_version());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn group_commit_linger_preserves_ordering_and_durability() {
    // V2 WAL batching tuning: the linger knob is correct and durable. NOTE:
    // under the default single-node admission lock, expansions are serialized
    // (each must see the preceding commit's writes), so only one commit is in
    // flight and every commit is its own group. Coalescing multiple commits
    // into one fsync requires the deferred D7 in-memory expansion overlay
    // (see 10-durability §5); the linger is effective where commits are
    // enqueued concurrently (the replication path releases admission).
    let dir = path("linger");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let writers = 8u64;
    let per_writer = 50u64;
    {
        let res = rushdb_storage::resources::StorageResources {
            max_group_delay_ms: 25,
            ..Default::default()
        };
        let store = Arc::new(Store::open_with_resources(&db, res).unwrap());
        let barrier = Arc::new(std::sync::Barrier::new(usize::try_from(writers).unwrap()));
        std::thread::scope(|scope| {
            for t in 0..writers {
                let store = store.clone();
                let barrier = barrier.clone();
                scope.spawn(move || {
                    let lmpg = Lmpg::new(store, TenantId::from_ordinal(300 + t));
                    barrier.wait();
                    for i in 0..per_writer {
                        lmpg.create_record(
                            "S",
                            None,
                            vec![("u".into(), FieldChange::Set(json!(i)))],
                        )
                        .unwrap();
                    }
                });
            }
        });
        assert_eq!(store.committed_version(), writers * per_writer);
        let groups = store.group_commit_count();
        assert!(
            groups >= 1 && groups <= writers * per_writer,
            "groups {groups}"
        );
    }
    // Every acknowledged commit survives a reopen with the linger configured.
    let reopened = Store::open(&db).unwrap();
    assert_eq!(reopened.committed_version(), writers * per_writer);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn dependency_free_autocommits_join_the_bounded_publish_group() {
    let dir = path("blind-group");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let store = Arc::new(
        Store::open_with_resources(
            &db,
            rushdb_storage::resources::StorageResources {
                max_group_delay_ms: 10,
                ..Default::default()
            },
        )
        .unwrap(),
    );
    store.set_expander(Arc::new(Box::new(|ci, _batch, txn| {
        txn.put(
            format!("blind/{ci}").into_bytes(),
            ci.to_be_bytes().to_vec(),
        );
        Ok(())
    })));
    let writers = 8u64;
    std::thread::scope(|scope| {
        for worker in 0..writers {
            let store = Arc::clone(&store);
            scope.spawn(move || {
                let tenant = TenantId::from_ordinal(10_000 + worker);
                let key = uuid::Uuid::now_v7();
                store
                    .commit_for_tenant(&tenant, key, move |_, _| {
                        Ok((
                            CommitBatch {
                                command_version: rushdb_types::command::COMMAND_VERSION,
                                tenant,
                                transaction_id: "blind-group".into(),
                                commit_key: key,
                                start_index: 0,
                                isolation: rushdb_types::command::IsolationMode::ReadCommitted,
                                preconditions: vec![],
                                mutations: vec![],
                            },
                            (),
                        ))
                    })
                    .unwrap();
            });
        }
    });
    assert_eq!(store.committed_version(), writers);
    assert!(store.group_commit_count() < writers);
    drop(store);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn incremental_snapshot_replays_a_wal_delta_onto_a_physical_base() {
    // V2 PITR: a base at commit B (a physical checkpoint) plus the WAL delta
    // (B, T] reconstructs the source at T exactly (same ordinals and ids).
    let dir = path("pitr");
    std::fs::create_dir_all(&dir).unwrap();
    let src_db = dir.join("src.redb");
    let base_db = dir.join("base.redb");
    let base_ci;
    {
        let store = Arc::new(Store::open(&src_db).unwrap());
        let lmpg = Lmpg::new(store.clone(), TENANT);
        for i in 0..20u64 {
            lmpg.create_record("S", None, vec![("u".into(), FieldChange::Set(json!(i)))])
                .unwrap();
        }
        base_ci = store.committed_version();
        assert_eq!(base_ci, 20);
        // Log-first: a physical base backup copies the materialization, so it
        // must be checkpointed (fsynced) first; the WAL delta then carries the
        // commits after the checkpoint.
        store.checkpoint().unwrap();
        std::fs::copy(&src_db, &base_db).unwrap();
        let src_manifest = std::path::PathBuf::from(format!("{}.layout.json", src_db.display()));
        let base_manifest = std::path::PathBuf::from(format!("{}.layout.json", base_db.display()));
        let mut manifest: Value =
            serde_json::from_slice(&std::fs::read(src_manifest).unwrap()).unwrap();
        manifest["shard_files"] = serde_json::json!(["base.redb"]);
        std::fs::write(base_manifest, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
        for i in 20..50u64 {
            lmpg.create_record("S", None, vec![("u".into(), FieldChange::Set(json!(i)))])
                .unwrap();
        }
        let mut delta = Vec::new();
        store
            .export_wal_segments(&mut delta, base_ci + 1, 50)
            .unwrap();

        // Replay the delta onto the physical base copy.
        let base = Arc::new(Store::open(&base_db).unwrap());
        let _base_lmpg = Lmpg::new(base.clone(), TENANT); // registers the expander
        assert_eq!(base.committed_version(), base_ci);
        let last = base.replay_wal_segments(&mut &delta[..]).unwrap();
        assert_eq!(last, 50, "replayed to the target commit");
        assert_eq!(base.committed_version(), 50);

        // The base now matches the source: identical ordinals and contents.
        let src_view = store.read_view().unwrap();
        let base_view = base.read_view().unwrap();
        let src_members = reader::try_access::label_members(&src_view, TENANT, "S").unwrap();
        let base_members = reader::try_access::label_members(&base_view, TENANT, "S").unwrap();
        assert_eq!(base_members, src_members);
        let ord = base_members[0];
        let src_rec = rushdb_lmpg::reconstruct::reconstruct_record(&src_view, TENANT, ord).unwrap();
        let base_rec =
            rushdb_lmpg::reconstruct::reconstruct_record(&base_view, TENANT, ord).unwrap();
        assert_eq!(base_rec, src_rec);
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn async_durability_checkpoint_flushes_queued_writes() {
    // V2 buffered async durability: commits acknowledge before fsync, and a
    // checkpoint persists everything queued. Reopen sees every commit.
    let dir = path("async-checkpoint");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    {
        let res = rushdb_storage::resources::StorageResources {
            durability: rushdb_storage::resources::Durability::Async,
            ..Default::default()
        };
        let store = Arc::new(Store::open_with_resources(&db, res).unwrap());
        let lmpg = Lmpg::new(store.clone(), TENANT);
        for i in 0..100u64 {
            lmpg.create_record("S", None, vec![("u".into(), FieldChange::Set(json!(i)))])
                .unwrap();
        }
        assert_eq!(store.committed_version(), 100);
        store.checkpoint().expect("checkpoint flushes async writes");
    }
    let reopened = Store::open(&db).unwrap();
    assert_eq!(reopened.committed_version(), 100);
    let view = reopened.read_view().unwrap();
    assert_eq!(
        reader::try_access::label_members(&view, TENANT, "S")
            .unwrap()
            .len(),
        100
    );
    std::fs::remove_dir_all(&dir).ok();
}
