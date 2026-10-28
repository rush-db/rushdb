//! End-to-end vertical slice over the LMPG mutation and read paths,
//! covering records, relationships, retyping, batching and reconstruction.

use rushdb_lmpg::import::import_document;
use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::reader::{keys_ext as kx, try_access};
use rushdb_lmpg::Lmpg;
use rushdb_storage::Store;

use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::keys::{key, P_FAMILY_SIG};
use serde_json::{json, Value as Json};

fn engine() -> Lmpg {
    Lmpg::new(
        std::sync::Arc::new(Store::new_in_memory()),
        TenantId::from_ordinal(42),
    )
}

fn rebuild_record(eng: &Lmpg, ord: u64) -> Json {
    let snap = eng.store.read_view().unwrap();
    rushdb_lmpg::reconstruct::reconstruct_record(&snap, eng.tenant, ord)
        .unwrap()
        .unwrap()
}

fn family_sig_exists(eng: &Lmpg, scope: OwnerKind, name: &str) -> bool {
    let sig = key(
        eng.tenant,
        &[
            P_FAMILY_SIG,
            b"default",
            &[scope.as_byte()],
            name.as_bytes(),
        ],
    );
    eng.store.read_view().unwrap().get(&sig).unwrap().is_some()
}

fn find_pid(eng: &Lmpg, norm_name: &str) -> u64 {
    use rushdb_types::keys::*;
    let sig = key(
        eng.tenant,
        &[
            P_FAMILY_SIG,
            b"default",
            &[OwnerKind::Record.as_byte()],
            norm_name.as_bytes(),
        ],
    );
    let raw = eng
        .store
        .read_view()
        .unwrap()
        .get(&sig)
        .unwrap()
        .expect("family sig exists");
    u64::from_be_bytes([
        raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
    ])
}

#[test]
fn vertical_slice_1_null_missing_and_unresolved_resolution() {
    let eng = engine();

    // First-null observation creates the family + neutral Unresolved definition.
    eng.create_record(
        "Person",
        None,
        vec![("age".into(), FieldChange::ExplicitNull)],
    )
    .unwrap();
    assert!(family_sig_exists(&eng, OwnerKind::Record, "age"));

    // A later typed value resolves a Known(Int64) definition; the null
    // occurrence stays attached to the neutral Unresolved definition.
    eng.create_record(
        "Person",
        None,
        vec![("age".into(), FieldChange::Set(json!(30)))],
    )
    .unwrap();

    let null_json = rebuild_record(&eng, 0);
    assert_eq!(null_json.get("age"), Some(&Json::Null));
    let typed_json = rebuild_record(&eng, 1);
    assert_eq!(typed_json.get("age"), Some(&json!(30)));
}

#[test]
fn vertical_slice_2_lossless_reconstruction_including_null_and_arrays() {
    let eng = engine();
    let doc = json!({
        "name": "Ada",
        "score": 99,
        "ratio": 0.5,
        "active": true,
        "nickname": null,
        "tags": ["a", "b", "c"],
        "nums": [1, 2]
    });
    import_document(&eng, "Scientist", &doc).unwrap();
    let j = rebuild_record(&eng, 0);

    assert_eq!(j.get("name"), Some(&json!("Ada")));
    assert_eq!(j.get("score"), Some(&json!(99)));
    assert_eq!(j.get("ratio"), Some(&json!(0.5)));
    assert_eq!(j.get("active"), Some(&json!(true)));
    assert_eq!(j.get("nickname"), Some(&Json::Null));
    assert!(!j.as_object().unwrap().contains_key("does_not_exist"));
    assert_eq!(j.get("tags"), Some(&json!(["a", "b", "c"])));
    assert_eq!(j.get("nums"), Some(&json!([1, 2])));
}

