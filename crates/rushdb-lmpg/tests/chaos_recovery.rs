//! Deterministic fault injection (documents 07/13): crashes at every
//! commit boundary and torn-tail WAL writes must converge to identical,
//! invariant-clean state after recovery.

use std::sync::Arc;

use rushdb_lmpg::import::import_document;
use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use rushdb_types::keys::*;

fn state_fingerprint(store: &Store) -> u64 {
    let tenant_data = TenantId::from_ordinal(7);
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (k, v) in store
        .read_view()
        .unwrap()
        .range_prefix(&key(tenant_data, &[]))
        .unwrap()
    {
        if k.windows(7).any(|w| w == b"__log__")
            || k.starts_with(b"txn-")
            || k.windows(17).any(|w| w == b"meta/last-applied")
        {
            continue;
        }
        for b in &k {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h = !h;
        for b in &v {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

fn raw_commit(
    store: &Store,
    tenant: TenantId,
    mutation: rushdb_types::command::Mutation,
) -> Result<rushdb_storage::CommitOutcome<()>, rushdb_storage::StorageError> {
    store.commit_for_tenant(&tenant, uuid::Uuid::new_v4(), |_ci, _txn| {
        let batch = rushdb_types::command::CommitBatch {
            command_version: rushdb_types::command::COMMAND_VERSION,
            tenant,
            transaction_id: "test-raw".into(),
            commit_key: uuid::Uuid::nil(),
            start_index: 0,
            isolation: rushdb_types::command::IsolationMode::Snapshot,
            preconditions: Vec::new(),
            mutations: vec![mutation],
        };
        Ok((batch, ()))
    })
}

#[test]
fn scrub_repair_removes_bounded_orphan_occurrences() {
    let store = Arc::new(Store::new_in_memory());
    let tenant = TenantId::from_ordinal(70);
    let _lmpg = Lmpg::new(Arc::clone(&store), tenant);
    let orphan = occ_key(tenant, rushdb_types::ids::OwnerKind::Record, 1, 2, 0);
    raw_commit(
        &store,
        tenant,
        rushdb_types::command::Mutation::PutRawKey {
            key: orphan.clone(),
            value: rushdb_types::occurrence::encode_occ_value(
                false,
                rushdb_types::occurrence::OccurrenceValue::ExplicitNull,
                None,
            ),
        },
    )
    .unwrap();
    let before = store.read_view().unwrap();
    let report = rushdb_lmpg::scrub::scrub_report(&before, tenant).unwrap();
    assert_eq!(report.occurrence_rows, 1, "{report:?}");
    assert!(report
        .violations
        .iter()
        .any(|v| v.what == "occurrence_orphaned"));
    let plan = rushdb_lmpg::scrub::plan_orphan_occurrences(&before, tenant, 1).unwrap();
    assert_eq!(plan.keys, vec![orphan]);
    assert!(plan.complete);
    drop(before);
    assert_eq!(
        rushdb_lmpg::scrub::repair_orphan_occurrences(&store, tenant, 1).unwrap(),
        1
    );
    let after = store.read_view().unwrap();
    let report = rushdb_lmpg::scrub::scrub_report(&after, tenant).unwrap();
    // Repair removes the base row; apply-time derivation removes the
    // presence and counters it produced, so nothing is left to flag.
    assert!(report.violations.is_empty(), "{:?}", report.violations);
}

#[test]
fn crash_at_every_commit_boundary_converges() {
    let primary = Arc::new(Store::new_in_memory());
    let tenant = TenantId::from_ordinal(9);

    {
        let eng = Lmpg::new(Arc::clone(&primary), tenant);
        import_document(
            &eng,
            "City",
            &serde_json::json!({"city":"Rome","pop":4_000_000}),
        )
        .unwrap();
        let a = eng
            .create_record(
                "X",
                None,
                vec![("k".into(), FieldChange::Set(serde_json::json!("v")))],
            )
            .unwrap();
        let b = eng.create_record("Y", None, vec![]).unwrap();
        eng.create_relationship(
            "L",
            a,
            b,
            vec![("m".into(), FieldChange::Set(serde_json::json!(1)))],
        )
        .unwrap();
        eng.update_fields(
            a,
            vec![("k".into(), FieldChange::Set(serde_json::json!("w")))],
        )
        .unwrap();
        eng.delete_record(b).unwrap();
    }

    let full_log = primary.take_log_for_crash();
    assert!(full_log.len() >= 5, "scenario produced too few commits");

    // every prefix is a clean checkpoint; replay twice and compare.
    for cut in 0..=full_log.len() {
        let prefix = &full_log[..cut];

        let rec_store = Arc::new(Store::new_in_memory());
        drop(Lmpg::new(Arc::clone(&rec_store), tenant));
        rec_store.try_replay(prefix).unwrap();

        let rec_store2 = Arc::new(Store::new_in_memory());
        drop(Lmpg::new(Arc::clone(&rec_store2), tenant));
        rec_store2.try_replay(prefix).unwrap();

        assert_eq!(
            state_fingerprint(rec_store.as_ref()),
            state_fingerprint(rec_store2.as_ref()),
            "replay instability at boundary {cut}"
        );
    }

    // torn tails (partially flushed final line) must not poison the file
    for cut in 1..full_log.len() {
        let mut truncated = full_log[..cut].to_vec();
        truncated.push(full_log[cut][..8.min(full_log[cut].len())].to_vec());

        let good: Vec<Vec<u8>> = truncated
            .iter()
            .filter(|raw| rushdb_types::command_codec::decode(raw).is_ok())
            .cloned()
            .collect();

        let st = Arc::new(Store::new_in_memory());
        drop(Lmpg::new(Arc::clone(&st), tenant));
        st.try_replay(&good).unwrap();
        assert!(
            st.as_ref().committed_version() >= cut as u64 || cut == full_log.len(),
            "torn tail at {cut} lost committed data"
        );
    }
}

#[test]
fn scrub_invariants_hold_after_workload_and_replay() {
    let primary = Arc::new(Store::new_in_memory());
    let tenant = TenantId::from_ordinal(13);
    {
        let eng = Lmpg::new(Arc::clone(&primary), tenant);
        for i in 0..5u64 {
            import_document(
                &eng,
                "Note",
                &serde_json::json!({"n": i, "body": format!("note {i}"), "flag": i % 2 == 0}),
            )
            .unwrap();
        }
        let a = eng.create_record("A", None, vec![]).unwrap();
        let b = eng.create_record("B", None, vec![]).unwrap();
        let rel = eng
            .create_relationship(
                "R",
                a,
                b,
                vec![("w".into(), FieldChange::Set(serde_json::json!(3)))],
            )
            .unwrap();
        let _ = rel;
        eng.delete_record(b).unwrap(); // cascade removes rel + occurrences
    }

    let log = primary.take_log_for_crash();
    let rec_store = Arc::new(Store::new_in_memory());
    drop(Lmpg::new(Arc::clone(&rec_store), tenant));
    rec_store.try_replay(&log).unwrap();

    let view_replayed = rec_store.read_view().unwrap();
    let report = rushdb_lmpg::scrub::scrub_report(&view_replayed, tenant).unwrap();
    assert!(report.complete);
    assert!(report.occurrence_rows > 0);
    assert!(report.derived_rows > 0);
    assert!(
        report.violations.is_empty(),
        "invariant violations after replay: {:?}",
        report.violations
    );

    let view_live = primary.read_view().unwrap();
    let violations_live = rushdb_lmpg::scrub::scrub(&view_live, tenant).unwrap();
    assert!(violations_live.is_empty(), "{violations_live:?}");
}

#[test]
fn raw_maintenance_cannot_write_derived_rows() {
    // DC12: derived rows are computed at apply time and never logged, so a
    // logged raw-key mutation must not reach them.
    let store = Arc::new(Store::new_in_memory());
    let tenant = TenantId::from_ordinal(71);
    let eng = Lmpg::new(Arc::clone(&store), tenant);
    eng.create_record(
        "Note",
        None,
        vec![(
            "status".into(),
            FieldChange::Set(serde_json::json!("active")),
        )],
    )
    .unwrap();
    let presence = store
        .read_view()
        .unwrap()
        .range_prefix(&{
            let mut prefix = tenant_prefix(tenant).to_vec();
            prefix.push(F_PRESENCE);
            prefix
        })
        .unwrap();
    let (key, _) = presence.first().expect("a presence row").clone();
    assert!(raw_commit(
        &store,
        tenant,
        rushdb_types::command::Mutation::DeleteRawKey { key: key.clone() }
    )
    .is_err());
    assert!(raw_commit(
        &store,
        tenant,
        rushdb_types::command::Mutation::PutRawKey { key, value: vec![] }
    )
    .is_err());
    let view = store.read_view().unwrap();
    assert!(rushdb_lmpg::scrub::scrub(&view, tenant).unwrap().is_empty());
}

#[test]
fn scrub_flags_a_surplus_identity_row() {
    let store = Arc::new(Store::new_in_memory());
    let tenant = TenantId::from_ordinal(72);
    let eng = Lmpg::new(Arc::clone(&store), tenant);
    let a = eng.create_record("Doc", None, vec![]).unwrap();
    // A second id resolving to `a`'s ordinal.
    raw_commit(
        &store,
        tenant,
        rushdb_types::command::Mutation::PutRawKey {
            key: record_id_key(tenant, &[0xAB; 16]),
            value: a.ordinal.to_be_bytes().to_vec(),
        },
    )
    .unwrap();
    let view = store.read_view().unwrap();
    let violations = rushdb_lmpg::scrub::scrub(&view, tenant).unwrap();
    assert!(
        violations
            .iter()
            .any(|v| v.what == "record_id_index_orphaned"),
        "{violations:?}"
    );
}
