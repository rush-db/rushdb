//! Similarity kernels: every score the engine returns is computed here.
//!
//! A score accumulates in `f64` over [`LANES`] independent lanes (component
//! `i` adds into lane `i % LANES`, in increasing `i`), the lanes are summed
//! in one fixed tree, and the result narrows to `f32`. Independent lanes let
//! the loop vectorize; the order of every addition is fixed by the lane
//! layout alone, so the AVX2 path and the portable path perform the same
//! IEEE operations and return bit-identical scores on every CPU. AVX2 is
//! picked at runtime; FMA is deliberately not used, because a fused
//! multiply-add rounds once where the portable path rounds twice and the
//! two paths would then disagree in the last bit.
//!
//! Compared with a strictly sequential `f64` sum, the lane order changes
//! the `f64` result by a relative error of about `dims * 2^-53` (1.7e-13 at
//! 1536 dimensions), far below half an `f32` ulp (`2^-24`): the narrowed
//! score is the same unless the `f64` value lies that close to an `f32`
//! rounding boundary, where the two differ by one ulp. Only a score near
//! zero relative to its terms (heavy cancellation in a dot product) can
//! differ by more, and there neither order is more accurate.
//!
//! The query is prepared once per search ([`PreparedQuery`]): its own norm
//! is not recomputed per candidate. Stored vectors are scored straight from
//! their little-endian bytes ([`PreparedQuery::score_stored`]), without
//! decoding them into a `Vec<f32>` first.

use rushdb_types::occurrence::Similarity;

/// Independent `f64` accumulators per sum.
pub(crate) const LANES: usize = 8;

type Acc = [f64; LANES];

/// The fixed reduction tree over the lanes.
fn reduce(lanes: &Acc) -> f64 {
    ((lanes[0] + lanes[1]) + (lanes[2] + lanes[3]))
        + ((lanes[4] + lanes[5]) + (lanes[6] + lanes[7]))
}

/// A vector's components: an `f32` slice or stored little-endian bytes.
#[derive(Clone, Copy)]
enum Components<'a> {
    F32(&'a [f32]),
    Le(&'a [u8]),
}

impl Components<'_> {
    fn len(self) -> usize {
        match self {
            Components::F32(values) => values.len(),
            Components::Le(bytes) => bytes.len() / 4,
        }
    }

    #[inline]
    fn at(self, i: usize) -> f32 {
        match self {
            Components::F32(values) => values[i],
            Components::Le(bytes) => f32::from_le_bytes([
                bytes[4 * i],
                bytes[4 * i + 1],
                bytes[4 * i + 2],
                bytes[4 * i + 3],
            ]),
        }
    }

    /// Unaligned pointer to the first component. Only meaningful on a
    /// little-endian target, where stored bytes are native `f32`s.
    #[cfg(target_arch = "x86_64")]
    fn ptr(self) -> *const f32 {
        match self {
            Components::F32(values) => values.as_ptr(),
            Components::Le(bytes) => bytes.as_ptr().cast(),
        }
    }
}

/// Which of the per-lane sums a kernel produces.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// `sum q*v` and `sum v*v`.
    DotNorm,
    /// `sum f64(q - v)^2` (difference in `f32`, as the metric defines it)
    /// and `sum v*v` (finiteness of the stored vector).
    L2Norm,
}

/// One lane step of `kind` for component `i`.
#[inline]
fn step(kind: Kind, q: f32, v: f32, first: &mut f64, second: &mut f64) {
    let b = f64::from(v);
    match kind {
        Kind::DotNorm => *first += f64::from(q) * b,
        Kind::L2Norm => {
            let d = f64::from(q - v);
            *first += d * d;
        }
    }
    *second += b * b;
}

