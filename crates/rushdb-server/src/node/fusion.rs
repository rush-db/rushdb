//! How a hybrid search combines its legs' rankings into one.
//!
//! - **RRF** (reciprocal rank fusion): `Σ weight / (rrfK + rank + 1)`, ranks
//!   from 0. Only ranks cross legs, so their scales never matter.
//! - **DBSF** (distribution-based score fusion, as Qdrant defines it): each
//!   leg's scores are normalized by that leg's own distribution,
//!   `(s − (μ − 3σ)) / 6σ` clamped to `[0, 1]`, then `Σ weight × normalized`.
//!   A leg whose scores do not spread (one hit, or all equal) normalizes them
//!   to 0.5.
//!
//! Equal fused scores are ordered by record, so pages are stable.

use rushdb_protocol::Fusion;

/// One leg's ranking, best first: `(record ordinal, raw score)`.
pub(super) struct Leg<'a> {
    /// `vector` or `lexical`, reported in provenance.
    pub name: &'static str,
    pub weight: f64,
    pub hits: &'a [(u64, f64)],
}

/// One fused record.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Fused {
    pub owner_ord: u64,
    pub score: f64,
    /// `(leg, rank)` for every leg that returned the record.
    pub provenance: Vec<(&'static str, usize)>,
}

/// The best `top_n` records of `legs` combined by `method`.
pub(super) fn fuse(legs: &[Leg<'_>], method: Fusion, rrf_k: f64, top_n: usize) -> Vec<Fused> {
    let mut fused: std::collections::BTreeMap<u64, Fused> = std::collections::BTreeMap::new();
    for leg in legs {
        let normalize = match method {
            Fusion::Rrf => None,
            Fusion::Dbsf => Some(distribution(leg.hits)),
        };
        for (rank, &(owner_ord, score)) in leg.hits.iter().enumerate() {
            let contribution = match normalize {
                None => leg.weight / (rrf_k + rank_f64(rank) + 1.0),
                Some((low, spread)) => {
                    let normalized = if spread > 0.0 {
                        ((score - low) / spread).clamp(0.0, 1.0)
                    } else {
                        0.5
                    };
                    leg.weight * normalized
                }
            };
            let entry = fused.entry(owner_ord).or_insert(Fused {
                owner_ord,
                score: 0.0,
                provenance: Vec::new(),
            });
            entry.score += contribution;
            entry.provenance.push((leg.name, rank));
        }
    }
    let mut out: Vec<Fused> = fused.into_values().collect();
    out.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(a.owner_ord.cmp(&b.owner_ord))
    });
    out.truncate(top_n);
    out
}

