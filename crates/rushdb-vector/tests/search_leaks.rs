//! Searching holds nothing: thousands of exact, ANN and thresholded searches
//! over an on-disk store leave the shared cache budget, the resident ANN
//! bytes and the store's files exactly as the first warm search left them.
//! (The `family_soak` bench measures the same over time, as an RSS slope.)

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_storage::resources::StorageResources;
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_vector::search::{SearchMode, SearchPath};
use rushdb_vector::{SourceRef, VectorEngine};
use serde_json::json;

const SEARCHES: usize = 2_000;

/// Every file under `dir` except the store's own database and log, which
/// legitimately change as the store runs.
fn files(dir: &Path) -> BTreeSet<PathBuf> {
    let mut found = BTreeSet::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.insert(path);
            }
        }
    }
    found
}

#[test]
fn repeated_searches_hold_no_memory_and_leave_no_files() {
    let dir = rushdb_storage::TempPath::dir("vector-search-leaks");
    std::fs::create_dir_all(&dir).unwrap();
    let store = Arc::new(
        rushdb_lmpg::open_store_with_resources(
            &dir.join("rushdb.redb"),
            StorageResources {
                logical_cache_bytes: 64 * 1024 * 1024,
                ..StorageResources::default()
            },
        )
        .unwrap(),
    );
    let budget = store.cache_budget().unwrap();
    let engine = VectorEngine::new(Arc::new(Lmpg::new(
        Arc::clone(&store),
        TenantId::from_ordinal(31),
    )));
    engine.ensure_space("docs", 8).unwrap();
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        f32::from(u16::try_from(seed % 2_000).unwrap()) / 1_000.0 - 1.0
    };
    for i in 0..500 {
        let record = engine
            .lmpg
            .create_record("Doc", None, vec![("i".into(), FieldChange::Set(json!(i)))])
            .unwrap();
        let vector: Vec<f32> = (0..8).map(|_| next()).collect();
        engine
            .bind_external(
                "docs",
                SourceRef {
                    kind: OwnerKind::Record,
                    owner_ord: record.ordinal,
                    pid: 0,
                    elem: 0,
                },
                &vector,
            )
            .unwrap();
    }
    engine.build_ann("docs").unwrap();

    let search = |round: usize, query: &[f32]| {
        let snap = store.read_view().unwrap();
        let space = engine.lookup_space_in_view(&snap, "docs").unwrap().unwrap();
        let (mode, min_score) = match round % 3 {
            0 => (SearchMode::Exact, None),
            1 => (SearchMode::Ann { ef: 64 }, None),
            _ => (SearchMode::Ann { ef: 64 }, Some(0.5)),
        };
        let outcome = engine
            .search_in_view(&snap, &space, query, 10, mode, None, min_score)
            .unwrap();
        if !round.is_multiple_of(3) {
            assert_eq!(outcome.path, SearchPath::Ann, "round {round}");
        }
    };
    // Warm every path once, then take the baseline.
    let queries: Vec<Vec<f32>> = (0..16).map(|_| (0..8).map(|_| next()).collect()).collect();
    for round in 0..3 {
        search(round, &queries[0]);
    }
    let cache_used = budget.snapshot().used;
    let resident = engine.resident_bytes();
    let on_disk = files(&dir);
    assert!(resident > 0, "the ANN graph is resident");

    for round in 0..SEARCHES {
        search(round, &queries[round % queries.len()]);
    }
    assert_eq!(
        budget.snapshot().used,
        cache_used,
        "cache budget after searches"
    );
    assert_eq!(engine.resident_bytes(), resident, "resident ANN bytes");
    let after: BTreeSet<PathBuf> = files(&dir);
    let added: Vec<_> = after.difference(&on_disk).collect();
    assert!(added.is_empty(), "searches wrote files: {added:?}");
    drop(engine);
    drop(store);
    std::fs::remove_dir_all(&dir).ok();
}
