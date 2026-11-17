//! A merge import (`mergeBy` / `mergeStrategy`) is one commit: all of a
//! call's records and relationships become visible together or not at all,
//! however many records it carries and wherever it stops; concurrent merges
//! over the same keys never duplicate a record; and the same merge staged
//! into a transaction matches what the transaction staged before it.

use std::sync::Arc;

use rushdb_lmpg::import::{self, BfsPlan, ImportOptions, UpsertIdMap};
use rushdb_lmpg::{Lmpg, LmpgError};
use rushdb_storage::resources::StorageResources;
use rushdb_storage::Store;
use rushdb_types::ids::{RecordId, TenantId};
use serde_json::{json, Value};

fn count_label(lmpg: &Lmpg, label: &str) -> usize {
    let snap = lmpg.store.read_view().unwrap();
    rushdb_lmpg::reader::try_access::label_members(&snap, lmpg.tenant, label)
        .unwrap()
        .len()
}

fn merge_by_id() -> ImportOptions {
    ImportOptions {
        merge_by: Some(vec!["id".to_owned()]),
        merge_strategy: Some("append".into()),
        ..Default::default()
    }
}

/// `n` authors keyed `a<i>`, each with one nested book keyed `b<i>`; the
/// last author repeats the first author's key, past the first lookup chunk.
fn authors(n: usize, tag: &str) -> Value {
    let mut rows: Vec<Value> = (0..n)
        .map(|i| json!({ "id": format!("a{i}"), "tag": tag, "book": { "id": format!("b{i}"), "tag": tag } }))
        .collect();
    rows.push(json!({ "id": "a0", "tag": tag, "late": true }));
    json!({ "Author": rows })
}

/// Every relationship of `plan` exists between the records `idmap` maps
/// its drafts to.
fn assert_linked(lmpg: &Lmpg, plan: &BfsPlan, idmap: &[UpsertIdMap]) {
    let snap = lmpg.store.read_view().unwrap();
    let persisted = |draft: [u8; 16]| {
        idmap
            .iter()
            .find(|m| m.draft_id == draft)
            .map_or(draft, |m| m.persisted_id)
    };
    let ordinal = |id: [u8; 16]| {
        rushdb_lmpg::reader::try_access::record_by_id(&snap, lmpg.tenant, &RecordId(id))
            .unwrap()
            .expect("endpoint exists")
            .ordinal
    };
    for rel in &plan.relations {
        let (source, target) = (
            ordinal(persisted(rel.source)),
            ordinal(persisted(rel.target)),
        );
        assert!(
            rushdb_lmpg::reader::try_access::edge_exists(
                &snap,
                lmpg.tenant,
                source,
                &rel.rel_type,
                target
            )
            .unwrap(),
            "relationship {} missing",
            rel.rel_type
        );
    }
}

#[test]
fn a_merge_call_is_one_commit_with_its_relationships() {
    let lmpg = Lmpg::new(
        Arc::new(Store::new_in_memory()),
        TenantId::from_ordinal(140),
    );
    let opts = merge_by_id();
    // Some of the keys exist before the call: they are updated, not created.
    let seed = import::serialize_bfs(&authors(100, "seed"), "", &opts);
    import::import_bfs(&lmpg, &seed, &opts).unwrap();
    let before = lmpg.store.committed_version();

    let plan = import::serialize_bfs(&authors(1_500, "call"), "", &opts);
    let idmap = import::import_bfs(&lmpg, &plan, &opts).unwrap();
    assert_eq!(
        lmpg.store.committed_version(),
        before + 1,
        "one merge call is one commit"
    );
    assert_eq!(count_label(&lmpg, "Author"), 1_500);
    assert_eq!(count_label(&lmpg, "book"), 1_500);
    assert_linked(&lmpg, &plan, &idmap);

    // Replaying the same call (an importer retrying it) changes nothing.
    let replay = import::serialize_bfs(&authors(1_500, "call"), "", &opts);
    import::import_bfs(&lmpg, &replay, &opts).unwrap();
    assert_eq!(count_label(&lmpg, "Author"), 1_500);
    assert_eq!(count_label(&lmpg, "book"), 1_500);
}