fn sums_portable(kind: Kind, q: &[f32], v: Components<'_>) -> (Acc, Acc) {
    let mut first = [0f64; LANES];
    let mut second = [0f64; LANES];
    let len = q.len();
    let full = len - len % LANES;
    let mut base = 0;
    while base < full {
        for lane in 0..LANES {
            step(
                kind,
                q[base + lane],
                v.at(base + lane),
                &mut first[lane],
                &mut second[lane],
            );
        }
        base += LANES;
    }
    for i in full..len {
        step(
            kind,
            q[i],
            v.at(i),
            &mut first[i - full],
            &mut second[i - full],
        );
    }
    (first, second)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
/// # Safety
/// AVX2 must be available, and `v` must hold `q.len()` components.
unsafe fn sums_avx2(kind: Kind, q: &[f32], v: Components<'_>) -> (Acc, Acc) {
    use std::arch::x86_64::{
        _mm256_add_pd, _mm256_castps256_ps128, _mm256_cvtps_pd, _mm256_extractf128_ps,
        _mm256_loadu_ps, _mm256_mul_pd, _mm256_setzero_pd, _mm256_storeu_pd, _mm256_sub_ps,
    };
    let len = q.len();
    let full = len - len % LANES;
    let (qp, vp) = (q.as_ptr(), v.ptr());
    let (mut first_lo, mut first_hi) = (_mm256_setzero_pd(), _mm256_setzero_pd());
    let (mut second_lo, mut second_hi) = (_mm256_setzero_pd(), _mm256_setzero_pd());
    let mut base = 0;
    while base < full {
        let qv = _mm256_loadu_ps(qp.add(base));
        let vv = _mm256_loadu_ps(vp.add(base));
        let v_lo = _mm256_cvtps_pd(_mm256_castps256_ps128(vv));
        let v_hi = _mm256_cvtps_pd(_mm256_extractf128_ps::<1>(vv));
        let (a_lo, a_hi, b_lo, b_hi) = match kind {
            Kind::DotNorm => (
                _mm256_cvtps_pd(_mm256_castps256_ps128(qv)),
                _mm256_cvtps_pd(_mm256_extractf128_ps::<1>(qv)),
                v_lo,
                v_hi,
            ),
            Kind::L2Norm => {
                let d = _mm256_sub_ps(qv, vv);
                let d_lo = _mm256_cvtps_pd(_mm256_castps256_ps128(d));
                let d_hi = _mm256_cvtps_pd(_mm256_extractf128_ps::<1>(d));
                (d_lo, d_hi, d_lo, d_hi)
            }
        };
        first_lo = _mm256_add_pd(first_lo, _mm256_mul_pd(a_lo, b_lo));
        first_hi = _mm256_add_pd(first_hi, _mm256_mul_pd(a_hi, b_hi));
        second_lo = _mm256_add_pd(second_lo, _mm256_mul_pd(v_lo, v_lo));
        second_hi = _mm256_add_pd(second_hi, _mm256_mul_pd(v_hi, v_hi));
        base += LANES;
    }
    let mut first = [0f64; LANES];
    let mut second = [0f64; LANES];
    _mm256_storeu_pd(first.as_mut_ptr(), first_lo);
    _mm256_storeu_pd(first.as_mut_ptr().add(4), first_hi);
    _mm256_storeu_pd(second.as_mut_ptr(), second_lo);
    _mm256_storeu_pd(second.as_mut_ptr().add(4), second_hi);
    for i in full..len {
        step(
            kind,
            q[i],
            v.at(i),
            &mut first[i - full],
            &mut second[i - full],
        );
    }
    (first, second)
}

/// Whether this CPU runs the AVX2 kernel (decided once per process).
fn use_avx2() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        static AVX2: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *AVX2.get_or_init(|| std::arch::is_x86_feature_detected!("avx2"))
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// A query prepared for scoring many vectors: its metric, its norm and the
/// kernel this CPU runs. The caller has checked the query is finite.
pub struct PreparedQuery<'a> {
    query: &'a [f32],
    similarity: Similarity,
    /// `sum q*q`, accumulated like every other sum.
    norm_sq: f64,
    avx2: bool,
}

impl<'a> PreparedQuery<'a> {
    /// Prepare `query` for `similarity`.
    #[must_use]
    pub fn new(query: &'a [f32], similarity: Similarity) -> Self {
        Self::with_kernel(query, similarity, use_avx2())
    }

    fn with_kernel(query: &'a [f32], similarity: Similarity, avx2: bool) -> Self {
        let mut prepared = Self {
            query,
            similarity,
            norm_sq: 0.0,
            avx2,
        };
        prepared.norm_sq = prepared.sums(Kind::DotNorm, Components::F32(query)).1;
        prepared
    }

