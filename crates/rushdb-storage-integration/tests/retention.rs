//! 3A — protected read-view retention and explicit expiry. A pinned historical
//! read must hold the history floor through GC; dropping the handle releases
//! the pin; expired (leaked) pins are reclaimed; reads at reclaimed CIs fail.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::json;

fn setup() -> (Lmpg, Arc<Store>) {
    let store = Arc::new(Store::new_in_memory());
    let lmpg = Lmpg::new(store.clone(), TenantId::from_ordinal(7));
    (lmpg, store)
}

fn current_value(lmpg: &Lmpg, view: &rushdb_storage::ReadView, ordinal: u64) -> i64 {
    let rebuilt = rushdb_lmpg::reconstruct::reconstruct_record(view, lmpg.tenant, ordinal)
        .unwrap()
        .expect("record exists");
    rebuilt
        .get("v")
        .and_then(serde_json::Value::as_i64)
        .unwrap()
}

#[test]
fn read_pin_protects_history_through_gc_then_releases() {
    let (lmpg, store) = setup();
    let rec = lmpg
        .create_record("Doc", None, vec![("v".into(), FieldChange::Set(json!(0)))])
        .unwrap();
    let pinned_ci = store.committed_version();

    // Pin a historical read at this commit.
    let pinned_view = store.read_at(pinned_ci).unwrap();
    assert_eq!(pinned_view.commit_index(), pinned_ci);
    assert!(store.retention_summary().1 >= 1, "pin must be recorded");

    // Advance the record far past the pinned CI, then GC up to the head.
    for i in 1..=50 {
        lmpg.update_fields(
            rushdb_lmpg::RecordRef {
                ordinal: rec.ordinal,
                record_id: rec.record_id,
            },
            vec![("v".into(), FieldChange::Set(json!(i)))],
        )
        .unwrap();
    }
    let head = store.committed_version();
    assert!(head > pinned_ci);
    store.retire_before(head).unwrap();

    // GC could not retire below the oldest active pin.
    let (oldest, _) = store.retention_summary();
    assert_eq!(oldest, Some(pinned_ci), "pin must hold the history floor");

    // The pinned historical read still sees the OLD value (v=0).
    assert_eq!(current_value(&lmpg, &pinned_view, rec.ordinal), 0);

    // Release the handle: the pin is gone, so a second GC reclaims the history
    // below the head and the old CI is no longer readable.
    drop(pinned_view);
    assert_eq!(store.retention_summary().1, 0, "pin released on drop");
    store.retire_before(head).unwrap();
    let err = store.read_at(pinned_ci);
    assert!(err.is_err(), "read at a reclaimed CI must fail");
}

#[test]
fn expire_read_pins_reclaims_leaked_readers() {
    let (lmpg, store) = setup();
    let rec = lmpg
        .create_record("Doc", None, vec![("v".into(), FieldChange::Set(json!(1)))])
        .unwrap();
    let _ = rec;
    let ci = store.committed_version();

    // A reader handle that is never dropped (leaked) still registers a pin.
    let _leaked = store.read_at(ci).unwrap();
    assert!(store.retention_summary().1 >= 1);

    // Explicit expiry reclaims it (ensure the lease is at least 1ms old).
    std::thread::sleep(std::time::Duration::from_millis(10));
    let released = store.expire_read_pins(0);
    assert_eq!(released, 1, "one expired pin reclaimed");
    assert_eq!(store.retention_summary().1, 0);
}

#[test]
fn retention_sweep_prunes_below_floor_then_stops_without_rescanning() {
    // Regression for the write-stall wedge: the sweep must stop at the
    // LSN-ordered floor instead of re-scanning the whole retained log and
    // payload window every tick (which held publish_lock and starved writes).
    let dir = rushdb_storage::TempPath::dir("retention");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let store = Arc::new(Store::open(&db).unwrap());
    let lmpg = Lmpg::new(store.clone(), TenantId::from_ordinal(7));
    for i in 0..300 {
        lmpg.create_record("Doc", None, vec![("v".into(), FieldChange::Set(json!(i)))])
            .unwrap();
    }
    assert_eq!(store.committed_version(), 300);

    // Prune commit metadata below CI 100 in bounded batches.
    let mut pruned = 0usize;
    loop {
        let n = store.retire_commit_metadata_before(100, 32).unwrap();
        if n == 0 {
            break;
        }
        assert!(
            n <= 64,
            "each call must stay within the batch bound, got {n}"
        );
        pruned += n;
    }
    // Exactly the outcome payloads of commits 1..=99 (the journal is the WAL;
    // conflict summaries are in memory since C4).
    assert_eq!(
        pruned, 99,
        "expected to prune the payload rows below the floor"
    );

    // Steady state: nothing below the floor remains, so the sweep is a cheap
    // no-op (ordered early-break), not a full-window re-scan.
    let start = std::time::Instant::now();
    assert_eq!(store.retire_commit_metadata_before(100, 32).unwrap(), 0);
    assert!(
        start.elapsed() < std::time::Duration::from_millis(200),
        "steady-state retention must not re-scan the retained window"
    );
    std::fs::remove_dir_all(&dir).ok();
}
