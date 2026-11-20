//! A space's vectors resident as `f32`, keyed by storage slot: a resident
//! layer (`rushdb_storage::resident`) per space.
//!
//! An arena holds every ready binding of one space at one snapshot, with
//! its vector and the vector's `sum v*v`, and stays valid until a commit
//! writes the space (a bind, an unbind, a vector, its version row). The
//! bindings are exactly the ones a scan of the space's binding rows at a
//! view the arena holds for returns, so an unfiltered exact search scores
//! them straight from here without reading storage. Other searches resolve
//! bindings through their view and take only the vector from here; a slot
//! the arena does not hold is read from storage.
//!
//! Scoring a resident row ([`crate::kernel::PreparedQuery::score_resident`])
//! is bit for bit the score of the stored bytes (`score_stored`): the same
//! kernel over the same lanes, with the row's norm computed once in the
//! lanes and tree a search would use.
//!
//! The rows are charged to a share of the vector cache budget
//! (`vector-cache-bytes`). When it cannot hold them there is no arena and
//! every search reads storage, as before; a search is never refused for it.

use std::collections::HashMap;

use rushdb_storage::resident::{Budget, BuildCx, BuildError, Dep, Resident};
use rushdb_storage::{ReadView, StorageError};
use rushdb_types::ids::TenantId;

use crate::segment::SegmentEntry;

/// Resident rows of one space, `dims` components each: row `i` is the
/// vector of `entries[i]`.
#[derive(PartialEq)]
pub(crate) struct VectorArena {
    dims: usize,
    /// Row of each slot held.
    rows: HashMap<u64, u32>,
    data: Vec<f32>,
    /// `sum v*v` of each row ([`crate::kernel::norm_sq`]).
    norms: Vec<f64>,
    /// The space's ready bindings, in slot order.
    entries: Vec<SegmentEntry>,
}

impl VectorArena {
    /// Bytes an arena of `count` rows of `dims` components takes: the rows,
    /// their norms and bindings, and the slot map.
    pub(crate) fn bytes(count: usize, dims: usize) -> usize {
        count.saturating_mul(
            dims.saturating_mul(std::mem::size_of::<f32>())
                .saturating_add(std::mem::size_of::<f64>())
                .saturating_add(std::mem::size_of::<SegmentEntry>())
                .saturating_add(32),
        )
    }

    /// An arena of `entries` with the vectors `read(slot)` returns. `None`
    /// when a slot has no stored vector of `dims` finite components (the
    /// arena would be partial in a way searches cannot tell from "bound
    /// later").
    pub(crate) fn build<E>(
        entries: Vec<SegmentEntry>,
        dims: usize,
        mut read: impl FnMut(u64, &mut Vec<f32>) -> Result<bool, E>,
    ) -> Result<Option<Self>, E> {
        let mut data = Vec::with_capacity(entries.len().saturating_mul(dims));
        let mut rows = HashMap::with_capacity(entries.len());
        let mut norms = Vec::with_capacity(entries.len());
        for (row, entry) in entries.iter().enumerate() {
            let Ok(row) = u32::try_from(row) else {
                return Ok(None);
            };
            let before = data.len();
            if !read(entry.slot, &mut data)? || data.len() != before + dims {
                return Ok(None);
            }
            norms.push(crate::kernel::norm_sq(&data[before..]));
            rows.insert(entry.slot, row);
        }
        Ok(Some(Self {
            dims,
            rows,
            data,
            norms,
            entries,
        }))
    }

    /// The score of `slot`'s row, when it is resident.
    pub(crate) fn score(&self, query: &crate::kernel::PreparedQuery<'_>, slot: u64) -> Option<f32> {
        Some(self.score_row(query, *self.rows.get(&slot)? as usize))
    }

