//! Merge/upsert import regression coverage: `merge: true` (shorthand for
//! `mergeBy: []` + append) and explicit `mergeBy: []` must not duplicate
//! records on re-import — even with arrays, empty arrays, nulls, datetimes.

use std::sync::Arc;
use std::time::{Duration, Instant};

use rushdb_lmpg::{import, Lmpg};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::json;

fn count_label(lmpg: &Lmpg, label: &str) -> u64 {
    let snap = lmpg.store.read_view().unwrap();
    rushdb_lmpg::reader::try_access::label_members(&snap, lmpg.tenant, label)
        .unwrap()
        .len() as u64
}

#[test]
fn merge_by_all_keys_does_not_duplicate() {
    let store = Arc::new(Store::new_in_memory());
    let lmpg = Lmpg::new(store, TenantId::from_ordinal(92));

    let dataset = json!({
        "Hero": [
            {
                "id": "h1",
                "name": "Luke",
                "aliases": [],
                "tags": ["jedi", "skywalker"],
                "nickname": null,
                "score": 10,
                "active": true,
                "joined_at": 1_700_000_000_000_i64
            },
            {
                "id": "h2",
                "name": "Leia",
                "aliases": ["princess"],
                "tags": [],
                "nickname": "organa",
                "score": 20,
                "active": false,
                "joined_at": 1_600_000_000_000_i64
            }
        ]
    });

    let plain = import::ImportOptions {
        merge_by: None,
        merge_strategy: None,
        ..Default::default()
    };
    let bfs_plan = import::serialize_bfs(&dataset, "", &plain);
    import::import_bfs(&lmpg, &bfs_plan, &plain).unwrap();
    let before = count_label(&lmpg, "Hero");

    let merge = import::ImportOptions {
        merge_by: Some(vec![]),
        merge_strategy: Some("append".into()),
        ..Default::default()
    };
    let merged_plan = import::serialize_bfs(&dataset, "", &merge);
    import::import_bfs(&lmpg, &merged_plan, &merge).unwrap();
    assert_eq!(
        count_label(&lmpg, "Hero"),
        before,
        "merge by all keys must not duplicate records"
    );
}

#[test]
fn merge_true_key_is_recognized() {
    // `options: { merge: true }` must request upsert-by-all-keys instead of
    // being silently ignored (which created duplicates on re-import).
    let store = Arc::new(Store::new_in_memory());
    let lmpg = Lmpg::new(store, TenantId::from_ordinal(93));
    let dataset = json!({
        "Hero": [{ "id": "h1", "name": "Luke", "score": 10 }]
    });
    let plain = import::ImportOptions {
        merge_by: None,
        merge_strategy: None,
        ..Default::default()
    };
    let bfs_plan = import::serialize_bfs(&dataset, "", &plain);
    import::import_bfs(&lmpg, &bfs_plan, &plain).unwrap();
    let before = count_label(&lmpg, "Hero");

    let opts = import::ImportOptions::from_json(&json!({ "merge": true }));
    assert!(opts.upsert_requested(), "merge:true must request upsert");
    assert_eq!(opts.merge_by, Some(vec![]));
    assert_eq!(opts.merge_strategy, Some("append".into()));
    let merged_plan = import::serialize_bfs(&dataset, "", &opts);
    import::import_bfs(&lmpg, &merged_plan, &opts).unwrap();
    assert_eq!(
        count_label(&lmpg, "Hero"),
        before,
        "merge:true must not duplicate"
    );
}

