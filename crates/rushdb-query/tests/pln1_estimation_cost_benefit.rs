//! Value-distribution cost/benefit test suite: quantifies what the value-distribution
//! histogram's bounded, O(1)-maintained approximation actually buys at read
//! time, across realistic data shapes — the half of "is this worth it" that
//! neither of the two other value-distribution test files covers:
//!
//! - `rushdb-lmpg/tests/derived_state.rs`'s
//!   `derived_value_distribution_matches_a_base_scan_including_through_a_max_delete`
//!   proves the histogram's *exact* fields (min/max/distinct) are correct.
//!   It says nothing about how good the *approximate* bucket interpolation
//!   is for a real bound.
//! - `rushdb-lmpg/tests/pln1_ingest_overhead.rs` measures the *write-side*
//!   maintenance cost in isolation (ns/value) and asserts it is bounded.
//!   It says nothing about what that cost buys on the read side.
//!
//! This file measures the read-side payoff directly: estimation accuracy
//! against a ground-truth oracle, across distribution shapes and property
//! types, and the planner-visible consequence of that accuracy (does a
//! correct/incorrect estimate change whether a query takes the index path).
//!
//! # Headline finding (read before changing `VALUE_DISTRIBUTION_BUCKETS` or
//! the bucket-assignment formula)
//!
//! [`rushdb_lmpg::derived::Histogram`]'s 64 buckets are fixed over the
//! *full encoded-type domain* — deliberately never relative to the observed
//! `[min, max]`, so a bucket assignment survives deletes without rehashing
//! (see the design doc and `fixed_bucket_index`'s doc comment). The price
//! of that stability: for `Int64`/`DateTime` properties, whose
//! order-preserving encoding is a sign-flipped integer, the bucket is `(top
//! 6 bits of the 64-bit encoding)` — so *every* value in roughly `[-2^57,
//! 2^57)` (±144 quadrillion) falls in the *same single bucket*. Ages,
//! prices, scores, Unix-millisecond timestamps, row counts — virtually
//! every realistic integer property — lands entirely inside one of the 64
//! buckets. [`int64_distribution_shape_is_invisible_past_one_bucket`] below
//! demonstrates this concretely: three Int64 populations with deliberately
//! different shapes (uniform, skewed, clustered) produce statistically
//! indistinguishable histogram estimates, because the histogram cannot see
//! any structure finer than "everything is in bucket 32."
//! `Float64` fares much better
//! ([`float64_histogram_resolves_orders_of_magnitude_but_not_within_one`])
//! because IEEE-754's exponent-then-mantissa bit layout makes the top bits
//! of the encoding vary with *magnitude*, giving the same fixed-top-bits
//! trick a de facto logarithmic histogram — but it still collapses once
//! values are within the same order-of-magnitude band (demonstrated below).
//!
//! This is not a bug — `apply_transition`'s O(1)-per-value maintenance and
//! stable-under-delete bucket assignment are real, deliberate properties
//! the design doc traded resolution for — but it means the current
//! implementation's *actual* benefit for the single most common property
//! shape (bounded-range integers) is close to a 2-point (min/max) linear
//! estimate, not a "coarse distribution." See the module-end `Suggested
//! metrics` doc comment for how to track this going forward instead of a
//! single pass/fail accuracy bound.

use std::sync::Arc;
use std::time::Instant;

use rushdb_lmpg::derived;
use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_query::document::parse_where;
use rushdb_storage::Store;
use rushdb_types::encoding::encode_order_preserving;
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::model::{LogicalType, TypeState};
use rushdb_types::value::Scalar;
use serde_json::json;

fn engine(seed: u64) -> Lmpg {
    Lmpg::new(
        Arc::new(Store::new_in_memory()),
        TenantId::from_ordinal(5_000 + seed),
    )
}

/// Resolve the `pid` for a property typed `ty`, after at least one record
/// carrying it has been committed (mirrors `derived_state.rs`'s own
/// `pid_of_typed`, duplicated here since integration tests share no code).
fn pid_of_typed(eng: &Lmpg, name: &str, ty: LogicalType) -> u64 {
    use rushdb_lmpg::reader::keys_ext;
    use rushdb_types::value::normalize_name;
    let view = eng.store.read_view().unwrap();
    let sig = keys_ext::family_sig_key(
        eng.tenant,
        "default",
        OwnerKind::Record,
        &normalize_name(name),
    );
    let raw = view.get(&sig).unwrap().expect("family signature");
    let fid = u64::from_be_bytes(raw[..8].try_into().unwrap());
    let dkey = keys_ext::definition_key(eng.tenant, fid, TypeState::Known(ty));
    let def = view.get(&dkey).unwrap().expect("definition");
    u64::from_be_bytes(def[..8].try_into().unwrap())
}

