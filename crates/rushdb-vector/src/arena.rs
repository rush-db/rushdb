//! A space's vectors resident as `f32`, keyed by storage slot.
//!
//! A vector at `(space, slot)` is written once: a rebind takes a new slot,
//! and a slot is reclaimed only after the GC watermark, never reused. So a
//! copy keyed by slot can never disagree with any snapshot that still binds
//! the slot, and an arena made for one generation stays correct for every
//! slot it holds in later ones. Searches still resolve bindings through
//! their view and only take the vector's components from here; a slot the
//! arena does not hold (bound after it was made) is read from storage.
//!
//! Scoring a resident row ([`crate::kernel::PreparedQuery::score`]) is bit
//! for bit the score of the stored bytes (`score_stored`): the same kernel
//! over the same lanes.
//!
//! The rows are charged to the vector cache budget (`vector-cache-bytes`).
//! When the budget cannot hold them there is no arena and every search reads
//! storage, as before; a search is never refused for it.

use std::collections::HashMap;

use rushdb_storage::resources::ByteReservation;

/// Resident rows of one space, `dims` components each.
pub(crate) struct VectorArena {
    dims: usize,
    /// Row of each slot held.
    rows: HashMap<u64, u32>,
    data: Vec<f32>,
    _reservation: ByteReservation,
}

impl VectorArena {
    /// Bytes an arena of `count` rows of `dims` components takes: the rows
    /// and the slot map.
    pub(crate) fn bytes(count: usize, dims: usize) -> usize {
        count.saturating_mul(
            dims.saturating_mul(std::mem::size_of::<f32>())
                .saturating_add(32),
        )
    }

    /// An arena of the vectors `read(slot)` returns for `slots`, under a
    /// reservation already sized by [`Self::bytes`]. `None` when a slot has
    /// no stored vector of `dims` components (the arena would be partial in
    /// a way searches cannot tell from "bound later").
    pub(crate) fn build<E>(
        slots: &[u64],
        dims: usize,
        reservation: ByteReservation,
        mut read: impl FnMut(u64, &mut Vec<f32>) -> Result<bool, E>,
    ) -> Result<Option<Self>, E> {
        let mut data = Vec::with_capacity(slots.len().saturating_mul(dims));
        let mut rows = HashMap::with_capacity(slots.len());
        for &slot in slots {
            let Ok(row) = u32::try_from(rows.len()) else {
                return Ok(None);
            };
            let before = data.len();
            if !read(slot, &mut data)? || data.len() != before + dims {
                return Ok(None);
            }
            rows.insert(slot, row);
        }
        Ok(Some(Self {
            dims,
            rows,
            data,
            _reservation: reservation,
        }))
    }

    /// The components of `slot`, when it is resident.
    pub(crate) fn get(&self, slot: u64) -> Option<&[f32]> {
        let row = *self.rows.get(&slot)? as usize;
        self.data.get(row * self.dims..(row + 1) * self.dims)
    }

    /// Rows held.
    pub(crate) fn len(&self) -> usize {
        self.rows.len()
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
    match arena.and_then(|arena| arena.get(slot)) {
        Some(row) => Ok(query.score(row)),
        None => Ok(stored()?.ok_or(rushdb_storage::StorageError::CorruptLog)?),
    }
}

/// Where a space's arena stands.
pub(crate) enum ArenaState {
    /// A build is running.
    Building,
    /// Resident and answering.
    Ready(std::sync::Arc<VectorArena>),
    /// The space at `version` did not fit the vector cache budget; no build
    /// starts again until the space changes.
    NotAdmitted { version: u64 },
}

impl crate::VectorEngine {
    /// The resident rows of `space_id`, when an arena is ready.
    pub(crate) fn arena(&self, space_id: u64) -> Option<std::sync::Arc<VectorArena>> {
        match self.arenas.lock().ok()?.get(&space_id)? {
            ArenaState::Ready(arena) => Some(std::sync::Arc::clone(arena)),
            ArenaState::Building | ArenaState::NotAdmitted { .. } => None,
        }
    }