#[test]
fn vertical_slice_3_relationship_properties_and_traversal() {
    let eng = engine();
    let a = eng
        .create_record(
            "Company",
            None,
            vec![("name".into(), FieldChange::Set(json!("Acme")))],
        )
        .unwrap();
    let b = eng
        .create_record(
            "Company",
            None,
            vec![("name".into(), FieldChange::Set(json!("Beta")))],
        )
        .unwrap();
    let rel = eng
        .create_relationship(
            "PARTNER_OF",
            a,
            b,
            vec![
                ("since".into(), FieldChange::Set(json!(2001))),
                ("tier".into(), FieldChange::ExplicitNull),
            ],
        )
        .unwrap();

    let snap = eng.store.read_view().unwrap();
    let rj = rushdb_lmpg::reconstruct::reconstruct_relationship(&snap, eng.tenant, rel.ordinal)
        .unwrap()
        .unwrap();
    assert_eq!(rj.get("since"), Some(&json!(2001)));
    assert_eq!(rj.get("tier"), Some(&Json::Null));

    let out = try_access::adj_out(&snap, eng.tenant, a.ordinal, None).unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].0, "PARTNER_OF");
    assert_eq!(out[0].1, b.ordinal);
    assert_eq!(out[0].2, rel.ordinal);
}

#[test]
fn vertical_slice_4_exact_support_and_null_stats() {
    use rushdb_types::model::{LogicalType, TypeState};
    let eng = engine();
    eng.create_record(
        "Person",
        None,
        vec![("age".into(), FieldChange::Set(json!(1)))],
    )
    .unwrap();
    eng.create_record(
        "Person",
        None,
        vec![("age".into(), FieldChange::ExplicitNull)],
    )
    .unwrap();
    eng.create_record("Person", None, vec![]).unwrap();

    // The value and the explicit null live on different definitions of the
    // family (Known(Int64) and the neutral Unresolved one); the family-wide
    // support and null counts are exact.
    let family = find_pid(&eng, "age");
    let view = eng.store.read_view().unwrap();
    let mut per_def = Vec::new();
    for def in try_access::definitions_of_family(&view, eng.tenant, family).unwrap() {
        let pid = def.property_id;
        let occ =
            rushdb_lmpg::derived::prop_support_total(&view, eng.tenant, OwnerKind::Record, pid)
                .unwrap();
        let nulls =
            rushdb_lmpg::derived::null_count(&view, eng.tenant, OwnerKind::Record, pid).unwrap();
        per_def.push((def.type_state, occ, nulls));
    }
    per_def.sort_by_key(|(_, occ, nulls)| (*occ, *nulls));
    assert_eq!(
        per_def,
        vec![
            (TypeState::Known(LogicalType::Int64), 1, 0),
            (TypeState::Unresolved, 1, 1)
        ]
    );
}

#[test]
fn vertical_slice_6_delete_cascades_incident_relationships() {
    let eng = engine();
    let a = eng.create_record("A", None, vec![]).unwrap();
    let b = eng.create_record("B", None, vec![]).unwrap();
    eng.create_relationship("REL", a, b, vec![("w".into(), FieldChange::Set(json!(5)))])
        .unwrap();

    eng.delete_record(a).unwrap();

    let snap = eng.store.read_view().unwrap();
    assert!(try_access::record_by_ord(&snap, eng.tenant, a.ordinal)
        .unwrap()
        .is_none());
    assert!(try_access::adj_out(&snap, eng.tenant, a.ordinal, None)
        .unwrap()
        .is_empty());
    assert!(try_access::adj_in(&snap, eng.tenant, b.ordinal, None)
        .unwrap()
        .is_empty());
}

