//! Exact search: score every candidate binding and keep the top `k`.
//!
//! Hits are ranked by a total order — score descending, then the canonical
//! source — so the top `k` is one well-defined set whatever order the
//! candidates are scored in. That is what lets a search split its bindings
//! into parts, score the parts on the shared thread budget
//! ([`rushdb_storage::parallel`]), and merge them into exactly the result a
//! single thread produces. Parts have a fixed size (never a function of the
//! threads lent), and every bound a part prunes with is fixed before the
//! part starts, so the work (and the reads) do not depend on timing either.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use rushdb_storage::{ReadView, ScanBounds, ScanProjection, StorageError};
use rushdb_types::keys::{
    emb_bindings_prefix, emb_owner_bindings_prefix, emb_vector_key, parse_emb_binding_key,
};
use rushdb_types::occurrence::{BindingStatus, BindingValue};

use crate::kernel::PreparedQuery;
use crate::{
    source_order, Candidates, EmbeddingSpace, ScoredSource, SourceRef, VectorEngine, VectorError,
};

/// Rows and bytes per batch when exact search streams an index's bindings.
const SCAN_BATCH_ROWS: usize = 1024;
const SCAN_BATCH_BYTES: usize = 4 * 1024 * 1024;
/// Candidate vectors one part scores.
pub(crate) const PART_VECTORS: usize = 2048;
/// Ready bindings a full scan collects before scoring them in parts: the
/// memory of one scan stays bounded by this, not by the index size.
pub(crate) const SCAN_CHUNK: usize = 32 * PART_VECTORS;
/// Candidate owners one part of an owner-set search reads.
pub(crate) const PART_OWNERS: usize = 1024;

/// One ready binding to score.
#[derive(Clone, Copy)]
struct Candidate {
    slot: u64,
    source: SourceRef,
    value_hash: u64,
}

/// A hit under the ranking order: greater is better.
struct Ranked(ScoredSource);

impl Ranked {
    fn order(score: f32, source: SourceRef, other_score: f32, other: SourceRef) -> Ordering {
        score
            .partial_cmp(&other_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| source_order(other).cmp(&source_order(source)))
    }
}

impl PartialEq for Ranked {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Ranked {}
impl PartialOrd for Ranked {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Ranked {
    fn cmp(&self, other: &Self) -> Ordering {
        Self::order(self.0.score, self.0.source, other.0.score, other.0.source)
    }
}

/// The best `k` hits seen so far; its top is the worst kept hit.
pub(crate) struct TopK {
    k: usize,
    heap: BinaryHeap<std::cmp::Reverse<Ranked>>,
}

/// The worst hit a full global top `k` holds, fixed when a batch of parts
/// starts: a candidate not above it cannot reach the final result.
#[derive(Clone, Copy)]
struct Bar {
    score: f32,
    source: SourceRef,
}

impl TopK {
    pub(crate) fn new(k: usize) -> Self {
        Self {
            k,
            heap: BinaryHeap::with_capacity(k.saturating_add(1).min(1 << 16)),
        }
    }

    /// Whether a hit of `score` from `source` would be kept now.
    fn admits(&self, score: f32, source: SourceRef) -> bool {
        if self.heap.len() < self.k {
            return true;
        }
        self.heap.peek().is_some_and(|worst| {
            Ranked::order(score, source, worst.0 .0.score, worst.0 .0.source) == Ordering::Greater
        })
    }

    fn push(&mut self, hit: ScoredSource) {
        if !self.admits(hit.score, hit.source) {
            return;
        }
        self.heap.push(std::cmp::Reverse(Ranked(hit)));
        if self.heap.len() > self.k {
            self.heap.pop();
        }
    }

    fn bar(&self) -> Option<Bar> {
        (self.heap.len() >= self.k)
            .then(|| self.heap.peek())
            .flatten()
            .map(|worst| Bar {
                score: worst.0 .0.score,
                source: worst.0 .0.source,
            })
    }

    fn merge(&mut self, other: TopK) {
        for hit in other.heap {
            self.push(hit.0 .0);
        }
    }

    /// The kept hits, best first.
    pub(crate) fn into_sorted(self) -> Vec<ScoredSource> {
        // `into_sorted_vec` ascends over `Reverse`: best first.
        self.heap
            .into_sorted_vec()
            .into_iter()
            .map(|hit| hit.0 .0)
            .collect()
    }
}

/// Everything one exact search shares across its parts.
struct Scorer<'a> {
    engine: &'a VectorEngine,
    snap: &'a ReadView,
    space: &'a EmbeddingSpace,
    query: PreparedQuery<'a>,
    k: usize,
    candidates: Candidates<'a>,
    min_score: Option<f32>,
}

impl Scorer<'_> {
    /// Score one candidate into `top`: the vector is scored straight from
    /// its stored bytes, and a point check (`Candidates::Check`) runs only
    /// for a hit that would be kept — the result is the same as checking
    /// first, since a hit that is not kept never enters any top `k`.
    fn score(
        &self,
        candidate: Candidate,
        bar: Option<Bar>,
        top: &mut TopK,
    ) -> Result<(), VectorError> {
        // Scored in place from the stored bytes: no per-candidate copy.
        let score = self
            .snap
            .with_value(
                &emb_vector_key(self.engine.tenant(), self.space.space_id, candidate.slot),
                |raw| raw.and_then(|raw| self.query.score_stored(raw)),
            )?
            .ok_or(StorageError::CorruptLog)?;
        if self.min_score.is_some_and(|min| score < min)
            || bar.is_some_and(|bar| {
                Ranked::order(score, candidate.source, bar.score, bar.source) != Ordering::Greater
            })
            || !top.admits(score, candidate.source)
        {
            return Ok(());
        }
        if let Candidates::Check(check) = self.candidates {
            if !check(candidate.source) {
                return Ok(());
            }
        }
        top.push(ScoredSource {
            source: candidate.source,
            score,
            value_hash: candidate.value_hash,
        });
        Ok(())
    }

