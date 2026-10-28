//! USE-1: per-tenant usage counters equal what a full scan of the tenant's
//! rows says, after every commit of a mixed workload, on both backends,
//! across a restart, and through a tenant purge.

use std::collections::BTreeSet;
use std::sync::Arc;

use rushdb_lmpg::derived::{tenant_usage, TenantUsage};
use rushdb_lmpg::import::import_document;
use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_storage::{is_usage_bytes_key, Store};
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::keys::{self as k, key, P_REL};
use serde_json::json;

/// Usage computed the slow way: scan every row of the tenant.
fn scanned(store: &Store, t: TenantId) -> TenantUsage {
    let view = store.read_view().unwrap();
    let mut usage = TenantUsage::default();
    let mut labels = BTreeSet::new();
    let mut properties = BTreeSet::new();
    let rel_prefix = key(t, &[P_REL, b""]);
    for (row, value) in view.range_prefix(&key(t, &[])).unwrap() {
        if is_usage_bytes_key(&row) {
            continue;
        }
        usage.logical_bytes += (row.len() + value.len()) as u64;
        match row.get(k::TENANT_PREFIX_LEN) {
            Some(&k::F_RECORD_HDR) => {
                usage.records += 1;
                labels.insert(k::decode_record_hdr(&value).unwrap().1.to_owned());
            }
            Some(&k::F_OCC) => {
                let (_, kind, _, pid, _) = k::parse_occ_key(&row).unwrap();
                if kind == OwnerKind::Record {
                    properties.insert(pid);
                }
            }
            _ if row.starts_with(&rel_prefix) => usage.relationships += 1,
            _ => {}
        }
    }
    usage.labels = labels.len() as u64;
    usage.properties = properties.len() as u64;
    usage
}

fn counted(store: &Store, t: TenantId) -> TenantUsage {
    let mut usage = tenant_usage(&store.read_view().unwrap(), t).unwrap();
    usage.vectors.clear();
    usage
}

fn assert_exact(store: &Store, t: TenantId, step: &str) {
    assert_eq!(counted(store, t), scanned(store, t), "after {step}");
}

/// A workload touching every counted figure in both directions, checked
/// after each commit. Two tenants share the store so cross-tenant leakage
/// would show.
fn workload(store: &Arc<Store>) {
    let a = Lmpg::new(Arc::clone(store), TenantId::from_ordinal(11));
    let b = Lmpg::new(Arc::clone(store), TenantId::from_ordinal(12));
    let check = |step: &str| {
        assert_exact(store, a.tenant, step);
        assert_exact(store, b.tenant, step);
    };
    check("nothing");
    let alice = a
        .create_record(
            "Person",
            None,
            vec![
                ("name".into(), FieldChange::Set(json!("Alice"))),
                ("age".into(), FieldChange::Set(json!(31))),
                ("tags".into(), FieldChange::Set(json!(["a", "b"]))),
            ],
        )
        .unwrap();
    check("first record");
    let bob = a
        .create_record(
            "Person",
            None,
            vec![
                ("name".into(), FieldChange::Set(json!("Bob"))),
                ("nick".into(), FieldChange::ExplicitNull),
            ],
        )
        .unwrap();
    let acme = a
        .create_record(
            "Company",
            None,
            vec![("name".into(), FieldChange::Set(json!("Acme")))],
        )
        .unwrap();
    b.create_record(
        "Other",
        None,
        vec![("x".into(), FieldChange::Set(json!(1)))],
    )
    .unwrap();
    check("records in two tenants");
    let works = a
        .create_relationship(
            "WORKS_AT",
            alice,
            acme,
            vec![("since".into(), FieldChange::Set(json!(2020)))],
        )
        .unwrap();
    a.create_relationship("KNOWS", alice, bob, vec![]).unwrap();
    a.create_relationship("KNOWS", bob, alice, vec![]).unwrap();
    check("relationships");
    import_document(
        &a,
        "Order",
        &json!({"total": 12.5, "lines": [{"sku": "x", "qty": 2}, {"sku": "y", "qty": 1}]}),
    )
    .unwrap();
    check("nested import");
    a.update_fields(
        alice,
        vec![
            ("age".into(), FieldChange::Delete),
            ("name".into(), FieldChange::Set(json!("Alice Longer Name"))),
        ],
    )
    .unwrap();
    check("update and property delete");
    a.update_fields(bob, vec![("nick".into(), FieldChange::Delete)])
        .unwrap();
    check("last carrier of a property removed");
    a.retype_record(acme, "Company", "Org").unwrap();
    check("retype empties a label");
    a.delete_relationship(works).unwrap();
    check("relationship delete");
    a.delete_record(bob).unwrap();
    check("record delete with incident edges");
}

#[test]
fn usage_counters_match_a_scan_in_memory() {
    workload(&Arc::new(Store::new_in_memory()));
}

#[test]
fn usage_counters_match_a_scan_on_disk_and_survive_restart() {
    let dir = rushdb_storage::TempPath::store("tenant-usage");
    let path = dir.path().to_path_buf();
    let before = {
        let store = Arc::new(rushdb_lmpg::open_store(&path).unwrap());
        workload(&store);
        let usage = counted(&store, TenantId::from_ordinal(11));
        assert!(usage.records > 0 && usage.logical_bytes > 0);
        usage
    };
    // Reopen replays any WAL tail beyond the last checkpoint through the
    // same apply path; the totals must come back unchanged and exact.
    let store = Arc::new(rushdb_lmpg::open_store(&path).unwrap());
    assert_eq!(counted(&store, TenantId::from_ordinal(11)), before);
    assert_exact(&store, TenantId::from_ordinal(11), "restart");
}

#[test]
fn purge_takes_usage_to_zero_and_spares_other_tenants() {
    let store = Arc::new(Store::new_in_memory());
    workload(&store);
    let a = Lmpg::new(Arc::clone(&store), TenantId::from_ordinal(11));
    let other = counted(&store, TenantId::from_ordinal(12));
    a.purge_tenant_chunked(16).unwrap();
    assert_exact(&store, a.tenant, "purge");
    let left = counted(&store, a.tenant);
    assert_eq!(
        (
            left.records,
            left.relationships,
            left.labels,
            left.properties
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(counted(&store, TenantId::from_ordinal(12)), other);
}