#[test]
fn schemaless_property_accepts_heterogeneous_types() {
    // Product parity: a property holds different types across records
    // (e.g. `value` is a string on one row, a number on another).
    let eng = engine();
    let a = eng
        .create_record("T", None, vec![("n".into(), FieldChange::Set(json!(10)))])
        .unwrap();
    let b = eng
        .create_record(
            "T",
            None,
            vec![("n".into(), FieldChange::Set(json!("oops")))],
        )
        .unwrap();

    // both writes committed with their own logical type
    let snap = eng.store.read_view().unwrap();
    let ra = rushdb_lmpg::reconstruct::reconstruct_record(&snap, eng.tenant, a.ordinal)
        .unwrap()
        .unwrap();
    let rb = rushdb_lmpg::reconstruct::reconstruct_record(&snap, eng.tenant, b.ordinal)
        .unwrap()
        .unwrap();
    assert_eq!(ra.get("n"), Some(&json!(10)));
    assert_eq!(rb.get("n"), Some(&json!("oops")));

    // and updating each record with a value of its OWN type still works
    eng.update_fields(a, vec![("n".into(), FieldChange::Set(json!(11)))])
        .unwrap();
    eng.update_fields(b, vec![("n".into(), FieldChange::Set(json!("ops2")))])
        .unwrap();
    let snap = eng.store.read_view().unwrap();
    let ra = rushdb_lmpg::reconstruct::reconstruct_record(&snap, eng.tenant, a.ordinal)
        .unwrap()
        .unwrap();
    let rb = rushdb_lmpg::reconstruct::reconstruct_record(&snap, eng.tenant, b.ordinal)
        .unwrap()
        .unwrap();
    assert_eq!(ra.get("n"), Some(&json!(11)));
    assert_eq!(rb.get("n"), Some(&json!("ops2")));
}

#[test]
fn nested_objects_promote_to_child_records() {
    let eng = engine();
    let doc = json!({
        "name": "Ada",
        "address": {"city": "London", "zip": "E1"}
    });
    import_document(&eng, "Person", &doc).unwrap();
    let parent = rebuild_record(&eng, 0);
    assert_eq!(parent.get("name"), Some(&json!("Ada")));

    let snap = eng.store.read_view().unwrap();
    let edges = try_access::adj_out(&snap, eng.tenant, 0, None).unwrap();
    assert_eq!(edges.len(), 1);
    let child_ord = edges[0].1;
    let child = rebuild_record(&eng, child_ord);
    assert_eq!(child.get("city"), Some(&json!("London")));
    assert_eq!(
        child.get("__label").and_then(|l| l.as_str()),
        Some("Person/ADDRESS")
    );
}

#[test]
fn crash_recovery_replays_to_identical_state() {
    let primary = std::sync::Arc::new(Store::new_in_memory());
    let tenant = TenantId::from_ordinal(7);
    {
        let eng = Lmpg::new(std::sync::Arc::clone(&primary), tenant);
        import_document(&eng, "City", &json!({"city": "Rome", "pop": 4_000_000})).unwrap();
        let a = eng.create_record("X", None, vec![]).unwrap();
        let b = eng.create_record("Y", None, vec![]).unwrap();
        eng.create_relationship("L", a, b, vec![("k".into(), FieldChange::Set(json!("v")))])
            .unwrap();
        eng.delete_record(b).unwrap();
    }

    let log = primary.take_log_for_crash();
    drop(primary);
    let eng = Lmpg::new(std::sync::Arc::new(Store::new_in_memory()), tenant);
    eng.store.try_replay(&log).unwrap();

    let city = rebuild_record(&eng, 0);
    assert_eq!(city.get("city"), Some(&json!("Rome")));
    assert_eq!(city.get("pop"), Some(&json!(4_000_000)));

    let snap = eng.store.read_view().unwrap();
    assert!(try_access::record_by_ord(&snap, eng.tenant, 1)
        .unwrap()
        .is_some());
    assert!(try_access::record_by_ord(&snap, eng.tenant, 3)
        .unwrap()
        .is_none());
}

