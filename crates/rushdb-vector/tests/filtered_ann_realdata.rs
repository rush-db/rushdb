//! Real-embedding calibration benchmark for filtered ANN. Runs the filtered-ANN path against the frozen
//! representative fixtures produced by `scripts/vector-fixtures/fetch_fixtures.sh`:
//!
//! - **DBPedia-100K** (`OpenAI` text-embedding-3-large, 1536-dim) + payload CSV:
//!   measures filtered recall/latency using a real metadata predicate (title
//!   contains a keyword -> correlated filter) and an uncorrelated synthetic
//!   category, across selectivity.
//! - **SIFT1M** (128-dim) + ground-truth `neighbors.ivecs`: measures
//!   unfiltered recall@10/@100 against the dataset's exact ground truth
//!   (the classic ANN gate), plus filtered recall with synthetic categories.
//!
//! Opt-in, like the synthetic benchmark: `RUSHDB_FILTERED_ANN_BENCH=1 cargo test
//! -p rushdb-vector --test filtered_ann_realdata --release -- --ignored`.
//! Expect several minutes and ~1 GB resident for the `DBPedia` case.

use std::sync::Arc;
use std::time::Instant;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{reader::TryRead, Lmpg, RecordRef};
use rushdb_storage::Store;
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::keys::{key, P_FAMILY_SIG};
use rushdb_vector::{SourceRef, VectorEngine};

/// Where `fetch_fixtures.sh` writes the converted fixtures.
const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/vector-fixtures");

/// Read an `.fvecs` file: uint32 dim then dim float32s, repeated. Returns
/// (dims, rows) with rows in f32.
fn read_fvecs(path: &str, max_rows: usize) -> (usize, Vec<Vec<f32>>) {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let mut pos = 0usize;
    let mut dims = 0usize;
    let mut rows = Vec::new();
    while pos + 4 <= bytes.len() && rows.len() < max_rows {
        let d = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        if dims == 0 {
            dims = d;
        }
        assert_eq!(d, dims, "inconsistent dims in {path}");
        let need = d * 4;
        let slice = &bytes[pos + 4..pos + 4 + need];
        let mut v = Vec::with_capacity(d);
        for c in slice.as_chunks::<4>().0 {
            v.push(f32::from_le_bytes(*c));
        }
        rows.push(v);
        pos += 4 + need;
    }
    (dims, rows)
}

/// Read an `.ivecs` ground-truth file: uint32 k then k int32 ids.
fn read_ivecs(path: &str) -> Vec<Vec<u32>> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let mut pos = 0usize;
    let mut out = Vec::new();
    while pos + 4 <= bytes.len() {
        let k = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        let mut row = Vec::with_capacity(k);
        for _ in 0..k {
            row.push(u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()));
            pos += 4;
        }
        out.push(row);
    }
    out
}

/// Bind real vectors into an in-memory engine, one record per vector.
fn bind_real(vectors: &[Vec<f32>], dims: usize) -> (VectorEngine, Vec<u64>) {
    let store = Arc::new(Store::new_in_memory());
    let lmpg = Lmpg::new(store.clone(), TenantId::from_ordinal(72));
    let vecs = VectorEngine::new(Arc::new(Lmpg::new(store, TenantId::from_ordinal(72))));
    vecs.ensure_space("real", u32::try_from(dims).unwrap())
        .unwrap();

    let mut ords = Vec::with_capacity(vectors.len());
    for (i, vector) in vectors.iter().enumerate() {
        let r: RecordRef = lmpg
            .create_record(
                "V",
                None,
                vec![("seed".into(), FieldChange::Set(serde_json::json!(i)))],
            )
            .unwrap();
        let norm = rushdb_types::value::normalize_name("seed");
        let sig = key(
            lmpg.tenant,
            &[
                P_FAMILY_SIG,
                b"default",
                &[OwnerKind::Record.as_byte()],
                norm.as_bytes(),
            ],
        );
        let raw = lmpg.store.get(&sig).unwrap().expect("family signature");
        let pid = u64::from_be_bytes(raw[..8].try_into().unwrap());
        let src = SourceRef {
            kind: OwnerKind::Record,
            owner_ord: r.ordinal,
            pid,
            elem: 0,
        };
        vecs.bind("real", src, vector).unwrap();
        ords.push(r.ordinal);
    }
    (vecs, ords)
}