#[test]
fn a_merge_stopped_in_any_phase_leaves_nothing_visible() {
    let opts = merge_by_id();
    // Stop at every poll the import makes (before each lookup chunk, before
    // the relationships, before the commit), until one runs to the end.
    for stop_at in 1.. {
        let lmpg = Lmpg::new(
            Arc::new(Store::new_in_memory()),
            TenantId::from_ordinal(141),
        );
        let before = lmpg.store.committed_version();
        let plan = import::serialize_bfs(&authors(2_100, "x"), "", &opts);
        let polls = std::cell::Cell::new(0);
        let result = import::import_bfs_with_cancel(&lmpg, plan.clone(), &opts, &|| {
            polls.set(polls.get() + 1);
            polls.get() >= stop_at
        });
        match result {
            Err(LmpgError::Conflict(reason)) if reason.contains("cancelled") => {
                assert_eq!(lmpg.store.committed_version(), before, "stop {stop_at}");
                assert_eq!(count_label(&lmpg, "Author"), 0, "stop {stop_at}");
                assert_eq!(count_label(&lmpg, "book"), 0, "stop {stop_at}");
            }
            Ok(idmap) => {
                assert!(stop_at > 3, "the import polls once per phase");
                assert_eq!(count_label(&lmpg, "Author"), 2_100);
                assert_linked(&lmpg, &plan, &idmap);
                break;
            }
            Err(other) => panic!("stop {stop_at}: {other}"),
        }
    }
}

#[test]
fn a_merge_stopped_before_its_relationships_is_gone_after_a_restart() {
    let path = rushdb_storage::TempPath::store("merge-atomic-restart");
    let tenant = TenantId::from_ordinal(142);
    let opts = merge_by_id();
    {
        let lmpg = Lmpg::new(Arc::new(Store::open(&path).unwrap()), tenant);
        let plan = import::serialize_bfs(&authors(1_100, "x"), "", &opts);
        // Two record chunks are staged; the poll before the relationships stops.
        let polls = std::cell::Cell::new(0);
        let result = import::import_bfs_with_cancel(&lmpg, plan, &opts, &|| {
            polls.set(polls.get() + 1);
            polls.get() == 3
        });
        assert!(result.is_err());
    }
    let lmpg = Lmpg::new(Arc::new(Store::open(&path).unwrap()), tenant);
    assert_eq!(count_label(&lmpg, "Author"), 0);
    assert_eq!(count_label(&lmpg, "book"), 0);
}

fn store_with_commit_limit(limit: usize) -> Lmpg {
    let store = Store::new_with_resources(StorageResources {
        max_expanded_commit_bytes: limit,
        ..StorageResources::default()
    })
    .unwrap();
    Lmpg::new(Arc::new(store), TenantId::from_ordinal(143))
}

#[test]
fn a_merge_over_the_commit_limit_fails_whole_and_one_under_it_lands() {
    let opts = merge_by_id();
    let call = || import::serialize_bfs(&authors(300, "x"), "", &opts);
    let lands = |limit: usize| {
        let lmpg = store_with_commit_limit(limit);
        match import::import_bfs(&lmpg, &call(), &opts) {
            Ok(_) => {
                assert_eq!(count_label(&lmpg, "Author"), 300);
                true
            }
            Err(error) => {
                let reason = error.to_reason();
                assert!(reason.contains("resource exhausted"), "{reason}");
                assert_eq!(
                    count_label(&lmpg, "Author"),
                    0,
                    "nothing of a refused call is visible"
                );
                assert_eq!(count_label(&lmpg, "book"), 0);
                false
            }
        }
    };
    // About the smallest limit the call fits (its size varies by a few bytes
    // between runs: fresh ids and timestamps): well above it the call lands
    // whole, well below it is refused whole.
    let (mut low, mut high) = (1usize, 64 << 20);
    assert!(lands(high));
    while high - low > high / 100 {
        let mid = low + (high - low) / 2;
        if lands(mid) {
            high = mid;
        } else {
            low = mid;
        }
    }
    assert!(lands(high + high / 10));
    assert!(!lands(high - high / 10));
}