#[test]
fn merge_lookup_is_index_backed_not_a_label_scan() {
    // Regression guard: matching every merge key by reconstructing every record
    // in the label made upserts O(rows * label_size). A 192-row merge took ~1
    // minute on a production store because the label held thousands of records.
    // The lookup must instead use the equality postings. We self-calibrate the
    // budget against the batched plain insert (one commit, O(n)) so the bound
    // tracks machine speed instead of an absolute constant.
    let store = Arc::new(Store::new_in_memory());
    let lmpg = Lmpg::new(store, TenantId::from_ordinal(94));

    let n = 2_000usize;
    let rows: Vec<serde_json::Value> = (0..n)
        .map(|i| json!({ "uid": format!("u{i}"), "rank": i }))
        .collect();
    let dataset = json!({ "Widget": rows });

    let plain = import::ImportOptions::default();
    let started = Instant::now();
    import::import_bfs(&lmpg, &import::serialize_bfs(&dataset, "", &plain), &plain).unwrap();
    let baseline = started.elapsed();

    let merge = import::ImportOptions {
        merge_by: Some(vec![]),
        merge_strategy: Some("append".into()),
        ..Default::default()
    };
    let started = Instant::now();
    import::import_bfs(&lmpg, &import::serialize_bfs(&dataset, "", &merge), &merge).unwrap();
    let elapsed = started.elapsed();

    assert_eq!(count_label(&lmpg, "Widget"), n as u64, "no duplicates");
    let budget = baseline * 10 + Duration::from_secs(5);
    assert!(
        elapsed < budget,
        "merge took {elapsed:?} (budget {budget:?}); matching likely regressed to a label scan"
    );
}

#[test]
fn merge_duplicate_keys_within_one_import_upsert_once() {
    // Two rows with the same merge key in one payload must resolve to a single
    // record: the first creates it, the second upserts the row the first
    // created. The batched path cannot see its own uncommitted writes, so this
    // chunk must fall back to per-record commits.
    let store = Arc::new(Store::new_in_memory());
    let lmpg = Lmpg::new(store, TenantId::from_ordinal(95));
    let dataset = json!({
        "Hero": [
            { "id": "h1", "name": "Luke", "score": 1 },
            { "id": "h1", "name": "Luke", "score": 2 },
            { "id": "h2", "name": "Leia", "score": 3 }
        ]
    });
    let opts = import::ImportOptions {
        merge_by: Some(vec!["id".to_string()]),
        merge_strategy: Some("append".into()),
        ..Default::default()
    };
    let plan = import::serialize_bfs(&dataset, "", &opts);
    import::import_bfs(&lmpg, &plan, &opts).unwrap();
    assert_eq!(
        count_label(&lmpg, "Hero"),
        2,
        "duplicate merge key must upsert, not create a second record"
    );
}

#[test]
fn merge_reimport_is_idempotent_across_type_coercions() {
    // Regression: the planner widens an Int64 write to an existing Float64
    // definition (storing the value's Int64 encoding under the Float64
    // definition id) and coerces date/datetime strings to DateTime. The merge
    // matcher must treat the incoming JSON shape and the stored canonical form
    // as the same key, or re-importing duplicates every such row.
    let store = Arc::new(Store::new_in_memory());
    let lmpg = Lmpg::new(store, TenantId::from_ordinal(97));
    let dataset = json!({
        "Item": [
            { "id": "a", "ratio": 12.5, "at": "1977-05-25T00:00:00Z", "tags": ["x", "y"], "note": null },
            { "id": "b", "ratio": 43, "at": "1999-05-19T00:00:00Z", "tags": [], "note": "n" },
            { "id": "c", "ratio": 7, "at": "1980-05-21T00:00:00Z", "tags": ["z"], "note": null }
        ]
    });
    let opts = import::ImportOptions {
        merge_by: Some(vec![]),
        merge_strategy: Some("append".into()),
        ..Default::default()
    };
    import::import_bfs(&lmpg, &import::serialize_bfs(&dataset, "", &opts), &opts).unwrap();
    let before = count_label(&lmpg, "Item");
    import::import_bfs(&lmpg, &import::serialize_bfs(&dataset, "", &opts), &opts).unwrap();
    assert_eq!(before, 3);
    assert_eq!(
        count_label(&lmpg, "Item"),
        before,
        "re-import must not duplicate widened/coerced keys"
    );
}

#[test]
fn merge_duplicate_key_spanning_chunks_upserts_once() {
    // A duplicate of the first key positioned past the batch boundary must
    // still find the earlier chunk's committed record.
    let store = Arc::new(Store::new_in_memory());
    let lmpg = Lmpg::new(store, TenantId::from_ordinal(96));
    let n = 1025usize;
    let mut rows: Vec<serde_json::Value> =
        (0..n).map(|i| json!({ "id": format!("k{i}") })).collect();
    rows.push(json!({ "id": "k0", "extra": "x" }));
    let dataset = json!({ "Thing": rows });
    let opts = import::ImportOptions {
        merge_by: Some(vec!["id".to_string()]),
        merge_strategy: Some("append".into()),
        ..Default::default()
    };
    let plan = import::serialize_bfs(&dataset, "", &opts);
    import::import_bfs(&lmpg, &plan, &opts).unwrap();
    assert_eq!(
        count_label(&lmpg, "Thing"),
        n as u64,
        "cross-chunk duplicate must upsert, not duplicate"
    );
}

