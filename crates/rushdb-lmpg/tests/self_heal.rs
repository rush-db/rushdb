//! SELF-HEAL: a deriver bug leaves a label's derived rows disagreeing with
//! its base rows; one logged re-derive makes them exactly what the base rows
//! imply again (the scrubber, re-deriving the whole tenant from scratch, is
//! the oracle), on the node that commits it and on every node that replays
//! its log.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{BatchOp, BatchTx, Lmpg};
use rushdb_storage::{DeriveRead, Deriver, Op, StorageError, Store};
use rushdb_types::ids::TenantId;
use rushdb_types::index::IndexKind;
use rushdb_types::keys as k;
use serde_json::json;

/// The real deriver with a bug: it loses `Bad`'s presence rows, dictionary
/// counts and index entries (whatever their commit).
struct Buggy;

impl Deriver for Buggy {
    fn derive(
        &self,
        ci: u64,
        ops: &[Op],
        before: &dyn DeriveRead,
    ) -> Result<Vec<Op>, StorageError> {
        let derived = rushdb_lmpg::derived::LmpgDeriver.derive(ci, ops, before)?;
        let lost = |key: &[u8]| {
            let bad = |prefix: Vec<u8>| key.starts_with(&prefix);
            let kind = rushdb_types::ids::OwnerKind::Record;
            bad(k::presence_label_prefix(
                k::parse_tenant(key).unwrap(),
                kind,
                "Bad",
            )) || bad(k::value_dict_label_prefix(
                k::parse_tenant(key).unwrap(),
                kind,
                "Bad",
            )) || key.get(k::TENANT_PREFIX_LEN) == Some(&k::F_INDEX_ENTRY)
        };
        Ok(derived
            .into_iter()
            .filter(|op| match op {
                Op::Put(key, _) | Op::Set(key, _) => k::parse_tenant(key).is_none() || !lost(key),
                Op::Del(_) | Op::Purge(_) => true,
            })
            .collect())
    }

    fn is_derived_key(&self, key: &[u8]) -> bool {
        rushdb_lmpg::derived::LmpgDeriver.is_derived_key(key)
    }

    fn prefix_reaches_derived(&self, prefix: &[u8]) -> bool {
        rushdb_lmpg::derived::LmpgDeriver.prefix_reaches_derived(prefix)
    }
}

fn records(lmpg: &Lmpg, label: &str, n: i64) {
    lmpg.run_batch(BatchTx {
        ops: (0..n)
            .map(|i| BatchOp::CreateRecord {
                label: label.into(),
                explicit_id: None,
                fields: vec![
                    ("n".into(), FieldChange::Set(json!(i))),
                    ("tag".into(), FieldChange::Set(json!(format!("t{}", i % 3)))),
                    (
                        "note".into(),
                        FieldChange::Set(json!(format!("{i} {}", "x".repeat(300)))),
                    ),
                    ("gone".into(), FieldChange::Set(json!(null))),
                    ("list".into(), FieldChange::Set(json!([i, i + 1]))),
                ],
            })
            .collect(),
    })
    .unwrap();
}

fn violations(store: &Store, tenant: TenantId) -> Vec<String> {
    let view = store.read_view().unwrap();
    rushdb_lmpg::scrub::scrub_report(&view, tenant)
        .unwrap()
        .violations
        .into_iter()
        .map(|v| format!("{}: {}", v.what, v.detail))
        .collect()
}

#[test]
fn a_rederive_restores_what_the_base_rows_imply() {
    let store = Arc::new(Store::new_in_memory());
    let tenant = TenantId::from_ordinal(41);
    let lmpg = Lmpg::new(Arc::clone(&store), tenant);
    records(&lmpg, "Good", 20);
    let def = lmpg
        .define_index("bad_n", "Bad", "n", IndexKind::Ordered)
        .unwrap();
    while !matches!(
        lmpg.index_step(def.id, 1_000).unwrap(),
        rushdb_lmpg::index::IndexStep::Finished | rushdb_lmpg::index::IndexStep::Idle
    ) {}
    // Correct history first: some `Bad` records, one of them deleted.
    records(&lmpg, "Bad", 5);
    let view = store.read_view().unwrap();
    let first = rushdb_lmpg::reader::try_access::label_members(&view, tenant, "Bad").unwrap()[0];
    let header = rushdb_lmpg::reader::try_access::record_by_ord(&view, tenant, first)
        .unwrap()
        .unwrap();
    drop(view);
    lmpg.delete_record(rushdb_lmpg::RecordRef {
        record_id: header.record_id,
        ordinal: header.ordinal,
    })
    .unwrap();
    // Then the bug.
    store.set_deriver(Arc::new(Buggy));
    records(&lmpg, "Bad", 30);
    store.set_deriver(Arc::new(rushdb_lmpg::derived::LmpgDeriver));
    assert!(
        !violations(&store, tenant).is_empty(),
        "the bug left no trace"
    );

    lmpg.rederive_label("Bad").unwrap();
    assert_eq!(violations(&store, tenant), Vec::<String>::new());
    // Already exact: a second re-derive changes nothing.
    let before = store.read_view().unwrap().commit_index();
    lmpg.rederive_label("Good").unwrap();
    assert_eq!(violations(&store, tenant), Vec::<String>::new());
    assert!(store.read_view().unwrap().commit_index() > before);
}