fn ingest_int64(eng: &Lmpg, label: &str, name: &str, values: &[i64]) {
    for &v in values {
        eng.create_record(label, None, vec![(name.into(), FieldChange::Set(json!(v)))])
            .unwrap();
    }
}

fn ingest_float64(eng: &Lmpg, label: &str, name: &str, values: &[f64]) {
    for &v in values {
        eng.create_record(label, None, vec![(name.into(), FieldChange::Set(json!(v)))])
            .unwrap();
    }
}

/// Ground truth: exact count of `values` satisfying `>= bound` (`above`) or
/// `< bound` (`!above`) — a plain linear scan over the known fixture, which
/// is the oracle every histogram estimate below is checked against.
fn exact_count_i64(values: &[i64], bound: i64, above: bool) -> u64 {
    values
        .iter()
        .filter(|&&v| if above { v >= bound } else { v < bound })
        .count() as u64
}

fn exact_count_f64(values: &[f64], bound: f64, above: bool) -> u64 {
    values
        .iter()
        .filter(|&&v| if above { v >= bound } else { v < bound })
        .count() as u64
}

fn histogram_estimate_i64(eng: &Lmpg, pid: u64, bound: i64, above: bool) -> u64 {
    let view = eng.store.read_view().unwrap();
    let hist = derived::value_distribution(
        &view,
        eng.tenant,
        OwnerKind::Record,
        None,
        pid,
        derived::WalkBudget::default(),
    )
    .unwrap()
    .expect("histogram present");
    let enc = encode_order_preserving(LogicalType::Int64, &Scalar::Int64(bound)).unwrap();
    hist.estimate(&enc, above)
}

fn histogram_estimate_f64(eng: &Lmpg, pid: u64, bound: f64, above: bool) -> u64 {
    let view = eng.store.read_view().unwrap();
    let hist = derived::value_distribution(
        &view,
        eng.tenant,
        OwnerKind::Record,
        None,
        pid,
        derived::WalkBudget::default(),
    )
    .unwrap()
    .expect("histogram present");
    let enc = encode_order_preserving(LogicalType::Float64, &Scalar::Float64(bound)).unwrap();
    hist.estimate(&enc, above)
}

/// Widen a small, exact loop/count value into `f64` for a statistics
/// computation. Every caller in this file passes dataset sizes/counts in
/// the low thousands, far under `f64`'s 52-bit exact-integer range.
#[expect(
    clippy::cast_precision_loss,
    reason = "this file's counters are small test-fixture sizes, exact in f64 in practice"
)]
fn f64_from(value: u64) -> f64 {
    value as f64
}

/// Round `fraction * (len - 1)` to the nearest valid index into a slice of
/// length `len`. Used only for percentile lookups into a sorted fixture.
#[expect(
    clippy::cast_precision_loss,
    reason = "fixture lengths in this file are small test sizes, exact in f64 in practice"
)]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "fraction is in [0.0, 1.0] and len > 0, so the rounded index is always in [0, len)"
)]
fn percentile_index(len: usize, fraction: f64) -> usize {
    (((len - 1) as f64) * fraction).round() as usize
}

/// Relative error of `estimate` against `exact`, as a fraction of the
/// dataset size `n` (not of `exact` itself, which would blow up to `inf`/
/// huge numbers whenever `exact` is near zero — a highly selective bound,
/// exactly the case a planner cares most about getting right). This is the
/// metric [`report`] and every accuracy assertion below uses.
fn relative_error(estimate: u64, exact: u64, n: u64) -> f64 {
    let diff = estimate.max(exact) - estimate.min(exact);
    f64_from(diff) / f64_from(n.max(1))
}

struct ErrorStats {
    mean: f64,
    median: f64,
    p95: f64,
    max: f64,
}

fn stats(mut errors: Vec<f64>) -> ErrorStats {
    errors.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = errors.len();
    let at = |p: f64| errors[percentile_index(n, p)];
    ErrorStats {
        mean: errors.iter().sum::<f64>() / f64_from(n as u64),
        median: at(0.5),
        p95: at(0.95),
        max: *errors.last().unwrap(),
    }
}

