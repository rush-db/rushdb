//! JSONL import format tests: line-delimited ingestion must produce the
//! expected records. (CSV is decoded by the protocol's `ingest.csv`.)

use std::sync::Arc;

use rushdb_lmpg::import::import_jsonl;
use rushdb_lmpg::reconstruct::reconstruct_record;
use rushdb_lmpg::Lmpg;
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;

fn setup() -> Lmpg {
    let store = Arc::new(Store::new_in_memory());
    Lmpg::new(store, TenantId::from_ordinal(71))
}

#[test]
fn jsonl_stream_imports_each_line() {
    let lmpg = setup();
    let bytes = b"{\"a\":1}\n\n{\"b\":\"x\",\"c\":null}\n";
    let refs = import_jsonl(&lmpg, "Row", &bytes[..]).unwrap();
    assert_eq!(refs.len(), 2);

    let snap = lmpg.store.read_view().unwrap();
    let r0 = reconstruct_record(&snap, lmpg.tenant, 0).unwrap().unwrap();
    assert_eq!(r0.get("a"), Some(&json_from_i64(1)));
    let r1 = reconstruct_record(&snap, lmpg.tenant, 1).unwrap().unwrap();
    assert_eq!(r1.get("b"), Some(&serde_json::json!("x")));
    assert_eq!(r1.get("c"), Some(&serde_json::Value::Null));
}

fn json_from_i64(v: i64) -> serde_json::Value {
    serde_json::json!(v)
}

#[test]
fn malformed_json_line_is_a_stable_error() {
    let lmpg = setup();
    let err = import_jsonl(&lmpg, "X", &b"{\"ok\"\n"[..]);
    match err {
        Err(rushdb_lmpg::LmpgError::Conflict(msg)) => assert!(msg.contains("bad json")),
        other => panic!("expected conflict error, got {other:?}"),
    }
}