    /// Both reduced sums of `kind` against `v` (same length as the query).
    fn sums(&self, kind: Kind, v: Components<'_>) -> (f64, f64) {
        debug_assert_eq!(v.len(), self.query.len());
        #[cfg(target_arch = "x86_64")]
        if self.avx2 && cfg!(target_endian = "little") {
            // SAFETY: AVX2 is available (checked at runtime), and both
            // operands hold `query.len()` components (checked by every
            // caller), so every unaligned load stays in bounds.
            let (first, second) = unsafe { sums_avx2(kind, self.query, v) };
            return (reduce(&first), reduce(&second));
        }
        let (first, second) = sums_portable(kind, self.query, v);
        (reduce(&first), reduce(&second))
    }

    /// The score of a finite vector of the query's length; higher is more
    /// similar for every metric (see [`crate::score`]).
    ///
    /// # Panics
    /// Panics when `v`'s length differs from the query's.
    #[must_use]
    pub fn score(&self, v: &[f32]) -> f32 {
        assert_eq!(
            v.len(),
            self.query.len(),
            "vector length differs from the query"
        );
        self.finish(Components::F32(v)).0
    }

    /// The score of a stored vector (little-endian `f32` bytes), or `None`
    /// when the bytes are not `dims` finite components: a stored vector is
    /// validated when bound, so either is corruption.
    #[must_use]
    pub fn score_stored(&self, raw: &[u8]) -> Option<f32> {
        if raw.len() != self.query.len() * 4 {
            return None;
        }
        let (score, finite) = self.finish(Components::Le(raw));
        finite.then_some(score)
    }