#[test]
fn overlapping_concurrent_merges_never_duplicate_a_record() {
    let lmpg = Arc::new(Lmpg::new(
        Arc::new(Store::new_in_memory()),
        TenantId::from_ordinal(144),
    ));
    let opts = merge_by_id();
    let conflicts = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for writer in 0..4 {
            let (lmpg, opts, conflicts) = (&lmpg, &opts, &conflicts);
            scope.spawn(move || {
                for round in 0..5 {
                    // Overlapping windows of the same 300 keys.
                    let start = (writer * 50 + round * 20) % 150;
                    let rows: Vec<Value> = (start..start + 150)
                        .map(|i| json!({ "id": format!("k{i}"), "by": writer, "book": { "id": format!("b{i}") } }))
                        .collect();
                    let plan = import::serialize_bfs(&json!({ "Author": rows }), "", opts);
                    match import::import_bfs(lmpg, &plan, opts) {
                        Ok(_) => {}
                        Err(LmpgError::WriteConflict(_)) => {
                            conflicts.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                        Err(other) => panic!("writer {writer}: {other}"),
                    }
                }
            });
        }
    });
    let snap = lmpg.store.read_view().unwrap();
    let members =
        rushdb_lmpg::reader::try_access::label_members(&snap, lmpg.tenant, "Author").unwrap();
    let mut keys: Vec<String> = members
        .iter()
        .map(|&ord| {
            rushdb_lmpg::reconstruct::reconstruct_record(&snap, lmpg.tenant, ord)
                .unwrap()
                .unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    let total = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(total, keys.len(), "a key was created twice");
    eprintln!(
        "{} distinct authors; calls that gave up after retries: {}",
        keys.len(),
        conflicts.load(std::sync::atomic::Ordering::Relaxed)
    );
}

#[test]
fn merges_updating_the_same_records_with_their_own_fields_do_not_conflict() {
    let lmpg = Arc::new(Lmpg::new(
        Arc::new(Store::new_in_memory()),
        TenantId::from_ordinal(146),
    ));
    let opts = merge_by_id();
    // Every key and every field exists: the merges below only update.
    let seed: Vec<Value> = (0..200)
        .map(|i| json!({ "id": format!("k{i}"), "w0": 0, "w1": 0, "w2": 0, "w3": 0 }))
        .collect();
    let plan = import::serialize_bfs(&json!({ "Author": seed }), "", &opts);
    import::import_bfs(&lmpg, &plan, &opts).unwrap();
    for round in 1..=3 {
        // Four merges staged at one snapshot, as if they ran at once, then
        // committed one after another: none read what another wrote.
        let staged: Vec<rushdb_storage::TxAccumulator> = (0..4)
            .map(|writer| {
                let field = format!("w{writer}");
                let rows: Vec<Value> = (0..200)
                    .map(|i| json!({ "id": format!("k{i}"), field.as_str(): round }))
                    .collect();
                let plan = import::serialize_bfs(&json!({ "Author": rows }), "", &opts);
                let view = lmpg.store.read_view().unwrap();
                let mut acc = rushdb_storage::TxAccumulator::new(lmpg.tenant, view.commit_index());
                acc.base_view = Some(Arc::new(view));
                let mut stage = |batch, acc: &mut rushdb_storage::TxAccumulator| {
                    lmpg.stage_batch_tx(batch, acc)
                };
                import::stage_bfs_merge(&lmpg, &plan, &opts, &|| false, &mut acc, &mut stage)
                    .unwrap();
                acc
            })
            .collect();
        for (writer, acc) in staged.iter().enumerate() {
            lmpg.store
                .tx_commit(uuid::Uuid::now_v7(), acc)
                .unwrap_or_else(|error| panic!("round {round} writer {writer}: {error}"));
        }
    }
    let snap = lmpg.store.read_view().unwrap();
    for ord in rushdb_lmpg::reader::try_access::label_members(&snap, lmpg.tenant, "Author").unwrap()
    {
        let record = rushdb_lmpg::reconstruct::reconstruct_record(&snap, lmpg.tenant, ord)
            .unwrap()
            .unwrap();
        for writer in 0..4 {
            assert_eq!(record[format!("w{writer}")], 3);
        }
    }
}

#[test]
fn a_merge_staged_twice_in_one_transaction_matches_its_own_staged_records() {
    let lmpg = Lmpg::new(
        Arc::new(Store::new_in_memory()),
        TenantId::from_ordinal(145),
    );
    let opts = merge_by_id();
    let view = lmpg.store.read_view().unwrap();
    let mut acc = rushdb_storage::TxAccumulator::new(lmpg.tenant, view.commit_index());
    acc.base_view = Some(Arc::new(view));
    let mut stage =
        |batch, acc: &mut rushdb_storage::TxAccumulator| lmpg.stage_batch_tx(batch, acc);
    for tag in ["first", "second"] {
        let plan = import::serialize_bfs(&authors(10, tag), "", &opts);
        import::stage_bfs_merge(&lmpg, &plan, &opts, &|| false, &mut acc, &mut stage).unwrap();
    }
    assert_eq!(count_label(&lmpg, "Author"), 0, "nothing before the commit");
    lmpg.store.tx_commit(uuid::Uuid::now_v7(), &acc).unwrap();
    assert_eq!(count_label(&lmpg, "Author"), 10);
    assert_eq!(count_label(&lmpg, "book"), 10);
}