/// Regression: typed adjacency scans must preserve the relationship ordinal
/// byte-for-byte. The key parser once truncated the trailing byte, so edges
/// with ordinal >= 256 all collapsed to the same ordinal.
#[test]
fn adjacency_typed_scan_preserves_ordinal_past_256() {
    let store = std::sync::Arc::new(rushdb_storage::Store::new_in_memory());
    let lmpg = rushdb_lmpg::Lmpg::new(store, rushdb_types::ids::TenantId::from_ordinal(7));
    // 261 records: 260 anchors + 1 hub; relationship ordinals 0..259
    let mut anchors = Vec::new();
    for _ in 0..260 {
        anchors.push(lmpg.create_record("Anchor", None, vec![]).unwrap());
    }
    let hub = lmpg.create_record("Hub", None, vec![]).unwrap();
    for a in &anchors {
        lmpg.create_relationship("links", *a, hub, vec![]).unwrap();
    }
    let snap = lmpg.store.read_view().unwrap();
    let low =
        rushdb_lmpg::reader::try_access::adj_out(&snap, lmpg.tenant, 1, Some("links")).unwrap();
    assert_eq!(low.len(), 1);
    assert_eq!(low[0].2, 1, "rel ordinal 1 preserved");
    let high =
        rushdb_lmpg::reader::try_access::adj_out(&snap, lmpg.tenant, 257, Some("links")).unwrap();
    assert_eq!(high.len(), 1);
    assert_eq!(
        high[0].2, 257,
        "rel ordinal 257 preserved (byte-truncation regression)"
    );

    // 47 is ASCII '/', which is valid inside both fixed-width binary
    // ordinals and must never be mistaken for a key separator.
    let slash =
        rushdb_lmpg::reader::try_access::adj_out(&snap, lmpg.tenant, 47, Some("links")).unwrap();
    assert_eq!(slash.len(), 1);
    assert_eq!(slash[0].1, hub.ordinal);
    assert_eq!(slash[0].2, 47);
    let slash_untyped =
        rushdb_lmpg::reader::try_access::adj_out(&snap, lmpg.tenant, 47, None).unwrap();
    assert_eq!(slash_untyped, slash);
}

#[test]
fn same_batch_keeps_distinct_typed_property_definitions() {
    use rushdb_lmpg::reader::try_access;
    use rushdb_types::ids::OwnerKind;
    use rushdb_types::keys::{key, P_FAMILY_SIG};

    let store = std::sync::Arc::new(rushdb_storage::Store::new_in_memory());
    let lmpg = rushdb_lmpg::Lmpg::new(store, rushdb_types::ids::TenantId::from_ordinal(8));
    lmpg.create_record(
        "Mixed",
        None,
        vec![
            ("score".into(), FieldChange::Set(json!(1))),
            ("SCORE".into(), FieldChange::Set(json!("one"))),
        ],
    )
    .unwrap();

    let snap = lmpg.store.read_view().unwrap();
    let signature = key(
        lmpg.tenant,
        &[
            P_FAMILY_SIG,
            b"default",
            &[OwnerKind::Record.as_byte()],
            b"score",
        ],
    );
    let raw = snap.get(&signature).unwrap().unwrap();
    let family_id = u64::from_be_bytes(raw[..8].try_into().unwrap());
    let definitions = try_access::definitions_of_family(&snap, lmpg.tenant, family_id).unwrap();
    assert_eq!(definitions.len(), 2);
    assert_ne!(definitions[0].property_id, definitions[1].property_id);
}

#[test]
fn purge_tenant_covers_value_heavy_keyspaces_without_materializing_values() {
    // Regression: purge enumerated key/value pairs and then discarded the
    // values, so any tenant whose VALUES exceeded the 64 MiB
    // transaction-range ceiling failed with "materialized transaction range
    // exceeds 64 MiB". Purge enumerates keys only (values are deleted
    // unread), so value-heavy tenants purge with a keys-sized footprint.
    let eng = engine();
    // ~80 MB of stored values across 200 records: keys stay far below the
    // ceiling while values exceed it, which fails on the old path.
    let big = "v".repeat(400 * 1024);
    for i in 0..200 {
        eng.create_record(
            "Bulk",
            None,
            vec![
                ("idx".into(), FieldChange::Set(Json::from(i))),
                ("blob".into(), FieldChange::Set(Json::from(big.clone()))),
            ],
        )
        .unwrap();
    }
    eng.purge_tenant().unwrap();
    // Tenant keyspace is empty: no label members remain.
    let snap = eng.store.read_view().unwrap();
    assert!(try_access::label_members(&snap, eng.tenant, "Bulk")
        .unwrap()
        .is_empty());
}