    /// The score of row `row` (`row < self.len()`).
    pub(crate) fn score_row(&self, query: &crate::kernel::PreparedQuery<'_>, row: usize) -> f32 {
        query.score_resident(
            &self.data[row * self.dims..(row + 1) * self.dims],
            self.norms[row],
        )
    }

    /// The bindings held, row `i` being `entries()[i]`'s.
    pub(crate) fn entries(&self) -> &[SegmentEntry] {
        &self.entries
    }

    /// Rows held.
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}

/// The score of the vector at `slot`: from `arena`'s row when it holds the
/// slot (bit for bit the stored bytes' score), else from the stored bytes
/// `stored` scores (`None` when they are not a valid vector: corruption).
pub(crate) fn score_slot(
    arena: Option<&VectorArena>,
    query: &crate::kernel::PreparedQuery<'_>,
    slot: u64,
    stored: impl FnOnce() -> Result<Option<f32>, rushdb_storage::StorageError>,
) -> Result<f32, crate::VectorError> {
    match arena.and_then(|arena| arena.score(query, slot)) {
        Some(score) => Ok(score),
        None => Ok(stored()?.ok_or(rushdb_storage::StorageError::CorruptLog)?),
    }
}

/// `(tenant, space id, dims)`.
pub(crate) type ArenaKey = (TenantId, u64, usize);

/// The arenas of every space, one entry per space.
#[derive(Default)]
pub(crate) struct Arenas;

impl Resident for Arenas {
    type Key = ArenaKey;
    type Value = VectorArena;
    const NAME: &'static str = "vector-arena";
    const BUDGET: Budget = Budget::Vector;

    fn deps(
        &self,
        (tenant, space, _): &ArenaKey,
        _view: &ReadView,
    ) -> Result<Vec<Dep>, StorageError> {
        Ok(vec![Dep::Space(*tenant, *space)])
    }

    fn build(
        &self,
        key: &ArenaKey,
        view: &ReadView,
        cx: &BuildCx,
    ) -> Result<Option<VectorArena>, BuildError> {
        build_arena(view, key, cx).map_err(|error| match error {
            crate::VectorError::Storage(error) => BuildError::Storage(error),
            other => BuildError::Storage(StorageError::InvalidRead(other.to_string())),
        })?
    }

    fn bytes(arena: &VectorArena) -> usize {
        VectorArena::bytes(arena.len(), arena.dims)
    }
}

/// The rows of every ready binding of a space at `view`; `None` when a
/// stored vector is not `dims` finite components (corruption a search
/// reports itself). Refused before any vector is read when the rows cannot
/// fit the layer.
fn build_arena(
    view: &ReadView,
    &(tenant, space_id, dims): &ArenaKey,
    cx: &BuildCx,
) -> Result<Result<Option<VectorArena>, BuildError>, crate::VectorError> {
    let entries = crate::ann::load_ready_segment_entries(view, tenant, space_id)?;
    if let Err(refused) = cx.admit(VectorArena::bytes(entries.len(), dims)) {
        return Ok(Err(refused));
    }
    let arena = VectorArena::build(entries, dims, |slot, out| {
        let key = rushdb_types::keys::emb_vector_key(tenant, space_id, slot);
        view.with_value(&key, |raw| {
            let Some(raw) = raw.filter(|raw| raw.len() == dims * 4) else {
                return false;
            };
            let start = out.len();
            out.extend(
                raw.as_chunks::<4>()
                    .0
                    .iter()
                    .map(|chunk| f32::from_le_bytes(*chunk)),
            );
            out[start..].iter().all(|component| component.is_finite())
        })
        .map_err(crate::VectorError::from)
    })?;
    Ok(Ok(arena))
}

impl crate::VectorEngine {
    fn arena_key(&self, space: &crate::EmbeddingSpace) -> ArenaKey {
        (self.tenant(), space.space_id, space.dims as usize)
    }

