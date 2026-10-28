//! Structural embeddings: one vector per record of a label, derived from the
//! graph around it, so records in similar positions (same community, same
//! neighbourhood shape) are near each other in a vector search.
//!
//! The model is `FastRP` (Chen et al., "Fast and Accurate Network Embeddings
//! via Very Sparse Random Projection", 2019), the algorithm Neo4j GDS ships:
//! every record gets a very sparse random vector `R`; one hop averages the
//! neighbours' `R` (`X1`), two hops average the neighbours' normalized `X1`
//! (`X2`); the embedding is `w0·R + X1 + X2`, normalized. Relationships of
//! every type count, in both directions. It is a derived, rebuildable
//! computation over the canonical graph, labeled by [`STRUCTURAL_MODEL`]:
//! the vectors are model output, not facts.
//!
//! Deterministic: `R` is a hash of the record ordinal and a fixed seed, and
//! every sum runs in ascending ordinal order, so the same graph at the same
//! commit yields bit-identical vectors.

use std::collections::{BTreeMap, BTreeSet};

use rushdb_lmpg::reader::try_access as access;
use rushdb_lmpg::reader::TryRead;
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::index::{IndexDef, IndexKind};

use crate::{SourceRef, VectorEngine, VectorError};

/// Name and version of the derivation, recorded with every structural index.
pub const STRUCTURAL_MODEL: &str = "fastrp-v1";

/// Fixed seed of the random projection.
const SEED: u64 = 0x5eed_fa57_0000_0001;
/// Weight of a record's own random vector (`FastRP`'s node self-influence),
/// so a record without relationships still has a direction of its own.
const SELF_WEIGHT: f32 = 0.1;
/// Records one applier step embeds.
const APPLY_BATCH: usize = 1_024;

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// The very sparse random vector of record `ord` (sparsity 3: each component
/// is `+√3` or `-√3` with probability 1/6 each, else 0), normalized. Never
/// zero: an all-zero draw sets one component.
fn initial(ord: u64, dims: usize) -> Vec<f32> {
    let scale = 3f32.sqrt();
    let mut out: Vec<f32> = (0..dims as u64)
        .map(|component| {
            match splitmix(SEED ^ ord.wrapping_mul(0x2545_f491_4f6c_dd1d) ^ component) % 6 {
                0 => scale,
                1 => -scale,
                _ => 0.0,
            }
        })
        .collect();
    if out.iter().all(|value| *value == 0.0) {
        let pick = usize::try_from(splitmix(SEED ^ ord) % dims as u64).unwrap_or(0);
        out[pick] = scale;
    }
    normalize(&mut out);
    out
}

fn normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in vector.iter_mut() {
            *value /= norm;
        }
    }
}

/// Records adjacent to `ord` in either direction, ascending, each once.
fn neighbours<R: TryRead + ?Sized>(r: &R, t: TenantId, ord: u64) -> Result<Vec<u64>, VectorError> {
    let mut out: Vec<u64> = access::adj_out(r, t, ord, None)?
        .into_iter()
        .chain(access::adj_in(r, t, ord, None)?)
        .map(|(_, other, _)| other)
        .collect();
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

/// Mean of `rows`, normalized; zero when there are none.
fn mean_normalized<'a>(dims: usize, rows: impl Iterator<Item = &'a Vec<f32>>) -> Vec<f32> {
    let mut sum = vec![0.0f32; dims];
    let mut count = 0usize;
    for row in rows {
        for (acc, value) in sum.iter_mut().zip(row) {
            *acc += value;
        }
        count += 1;
    }
    if count > 0 {
        normalize(&mut sum);
    }
    sum
}

/// Bytes [`embed`] holds per record it keeps a vector for.
fn bytes_per_vector(dims: usize) -> usize {
    dims.saturating_mul(std::mem::size_of::<f32>())
        .saturating_add(64)
}