/// Deterministic LCG, matching the style already used by this crate's other
/// fixtures (`planner_differential.rs`) — no external RNG dependency.
fn lcg(seed: u64) -> impl FnMut() -> u64 {
    let mut state = seed;
    move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state >> 33
    }
}

const N: usize = 4_000;

#[expect(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "span is always a small positive constant in this file, so the modulo result \
              fits i64 without wrapping"
)]
fn uniform_i64(seed: u64, span: i64) -> Vec<i64> {
    let mut next = lcg(seed);
    (0..N).map(|_| (next() % span as u64) as i64).collect()
}

/// Skewed/exponential-like: most mass packed near zero, a long thin tail —
/// the shape of e.g. request latencies or file sizes.
#[expect(
    clippy::cast_possible_truncation,
    reason = "shaped is in [0.0, 1.0) and span is a small positive constant, so the product \
              is always a small non-negative value that fits i64"
)]
fn skewed_i64(seed: u64, span: i64) -> Vec<i64> {
    let mut next = lcg(seed);
    (0..N)
        .map(|_| {
            let u = f64_from(next() % 1_000_000) / 1_000_000.0;
            let shaped = u * u * u; // cubing concentrates mass near 0
            (shaped * f64_from(span.unsigned_abs())) as i64
        })
        .collect()
}

/// Bimodal: two tight clusters far apart — the shape of e.g. a boolean-ish
/// business metric with two common operating points.
#[expect(
    clippy::cast_possible_wrap,
    reason = "span is always a small positive constant in this file, so these intermediate \
              values fit i64 without wrapping"
)]
fn clustered_i64(seed: u64, span: i64) -> Vec<i64> {
    let mut next = lcg(seed);
    (0..N)
        .map(|_| {
            let cluster_center = if next().is_multiple_of(2) {
                span / 10
            } else {
                span * 9 / 10
            };
            let jitter = (next() % 1000) as i64 - 500;
            (cluster_center + jitter).clamp(0, span - 1)
        })
        .collect()
}

/// Percentile bounds to probe: covers both "very selective" (near the
/// edges, where a planner most needs a good estimate to avoid materializing
/// a huge leaf) and "barely selective" (near the middle).
fn percentile_bounds_i64(sorted: &[i64]) -> Vec<i64> {
    [0.05, 0.1, 0.25, 0.5, 0.75, 0.9, 0.95]
        .iter()
        .map(|&p| sorted[percentile_index(sorted.len(), p)])
        .collect()
}

fn percentile_bounds_f64(sorted: &[f64]) -> Vec<f64> {
    [0.05, 0.1, 0.25, 0.5, 0.75, 0.9, 0.95]
        .iter()
        .map(|&p| sorted[percentile_index(sorted.len(), p)])
        .collect()
}

fn report(label: &str, stats: &ErrorStats) {
    eprintln!(
        "{label:<32} mean={:.4} median={:.4} p95={:.4} max={:.4}  (fraction of N={N})",
        stats.mean, stats.median, stats.p95, stats.max
    );
}

type ShapeGenerator = fn(u64, i64) -> Vec<i64>;