#[test]
fn purge_tenant_chunks_erase_large_tenants_and_retain_no_history() {
    // Chunked purge: many commits (bounded per commit), full erasure, and no
    // MVCC history retained for erased values. The legacy single-commit purge
    // retained a version per deleted key and blew the materialization ceiling.
    let eng = engine();
    for i in 0..120 {
        let r = eng
            .create_record(
                "Bulk",
                None,
                vec![
                    ("idx".into(), FieldChange::Set(Json::from(i))),
                    (
                        "blob".into(),
                        FieldChange::Set(Json::from("v".repeat(2000))),
                    ),
                ],
            )
            .unwrap();
        // One update builds retained history to prove purge drops it.
        eng.update_fields(
            r,
            vec![("idx".into(), FieldChange::Set(Json::from(i + 1000)))],
        )
        .unwrap();
    }
    let (_, hist_before) = eng.store.approximate_map_bytes();
    assert!(hist_before > 0, "updates retain history before purge");
    // A representative tenant key whose retained versions must vanish.
    let sample = eng
        .store
        .read_view()
        .unwrap()
        .range_prefix(&key(eng.tenant, &[]))
        .unwrap()
        .into_iter()
        .map(|(k, _)| k)
        .find(|k| !k.starts_with(&rushdb_types::keys::security_prefix(eng.tenant)))
        .expect("tenant has keys before purge");

    let before = eng.store.committed_version();
    let last = eng.purge_tenant_chunked(64).unwrap();
    assert!(last > before, "purge advanced the commit index");
    assert!(
        last - before > 1,
        "a small chunk bound must take multiple commits, took {}",
        last - before
    );

    let snap = eng.store.read_view().unwrap();
    assert!(try_access::label_members(&snap, eng.tenant, "Bulk")
        .unwrap()
        .is_empty());
    let tenant_keys: Vec<Vec<u8>> = snap
        .range_prefix(&key(eng.tenant, &[]))
        .unwrap()
        .into_iter()
        .map(|(k, _)| k)
        .filter(|k| !k.starts_with(&rushdb_types::keys::security_prefix(eng.tenant)))
        .collect();
    assert!(
        tenant_keys.is_empty(),
        "tenant keyspace not fully purged: {} keys remain",
        tenant_keys.len()
    );
    // Destructive erase: the key is gone now *and* at every older snapshot
    // (no retained version), which the raw history bytes cannot show because
    // global `meta/last-applied` also accrues history.
    assert!(
        eng.store
            .read_view()
            .unwrap()
            .get(&sample)
            .unwrap()
            .is_none(),
        "key still readable"
    );
    assert!(
        eng.store.try_get_at(&sample, before).unwrap().is_none(),
        "purge retained a version of an erased key"
    );
}

#[test]
fn purge_tenant_chunked_with_cancel_aborts_between_chunks() {
    let eng = engine();
    for i in 0..80 {
        eng.create_record(
            "Cancel",
            None,
            vec![("i".into(), FieldChange::Set(Json::from(i)))],
        )
        .unwrap();
    }
    // Cancel on the second poll: the first chunk is applied, then the purge
    // aborts with a cancellation error rather than running to completion.
    let calls = std::cell::Cell::new(0usize);
    let err = eng
        .purge_tenant_chunked_with_cancel(16, &|| {
            let n = calls.get();
            calls.set(n + 1);
            n >= 1
        })
        .unwrap_err();
    assert!(
        err.to_string().contains("operation cancelled"),
        "expected cancellation, got: {err}"
    );
    // Resumable: an uncancelled purge finishes the job.
    eng.purge_tenant().unwrap();
    let snap = eng.store.read_view().unwrap();
    assert!(try_access::label_members(&snap, eng.tenant, "Cancel")
        .unwrap()
        .is_empty());
}

#[test]
fn purge_tenant_is_noop_on_an_already_empty_tenant() {
    let eng = engine();
    eng.create_record(
        "Bulk",
        None,
        vec![("idx".into(), FieldChange::Set(Json::from(1)))],
    )
    .unwrap();
    eng.purge_tenant().unwrap();
    // Reissuing erases nothing new; the tenant stays gone.
    eng.purge_tenant().unwrap();
    let snap = eng.store.read_view().unwrap();
    assert!(try_access::label_members(&snap, eng.tenant, "Bulk")
        .unwrap()
        .is_empty());
    let has_tenant_key = snap
        .range_prefix(&key(eng.tenant, &[]))
        .unwrap()
        .into_iter()
        .map(|(k, _)| k)
        .any(|k| !k.starts_with(&rushdb_types::keys::security_prefix(eng.tenant)));
    assert!(!has_tenant_key);
}