#[test]
fn a_replica_with_the_same_bug_converges_by_replaying_the_repair() {
    // The repair is a logged command, not rows: a node that replays the log
    // recomputes the label from its own base rows through the deriver.
    let dir = rushdb_storage::TempPath::dir("self-heal-replica");
    std::fs::create_dir_all(&dir).unwrap();
    let primary = Arc::new(rushdb_lmpg::open_store(&dir.join("rushdb.redb")).unwrap());
    let tenant = TenantId::from_ordinal(42);
    let lmpg = Lmpg::new(Arc::clone(&primary), tenant);
    primary.set_deriver(Arc::new(Buggy));
    records(&lmpg, "Bad", 25);
    primary.set_deriver(Arc::new(rushdb_lmpg::derived::LmpgDeriver));
    let broken = primary.committed_version();
    lmpg.rederive_label("Bad").unwrap();
    let repaired = primary.committed_version();
    assert_eq!(violations(&primary, tenant), Vec::<String>::new());

    let replica = Store::new_in_memory();
    let segment = |from: u64, to: u64| {
        let mut out = Vec::new();
        primary.export_wal_segments(&mut out, from, to).unwrap();
        out
    };
    replica.set_deriver(Arc::new(Buggy));
    replica
        .replay_wal_segments(&mut segment(1, broken).as_slice())
        .unwrap();
    assert!(!violations(&replica, tenant).is_empty());
    replica.set_deriver(Arc::new(rushdb_lmpg::derived::LmpgDeriver));
    replica
        .replay_wal_segments(&mut segment(broken + 1, repaired).as_slice())
        .unwrap();
    assert_eq!(violations(&replica, tenant), Vec::<String>::new());
    drop(lmpg);
    drop(primary);
    std::fs::remove_dir_all(dir).unwrap();
}

/// The real deriver with a bug that loses every null-count change.
struct LosesNulls;

impl Deriver for LosesNulls {
    fn derive(
        &self,
        ci: u64,
        ops: &[Op],
        before: &dyn DeriveRead,
    ) -> Result<Vec<Op>, StorageError> {
        let derived = rushdb_lmpg::derived::LmpgDeriver.derive(ci, ops, before)?;
        Ok(derived
            .into_iter()
            .filter(|op| match op {
                Op::Put(key, _) | Op::Del(key) => k::parse_tenant(key).is_none_or(|tenant| {
                    // Null counts of any property of records.
                    !(0..64).any(|pid| {
                        key == &k::null_count_key(tenant, rushdb_types::ids::OwnerKind::Record, pid)
                    })
                }),
                _ => true,
            })
            .collect())
    }

    fn is_derived_key(&self, key: &[u8]) -> bool {
        rushdb_lmpg::derived::LmpgDeriver.is_derived_key(key)
    }

    fn prefix_reaches_derived(&self, prefix: &[u8]) -> bool {
        rushdb_lmpg::derived::LmpgDeriver.prefix_reaches_derived(prefix)
    }
}

#[test]
fn a_rederive_recounts_its_properties_nulls_across_labels() {
    let store = Arc::new(Store::new_in_memory());
    let tenant = TenantId::from_ordinal(44);
    let lmpg = Lmpg::new(Arc::clone(&store), tenant);
    // `gone` is null on every record of both labels.
    records(&lmpg, "Good", 7);
    store.set_deriver(Arc::new(LosesNulls));
    records(&lmpg, "Bad", 9);
    store.set_deriver(Arc::new(rushdb_lmpg::derived::LmpgDeriver));
    assert!(
        !violations(&store, tenant).is_empty(),
        "the bug left no trace"
    );
    lmpg.rederive_label("Bad").unwrap();
    assert_eq!(violations(&store, tenant), Vec::<String>::new());
}

#[test]
fn a_suspect_marker_survives_a_reopen_until_the_repair() {
    let dir = rushdb_storage::TempPath::dir("self-heal-marker");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rushdb.redb");
    let tenant = TenantId::from_ordinal(45);
    {
        let store = Arc::new(rushdb_lmpg::open_store(&path).unwrap());
        let lmpg = Lmpg::new(Arc::clone(&store), tenant);
        records(&lmpg, "Bad", 3);
        lmpg.mark_suspect("Bad").unwrap();
        store.checkpoint().unwrap();
    }
    {
        let store = Arc::new(rushdb_lmpg::open_store(&path).unwrap());
        assert!(!store.derived_health().is_suspect(tenant, "Bad"));
        assert_eq!(rushdb_lmpg::heal::load_suspects(&store, tenant).unwrap(), 1);
        assert!(store.derived_health().is_suspect(tenant, "Bad"));
        // The repair deletes the marker with the re-derive.
        Lmpg::new(Arc::clone(&store), tenant)
            .rederive_label("Bad")
            .unwrap();
        assert!(!store.derived_health().is_suspect(tenant, "Bad"));
        store.checkpoint().unwrap();
    }
    let store = rushdb_lmpg::open_store(&path).unwrap();
    assert_eq!(rushdb_lmpg::heal::load_suspects(&store, tenant).unwrap(), 0);
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}