/// The headline finding (see module docs): three Int64 populations with
/// deliberately different shapes — uniform, skewed, bimodal-clustered —
/// over a realistic bounded range (0..100,000, nowhere near the ~2^57
/// bucket-collapse threshold) produce estimation error that is
/// statistically indistinguishable between shapes, because every value in
/// this range falls in the same one of 64 fixed buckets: the histogram
/// cannot see the shape difference at all. The assertions here are
/// deliberately loose (this test's job is to document and quantify the
/// collapse, not to pretend it doesn't exist) — they would fail only if the
/// *correctness* of the bucket's own linear interpolation regressed, not if
/// its precision does (it has none to lose, within this range).
#[test]
fn int64_distribution_shape_is_invisible_past_one_bucket() {
    let span = 100_000i64;
    let shapes: [(&str, ShapeGenerator, u64); 3] = [
        ("uniform", uniform_i64, 10),
        ("skewed", skewed_i64, 11),
        ("clustered", clustered_i64, 12),
    ];
    let mut per_shape_means = Vec::new();
    for (name, make, seed) in shapes {
        let values = make(seed, span);
        let eng = engine(seed);
        ingest_int64(&eng, "Row", "v", &values);
        let pid = pid_of_typed(&eng, "v", LogicalType::Int64);
        let mut sorted = values.clone();
        sorted.sort_unstable();
        let mut errors = Vec::new();
        for bound in percentile_bounds_i64(&sorted) {
            let exact = exact_count_i64(&values, bound, true);
            let estimate = histogram_estimate_i64(&eng, pid, bound, true);
            errors.push(relative_error(estimate, exact, values.len() as u64));
        }
        let s = stats(errors);
        report(&format!("int64/{name}"), &s);
        per_shape_means.push(s.mean);
    }
    // Document the collapse quantitatively: every shape's mean error must be
    // within a small absolute band of every other shape's — if a future
    // change to the bucket formula gives Int64 real shape-awareness, this
    // assertion (not a human reading eprintln output) is what will catch it
    // and prompt updating this test's framing.
    let spread = per_shape_means.iter().copied().fold(0.0, f64::max)
        - per_shape_means.iter().copied().fold(1.0, f64::min);
    assert!(
        spread < 0.05,
        "if this fails, Int64 histograms just gained real shape resolution — \
         update this test's framing and the module doc, don't just raise the bound. \
         means were {per_shape_means:?}"
    );
}

/// The contrast case: Float64's order-preserving encoding puts the IEEE-754
/// exponent in the top bits, so the same fixed-top-bits bucketing gives a
/// de facto log-scale histogram — real resolution *across* orders of
/// magnitude, even though (as the second assertion shows) it still
/// collapses *within* one order of magnitude, same as Int64 does within its
/// single bucket.
#[test]
fn float64_histogram_resolves_orders_of_magnitude_but_not_within_one() {
    let eng = engine(900);
    let mut next = lcg(11);
    // Spans 1.0 .. 1e12: many orders of magnitude.
    let values: Vec<f64> = (0..N)
        .map(|_| {
            let exponent = f64_from(next() % 12);
            let mantissa = 1.0 + f64_from(next() % 1_000_000) / 1_000_000.0;
            mantissa * 10f64.powf(exponent)
        })
        .collect();
    ingest_float64(&eng, "Row", "v", &values);
    let pid = pid_of_typed(&eng, "v", LogicalType::Float64);

    let cross_magnitude_bounds = [1.0, 10.0, 100.0, 1_000.0, 1_000_000.0, 1e9];
    let mut cross_errors = Vec::new();
    for &bound in &cross_magnitude_bounds {
        let exact = exact_count_f64(&values, bound, true);
        let estimate = histogram_estimate_f64(&eng, pid, bound, true);
        cross_errors.push(relative_error(estimate, exact, values.len() as u64));
    }
    let cross = stats(cross_errors);
    report("float64/cross-magnitude", &cross);

    // Within one narrow band (same order of magnitude): same collapse as
    // Int64, same reason (all these bounds share the IEEE-754 exponent, so
    // the top 6 bits — and therefore the bucket — do not move).
    let narrow_values: Vec<f64> = values
        .iter()
        .copied()
        .filter(|&v| (100.0..1_000.0).contains(&v))
        .collect();
    assert!(
        narrow_values.len() > 50,
        "fixture must produce enough values in the narrow band to test it"
    );
    let eng2 = engine(901);
    ingest_float64(&eng2, "Row", "v", &narrow_values);
    let pid2 = pid_of_typed(&eng2, "v", LogicalType::Float64);
    let mut narrow_sorted = narrow_values.clone();
    narrow_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut narrow_errors = Vec::new();
    for bound in percentile_bounds_f64(&narrow_sorted) {
        let exact = exact_count_f64(&narrow_values, bound, true);
        let estimate = histogram_estimate_f64(&eng2, pid2, bound, true);
        narrow_errors.push(relative_error(estimate, exact, narrow_values.len() as u64));
    }
    let narrow = stats(narrow_errors);
    report("float64/within-one-band", &narrow);

    // The cross-magnitude case must be meaningfully more accurate than the
    // within-one-band case — the quantified version of "floats get real
    // resolution across orders of magnitude, not within one."
    assert!(
        cross.mean < narrow.mean,
        "cross-magnitude estimation (mean err {:.4}) should beat within-one-band \
         estimation (mean err {:.4}) — if it does not, Float64's log-scale \
         advantage from the IEEE-754 bit layout has regressed",
        cross.mean,
        narrow.mean
    );
}

