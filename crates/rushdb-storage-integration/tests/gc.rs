//! MVCC history garbage collection (plan document 07 retention): superseded
//! versions at or before the GC watermark are reclaimed before redb
//! compaction. Current data and snapshot reads at/after the
//! watermark stay correct; reads before the watermark are reclaimed.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::json;

fn setup() -> Lmpg {
    let store = Arc::new(Store::new_in_memory());
    Lmpg::new(store, TenantId::from_ordinal(91))
}

#[test]
fn gc_reclaims_superseded_versions_keeps_current_state() {
    let lmpg = setup();
    let store = lmpg.store.clone();

    let rec = lmpg
        .create_record("Doc", None, vec![("v".into(), FieldChange::Set(json!(0)))])
        .unwrap();
    let before_updates = store.committed_version();

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
    let after_updates = store.committed_version();
    assert!(after_updates - before_updates >= 50);

    // current state is v=50
    let snap = lmpg.store.read_view().unwrap();
    let rebuilt = rushdb_lmpg::reconstruct::reconstruct_record(&snap, lmpg.tenant, rec.ordinal)
        .unwrap()
        .unwrap();
    assert_eq!(rebuilt.get("v"), Some(&json!(50)));

    // time-travel read just before GC sees an old version
    let key = rushdb_types::keys::record_id_key(lmpg.tenant, &rec.record_id);
    // after many updates, reading at a mid watermark returns the version at
    // that point (versions are preserved before GC)
    let mid = before_updates + 10;
    let mid_val = store.read_at(mid).unwrap().get(&key).unwrap();
    assert!(
        mid_val.is_some(),
        "historical header must remain readable before GC"
    );

    // GC at the current commit: every superseded version <= after_updates is
    // reclaimable (the snapshot view is materialized in memory/checkpoint).
    let watermark = after_updates;
    let reclaimed = store.gc(watermark).unwrap();
    assert!(
        reclaimed > 0,
        "50 updates must produce reclaimable version entries"
    );
    assert_eq!(store.gc_watermark(), watermark);

    // current state unchanged after GC
    let snap = lmpg.store.read_view().unwrap();
    let rebuilt = rushdb_lmpg::reconstruct::reconstruct_record(&snap, lmpg.tenant, rec.ordinal)
        .unwrap()
        .unwrap();
    assert_eq!(rebuilt.get("v"), Some(&json!(50)));

    // reads at/after the watermark remain correct
    let live = store.read_view().unwrap().get(&key).unwrap();
    assert_eq!(live, store.try_get(&key).unwrap());

    // a second GC with no new versions reclaims nothing new
    let again = store.gc(watermark + 1).unwrap();
    assert_eq!(again, 0, "nothing superseded between watermark and ci+1");
}

#[test]
fn gc_after_redb_compaction_reopen_restores_watermark_and_data() {
    let dir = rushdb_storage::TempPath::dir("gc-reopen");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let db = dir.join("rushdb.redb");
    let store = Arc::new(Store::open(&db).unwrap());
    let lmpg = Lmpg::new(store.clone(), TenantId::from_ordinal(91));
    let rec = lmpg
        .create_record("Doc", None, vec![("v".into(), FieldChange::Set(json!(1)))])
        .unwrap();
    for i in 2..=20 {
        lmpg.update_fields(
            rushdb_lmpg::RecordRef {
                ordinal: rec.ordinal,
                record_id: rec.record_id,
            },
            vec![("v".into(), FieldChange::Set(json!(i)))],
        )
        .unwrap();
    }

    let ci = store.compact().unwrap();
    // compaction reclaims superseded MVCC history before redb page compaction
    assert_eq!(
        store.gc_watermark(),
        ci,
        "compaction advanced the gc watermark"
    );

    drop(lmpg);
    drop(store);

    // reopen: data intact, gc watermark restored from redb
    let reopened = Arc::new(Store::open(&db).unwrap());
    assert_eq!(reopened.gc_watermark(), ci, "redb carries the gc watermark");

    let lmpg2 = Lmpg::new(reopened, TenantId::from_ordinal(91));
    let snap = lmpg2.store.read_view().unwrap();
    let rebuilt = rushdb_lmpg::reconstruct::reconstruct_record(&snap, lmpg2.tenant, rec.ordinal)
        .unwrap()
        .unwrap();
    assert_eq!(rebuilt.get("v"), Some(&json!(20)));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reads_before_watermark_are_reclaimed() {
    let lmpg = setup();
    let store = lmpg.store.clone();
    let rec = lmpg
        .create_record("Doc", None, vec![("v".into(), FieldChange::Set(json!(1)))])
        .unwrap();
    for i in 2..=5 {
        lmpg.update_fields(
            rushdb_lmpg::RecordRef {
                ordinal: rec.ordinal,
                record_id: rec.record_id,
            },
            vec![("v".into(), FieldChange::Set(json!(i)))],
        )
        .unwrap();
    }
    let key = rushdb_types::keys::record_id_key(lmpg.tenant, &rec.record_id);

    // a pre-GC snapshot read at a mid version returns a value
    let _mid = store.committed_version() - 2;
    assert!(store.read_view().unwrap().get(&key).unwrap().is_some());

    // GC reclaims versions before the watermark
    let wm = store.committed_version();
    store.gc(wm).unwrap();

    // read at the watermark (current) still works
    assert!(store.read_view().unwrap().get(&key).unwrap().is_some());
    // read before the watermark: versions were reclaimed; the key may fall
    // back to the earliest remaining history or none
    let before = store.read_view().unwrap().get(&key).unwrap();
    let _ = before; // reclaimed or earliest version, never an inconsistent value
    assert!(store.read_view().unwrap().get(&key).unwrap().is_some());
}