#[test]
fn label_catalog_tracks_create_retype_delete() {
    let eng = engine();
    let a1 = eng.create_record("A", None, vec![]).unwrap();
    let a2 = eng.create_record("A", None, vec![]).unwrap();
    let b = eng.create_record("B", None, vec![]).unwrap();

    let snap = eng.store.read_view().unwrap();
    assert_eq!(
        try_access::labels(&snap, eng.tenant).unwrap(),
        vec!["A", "B"]
    );
    assert_eq!(
        try_access::label_counts(&snap, eng.tenant).unwrap(),
        vec![("A".to_string(), 2), ("B".to_string(), 1)]
    );

    eng.retype_record(b, "B", "C").unwrap();
    let snap = eng.store.read_view().unwrap();
    assert_eq!(
        try_access::labels(&snap, eng.tenant).unwrap(),
        vec!["A", "C"]
    );
    assert_eq!(
        try_access::label_counts(&snap, eng.tenant).unwrap(),
        vec![("A".to_string(), 2), ("C".to_string(), 1)]
    );

    eng.delete_record(a1).unwrap();
    let snap = eng.store.read_view().unwrap();
    assert_eq!(
        try_access::label_counts(&snap, eng.tenant).unwrap(),
        vec![("A".to_string(), 1), ("C".to_string(), 1)]
    );

    eng.delete_record(a2).unwrap();
    // `b` kept the same identity through the retype; deleting it removes C.
    eng.delete_record(b).unwrap();
    let snap = eng.store.read_view().unwrap();
    assert!(try_access::labels(&snap, eng.tenant).unwrap().is_empty());
    assert!(try_access::label_counts(&snap, eng.tenant)
        .unwrap()
        .is_empty());
}

#[test]
fn derived_counter_history_is_reclaimed_by_gc() {
    // Derived counters are MVCC rows written at their base commit's index
    //, so a pinned view answers exactly for its snapshot. Their
    // superseded versions are ordinary history: GC past the watermark
    // reclaims all of it.
    let eng = engine();
    for i in 0..50 {
        eng.create_record(
            "H",
            None,
            vec![("n".into(), FieldChange::Set(Json::from(i)))],
        )
        .unwrap();
    }
    let (current, history) = eng.store.approximate_map_bytes();
    assert!(current > 0, "records exist");
    assert!(history > 0, "50 commits superseded the label counter");
    eng.store.gc(u64::MAX).unwrap();
    let (after_gc, history) = eng.store.approximate_map_bytes();
    assert_eq!(history, 0, "GC reclaims all superseded derived versions");
    assert_eq!(after_gc, current, "current state is untouched by GC");
}

#[test]
fn relationship_catalog_tracks_type_and_endpoint_labels() {
    // Maintained relationship catalog: (type, source_label, target_label) ->
    // count, decremented exactly on delete (endpoint labels are captured at
    // create so a cascade cannot lose them).
    let eng = engine();
    let a1 = eng.create_record("A", None, vec![]).unwrap();
    let a2 = eng.create_record("A", None, vec![]).unwrap();
    let b1 = eng.create_record("B", None, vec![]).unwrap();
    let c1 = eng.create_record("C", None, vec![]).unwrap();
    let r1 = eng.create_relationship("R", a1, b1, vec![]).unwrap();
    let _r2 = eng.create_relationship("R", a2, b1, vec![]).unwrap();
    let _s1 = eng.create_relationship("S", a1, c1, vec![]).unwrap();

    let snap = eng.store.read_view().unwrap();
    assert_eq!(
        try_access::relationship_catalog(&snap, eng.tenant).unwrap(),
        vec![
            ("R".to_string(), "A".to_string(), "B".to_string(), 2),
            ("S".to_string(), "A".to_string(), "C".to_string(), 1),
        ]
    );

    eng.delete_relationship(r1).unwrap();
    let snap = eng.store.read_view().unwrap();
    assert_eq!(
        try_access::relationship_catalog(&snap, eng.tenant).unwrap(),
        vec![
            ("R".to_string(), "A".to_string(), "B".to_string(), 1),
            ("S".to_string(), "A".to_string(), "C".to_string(), 1),
        ]
    );
}