/// Planner-visible consequence, not just an isolated accuracy number: a
/// highly selective `Eq` leaf paired with a range leaf. Regardless of how
/// coarse the range leaf's histogram estimate is (per the finding above,
/// for Int64 it usually is), `explain_range_selectivity`'s fraction must
/// stay a bounded `[0.0, 1.0]` value, and — the actual regression this test
/// guards — the planner must still choose the selective `Eq` leaf first
/// (i.e. intersect against the small set, not materialize the large range
/// leaf's candidates first), which `index_candidates_with_budget` achieves
/// by sorting eligible leaves by estimate and the exact `Eq` leaf will
/// always sort below `u64::MAX`-or-worse range noise. Verified indirectly
/// via timing: a query over a large population should run close to the
/// selective leaf's own cost, not the range leaf's.
#[test]
#[expect(
    clippy::cast_possible_wrap,
    reason = "total is a small test-fixture size, far under i64::MAX"
)]
fn mixed_predicate_query_stays_fast_regardless_of_range_estimate_coarseness() {
    let eng = engine(950);
    let total = 20_000u64;
    for i in 0..total {
        let category = if i == total / 2 { "needle" } else { "haystack" };
        eng.create_record(
            "Row",
            None,
            vec![
                ("cat".into(), FieldChange::Set(json!(category))),
                ("score".into(), FieldChange::Set(json!(i as i64))),
            ],
        )
        .unwrap();
    }

    let where_ = parse_where(&json!({
        "cat": "needle",
        "score": {"$gte": 0}
    }))
    .unwrap();
    let view = eng.store.read_view().unwrap();
    let selectivity = rushdb_query::explain_range_selectivity(&view, eng.tenant, &where_)
        .unwrap()
        .expect("range leaf has a usable histogram");
    assert!(
        (0.0..=1.0).contains(&selectivity),
        "selectivity estimate must stay a bounded fraction: {selectivity}"
    );
    // `score >= 0` matches everything, so the (coarse) estimate should at
    // least get the *direction* right: at or near full selectivity, not
    // something absurd like near-zero.
    assert!(
        selectivity > 0.9,
        "a `>= 0` bound over an all-nonnegative population must estimate as \
         nearly fully selective, got {selectivity}"
    );

    let start = Instant::now();
    let parsed = rushdb_query::document::parse_document(&json!({
        "where": {"cat": "needle", "score": {"$gte": 0}}
    }))
    .unwrap();
    let page = rushdb_query::find_records(&eng, &parsed.search).unwrap();
    let indexed_elapsed = start.elapsed();
    assert_eq!(page.records.len(), 1, "exactly one needle record");

    let mut forced = parsed;
    forced.search.force_scan = true;
    let start = Instant::now();
    let scan_page = rushdb_query::find_records(&eng, &forced.search).unwrap();
    let scan_elapsed = start.elapsed();
    assert_eq!(scan_page.records.len(), 1);

    eprintln!(
        "mixed-predicate query: indexed path {indexed_elapsed:?}, forced full scan \
         {scan_elapsed:?} over {total} records"
    );
    // Not a tight timing assertion (wall-clock is inherently noisy) — the
    // qualitative claim this guards is that the index path is not SLOWER
    // than a full scan despite the range leaf's coarse estimate; a real
    // regression (e.g. the planner materializing the range leaf's ~20,000
    // owners before intersecting) would show up as the indexed path taking
    // a large multiple of the scan's time, not a noisy tie.
    assert!(
        indexed_elapsed <= scan_elapsed.saturating_mul(4) + std::time::Duration::from_millis(5),
        "indexed path ({indexed_elapsed:?}) should not be dramatically slower than \
         a full scan ({scan_elapsed:?}) — suggests the coarse range estimate \
         caused the planner to materialize the wrong leaf first"
    );
}

