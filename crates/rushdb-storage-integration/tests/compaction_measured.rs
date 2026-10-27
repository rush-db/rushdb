//! 3A — measured physical compaction. Many hot-key updates grow retained MVCC
//! history; `Store::compact` must reclaim physical disk space measurably while
//! preserving current + post-compaction reads.

use std::path::PathBuf;
use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::json;

fn path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("rushdb-{name}-{}", uuid::Uuid::now_v7()))
}

#[test]
fn compaction_measurably_reclaims_disk_after_hot_key_updates() {
    let dir = path("compact-measured");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");

    let store = Arc::new(Store::open(&db).unwrap());
    let lmpg = Lmpg::new(store.clone(), TenantId::from_ordinal(21));
    let record = lmpg.create_record("Doc", None, vec![]).unwrap();
    // A hot key: many superseded versions of incompressible ~2 KB values,
    // so retained history (not engine metadata files) dominates the size.
    let value_at = |value: u64| -> String {
        let mut seed = value.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        (0..256)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                format!("{:02x}", seed.to_le_bytes()[0]) + &format!("{:06x}", seed >> 40)
            })
            .collect()
    };
    for value in 0..300 {
        lmpg.update_fields(
            record,
            vec![("value".into(), FieldChange::Set(json!(value_at(value))))],
        )
        .unwrap();
    }
    drop(lmpg);
    drop(store);
    let size_before = std::fs::metadata(&db).unwrap().len();

    // Reopen + compact (no read pins active) + drop to flush.
    let store = Arc::new(Store::open(&db).unwrap());
    store.compact().unwrap();
    drop(store);
    let size_after = std::fs::metadata(&db).unwrap().len();

    assert!(
        size_after < size_before,
        "compaction must reclaim physical space: before={size_before}B after={size_after}B"
    );

    // Reopen: current state is intact (the last value).
    let store = Arc::new(Store::open(&db).unwrap());
    let lmpg = Lmpg::new(store.clone(), TenantId::from_ordinal(21));
    let value = rushdb_lmpg::reconstruct::reconstruct_record(
        &lmpg.store.read_view().unwrap(),
        TenantId::from_ordinal(21),
        record.ordinal,
    )
    .unwrap()
    .unwrap();
    assert_eq!(value.get("value"), Some(&json!(value_at(299))));
    drop(lmpg);
    drop(store);
    let _ = std::fs::remove_dir_all(&dir);
}