/// The structural embedding of each record in `targets` (ascending
/// ordinals), at `view`. The working set (one-hop vectors of every record
/// within reach) is reserved from `budget` before it is computed: over
/// budget is an [`VectorError::AnnBudget`] refusal.
///
/// # Errors
/// Returns [`VectorError::Storage`] when a read fails and
/// [`VectorError::AnnBudget`] when the working set does not fit.
pub fn embed<R: TryRead + ?Sized>(
    view: &R,
    tenant: TenantId,
    targets: &[u64],
    dims: usize,
    budget: &rushdb_storage::resources::ByteBudget,
) -> Result<Vec<(u64, Vec<f32>)>, VectorError> {
    let mut adjacency: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for target in targets {
        adjacency.insert(*target, neighbours(view, tenant, *target)?);
    }
    // Every record whose one-hop vector a target reads: the targets and
    // their neighbours.
    let reach: BTreeSet<u64> = adjacency
        .iter()
        .flat_map(|(target, around)| std::iter::once(*target).chain(around.iter().copied()))
        .collect();
    let _working_set = budget
        .try_reserve(reach.len().saturating_mul(bytes_per_vector(dims)))
        .map_err(|error| VectorError::AnnBudget(error.to_string()))?;
    let mut first_hop: BTreeMap<u64, Vec<f32>> = BTreeMap::new();
    for ord in &reach {
        let around = match adjacency.get(ord) {
            Some(around) => around.clone(),
            None => neighbours(view, tenant, *ord)?,
        };
        let initials: Vec<Vec<f32>> = around.iter().map(|other| initial(*other, dims)).collect();
        first_hop.insert(*ord, mean_normalized(dims, initials.iter()));
    }
    Ok(targets
        .iter()
        .map(|target| {
            let around = &adjacency[target];
            let second_hop = mean_normalized(dims, around.iter().map(|other| &first_hop[other]));
            let own = initial(*target, dims);
            let mut embedding: Vec<f32> = own
                .iter()
                .zip(&first_hop[target])
                .zip(&second_hop)
                .map(|((r, x1), x2)| SELF_WEIGHT * r + x1 + x2)
                .collect();
            normalize(&mut embedding);
            (*target, embedding)
        })
        .collect())
}

impl VectorEngine {
    /// One step of the structural applier for index `def`: embed up to
    /// [`APPLY_BATCH`] pending records at the current snapshot, bind their
    /// vectors into the space named after the index, retire the bindings of
    /// records that left it, and clear their pending rows (a record marked
    /// again after the snapshot stays pending). Returns the records
    /// processed; `0` when nothing is pending.
    ///
    /// # Errors
    /// Returns [`VectorError::AnnBudget`] when the vector budget cannot hold
    /// the working set (nothing is bound or cleared), and storage or commit
    /// errors otherwise.
    pub fn apply_structural(&self, def: &IndexDef) -> Result<usize, VectorError> {
        let IndexKind::Structural { dimensions } = def.kind else {
            return Ok(0);
        };
        let snap = self.snapshot()?;
        let tenant = self.tenant();
        let owners = rushdb_lmpg::index::pending(&snap, tenant, def.id, APPLY_BATCH)?;
        if owners.is_empty() {
            return Ok(0);
        }
        if self.lookup_space_in_view(&snap, &def.name)?.is_none() {
            self.ensure_space_similarity(
                &def.name,
                dimensions,
                rushdb_types::occurrence::Similarity::Cosine,
            )?;
        }
        let mut members = Vec::with_capacity(owners.len());
        for owner in &owners {
            if access::record_by_ord(&snap, tenant, *owner)?
                .is_some_and(|header| header.label == def.label)
            {
                members.push(*owner);
            }
        }
        let budget = self.store.vector_cache_budget().ok_or_else(|| {
            VectorError::AnnBudget(
                "no logical cache budget (resources.logical-cache-bytes: 0)".into(),
            )
        })?;
        let vectors = embed(&snap, tenant, &members, dimensions as usize, &budget)?;
        self.bind_external_many(
            &def.name,
            vectors
                .into_iter()
                .map(|(owner, vector)| (structural_source(owner), vector))
                .collect(),
        )?;
        if members.len() < owners.len() {
            // Deleted records: their bindings retire.
            self.revalidate_bindings(&def.name)?;
        }
        let processed = owners.len();
        self.lmpg
            .index_applied(def.id, snap.commit_index(), owners)
            .map_err(|error| VectorError::Engine(error.to_reason()))?;
        Ok(processed)
    }
}