/// Presence and occurrence reads the calling thread made since the last call.
fn presence_and_occurrence_reads() -> u64 {
    rushdb_storage::read_accounting::take_local()
        .iter()
        .filter(|((_, family), _)| family == "presence" || family == "occurrence")
        .map(|(_, count)| count)
        .sum()
}

#[test]
fn merge_by_an_indexed_key_reads_the_index_not_the_label() {
    // MERGE-1: without a declared index a merge lookup walks every record of
    // the label carrying the key (once per import call). With a Ready ordered
    // index on the key it reads the matching index entries only, and finds
    // the same records, including widened and coerced values. Read
    // accounting counts the disk backend's reads, so this store is on disk.
    let dir = rushdb_storage::TempPath::store("merge-indexed");
    let store = Arc::new(rushdb_lmpg::open_store(dir.path()).unwrap());
    let lmpg = Lmpg::new(store, TenantId::from_ordinal(98));
    let n = 3_000usize;
    let rows: Vec<serde_json::Value> = (0..n)
        .map(|i| json!({ "uid": i, "at": "1977-05-25T00:00:00Z" }))
        .collect();
    let plain = import::ImportOptions::default();
    import::import_bfs(
        &lmpg,
        &import::serialize_bfs(&json!({ "Widget": rows }), "", &plain),
        &plain,
    )
    .unwrap();
    let upsert = |key: &str, row: serde_json::Value| {
        let opts = import::ImportOptions {
            merge_by: Some(vec![key.to_string()]),
            merge_strategy: Some("append".into()),
            ..Default::default()
        };
        let upsert_plan = import::serialize_bfs(&json!({ "Widget": [row] }), "", &opts);
        rushdb_storage::read_accounting::enable(true);
        let _ = rushdb_storage::read_accounting::take_local();
        import::import_bfs(&lmpg, &upsert_plan, &opts).unwrap();
        presence_and_occurrence_reads()
    };
    let unindexed = upsert("uid", json!({ "uid": 7, "seen": 1 }));
    assert!(
        unindexed >= n as u64,
        "the unindexed lookup walks the label: {unindexed} reads"
    );
    // A number written as 7.0 matches the stored 7 without an index too.
    upsert("uid", json!({ "uid": 7.0, "seen": 5 }));
    assert_eq!(count_label(&lmpg, "Widget"), n as u64, "7.0 matched 7");

    for (name, property) in [("widget_uid", "uid"), ("widget_at", "at")] {
        let def = lmpg
            .define_index(
                name,
                "Widget",
                property,
                rushdb_types::index::IndexKind::Ordered,
            )
            .unwrap();
        while !matches!(
            lmpg.index_step(def.id, 1_000).unwrap(),
            rushdb_lmpg::index::IndexStep::Finished | rushdb_lmpg::index::IndexStep::Idle
        ) {}
    }
    let indexed = upsert("uid", json!({ "uid": 8, "seen": 2 }));
    assert!(
        indexed < 100,
        "the indexed lookup must not walk the label: {indexed} reads"
    );
    // A date string is stored as a DateTime, so the indexed lookup must probe
    // that encoding too; every row shares the date, so key on both.
    let both = import::ImportOptions {
        merge_by: Some(vec!["uid".into(), "at".into()]),
        merge_strategy: Some("append".into()),
        ..Default::default()
    };
    let row = json!({ "Widget": [{ "uid": 9, "at": "1977-05-25T00:00:00Z", "seen": 3 }] });
    import::import_bfs(&lmpg, &import::serialize_bfs(&row, "", &both), &both).unwrap();
    assert_eq!(
        count_label(&lmpg, "Widget"),
        n as u64,
        "uid + date string matched"
    );
    // A number written as 9.0 matches the stored 9.
    upsert("uid", json!({ "uid": 9.0, "seen": 4 }));
    assert_eq!(
        count_label(&lmpg, "Widget"),
        n as u64,
        "every upsert matched"
    );
}