    /// The narrowed score and whether every component of `v` was finite: a
    /// finite `f32` squared cannot overflow `f64`, so `sum v*v` is finite
    /// exactly when every component is.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "scores accumulate in f64, then narrow to the f32 public API"
    )]
    fn finish(&self, v: Components<'_>) -> (f32, bool) {
        match self.similarity {
            Similarity::Cosine => {
                let (dot, norm_sq) = self.sums(Kind::DotNorm, v);
                let score = if self.norm_sq == 0.0 || norm_sq == 0.0 {
                    0.0
                } else {
                    (dot / (self.norm_sq.sqrt() * norm_sq.sqrt())) as f32
                };
                (score, norm_sq.is_finite())
            }
            Similarity::Dot => {
                let (dot, norm_sq) = self.sums(Kind::DotNorm, v);
                (dot as f32, norm_sq.is_finite())
            }
            Similarity::Euclidean => {
                let (dist_sq, norm_sq) = self.sums(Kind::L2Norm, v);
                ((1.0 / (1.0 + dist_sq.sqrt())) as f32, norm_sq.is_finite())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scoring formula before the lane kernels: one strictly sequential
    /// `f64` sum per quantity, the query norm recomputed per call.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the old formula narrowed the same way"
    )]
    fn sequential(query: &[f32], vec: &[f32], similarity: Similarity) -> f32 {
        match similarity {
            Similarity::Cosine => {
                let (mut dot, mut na, mut nb) = (0f64, 0f64, 0f64);
                for i in 0..query.len() {
                    dot += f64::from(query[i]) * f64::from(vec[i]);
                    na += f64::from(query[i]).powi(2);
                    nb += f64::from(vec[i]).powi(2);
                }
                if na == 0.0 || nb == 0.0 {
                    0.0
                } else {
                    (dot / (na.sqrt() * nb.sqrt())) as f32
                }
            }
            Similarity::Dot => {
                let mut dot = 0f64;
                for i in 0..query.len() {
                    dot += f64::from(query[i]) * f64::from(vec[i]);
                }
                dot as f32
            }
            Similarity::Euclidean => {
                let mut dist = 0f64;
                for i in 0..query.len() {
                    let d = f64::from(query[i] - vec[i]);
                    dist += d * d;
                }
                (1.0 / (1.0 + dist.sqrt())) as f32
            }
        }
    }

    struct Rng(u64);
    impl Rng {
        fn unit(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            #[expect(clippy::cast_precision_loss, reason = "a test distribution")]
            let x = (self.0 >> 40) as f32 / (1u64 << 24) as f32;
            x * 2.0 - 1.0
        }
        fn vector(&mut self, dims: usize) -> Vec<f32> {
            (0..dims).map(|_| self.unit()).collect()
        }
    }

    fn ulps(a: f32, b: f32) -> u32 {
        // Same-signed finite scores: the distance of their bit patterns.
        if a.to_bits() == b.to_bits() {
            return 0;
        }
        assert_eq!(a.is_sign_negative(), b.is_sign_negative(), "{a} vs {b}");
        a.to_bits().abs_diff(b.to_bits())
    }

    const METRICS: [Similarity; 3] = [Similarity::Cosine, Similarity::Dot, Similarity::Euclidean];

    /// The lane kernels agree with the old sequential formula within one
    /// `f32` ulp (module docs: only the `f64` summation order differs), and
    /// rank every candidate set identically.
    #[test]
    fn lanes_match_the_sequential_formula_within_one_ulp_and_rank_identically() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let mut off_by_one = 0usize;
        for dims in [1usize, 3, 7, 8, 9, 31, 64, 100, 384, 768, 1536] {
            for similarity in METRICS {
                let query = rng.vector(dims);
                let prepared = PreparedQuery::new(&query, similarity);
                let vectors: Vec<Vec<f32>> = (0..200).map(|_| rng.vector(dims)).collect();
                let mut old = Vec::new();
                let mut new = Vec::new();
                for (i, v) in vectors.iter().enumerate() {
                    let (a, b) = (sequential(&query, v, similarity), prepared.score(v));
                    let distance = ulps(a, b);
                    assert!(distance <= 1, "dims {dims} {similarity:?}: {a} vs {b}");
                    off_by_one += distance as usize;
                    old.push((a, i));
                    new.push((b, i));
                }
                let rank = |scores: &mut Vec<(f32, usize)>| {
                    scores.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap().then(x.1.cmp(&y.1)));
                    scores.iter().map(|(_, i)| *i).collect::<Vec<_>>()
                };
                assert_eq!(rank(&mut old), rank(&mut new), "dims {dims} {similarity:?}");
            }
        }
        // Most scores are bit-identical; the bound is reached, not typical.
        assert!(off_by_one < 200, "{off_by_one} scores differ by one ulp");
    }

    /// The AVX2 kernel and the portable kernel return the same bits, so a
    /// score does not depend on the CPU that computed it.
    #[test]
    fn every_kernel_returns_the_same_bits() {
        let mut rng = Rng(42);
        for dims in [1usize, 5, 8, 17, 1536] {
            for similarity in METRICS {
                let query = rng.vector(dims);
                let portable = PreparedQuery::with_kernel(&query, similarity, false);
                let native = PreparedQuery::new(&query, similarity);
                for _ in 0..50 {
                    let v = rng.vector(dims);
                    let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
                    let expected = portable.score(&v).to_bits();
                    assert_eq!(native.score(&v).to_bits(), expected);
                    assert_eq!(
                        native.score_stored(&bytes).map(f32::to_bits),
                        Some(expected)
                    );
                    assert_eq!(
                        portable.score_stored(&bytes).map(f32::to_bits),
                        Some(expected)
                    );
                }
            }
        }
    }

    #[test]
    fn a_stored_vector_with_a_non_finite_component_or_wrong_length_is_rejected() {
        let query = [0.5f32, 0.0, -1.0, 2.0, 1.0, 1.0, 1.0, 1.0, 3.0];
        for similarity in METRICS {
            let prepared = PreparedQuery::new(&query, similarity);
            for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                for at in [1usize, 8] {
                    let mut v = vec![1.0f32; query.len()];
                    v[at] = bad;
                    let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
                    assert_eq!(prepared.score_stored(&bytes), None, "{similarity:?} {bad}");
                }
            }
            assert_eq!(prepared.score_stored(&[0u8; 8]), None);
        }
        // A zero vector scores 0 under cosine, as before.
        assert_eq!(
            PreparedQuery::new(&[0.0; 4], Similarity::Cosine)
                .score(&[1.0; 4])
                .to_bits(),
            0.0f32.to_bits()
        );
    }
}