/// The binding source of a record's structural vector.
#[must_use]
pub fn structural_source(owner_ord: u64) -> SourceRef {
    SourceRef {
        kind: OwnerKind::Record,
        owner_ord,
        pid: 0,
        elem: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rushdb_lmpg::{Lmpg, RecordRef};
    use rushdb_storage::Store;
    use std::sync::Arc;

    /// Two 6-record cliques (`A`, `B`) joined by one relationship, plus an
    /// isolated record; returns the graph and the record refs per group.
    fn communities() -> (Lmpg, Vec<RecordRef>, Vec<RecordRef>, RecordRef) {
        let graph = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(5));
        let group = |graph: &Lmpg| -> Vec<RecordRef> {
            let members: Vec<RecordRef> = (0..6)
                .map(|_| graph.create_record("Node", None, vec![]).unwrap())
                .collect();
            for (i, a) in members.iter().enumerate() {
                for b in &members[i + 1..] {
                    graph.create_relationship("LINK", *a, *b, vec![]).unwrap();
                }
            }
            members
        };
        let a = group(&graph);
        let b = group(&graph);
        graph
            .create_relationship("LINK", a[0], b[0], vec![])
            .unwrap();
        let alone = graph.create_record("Node", None, vec![]).unwrap();
        (graph, a, b, alone)
    }

    fn budget() -> rushdb_storage::resources::ByteBudget {
        rushdb_storage::resources::ByteBudget::new("test", 64 * 1024 * 1024).unwrap()
    }

    fn cosine(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b).map(|(x, y)| x * y).sum()
    }

    #[test]
    fn embeddings_are_deterministic_and_separate_communities() {
        let (graph, a, b, alone) = communities();
        let view = graph.store.read_view().unwrap();
        let mut targets: Vec<u64> = a.iter().chain(&b).map(|r| r.ordinal).collect();
        targets.push(alone.ordinal);
        targets.sort_unstable();
        let first = embed(&view, graph.tenant, &targets, 64, &budget()).unwrap();
        let again = embed(&view, graph.tenant, &targets, 64, &budget()).unwrap();
        assert_eq!(first, again, "same graph, same commit: identical vectors");

        let vector = |ord: u64| first.iter().find(|(o, _)| *o == ord).unwrap().1.clone();
        for v in first.iter().map(|(_, v)| v) {
            let norm = cosine(v, v).sqrt();
            assert!((norm - 1.0).abs() < 1e-4, "unit length");
        }
        // Members of one clique are closer to each other than to the other.
        let within = cosine(&vector(a[1].ordinal), &vector(a[2].ordinal));
        let across = cosine(&vector(a[1].ordinal), &vector(b[2].ordinal));
        assert!(within > across + 0.3, "within {within} vs across {across}");
        // A record without relationships keeps its own random direction.
        let isolated = vector(alone.ordinal);
        assert!(isolated.iter().any(|v| *v != 0.0));
    }

    #[test]
    fn a_working_set_over_budget_is_refused() {
        let (graph, a, _, _) = communities();
        let view = graph.store.read_view().unwrap();
        let tiny = rushdb_storage::resources::ByteBudget::new("tiny", 256).unwrap();
        let targets: Vec<u64> = a.iter().map(|r| r.ordinal).collect();
        assert!(matches!(
            embed(&view, graph.tenant, &targets, 64, &tiny),
            Err(VectorError::AnnBudget(_))
        ));
        assert_eq!(tiny.snapshot().used, 0, "nothing stays reserved");
    }
}