/// `(μ − 3σ, 6σ)` of a leg's scores: the low end and width DBSF maps to
/// `[0, 1]`.
fn distribution(hits: &[(u64, f64)]) -> (f64, f64) {
    if hits.is_empty() {
        return (0.0, 0.0);
    }
    let count = rank_f64(hits.len());
    let mean = hits.iter().map(|(_, score)| score).sum::<f64>() / count;
    let variance = hits
        .iter()
        .map(|(_, score)| (score - mean).powi(2))
        .sum::<f64>()
        / count;
    let deviation = variance.sqrt();
    (mean - 3.0 * deviation, 6.0 * deviation)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "ranks and hit counts are bounded by the prefetch limit, far below f64's exact-integer range"
)]
fn rank_f64(rank: usize) -> f64 {
    rank as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leg<'a>(name: &'static str, weight: f64, hits: &'a [(u64, f64)]) -> Leg<'a> {
        Leg { name, weight, hits }
    }

    fn order(fused: &[Fused]) -> Vec<u64> {
        fused.iter().map(|hit| hit.owner_ord).collect()
    }

    #[test]
    fn rrf_favours_agreement_and_is_deterministic() {
        let vector = [(7, 0.9), (3, 0.8)];
        let lexical = [(4, 12.0), (3, 9.0)];
        let legs = [leg("vector", 1.0, &vector), leg("lexical", 1.0, &lexical)];
        let fused = fuse(&legs, Fusion::Rrf, 60.0, 10);
        assert_eq!(fused[0].owner_ord, 3, "owner 3 is in both rankings");
        assert_eq!(fused[0].provenance, vec![("vector", 1), ("lexical", 1)]);
        assert!((fused[0].score - 2.0 / 62.0).abs() < 1e-12);
        // 7 and 4 tie at rank 0 of one leg: the record breaks the tie.
        assert_eq!(order(&fused), vec![3, 4, 7]);
        assert_eq!(fused, fuse(&legs, Fusion::Rrf, 60.0, 10));
    }

    #[test]
    fn weights_scale_each_legs_share() {
        let vector = [(1, 0.9), (2, 0.5)];
        let lexical = [(2, 8.0), (1, 1.0)];
        for method in [Fusion::Rrf, Fusion::Dbsf] {
            let favour_text = fuse(
                &[leg("vector", 0.2, &vector), leg("lexical", 1.0, &lexical)],
                method,
                60.0,
                10,
            );
            assert_eq!(order(&favour_text), vec![2, 1], "{method:?}");
            let text_off = fuse(
                &[leg("vector", 1.0, &vector), leg("lexical", 0.0, &lexical)],
                method,
                60.0,
                10,
            );
            assert_eq!(
                order(&text_off),
                vec![1, 2],
                "{method:?}: the vector order alone"
            );
        }
        let weighted = fuse(&[leg("vector", 2.0, &vector)], Fusion::Rrf, 60.0, 10);
        assert!((weighted[0].score - 2.0 / 61.0).abs() < 1e-12);
    }

    #[test]
    fn dbsf_maps_each_leg_by_its_own_distribution() {
        // Scores 1, 2, 3: μ = 2, σ = √(2/3); the range [μ − 3σ, μ + 3σ] maps
        // to [0, 1], so 2 lands on 0.5 and 3, one above the mean, on
        // 0.5 + 1 / 6σ.
        let hits = [(10, 3.0), (11, 2.0), (12, 1.0)];
        let fused = fuse(&[leg("lexical", 1.0, &hits)], Fusion::Dbsf, 60.0, 10);
        let score = |ord| fused.iter().find(|hit| hit.owner_ord == ord).unwrap().score;
        let step = 1.0 / (6.0 * (2.0_f64 / 3.0).sqrt());
        assert!((score(11) - 0.5).abs() < 1e-12);
        assert!((score(10) - (0.5 + step)).abs() < 1e-12);
        assert!((score(12) - (0.5 - step)).abs() < 1e-12);

        // Scale does not matter: BM25-sized and cosine-sized scores with
        // the same shape normalize alike.
        let scaled = [(10, 300.0), (11, 200.0), (12, 100.0)];
        let again = fuse(&[leg("lexical", 1.0, &scaled)], Fusion::Dbsf, 60.0, 10);
        for (a, b) in fused.iter().zip(&again) {
            assert!((a.score - b.score).abs() < 1e-12);
        }

        // No spread: every score is the middle.
        let flat = fuse(
            &[leg("vector", 1.0, &[(1, 0.7), (2, 0.7)])],
            Fusion::Dbsf,
            60.0,
            10,
        );
        assert!(flat.iter().all(|hit| (hit.score - 0.5).abs() < 1e-12));
        let single = fuse(&[leg("vector", 1.0, &[(1, 0.3)])], Fusion::Dbsf, 60.0, 10);
        assert!((single[0].score - 0.5).abs() < 1e-12);
    }

    #[test]
    fn dbsf_clamps_outliers_and_sums_legs() {
        // One outlier among 20 equal scores lies beyond μ + 3σ.
        let mut hits: Vec<(u64, f64)> = (0..20).map(|ord| (ord, 1.0)).collect();
        hits.insert(0, (99, 1_000.0));
        let fused = fuse(&[leg("lexical", 1.0, &hits)], Fusion::Dbsf, 60.0, 30);
        assert_eq!(fused[0].owner_ord, 99);
        assert!((fused[0].score - 1.0).abs() < 1e-12, "clamped to 1");

        let vector = [(1, 0.9), (2, 0.1)];
        let lexical = [(1, 5.0), (2, 1.0)];
        let both = fuse(
            &[leg("vector", 1.0, &vector), leg("lexical", 0.5, &lexical)],
            Fusion::Dbsf,
            60.0,
            10,
        );
        // Two points sit at μ ± σ: normalized 0.5 ± 1/6 in each leg.
        assert!((both[0].score - 1.5 * (0.5 + 1.0 / 6.0)).abs() < 1e-12);
        assert!(both.iter().all(|hit| hit.score <= 1.5));
    }
}