    /// Score `chunk` in fixed-size parts on the shared budget and merge
    /// them in part order into `top`.
    fn score_chunk(&self, chunk: &[Candidate], top: &mut TopK) -> Result<(), VectorError> {
        let bar = top.bar();
        let parts = chunk.len().div_ceil(PART_VECTORS);
        let scored = rushdb_storage::parallel::run_parts(parts, |part| {
            let mut local = TopK::new(self.k);
            let end = ((part + 1) * PART_VECTORS).min(chunk.len());
            for candidate in &chunk[part * PART_VECTORS..end] {
                self.score(*candidate, bar, &mut local)?;
            }
            Ok::<_, VectorError>(local)
        })?;
        for local in scored {
            top.merge(local);
        }
        Ok(())
    }

    /// Stream every binding under `prefix` into `visit`, decoding only the
    /// ready ones an owner set (if any) admits by key.
    fn scan(
        &self,
        prefix: &[u8],
        visit: &mut dyn FnMut(Candidate) -> Result<(), VectorError>,
    ) -> Result<(), VectorError> {
        let mut cursor = self
            .snap
            .scan(ScanBounds::prefix(prefix), ScanProjection::KeyValue)?;
        loop {
            let batch = cursor.next_batch(SCAN_BATCH_ROWS, SCAN_BATCH_BYTES)?;
            for row in batch.rows {
                let (_, kind, owner_ord, pid, elem) =
                    parse_emb_binding_key(&row.key).ok_or(StorageError::CorruptLog)?;
                if let Candidates::Owners { kind: wanted, ords } = self.candidates {
                    if kind != wanted || !ords.contains(owner_ord) {
                        continue;
                    }
                }
                let value = BindingValue::decode(row.value.as_deref().unwrap_or_default())
                    .ok_or(StorageError::CorruptLog)?;
                if value.status != BindingStatus::Ready {
                    continue;
                }
                visit(Candidate {
                    slot: value.vector_slot,
                    source: SourceRef {
                        kind,
                        owner_ord,
                        pid,
                        elem,
                    },
                    value_hash: value.source_hash,
                })?;
            }
            if batch.done {
                return Ok(());
            }
        }
    }
}

impl VectorEngine {
    /// Exact top-`k` search among `candidates`, keeping only hits scoring
    /// at least `min_score`; a lower hit never enters the top `k`.
    ///
    /// An owner set no larger than the space is answered by reading each
    /// candidate owner's bindings (one prefix seek per owner), so the cost
    /// follows the candidate count, not the space size; the owners are
    /// split into parts of [`PART_OWNERS`]. Every other search streams the
    /// space's bindings once and scores them in parts of [`PART_VECTORS`].
    pub(crate) fn exact_in_view(
        &self,
        snap: &ReadView,
        space: &EmbeddingSpace,
        query: &[f32],
        k: usize,
        candidates: Candidates<'_>,
        min_score: Option<f32>,
    ) -> Result<Vec<ScoredSource>, VectorError> {
        self.validate_view(snap)?;
        let t = self.tenant();
        if query.len() != space.dims as usize {
            return Err(VectorError::DimMismatch(space.dims, query.len()));
        }
        if query.iter().any(|v| !v.is_finite()) {
            return Err(VectorError::InvalidVector(
                "query components must be finite".into(),
            ));
        }
        if k == 0 {
            return Ok(Vec::new());
        }
        let scorer = Scorer {
            engine: self,
            snap,
            space,
            query: PreparedQuery::new(query, space.similarity),
            k,
            candidates,
            min_score,
        };
        let mut top = TopK::new(k);
        match candidates {
            Candidates::Owners { kind, ords }
                if ords.len() <= self.ready_vectors_in_view(snap, space)? =>
            {
                // At most the space's ready vectors, 8 bytes each.
                let owners: Vec<u64> = ords.iter().collect();
                let parts = owners.len().div_ceil(PART_OWNERS);
                let tops = rushdb_storage::parallel::run_parts(parts, |part| {
                    let mut local = TopK::new(k);
                    let end = ((part + 1) * PART_OWNERS).min(owners.len());
                    for owner_ord in &owners[part * PART_OWNERS..end] {
                        let prefix = emb_owner_bindings_prefix(t, space.space_id, kind, *owner_ord);
                        scorer.scan(&prefix, &mut |candidate| {
                            scorer.score(candidate, None, &mut local)
                        })?;
                    }
                    Ok::<_, VectorError>(local)
                })?;
                for local in tops {
                    top.merge(local);
                }
            }
            _ => {
                let mut chunk = Vec::with_capacity(SCAN_CHUNK);
                scorer.scan(&emb_bindings_prefix(t, space.space_id), &mut |candidate| {
                    chunk.push(candidate);
                    if chunk.len() == SCAN_CHUNK {
                        scorer.score_chunk(&chunk, &mut top)?;
                        chunk.clear();
                    }
                    Ok(())
                })?;
                scorer.score_chunk(&chunk, &mut top)?;
            }
        }
        Ok(top.into_sorted())
    }
}