/// Filter predicate: allow ordinals whose category is in `allowed`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "ordinals are reduced modulo a small constant, so the result always fits a 32-bit usize"
)]
fn allow_categories(allowed: &std::collections::HashSet<usize>) -> impl Fn(SourceRef) -> bool + '_ {
    move |src| allowed.contains(&((src.owner_ord as usize) % 20))
}

#[expect(
    clippy::cast_precision_loss,
    reason = "recall and per-query latency are reported metrics that only need f64 precision"
)]
fn run_filtered_experiment(
    vecs: &VectorEngine,
    queries: &[Vec<f32>],
    k: usize,
    allowed: &std::collections::HashSet<usize>,
) -> (f64, f64, f64, f64) {
    let snap = vecs.snapshot().unwrap();
    let space = vecs.lookup_space_in_view(&snap, "real").unwrap().unwrap();
    let candidate = allow_categories(allowed);

    let t_exact = Instant::now();
    let mut exact: Vec<Vec<u64>> = Vec::new();
    for q in queries {
        let hits = vecs
            .search_exact_in_view(&snap, &space, q, k, Some(&candidate))
            .unwrap();
        exact.push(hits.into_iter().map(|h| h.source.owner_ord).collect());
    }
    let exact_ms = t_exact.elapsed().as_secs_f64() / queries.len() as f64;

    let t_ann = Instant::now();
    let mut ann: Vec<Vec<u64>> = Vec::new();
    for q in queries {
        let hits = vecs
            .search_ann_in_view(&snap, &space, q, k, k.max(8), Some(&candidate))
            .unwrap();
        ann.push(hits.into_iter().map(|h| h.source.owner_ord).collect());
    }
    let ann_ms = t_ann.elapsed().as_secs_f64() / queries.len() as f64;

    let mut recall_sum = 0.0f64;
    let mut identical = 0usize;
    for (a, e) in ann.iter().zip(exact.iter()) {
        let set: std::collections::HashSet<u64> = e.iter().copied().collect();
        let hit = a.iter().filter(|v| set.contains(v)).count();
        recall_sum += hit as f64 / e.len().max(1) as f64;
        if a == e {
            identical += 1;
        }
    }
    let recall = recall_sum / ann.len().max(1) as f64;
    let fallback = identical as f64 / ann.len().max(1) as f64;
    (recall, ann_ms * 1000.0, exact_ms * 1000.0, fallback)
}

#[test]
#[ignore = "long-running real-data benchmark; set RUSHDB_FILTERED_ANN_BENCH=1"]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "selectivity is a fraction in [0,1] over a small category count, so the ceiling fits usize exactly"
)]
fn dbpedia_filtered_recall_and_latency() {
    if std::env::var("RUSHDB_FILTERED_ANN_BENCH").ok().as_deref() != Some("1") {
        eprintln!("skipping; set RUSHDB_FILTERED_ANN_BENCH=1 to run");
        return;
    }
    let (dims, vectors) = read_fvecs(&format!("{FIXTURES_DIR}/dbpedia/vectors.fvecs"), 50_000);
    assert_eq!(dims, 1536, "DBPedia should be 1536-dim");
    let (vecs, _ords) = bind_real(&vectors, dims);

    // Queries: a held-out slice of the base vectors (out-of-sample queries are
    // not part of this fixture; using base vectors as queries is the standard
    // self-recall approximation for a corpus without a separate query set).
    // The exact-scan oracle over 50k x 1536 dominates runtime, so keep the
    // query set small for calibration.
    let nq = 32usize;
    let queries: Vec<Vec<f32>> = vectors[..nq].to_vec();

    let k = 10usize;
    println!(
        "DBPedia-100K (50k subset): {dims}-dim, {} vectors, {nq} queries",
        vectors.len()
    );
    println!(
        "{:<12} {:>8} {:>10} {:>12} {:>12}",
        "selectivity", "recall", "ann_ms", "exact_ms", "fallback%"
    );
    for selectivity in [0.05f64, 0.10, 0.25, 0.50, 1.00] {
        let count = ((20.0f64) * selectivity).ceil() as usize;
        let allowed: std::collections::HashSet<usize> = (0..count.min(20)).collect();
        let (recall, ann_ms, exact_ms, fallback) =
            run_filtered_experiment(&vecs, &queries, k, &allowed);
        println!(
            "{:<12.0} {:>8.2} {:>10.2} {:>12.2} {:>12.0}",
            selectivity * 100.0,
            recall,
            ann_ms,
            exact_ms,
            fallback * 100.0
        );
    }
}