/// A property with only a handful of
/// distinct values is exactly where the histogram's uniform-within-bucket
/// assumption is weakest (see the module doc's Int64 single-bucket-collapse
/// finding) — and exactly where it's cheap to skip the assumption entirely,
/// since there are so few distinct ordered postings to walk. Below the
/// threshold private to `planner.rs` (`LOW_CARDINALITY_EXACT_THRESHOLD`),
/// `range_leaf_histogram_estimate` now answers the range predicate exactly
/// by summing each matching distinct value's maintained exact posting
/// cardinality instead of interpolating a bucket — the same counter an
/// `Eq` leaf already uses. This test proves both halves: the raw bucket
/// estimate (`Histogram::estimate`, unchanged by this addition) is badly
/// wrong for this shape, and the planner-visible channel
/// (`explain_range_selectivity`) is now exact despite that.
#[test]
fn low_distinct_range_leaf_is_answered_exactly_not_estimated() {
    let eng = engine(970);
    // Five distinct tiers, unevenly weighted, so no uniform assumption
    // (over [min, max], or over one collapsed bucket) can get this right
    // by accident.
    let counts = [1000u64, 100, 100, 100, 700];
    let mut values = Vec::new();
    for (tier, &count) in counts.iter().enumerate() {
        for _ in 0..count {
            values.push(i64::try_from(tier).unwrap());
        }
    }
    let total = values.len() as u64;
    ingest_int64(&eng, "Row", "tier", &values);
    let pid = pid_of_typed(&eng, "tier", LogicalType::Int64);

    let bound = 2i64;
    let exact = exact_count_i64(&values, bound, true);
    assert_eq!(exact, 900, "tiers 2, 3, 4 sum to 100 + 100 + 700");

    // The raw bucket estimate (unchanged by this addition) is badly wrong
    // for this shape: Int64's single-bucket collapse plus a bound tiny
    // relative to that bucket's huge fixed width makes it estimate close
    // to "almost everything", not the true, unevenly-weighted 45%.
    let raw_estimate = histogram_estimate_i64(&eng, pid, bound, true);
    let raw_error = raw_estimate.max(exact) - raw_estimate.min(exact);
    assert!(
        raw_error > total / 10,
        "expected the raw bucket estimate ({raw_estimate}) to be badly wrong \
         against the exact count ({exact}) for this low-distinct shape, to set \
         up the contrast this test checks — if this now holds, the bucket \
         estimate improved for reasons unrelated to this addition"
    );

    // The planner-visible channel, after the low-cardinality bypass, must
    // be exact: estimate/total recovers exact/total with no interpolation
    // error.
    let where_ = parse_where(&json!({"tier": {"$gte": bound}})).unwrap();
    let view = eng.store.read_view().unwrap();
    let selectivity = rushdb_query::explain_range_selectivity(&view, eng.tenant, &where_)
        .unwrap()
        .expect("range leaf has a usable histogram");
    let expected = f64_from(exact) / f64_from(total);
    assert!(
        (selectivity - expected).abs() < 1e-9,
        "a low-distinct range leaf should be answered exactly: got {selectivity}, \
         expected {expected} ({exact}/{total})"
    );
}

// ---------------------------------------------------------------------------
// Suggested metrics (per the task that asked for this file): a single
// pass/fail "estimation accuracy" bound, as `pln1_ingest_overhead.rs` uses
// on the write side, does not fit the read side well, because:
//
// 1. Accuracy is fundamentally a *distribution* of errors, not one number,
//    and it is strongly bimodal by *type* (Int64/DateTime vs Float64) and by
//    *value magnitude relative to the bucket-collapse threshold* (~2^57) —
//    collapsing it to a mean hides exactly the cases (small-range integers)
//    that are the most common in practice and the worst-served today.
//    Prefer tracking (and alerting on regressions in) the per-type,
//    per-magnitude-band percentile breakdown this file prints, not a single
//    threshold.
// 2. What actually matters operationally is not the estimate's error in
//    isolation, but its *regret*: does a bad estimate cause the planner to
//    do more work than the best achievable plan would have? This file's
//    `mixed_predicate_query_stays_fast_regardless_of_range_estimate_coarseness`
//    is a first cut at a regret metric (indexed-path time vs. forced-scan
//    time); a fuller version would sweep many selectivity ratios and report
//    the worst observed regret ratio, rather than one fixed scenario.
// 3. If Int64/DateTime resolution is ever worth improving, the right
//    before/after metric is the `int64_distribution_shape_is_invisible_past_one_bucket`
//    spread-between-shapes number above — it should move from "near zero
//    spread" (shapes indistinguishable) to "shapes separated by error" once
//    a change gives the histogram real per-shape resolution.
// ---------------------------------------------------------------------------