    /// The resident rows of `space` valid at `snap`, when there are some.
    pub(crate) fn arena(
        &self,
        snap: &ReadView,
        space: &crate::EmbeddingSpace,
    ) -> Option<std::sync::Arc<VectorArena>> {
        snap.residents()
            .layer::<Arenas>()
            .get(&self.arena_key(space), snap)
    }

    /// A search of `space` at `snap` read its vectors from storage: once
    /// that pays for the space's rows the arena is made off the request
    /// (the first search pays: it reads as many rows as the arena holds).
    pub fn rent_arena(&self, snap: &ReadView, space: &crate::EmbeddingSpace) {
        let Ok(ready) = self.ready_vectors_in_view(snap, space) else {
            return;
        };
        if ready == 0 {
            return;
        }
        snap.residents()
            .layer::<Arenas>()
            .rent(&self.arena_key(space), snap, ready, ready);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rushdb_lmpg::{FieldChange, Lmpg};
    use rushdb_storage::Store;
    use rushdb_types::ids::OwnerKind;
    use rushdb_types::ids::TenantId;
    use serde_json::json;

    use crate::{Candidates, ScoredSource, SourceRef, VectorEngine};

    fn engine(vector_cache_bytes: usize) -> Arc<VectorEngine> {
        let config = rushdb_storage::resources::StorageResources {
            vector_cache_bytes,
            ..Default::default()
        };
        let store = Arc::new(Store::new_with_resources(config).unwrap());
        Arc::new(VectorEngine::new(Arc::new(Lmpg::new(
            store,
            TenantId::from_ordinal(75),
        ))))
    }

    /// `n` vectors of `dims` irregular components (every bit of the score
    /// matters), bound to `alias`; returns their owners.
    #[expect(
        clippy::cast_precision_loss,
        reason = "small integers mixed into a deterministic irregular vector"
    )]
    fn bind(engine: &VectorEngine, alias: &str, dims: u32, n: usize) -> Vec<u64> {
        engine.ensure_space(alias, dims).unwrap();
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        (0..n)
            .map(|i| {
                let root = engine
                    .lmpg
                    .create_record("Doc", None, vec![("i".into(), FieldChange::Set(json!(i)))])
                    .unwrap();
                let vector: Vec<f32> = (0..dims)
                    .map(|_| {
                        seed ^= seed << 13;
                        seed ^= seed >> 7;
                        seed ^= seed << 17;
                        (seed % 10_007) as f32 / 977.0 - 5.0
                    })
                    .collect();
                engine
                    .bind_external(
                        alias,
                        SourceRef {
                            kind: OwnerKind::Record,
                            owner_ord: root.ordinal,
                            pid: 0,
                            elem: 0,
                        },
                        &vector,
                    )
                    .unwrap();
                root.ordinal
            })
            .collect()
    }

    /// `(owner, score bits)` of each hit, in order.
    fn bits(hits: &[ScoredSource]) -> Vec<(u64, u32)> {
        hits.iter()
            .map(|hit| (hit.source.owner_ord, hit.score.to_bits()))
            .collect()
    }

    /// Reads one search does on this thread (and helpers it lent work to).
    fn reads<T>(search: impl FnOnce() -> T) -> (T, u64) {
        rushdb_storage::read_accounting::enable(true);
        let _ = rushdb_storage::read_accounting::take_local();
        let out = search();
        let reads = rushdb_storage::read_accounting::take_local().values().sum();
        (out, reads)
    }

    #[test]
    fn resident_rows_score_bit_for_bit_like_the_stored_bytes() {
        let engine = engine(256 * 1024 * 1024);
        let owners = bind(&engine, "docs", 24, 400);
        engine.build_ann("docs").unwrap();
        let snap = engine.snapshot().unwrap();
        let space = engine.lookup_space_in_view(&snap, "docs").unwrap().unwrap();
        let filter: roaring::RoaringTreemap = owners.iter().copied().step_by(7).collect();
        let queries: Vec<Vec<f32>> = (0..5u8)
            .map(|q| {
                (0..24u8)
                    .map(|d| f32::from(d % 5) - f32::from(q) * 0.37)
                    .collect()
            })
            .collect();
        let searches = |query: &[f32]| {
            let owners = Candidates::Owners {
                kind: OwnerKind::Record,
                ords: &filter,
            };
            [
                engine
                    .search_exact("docs", query, 10, Candidates::All)
                    .unwrap(),
                engine.search_exact("docs", query, 10, owners).unwrap(),
                engine
                    .search_ann("docs", query, 10, 160, Candidates::All)
                    .unwrap(),
                engine.search_ann("docs", query, 10, 160, owners).unwrap(),
            ]
        };
        let (from_storage, storage_reads) =
            reads(|| queries.iter().map(|q| searches(q)).collect::<Vec<_>>());

        engine.rent_arena(&snap, &space);
        engine.lmpg.store.residents().wait_idle();
        let on = rushdb_storage::resident::residents_enabled();
        let resident = engine.arena(&engine.snapshot().unwrap(), &space);
        assert_eq!(
            resident.as_ref().map(|arena| arena.len()),
            on.then_some(400)
        );
        let (from_arena, arena_reads) =
            reads(|| queries.iter().map(|q| searches(q)).collect::<Vec<_>>());

        for (stored, resident) in from_storage.iter().zip(&from_arena) {
            for (stored, resident) in stored.iter().zip(resident) {
                assert!(!stored.is_empty());
                assert_eq!(bits(stored), bits(resident));
            }
        }
        // The vectors came from memory: only bindings were read.
        assert_eq!(
            arena_reads < storage_reads,
            on,
            "{arena_reads} reads with the arena, {storage_reads} without"
        );
    }

    /// `vectors` bound to `alias` (a space of `similarity`), one record
    /// each; returns the records.
    fn bind_vectors(
        engine: &VectorEngine,
        alias: &str,
        similarity: rushdb_types::occurrence::Similarity,
        vectors: &[Vec<f32>],
    ) -> Vec<rushdb_lmpg::RecordRef> {
        let dims = u32::try_from(vectors[0].len()).unwrap();
        engine
            .ensure_space_similarity(alias, dims, similarity)
            .unwrap();
        vectors
            .iter()
            .enumerate()
            .map(|(i, vector)| {
                let record = engine
                    .lmpg
                    .create_record("Doc", None, vec![("i".into(), FieldChange::Set(json!(i)))])
                    .unwrap();
                engine
                    .bind_external(alias, source(record.ordinal), vector)
                    .unwrap();
                record
            })
            .collect()
    }

    fn source(owner_ord: u64) -> SourceRef {
        SourceRef {
            kind: OwnerKind::Record,
            owner_ord,
            pid: 0,
            elem: 0,
        }
    }

    /// Irregular vectors (every bit of a score matters); every fifth is a
    /// copy of the first, so a query equal to it ties across the top `k`.
    #[expect(clippy::cast_precision_loss, reason = "a test distribution")]
    fn vectors_with_ties(n: usize, dims: usize) -> Vec<Vec<f32>> {
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut out: Vec<Vec<f32>> = Vec::with_capacity(n);
        for i in 0..n {
            if i > 0 && i % 5 == 0 {
                out.push(out[0].clone());
                continue;
            }
            out.push(
                (0..dims)
                    .map(|_| {
                        seed ^= seed << 13;
                        seed ^= seed >> 7;
                        seed ^= seed << 17;
                        (seed % 10_007) as f32 / 977.0 - 5.0
                    })
                    .collect(),
            );
        }
        out
    }

    /// Exact search over a resident arena (its bindings and rows, no
    /// storage scan) returns the ids and score bits storage returns, for
    /// every metric: unfiltered, under a point check, an owner set larger
    /// than the space (scored from the arena's bindings) and a small one
    /// (read per owner), with ties at the `k`-th score.
    #[test]
    fn exact_search_from_resident_bindings_matches_storage_for_every_metric() {
        use rushdb_types::occurrence::Similarity;
        for similarity in [Similarity::Cosine, Similarity::Dot, Similarity::Euclidean] {
            let engine = engine(256 * 1024 * 1024);
            let vectors = vectors_with_ties(300, 24);
            let records = bind_vectors(&engine, "docs", similarity, &vectors);
            let snap = engine.snapshot().unwrap();
            let space = engine.lookup_space_in_view(&snap, "docs").unwrap().unwrap();
            let large: roaring::RoaringTreemap = records
                .iter()
                .map(|record| record.ordinal)
                .chain(1_000_000..1_000_400)
                .collect();
            let small: roaring::RoaringTreemap = records
                .iter()
                .map(|record| record.ordinal)
                .step_by(9)
                .collect();
            let rejects_thirds = |source: SourceRef| !source.owner_ord.is_multiple_of(3);
            let searches = || {
                let mut out = Vec::new();
                for query in [vectors[0].clone(), vectors[7].clone()] {
                    for k in [1usize, 10, 70] {
                        for candidates in [
                            Candidates::All,
                            Candidates::Check(&rejects_thirds),
                            Candidates::Owners {
                                kind: OwnerKind::Record,
                                ords: &large,
                            },
                            Candidates::Owners {
                                kind: OwnerKind::Record,
                                ords: &small,
                            },
                        ] {
                            let hits = engine.search_exact("docs", &query, k, candidates).unwrap();
                            assert!(!hits.is_empty());
                            out.push(bits(&hits));
                        }
                    }
                }
                out
            };
            let residents = engine.lmpg.store.residents();
            residents.set_enabled(false);
            let (from_storage, storage_reads) = reads(searches);
            residents.set_enabled(rushdb_storage::resident::residents_enabled());
            engine.rent_arena(&snap, &space);
            residents.wait_idle();
            let on = rushdb_storage::resident::residents_enabled();
            assert_eq!(engine.arena(&snap, &space).is_some(), on, "{similarity:?}");
            let (from_arena, arena_reads) = reads(searches);
            assert_eq!(from_storage, from_arena, "{similarity:?}");
            assert_eq!(
                arena_reads < storage_reads,
                on,
                "{similarity:?}: {arena_reads} reads with the arena, {storage_reads} without"
            );
        }
    }

    /// The arena reflects the space at its snapshot: a rebind, a deleted
    /// record's binding retired by revalidation and a new bind each write
    /// the space, so
    /// the next search misses it and answers like storage; a write to
    /// another space keeps it.
    #[test]
    fn writes_to_the_space_drop_the_arena_and_writes_elsewhere_keep_it() {
        let engine = engine(256 * 1024 * 1024);
        let vectors = vectors_with_ties(120, 16);
        let records = bind_vectors(
            &engine,
            "docs",
            rushdb_types::occurrence::Similarity::Cosine,
            &vectors,
        );
        bind_vectors(
            &engine,
            "other",
            rushdb_types::occurrence::Similarity::Cosine,
            &vectors[..3],
        );
        let space = engine
            .lookup_space_in_view(&engine.snapshot().unwrap(), "docs")
            .unwrap()
            .unwrap();
        let on = rushdb_storage::resident::residents_enabled();
        let residents = engine.lmpg.store.residents();
        // Every write to "docs" below changes the hits scoring 1.0 here.
        let query = vectors[2].clone();
        let answer = || {
            bits(
                &engine
                    .search_exact("docs", &query, 15, Candidates::All)
                    .unwrap(),
            )
        };
        let reference = || {
            residents.set_enabled(false);
            let out = answer();
            residents.set_enabled(on);
            out
        };
        let warm = || {
            let snap = engine.snapshot().unwrap();
            engine.rent_arena(&snap, &space);
            residents.wait_idle();
            assert_eq!(engine.arena(&snap, &space).is_some(), on);
        };
        let writes: [(&str, &dyn Fn()); 4] = [
            ("rebind", &|| {
                engine
                    .bind_external("docs", source(records[1].ordinal), &vectors[2])
                    .unwrap();
            }),
            ("delete record", &|| {
                // Revalidation retires the deleted owner's binding.
                engine.lmpg.delete_record(records[2]).unwrap();
                engine.revalidate_bindings("docs").unwrap();
            }),
            ("bind", &|| {
                bind_vectors(
                    &engine,
                    "docs",
                    rushdb_types::occurrence::Similarity::Cosine,
                    &vectors[2..3],
                );
            }),
            ("bind elsewhere", &|| {
                engine
                    .bind_external("other", source(records[0].ordinal), &vectors[4])
                    .unwrap();
            }),
        ];
        for (name, write) in writes {
            warm();
            let before = answer();
            write();
            let kept = engine.arena(&engine.snapshot().unwrap(), &space).is_some();
            assert_eq!(kept, on && name == "bind elsewhere", "{name}");
            let after = answer();
            assert_eq!(after, reference(), "{name}");
            assert_eq!(after != before, name != "bind elsewhere", "{name}");
            warm();
            assert_eq!(answer(), after, "{name}: rebuilt arena");
        }
    }

    fn arena_builds(engine: &VectorEngine) -> u64 {
        engine
            .lmpg
            .store
            .residents()
            .snapshots()
            .into_iter()
            .find(|layer| layer.layer == "vector-arena")
            .map_or(0, |layer| layer.builds)
    }

    #[test]
    fn searches_of_a_resident_arena_build_it_once() {
        let engine = engine(256 * 1024 * 1024);
        bind(&engine, "docs", 16, 200);
        let snap = engine.snapshot().unwrap();
        let space = engine.lookup_space_in_view(&snap, "docs").unwrap().unwrap();
        // Every search rents the space (as `run_vector_search` does).
        for _ in 0..10 {
            engine.rent_arena(&snap, &space);
            engine.lmpg.store.residents().wait_idle();
        }
        let on = rushdb_storage::resident::residents_enabled();
        assert_eq!(arena_builds(&engine), u64::from(on));
        assert_eq!(engine.arena(&snap, &space).is_some(), on);
        // A bind writes the space: the arena is made again, once.
        bind(&engine, "docs", 16, 1);
        let now = engine.snapshot().unwrap();
        for _ in 0..10 {
            engine.rent_arena(&now, &space);
            engine.lmpg.store.residents().wait_idle();
        }
        assert_eq!(arena_builds(&engine), 2 * u64::from(on));
        assert_eq!(
            engine.arena(&now, &space).map(|arena| arena.len()),
            on.then_some(201)
        );
    }

    #[test]
    fn a_space_larger_than_the_vector_budget_answers_from_storage() {
        // 300 rows of 64 components do not fit 32 KiB.
        let engine = engine(32 * 1024);
        bind(&engine, "docs", 64, 300);
        let snap = engine.snapshot().unwrap();
        let space = engine.lookup_space_in_view(&snap, "docs").unwrap().unwrap();
        engine.rent_arena(&snap, &space);
        engine.lmpg.store.residents().wait_idle();
        assert!(engine.arena(&snap, &space).is_none());
        let refused = engine
            .lmpg
            .store
            .residents()
            .snapshots()
            .into_iter()
            .find(|layer| layer.layer == "vector-arena")
            .map_or(0, |layer| layer.build_refused);
        assert_eq!(
            refused,
            u64::from(rushdb_storage::resident::residents_enabled())
        );
        // Never refused: exact search reads the stored vectors.
        let query = vec![0.5f32; 64];
        let hits = engine
            .search_exact("docs", &query, 5, Candidates::All)
            .unwrap();
        assert_eq!(hits.len(), 5);
    }
}