    /// Make `space`'s arena off the request path when it has none, or when
    /// the one it has holds fewer than nine in ten of its ready vectors (it
    /// was made before most of them were bound). Searches read storage until
    /// it is ready; at most one build runs per space.
    pub fn warm_arena_in_background(
        self: &std::sync::Arc<Self>,
        snap: &rushdb_storage::ReadView,
        space: &crate::EmbeddingSpace,
    ) {
        let space_id = space.space_id;
        let Ok((_, version)) = crate::ann::space_version(snap, self.tenant(), space_id) else {
            return;
        };
        {
            let Ok(mut arenas) = self.arenas.lock() else {
                return;
            };
            match arenas.get(&space_id) {
                Some(ArenaState::Building) => return,
                Some(ArenaState::NotAdmitted { version: refused }) if *refused == version => return,
                Some(ArenaState::Ready(arena)) => {
                    let Ok(ready) = self.ready_vectors_in_view(snap, space) else {
                        return;
                    };
                    if (arena.len() as u64).saturating_mul(10) >= ready.saturating_mul(9) {
                        return;
                    }
                }
                _ => {}
            }
            // The replaced rows release their budget before the new ones
            // reserve it; searches read storage in between.
            arenas.insert(space_id, ArenaState::Building);
        }
        let engine = std::sync::Arc::clone(self);
        let space = space.clone();
        let spawned = std::thread::Builder::new()
            .name("vector-arena".into())
            .spawn(move || {
                let built = engine.build_arena(&space);
                let Ok(mut arenas) = engine.arenas.lock() else {
                    return;
                };
                match built {
                    Ok(Some(arena)) => {
                        arenas.insert(
                            space.space_id,
                            ArenaState::Ready(std::sync::Arc::new(arena)),
                        );
                    }
                    Ok(None) | Err(crate::VectorError::AnnBudget(_)) => {
                        arenas.insert(space.space_id, ArenaState::NotAdmitted { version });
                    }
                    Err(error) => {
                        tracing::warn!(space = space.space_id, %error, "vector arena build failed");
                        arenas.remove(&space.space_id);
                    }
                }
            });
        if spawned.is_err() {
            if let Ok(mut arenas) = self.arenas.lock() {
                arenas.remove(&space_id);
            }
        }
    }

    /// The rows of every ready binding of `space` at a current snapshot,
    /// under a vector-budget reservation; `None` when a stored vector is
    /// not `dims` finite components (corruption a search reports itself).
    fn build_arena(
        &self,
        space: &crate::EmbeddingSpace,
    ) -> Result<Option<VectorArena>, crate::VectorError> {
        let snap = self.snapshot()?;
        let tenant = self.tenant();
        let dims = space.dims as usize;
        let slots: Vec<u64> =
            crate::ann::load_ready_segment_entries(&snap, tenant, space.space_id)?
                .iter()
                .map(|entry| entry.slot)
                .collect();
        let reservation = crate::ann::vector_cache_budget(&self.store)?
            .try_reserve(VectorArena::bytes(slots.len(), dims))
            .map_err(|error| crate::VectorError::AnnBudget(error.to_string()))?;
        VectorArena::build(&slots, dims, reservation, |slot, out| {
            let key = rushdb_types::keys::emb_vector_key(tenant, space.space_id, slot);
            snap.with_value(&key, |raw| {
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
        })
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

    use super::*;
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

        let arena = engine.build_arena(&space).unwrap().expect("rows");
        assert_eq!(arena.len(), 400);
        engine
            .arenas
            .lock()
            .unwrap()
            .insert(space.space_id, ArenaState::Ready(Arc::new(arena)));
        let (from_arena, arena_reads) =
            reads(|| queries.iter().map(|q| searches(q)).collect::<Vec<_>>());

        for (stored, resident) in from_storage.iter().zip(&from_arena) {
            for (stored, resident) in stored.iter().zip(resident) {
                assert!(!stored.is_empty());
                assert_eq!(bits(stored), bits(resident));
            }
        }
        // The vectors came from memory: only bindings were read.
        assert!(
            arena_reads < storage_reads,
            "{arena_reads} reads with the arena, {storage_reads} without"
        );
    }

    #[test]
    fn a_space_larger_than_the_vector_budget_answers_from_storage() {
        // 300 rows of 64 components do not fit 32 KiB.
        let engine = engine(32 * 1024);
        bind(&engine, "docs", 64, 300);
        let snap = engine.snapshot().unwrap();
        let space = engine.lookup_space_in_view(&snap, "docs").unwrap().unwrap();
        assert!(matches!(
            engine.build_arena(&space),
            Err(crate::VectorError::AnnBudget(_))
        ));
        engine.warm_arena_in_background(&snap, &space);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while matches!(
            engine.arenas.lock().unwrap().get(&space.space_id),
            Some(ArenaState::Building)
        ) {
            assert!(
                std::time::Instant::now() < deadline,
                "the build never finished"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(matches!(
            engine.arenas.lock().unwrap().get(&space.space_id),
            Some(ArenaState::NotAdmitted { .. })
        ));
        assert!(engine.arena(space.space_id).is_none());
        // Never refused: exact search reads the stored vectors.
        let query = vec![0.5f32; 64];
        let hits = engine
            .search_exact("docs", &query, 5, Candidates::All)
            .unwrap();
        assert_eq!(hits.len(), 5);
    }
}
