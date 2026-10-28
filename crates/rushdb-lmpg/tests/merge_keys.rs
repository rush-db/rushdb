//! MERGE-1 (b): which keys an upsert may match on. `__id` is the record
//! identity (looked up by id, never stored as a property), every other name
//! starting with `__` is reserved for the engine, `_id` is an ordinary
//! property, and a key without an index whose population is over the
//! unindexed-walk limit is refused before the walk with the index to
//! declare. With no limit (the default, QRY-3) the walked values count
//! against `query-memory-max`.

use std::sync::Arc;

use rushdb_lmpg::{import, Lmpg};
use rushdb_storage::resources::StorageResources;
use rushdb_storage::Store;
use rushdb_types::ids::{OwnerKind, TenantId};
use serde_json::{json, Value};

fn engine(walk_limit: u64) -> Lmpg {
    let store = Store::new_with_resources(StorageResources {
        unindexed_walk_max_owners: walk_limit,
        ..StorageResources::default()
    })
    .unwrap();
    Lmpg::new(Arc::new(store), TenantId::single())
}

fn upsert(lmpg: &Lmpg, data: &Value, merge_by: &[&str]) -> Result<(), String> {
    let options = import::ImportOptions {
        merge_by: Some(merge_by.iter().map(|key| (*key).to_owned()).collect()),
        merge_strategy: Some("append".into()),
        ..Default::default()
    };
    import::import_bfs(lmpg, &import::serialize_bfs(data, "", &options), &options)
        .map(|_| ())
        .map_err(|error| error.to_reason())
}

fn records(lmpg: &Lmpg, label: &str) -> Vec<Value> {
    let snap = lmpg.store.read_view().unwrap();
    rushdb_lmpg::reader::try_access::label_members(&snap, lmpg.tenant, label)
        .unwrap()
        .into_iter()
        .map(|ord| {
            rushdb_lmpg::reconstruct::reconstruct_record(&snap, lmpg.tenant, ord)
                .unwrap()
                .unwrap()
        })
        .collect()
}

fn has_property(lmpg: &Lmpg, name: &str) -> bool {
    let snap = lmpg.store.read_view().unwrap();
    let key = rushdb_lmpg::reader::keys_ext::family_sig_key(
        lmpg.tenant,
        "default",
        OwnerKind::Record,
        name,
    );
    snap.get(&key).unwrap().is_some()
}

#[test]
fn an_upsert_by_id_finds_the_record_by_id_and_never_stores_an_id_property() {
    let lmpg = engine(0);
    let id = "0192f1f0c0de7000a000000000000001";
    upsert(
        &lmpg,
        &json!({ "Hero": [{ "__id": id, "name": "Luke" }] }),
        &["__id"],
    )
    .unwrap();
    let created = records(&lmpg, "Hero");
    assert_eq!(created.len(), 1, "a miss creates the record");
    assert_eq!(created[0]["__id"], id, "with the given id");

    let dashed = "0192f1f0-c0de-7000-a000-000000000001";
    upsert(
        &lmpg,
        &json!({ "Hero": [{ "__id": dashed, "name": "Han" }] }),
        &["__id"],
    )
    .unwrap();
    let updated = records(&lmpg, "Hero");
    assert_eq!(updated.len(), 1, "a hit updates in place: {updated:?}");
    assert_eq!(updated[0]["name"], "Han");
    assert!(
        !has_property(&lmpg, "__id"),
        "`__id` is the identity, not a property"
    );

    // Every incoming key (merge: true) includes the id; a different value of
    // another key means no match, so the create collides with the id.
    let error = upsert(
        &lmpg,
        &json!({ "Hero": [{ "__id": id, "name": "Leia" }] }),
        &[],
    )
    .unwrap_err();
    assert!(
        error.contains("precondition failed") && error.contains(id),
        "{error}"
    );
    assert_eq!(records(&lmpg, "Hero").len(), 1);

    let error = upsert(
        &lmpg,
        &json!({ "Hero": [{ "__id": "not-an-id" }] }),
        &["__id"],
    )
    .unwrap_err();
    assert!(error.starts_with("invalid __id"), "{error}");
}