fn pid_of(eng: &Lmpg, name: &str, ty: rushdb_types::model::LogicalType) -> u64 {
    use rushdb_types::model::TypeState;
    let sig = kx::family_sig_key(
        eng.tenant,
        "default",
        OwnerKind::Record,
        &rushdb_types::value::normalize_name(name),
    );
    let raw = eng
        .store
        .read_view()
        .unwrap()
        .get(&sig)
        .unwrap()
        .expect("family signature");
    let fid = u64::from_be_bytes(raw[..8].try_into().unwrap());
    let dkey = kx::definition_key(eng.tenant, fid, TypeState::Known(ty));
    let def = eng
        .store
        .read_view()
        .unwrap()
        .get(&dkey)
        .unwrap()
        .expect("definition");
    u64::from_be_bytes(def[..8].try_into().unwrap())
}

#[test]
fn owner_support_counts_are_owner_grain_and_label_scoped() {
    use rushdb_types::model::LogicalType;
    let eng = engine();
    let a1 = eng
        .create_record(
            "A",
            None,
            vec![
                ("n".into(), FieldChange::Set(Json::from(1))),
                ("arr".into(), FieldChange::Set(json!([1, 2, 2]))),
            ],
        )
        .unwrap();
    let a2 = eng
        .create_record(
            "A",
            None,
            vec![("n".into(), FieldChange::Set(Json::from(3)))],
        )
        .unwrap();
    let b1 = eng
        .create_record(
            "B",
            None,
            vec![("n".into(), FieldChange::Set(Json::from(5)))],
        )
        .unwrap();

    let n_pid = pid_of(&eng, "n", LogicalType::Int64);
    let arr_pid = pid_of(&eng, "arr", LogicalType::Int64);
    let snap = eng.store.read_view().unwrap();
    assert_eq!(
        try_access::prop_support_total(&snap, eng.tenant, n_pid).unwrap(),
        3
    );
    assert_eq!(
        try_access::prop_support_label(&snap, eng.tenant, n_pid, "A").unwrap(),
        2
    );
    assert_eq!(
        try_access::prop_support_label(&snap, eng.tenant, n_pid, "B").unwrap(),
        1
    );
    // Array elements for one record count as one owner.
    assert_eq!(
        try_access::prop_support_total(&snap, eng.tenant, arr_pid).unwrap(),
        1
    );

    // Overwriting the value keeps the owner counted once.
    eng.update_fields(a1, vec![("n".into(), FieldChange::Set(Json::from(9)))])
        .unwrap();
    let snap = eng.store.read_view().unwrap();
    assert_eq!(
        try_access::prop_support_total(&snap, eng.tenant, n_pid).unwrap(),
        3
    );

    // Deleting a record drops its support.
    eng.delete_record(a2).unwrap();
    let snap = eng.store.read_view().unwrap();
    assert_eq!(
        try_access::prop_support_total(&snap, eng.tenant, n_pid).unwrap(),
        2
    );
    assert_eq!(
        try_access::prop_support_label(&snap, eng.tenant, n_pid, "A").unwrap(),
        1
    );

    // Retype moves label-scoped support.
    eng.retype_record(b1, "B", "C").unwrap();
    let snap = eng.store.read_view().unwrap();
    assert_eq!(
        try_access::prop_support_label(&snap, eng.tenant, n_pid, "B").unwrap(),
        0
    );
    assert_eq!(
        try_access::prop_support_label(&snap, eng.tenant, n_pid, "C").unwrap(),
        1
    );
    assert_eq!(
        try_access::prop_support_total(&snap, eng.tenant, n_pid).unwrap(),
        2
    );
}