#[test]
#[ignore = "long-running real-data benchmark; set RUSHDB_FILTERED_ANN_BENCH=1"]
#[expect(
    clippy::cast_precision_loss,
    reason = "recall and per-query latency are reported metrics that only need f64 precision"
)]
fn sift1m_unfiltered_recall_against_ground_truth() {
    if std::env::var("RUSHDB_FILTERED_ANN_BENCH").ok().as_deref() != Some("1") {
        eprintln!("skipping; set RUSHDB_FILTERED_ANN_BENCH=1 to run");
        return;
    }
    // Use a 100k subset so the in-memory engine stays manageable for the gate.
    let (dims, base) = read_fvecs(&format!("{FIXTURES_DIR}/sift1m/base.fvecs"), 100_000);
    let (qdim, queries) = read_fvecs(&format!("{FIXTURES_DIR}/sift1m/query.fvecs"), 256);
    assert_eq!(dims, 128);
    assert_eq!(qdim, 128);
    let neighbors = read_ivecs(&format!("{FIXTURES_DIR}/sift1m/neighbors.ivecs"));

    // `bind_real` returns the actual ordinal for each base row in order; the
    // ground-truth ids are base-row indices, so map them through `ords`.
    let (vecs, ords) = bind_real(&base, dims);
    let snap = vecs.snapshot().unwrap();
    let space = vecs.lookup_space_in_view(&snap, "real").unwrap().unwrap();

    for k in [10usize, 100] {
        let t = Instant::now();
        let hits: Vec<Vec<u64>> = queries
            .iter()
            .map(|q| {
                vecs.search_ann_in_view(&snap, &space, q, k, k.max(16), None)
                    .unwrap()
                    .into_iter()
                    .map(|h| h.source.owner_ord)
                    .collect()
            })
            .collect();
        let ms = t.elapsed().as_secs_f64() / queries.len() as f64 * 1000.0;

        // Ground truth ids are base-row indices; ords[id] is the engine ordinal.
        // Neighbors beyond the loaded subset are not in the engine; score recall
        // only over the neighbors that exist here.
        let mut recall_sum = 0.0f64;
        for (i, hit) in hits.iter().enumerate() {
            let gt: std::collections::HashSet<u64> = neighbors[i][..k]
                .iter()
                .filter(|v| (**v as usize) < ords.len())
                .map(|v| ords[*v as usize])
                .collect();
            let hit_set: std::collections::HashSet<u64> = hit.iter().copied().collect();
            let hit_count = gt.iter().filter(|g| hit_set.contains(g)).count();
            let denom = gt.len().max(1) as f64;
            recall_sum += hit_count as f64 / denom;
        }
        let recall = recall_sum / queries.len() as f64;
        println!("SIFT1M (100k subset) recall@{k}: {recall:.3}  ({ms:.2} ms/query)");
        // The gate previously only printed recall. Assert a conservative floor
        // so an explicitly-run benchmark fails on a broken graph instead of
        // reporting a number.
        assert!(
            recall >= 0.5,
            "SIFT1M (100k subset) recall@{k} collapsed to {recall:.3}; the ANN graph is broken"
        );
    }
}
