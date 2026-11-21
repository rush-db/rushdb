//! L1: the ANN graph's build parameters measured on a real embedding set.
//!
//! For each grid point (graph scalar, connectivity, build expansion) this
//! builds a usearch index the way a segment build does (the first chunk on
//! one thread, the rest from several), then answers held-out queries the
//! way a search does: gather `max(k, ef)` candidates, rerank them exactly
//! against the f32 vectors, keep the top `k`. It reports build time, the
//! index's memory and serialized size, and per `ef` the recall@k against a
//! brute-force answer and the gather's latency (one thread, p50 and p99).
//!
//! ```text
//! cargo run --release -p rushdb-vector --example ann_grid -- \
//!     BASE.npy QUERIES.npy [--threads 8] [--queries 1000] [--out grid.jsonl]
//! ```
//! `BASE.npy` and `QUERIES.npy` are little-endian `<f4` arrays of shape
//! (n, dims). The brute-force answer is cached beside `QUERIES.npy`.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "a measurement harness: keys and counts fit, ratios are reported"
)]

use std::io::{Read, Write};
use std::time::Instant;

const K: usize = 10;
const EFS: [usize; 5] = [16, 32, 64, 128, 256];
/// Vectors inserted on one thread before the rest go in parallel (as a
/// segment build does).
const FIRST_CHUNK: usize = 8 * 1024;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .position(|arg| arg == name)
            .and_then(|at| args.get(at + 1))
            .cloned()
    };
    let (Some(base_path), Some(query_path)) = (args.first(), args.get(1)) else {
        eprintln!("usage: ann_grid BASE.npy QUERIES.npy [--threads N] [--queries N] [--out FILE]");
        std::process::exit(2);
    };
    let threads: usize = flag("--threads").map_or(8, |v| v.parse().expect("--threads"));
    let (dims, base) = read_npy(base_path);
    let (query_dims, mut queries) = read_npy(query_path);
    assert_eq!(dims, query_dims, "base and queries differ in dimensions");
    let count = flag("--queries").map_or(queries.len() / dims, |v| v.parse().expect("--queries"));
    queries.truncate(count * dims);
    let truth = ground_truth(
        &base,
        &queries,
        dims,
        &format!("{query_path}.top{K}-{}", base.len() / dims),
    );
    let mut out = flag("--out").map(|path| std::fs::File::create(path).expect("--out"));
    let grid = [
        (usearch::ScalarKind::F16, 0, 0),
        (usearch::ScalarKind::F32, 0, 0),
        (usearch::ScalarKind::BF16, 0, 0),
        (usearch::ScalarKind::I8, 0, 0),
        (usearch::ScalarKind::F16, 24, 0),
        (usearch::ScalarKind::F16, 32, 0),
        (usearch::ScalarKind::F32, 32, 0),
        (usearch::ScalarKind::F16, 0, 256),
        (usearch::ScalarKind::F16, 32, 256),
    ];
    for (quantization, connectivity, expansion_add) in grid {
        let line = measure(
            &base,
            &queries,
            &truth,
            dims,
            threads,
            (quantization, connectivity, expansion_add),
        );
        println!("{line}");
        if let Some(file) = out.as_mut() {
            writeln!(file, "{line}").expect("write --out");
        }
    }
}

fn measure(
    base: &[f32],
    queries: &[f32],
    truth: &[Vec<u64>],
    dims: usize,
    threads: usize,
    (quantization, connectivity, expansion_add): (usearch::ScalarKind, usize, usize),
) -> String {
    let options = usearch::IndexOptions {
        dimensions: dims,
        metric: usearch::MetricKind::Cos,
        quantization,
        connectivity,
        expansion_add,
        expansion_search: 1,
        multi: false,
    };
    let n = base.len() / dims;
    let index = usearch::Index::new(&options).expect("index");
    index
        .reserve_capacity_and_threads(n, threads)
        .expect("reserve");
    let vector = |key: usize| &base[key * dims..(key + 1) * dims];
    let started = Instant::now();
    for key in 0..FIRST_CHUNK.min(n) {
        index.add(key as u64, vector(key)).expect("add");
    }
    let rest: Vec<usize> = (FIRST_CHUNK.min(n)..n).collect();
    std::thread::scope(|scope| {
        for part in rest.chunks(rest.len().div_ceil(threads).max(1)) {
            let index = &index;
            scope.spawn(move || {
                for &key in part {
                    index.add(key as u64, vector(key)).expect("add");
                }
            });
        }
    });
    let build_s = started.elapsed().as_secs_f64();
    let mut per_ef = Vec::new();
    for ef in EFS {
        let mut latencies = Vec::with_capacity(truth.len());
        let mut hits = 0usize;
        for (q, want) in queries.chunks(dims).zip(truth) {
            let started = Instant::now();
            let found = index.search(q, K.max(ef)).expect("search");
            latencies.push(started.elapsed().as_secs_f64() * 1e3);
            // The exact rerank a search does on the gathered candidates.
            let mut reranked: Vec<(f32, u64)> = found
                .keys
                .iter()
                .map(|&key| (cosine_distance(q, vector(key as usize)), key))
                .collect();
            reranked.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            hits += reranked
                .iter()
                .take(K)
                .filter(|(_, key)| want.contains(key))
                .count();
        }
        latencies.sort_by(f64::total_cmp);
        let pct = |p: usize| latencies[(latencies.len() * p / 100).min(latencies.len() - 1)];
        per_ef.push(format!(
            "{{\"ef\":{ef},\"recall\":{:.4},\"p50_ms\":{:.4},\"p99_ms\":{:.4}}}",
            hits as f64 / (truth.len() * K) as f64,
            pct(50),
            pct(99)
        ));
    }
    format!(
        "{{\"quantization\":\"{quantization:?}\",\"connectivity\":{connectivity},\"expansion_add\":{expansion_add},\
         \"vectors\":{n},\"build_s\":{build_s:.2},\"memory_bytes\":{},\"serialized_bytes\":{},\"search\":[{}]}}",
        index.memory_usage(),
        index.serialized_length(),
        per_ef.join(",")
    )
}

fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let (mut dot, mut aa, mut bb) = (0f32, 0f32, 0f32);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        aa += x * x;
        bb += y * y;
    }
    1.0 - dot / (aa.sqrt() * bb.sqrt()).max(f32::MIN_POSITIVE)
}

/// The exact top-`K` of every query, computed once and cached in `cache`.
fn ground_truth(base: &[f32], queries: &[f32], dims: usize, cache: &str) -> Vec<Vec<u64>> {
    let count = queries.len() / dims;
    if let Ok(raw) = std::fs::read(cache) {
        if raw.len() == count * K * 8 {
            return raw
                .chunks(K * 8)
                .map(|row| {
                    row.chunks(8)
                        .map(|key| u64::from_le_bytes(key.try_into().expect("8 bytes")))
                        .collect()
                })
                .collect();
        }
    }
    let n = base.len() / dims;
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let rows: Vec<Vec<u64>> = std::thread::scope(|scope| {
        // Every thread is spawned before any is joined.
        #[expect(clippy::needless_collect, reason = "spawn all, then join")]
        let handles: Vec<_> = queries
            .chunks(dims * count.div_ceil(threads).max(1))
            .map(|block| {
                scope.spawn(move || {
                    block
                        .chunks(dims)
                        .map(|q| {
                            let mut scored: Vec<(f32, u64)> = (0..n)
                                .map(|key| {
                                    (
                                        cosine_distance(q, &base[key * dims..(key + 1) * dims]),
                                        key as u64,
                                    )
                                })
                                .collect();
                            scored.select_nth_unstable_by(K, |a, b| a.0.total_cmp(&b.0));
                            scored.truncate(K);
                            scored.into_iter().map(|(_, key)| key).collect::<Vec<u64>>()
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("truth"))
            .collect()
    });
    let mut raw = Vec::with_capacity(count * K * 8);
    for row in &rows {
        for key in row {
            raw.extend_from_slice(&key.to_le_bytes());
        }
    }
    std::fs::write(cache, raw).expect("cache the brute-force answer");
    rows
}

/// A little-endian `<f4` `.npy` array of shape (n, dims): `(dims, values)`.
fn read_npy(path: &str) -> (usize, Vec<f32>) {
    let mut raw = Vec::new();
    std::fs::File::open(path)
        .and_then(|mut file| file.read_to_end(&mut raw))
        .unwrap_or_else(|error| panic!("{path}: {error}"));
    assert_eq!(&raw[..6], b"\x93NUMPY", "{path}: not an .npy file");
    let (header_len, header_start) = if raw[6] == 1 {
        (usize::from(u16::from_le_bytes([raw[8], raw[9]])), 10)
    } else {
        (
            u32::from_le_bytes([raw[8], raw[9], raw[10], raw[11]]) as usize,
            12,
        )
    };
    let header =
        std::str::from_utf8(&raw[header_start..header_start + header_len]).expect("header");
    assert!(
        header.contains("'<f4'"),
        "{path}: not little-endian f32: {header}"
    );
    assert!(
        header.contains("'fortran_order': False"),
        "{path}: Fortran order"
    );
    let shape = &header[header.find("'shape': (").expect("shape") + 10..];
    let dims: usize = shape[..shape.find(')').expect("shape end")]
        .split(',')
        .nth(1)
        .and_then(|d| d.trim().parse().ok())
        .expect("a 2-d shape");
    let values = raw[header_start + header_len..]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect();
    (dims, values)
}