#[test]
fn an_underscore_id_is_an_ordinary_property() {
    let lmpg = engine(0);
    upsert(
        &lmpg,
        &json!({ "Hero": [{ "_id": "ext-1", "name": "Luke" }] }),
        &["_id"],
    )
    .unwrap();
    upsert(
        &lmpg,
        &json!({ "Hero": [{ "_id": "ext-1", "name": "Han" }] }),
        &["_id"],
    )
    .unwrap();
    let stored = records(&lmpg, "Hero");
    assert_eq!(stored.len(), 1, "`_id` matched as a key: {stored:?}");
    assert_eq!(stored[0]["_id"], "ext-1", "stored and returned as written");
    assert_eq!(stored[0]["name"], "Han");
    assert_ne!(
        stored[0]["__id"], stored[0]["_id"],
        "the identity is separate"
    );
    assert!(has_property(&lmpg, "_id"));
}

#[test]
fn reserved_names_are_refused_as_properties_and_merge_keys() {
    let lmpg = engine(0);
    for name in ["__score", "__proptypes", "__Label", "__id_"] {
        let error = upsert(
            &lmpg,
            &json!({ "Hero": [{ "name": "Luke", name: 1 }] }),
            &["name"],
        )
        .unwrap_err();
        assert!(error.contains("invalid property name"), "{name}: {error}");
        assert!(!has_property(&lmpg, &name.to_lowercase()), "{name}");
    }
    assert!(
        records(&lmpg, "Hero").is_empty(),
        "the whole commit is refused"
    );

    let error = upsert(
        &lmpg,
        &json!({ "Hero": [{ "name": "Luke" }] }),
        &["__source"],
    )
    .unwrap_err();
    assert!(error.starts_with("invalid merge key `__source`"), "{error}");
}

#[test]
fn an_unindexed_merge_key_over_the_walk_limit_is_refused_with_the_index_to_declare() {
    let lmpg = engine(3);
    let rows: Vec<Value> = (0..4)
        .map(|i| json!({ "email": format!("u{i}@x") }))
        .collect();
    upsert(&lmpg, &json!({ "User": rows }), &["email"]).unwrap();
    assert_eq!(
        records(&lmpg, "User").len(),
        4,
        "the first upsert finds no property yet"
    );

    let error = upsert(&lmpg, &json!({ "User": [{ "email": "u1@x" }] }), &["email"]).unwrap_err();
    assert!(error.starts_with("unindexed walk refused"), "{error}");
    assert!(error.contains("would check 4 records of `User`"), "{error}");
    assert!(
        error.contains("declare an ordered index on `email`"),
        "{error}"
    );

    let small = engine(4);
    let rows: Vec<Value> = (0..4)
        .map(|i| json!({ "email": format!("u{i}@x") }))
        .collect();
    upsert(&small, &json!({ "User": rows }), &["email"]).unwrap();
    upsert(
        &small,
        &json!({ "User": [{ "email": "u1@x" }] }),
        &["email"],
    )
    .unwrap();
    assert_eq!(
        records(&small, "User").len(),
        4,
        "within the limit the walk runs"
    );
}

#[test]
fn an_unindexed_merge_key_holds_its_values_within_the_memory_cap() {
    let store = Store::new_with_resources(StorageResources {
        query_memory_max_bytes: 400,
        ..StorageResources::default()
    })
    .unwrap();
    let lmpg = Lmpg::new(Arc::new(store), TenantId::single());
    let rows: Vec<Value> = (0..20)
        .map(|i| json!({ "email": format!("user{i}@example.com") }))
        .collect();
    upsert(&lmpg, &json!({ "User": rows }), &["email"]).unwrap();

    // 20 values of ~20 bytes plus their bookkeeping are over 400 bytes.
    let error = upsert(
        &lmpg,
        &json!({ "User": [{ "email": "user1@example.com" }] }),
        &["email"],
    )
    .unwrap_err();
    assert!(error.starts_with("query working set exceeded"), "{error}");
    assert!(
        error.contains("declare an ordered index on `email`"),
        "{error}"
    );

    // Without a cap the walk runs and the upsert matches.
    let lmpg = engine(0);
    let rows: Vec<Value> = (0..20)
        .map(|i| json!({ "email": format!("user{i}@example.com") }))
        .collect();
    upsert(&lmpg, &json!({ "User": rows }), &["email"]).unwrap();
    upsert(
        &lmpg,
        &json!({ "User": [{ "email": "user1@example.com" }] }),
        &["email"],
    )
    .unwrap();
    assert_eq!(records(&lmpg, "User").len(), 20);
}