#[test]
fn owner_support_counts_are_exact_across_owners() {
    // The derived index maintains exact owner support without the sharded
    // summary rows (sharding is obsolete with derived state).
    use rushdb_types::model::LogicalType;
    let eng = engine();
    for _ in 0..64 {
        eng.create_record(
            "Hub",
            None,
            vec![("status".into(), FieldChange::Set(json!("active")))],
        )
        .unwrap();
    }
    let status_pid = pid_of(&eng, "status", LogicalType::Utf8);
    let snap = eng.store.read_view().unwrap();
    assert_eq!(
        try_access::prop_support_total(&snap, eng.tenant, status_pid).unwrap(),
        64
    );
    assert_eq!(
        try_access::prop_support_label(&snap, eng.tenant, status_pid, "Hub").unwrap(),
        64
    );
}

#[test]
fn concurrent_hot_property_writes_keep_exact_support() {
    // Many writers hammering one hot property must not lose counter
    // updates (shard-and-sum stays exact under the serialized publisher).
    use rushdb_types::model::LogicalType;
    let store = std::sync::Arc::new(Store::new_in_memory());
    let tenant = TenantId::from_ordinal(7);
    let eng = Lmpg::new(store.clone(), tenant);
    let mut handles = Vec::new();
    for _ in 0..8 {
        let store = store.clone();
        handles.push(std::thread::spawn(move || {
            let lmpg = Lmpg::new(store, tenant);
            for _ in 0..25 {
                lmpg.create_record(
                    "Hub",
                    None,
                    vec![("status".into(), FieldChange::Set(json!("active")))],
                )
                .unwrap();
            }
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
    let status_pid = pid_of(&eng, "status", LogicalType::Utf8);
    let snap = eng.store.read_view().unwrap();
    assert_eq!(
        try_access::prop_support_total(&snap, tenant, status_pid).unwrap(),
        200
    );
    assert_eq!(
        try_access::prop_support_label(&snap, tenant, status_pid, "Hub").unwrap(),
        200
    );
}

fn pid_of_kind(
    eng: &Lmpg,
    kind: OwnerKind,
    name: &str,
    ty: rushdb_types::model::LogicalType,
) -> u64 {
    use rushdb_types::model::TypeState;
    let sig = kx::family_sig_key(
        eng.tenant,
        "default",
        kind,
        &rushdb_types::value::normalize_name(name),
    );
    let raw = eng
        .store
        .read_view()
        .unwrap()
        .get(&sig)
        .unwrap()
        .expect("family signature");
    let fid = u64::from_be_bytes(raw[..8].try_into().unwrap());
    let dkey = kx::definition_key(eng.tenant, fid, TypeState::Known(ty));
    let def = eng
        .store
        .read_view()
        .unwrap()
        .get(&dkey)
        .unwrap()
        .expect("definition");
    u64::from_be_bytes(def[..8].try_into().unwrap())
}

#[test]
fn relationship_property_counts_are_owner_grain() {
    use rushdb_types::model::LogicalType;
    let eng = engine();
    let a = eng.create_record("A", None, vec![]).unwrap();
    let b = eng.create_record("B", None, vec![]).unwrap();
    let c = eng.create_record("C", None, vec![]).unwrap();
    let r1 = eng
        .create_relationship("R", a, b, vec![("w".into(), FieldChange::Set(json!(1)))])
        .unwrap();
    let _r2 = eng
        .create_relationship(
            "R",
            c,
            b,
            vec![("w".into(), FieldChange::Set(json!([1, 2, 2])))],
        )
        .unwrap();

    let w_pid = pid_of_kind(&eng, OwnerKind::Relationship, "w", LogicalType::Int64);
    let snap = eng.store.read_view().unwrap();
    assert_eq!(
        try_access::rel_prop_counts(&snap, eng.tenant, ("R", "A", "B")).unwrap(),
        vec![(w_pid, 1)]
    );
    assert_eq!(
        try_access::rel_prop_counts(&snap, eng.tenant, ("R", "C", "B")).unwrap(),
        vec![(w_pid, 1)],
        "array elements count a relationship owner once"
    );

    eng.delete_relationship(r1).unwrap();
    let snap = eng.store.read_view().unwrap();
    assert_eq!(
        try_access::rel_prop_counts(&snap, eng.tenant, ("R", "A", "B")).unwrap(),
        vec![]
    );
    assert_eq!(
        try_access::rel_prop_counts(&snap, eng.tenant, ("R", "C", "B")).unwrap(),
        vec![(w_pid, 1)]
    );
}
