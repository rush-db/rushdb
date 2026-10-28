//! Regression: a JSON `null` scalar is a nullable occurrence, not a
//! mixed-type array. Previously `FieldChange::Set(json!(null))` was rejected
//! with `PlanError::MixedArray` (and, once a typed sibling existed, with a
//! `TypeMismatch { found: "?" }`), which made any record carrying an explicit
//! null property impossible to write and failed the whole SQL label.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::reader::try_access;
use rushdb_lmpg::Lmpg;
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::json;

#[test]
fn null_scalar_after_a_typed_value_is_accepted() {
    let store = Arc::new(Store::new_in_memory());
    let graph = Lmpg::new(store.clone(), TenantId::single());

    graph
        .create_record("P", None, vec![("n".into(), FieldChange::Set(json!("Al")))])
        .unwrap();
    let null_record = graph
        .create_record("P", None, vec![("n".into(), FieldChange::Set(json!(null)))])
        .unwrap();

    let view = store.read_view().unwrap();
    assert_eq!(
        try_access::label_members(&view, TenantId::single(), "P")
            .unwrap()
            .len(),
        2
    );
    let rebuilt = rushdb_lmpg::reconstruct::reconstruct_record(
        &view,
        TenantId::single(),
        null_record.ordinal,
    )
    .unwrap()
    .unwrap();
    assert!(
        rebuilt.get("n").is_none_or(serde_json::Value::is_null),
        "explicit null must reconstruct as null/absent, got {rebuilt:?}"
    );
}

#[test]
fn null_scalar_with_no_prior_type_then_a_string_is_accepted() {
    let store = Arc::new(Store::new_in_memory());
    let graph = Lmpg::new(store.clone(), TenantId::single());

    // Null first (no known definition yet), then a string.
    graph
        .create_record("Q", None, vec![("v".into(), FieldChange::Set(json!(null)))])
        .unwrap();
    graph
        .create_record("Q", None, vec![("v".into(), FieldChange::Set(json!("x")))])
        .unwrap();
    graph
        .create_record("Q", None, vec![("v".into(), FieldChange::Set(json!(null)))])
        .unwrap();

    let view = store.read_view().unwrap();
    assert_eq!(
        try_access::label_members(&view, TenantId::single(), "Q")
            .unwrap()
            .len(),
        3
    );
}
