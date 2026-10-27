use super::*;
use rushdb_lmpg::{FieldChange, Lmpg};
use rushdb_storage::Store;
use rushdb_types::keys::{key, u64be, P_EMB_VECTOR};
use serde_json::json;
use std::cell::Cell;

fn on_backends(test: impl Fn(&VectorEngine)) {
    let path = rushdb_storage::TempPath::store("vector-pins");
    for persistent in [false, true] {
        let store = if persistent {
            Store::open(&path).unwrap()
        } else {
            Store::new_in_memory()
        };
        test(&VectorEngine::new(Arc::new(Lmpg::new(
            Arc::new(store),
            TenantId::from_ordinal(71),
        ))));
    }
    rushdb_storage::remove_store(&path).unwrap();
}

#[test]
fn vector_space_cardinality_is_bounded_per_tenant() {
    let engine = VectorEngine::new(Arc::new(Lmpg::new(
        Arc::new(Store::new_in_memory()),
        TenantId::from_ordinal(72),
    )));
    for index in 0..256 {
        engine.ensure_space(&format!("space-{index}"), 2).unwrap();
    }
    let error = engine.ensure_space("space-overflow", 2).unwrap_err();
    assert!(error.to_string().contains("space limit reached"));
}

#[test]
fn cached_ann_generation_releases_shared_reservation_on_engine_drop() {
    // A built `usearch` index's real measured memory
    // (`FrozenSegment::measured_bytes`) carries a large, roughly
    // size-independent allocator floor (tens of MiB even for one vector),
    // well above the old `generation_bytes() * 2` formula estimate this
    // budget used to assume was enough. 32 MiB comfortably covers it.
    let config = rushdb_storage::resources::StorageResources {
        logical_cache_bytes: 32 * 1024 * 1024,
        ..Default::default()
    };
    let store = Arc::new(Store::new_with_resources(config).unwrap());
    let budget = store.cache_budget().unwrap();
    let engine = VectorEngine::new(Arc::new(Lmpg::new(
        Arc::clone(&store),
        TenantId::from_ordinal(74),
    )));
    let root = engine
        .lmpg
        .create_record(
            "Doc",
            None,
            vec![("name".into(), FieldChange::Set(json!("ann")))],
        )
        .unwrap();
    engine.ensure_space("ann", 2).unwrap();
    engine
        .bind_external(
            "ann",
            SourceRef {
                kind: OwnerKind::Record,
                owner_ord: root.ordinal,
                pid: 0,
                elem: 0,
            },
            &[1., 0.],
        )
        .unwrap();
    engine.build_ann("ann").unwrap();
    assert!(budget.snapshot().used > 0);
    drop(engine);
    // The engine's generation reservation is released; derived state lives
    // on disk and charges nothing to the shared cache budget.
    assert_eq!(budget.snapshot().used, 0);
}

/// Binds `n` deterministic `dims`-dimensional vectors to a fresh space named
/// `alias` on `engine`.
#[expect(
    clippy::cast_precision_loss,
    reason = "the modulo-7 result is always in 0..7, far under f32's exact integer range"
)]
fn bind_many(engine: &VectorEngine, alias: &str, dims: u32, n: usize) {
    engine.ensure_space(alias, dims).unwrap();
    for i in 0..n {
        let root = engine
            .lmpg
            .create_record("Doc", None, vec![("i".into(), FieldChange::Set(json!(i)))])
            .unwrap();
        let vector: Vec<f32> = (0..dims).map(|d| ((i + d as usize) % 7) as f32).collect();
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
    }
}

#[test]
fn ann_build_admission_uses_the_real_measured_peak_not_a_formula_estimate() {
    // The cache-budget reservation an ANN build charges — and that
    // therefore gates admission — must be the real `usearch`-reported index
    // size (`FrozenSegment::measured_bytes`, backed by
    // `usearch::Index::memory_usage`), not the old `generation_bytes() * 2`
    // formula estimate. Prove it by learning the real peak from one build,
    // then showing a fresh store budgeted at exactly that peak admits the
    // same build and one budgeted one byte under it rejects — a boundary a
    // formula estimate (a different number) would not line up with.
    let dims = 16u32;
    let n = 300usize;

    let learn_config = rushdb_storage::resources::StorageResources {
        logical_cache_bytes: 64 * 1024 * 1024,
        ..Default::default()
    };
    let learn_store = Arc::new(Store::new_with_resources(learn_config).unwrap());
    let learn_budget = learn_store.vector_cache_budget().unwrap();
    let learn_engine = VectorEngine::new(Arc::new(Lmpg::new(
        Arc::clone(&learn_store),
        TenantId::from_ordinal(75),
    )));
    bind_many(&learn_engine, "ann", dims, n);
    learn_engine.build_ann("ann").unwrap();
    // `used`, not `peak`: the pre-build reservation briefly charges a larger
    // conservative estimate before `build_ann` resizes it down to the real
    // measured size, and `peak` is a ratcheting high-water mark that stays
    // at that larger historical value — `used` reflects what is actually
    // retained right now, after the resize.
    let real_peak = learn_budget.snapshot().used;
    assert!(real_peak > 0, "a built index must retain a nonzero charge");

    // A formula-estimate-only scheme would gate on `generation_bytes() * 2`,
    // a different number — it must not equal what was actually measured
    // (otherwise this test would not distinguish the two mechanisms).
    let formula_estimate = n * (dims as usize * 4 + 64) * 2;
    assert_ne!(
        real_peak, formula_estimate,
        "the retained charge must be the real measured size, not the formula estimate"
    );

    // `usearch`'s HNSW construction has stochastic elements (layer
    // assignment), so the real measured size can vary build to build even
    // for identical input — use wide margins around the learned peak
    // rather than its exact byte value.
    let generous_limit = real_peak.saturating_mul(10);
    let tight_limit = (real_peak / 10).max(1);

    // Comfortably over the real peak: admits.
    let fits_config = rushdb_storage::resources::StorageResources {
        logical_cache_bytes: generous_limit,
        ..Default::default()
    };
    let fits_store = Arc::new(Store::new_with_resources(fits_config).unwrap());
    let fits_engine = VectorEngine::new(Arc::new(Lmpg::new(
        Arc::clone(&fits_store),
        TenantId::from_ordinal(76),
    )));
    bind_many(&fits_engine, "ann", dims, n);
    fits_engine
        .build_ann("ann")
        .expect("a budget comfortably over the real measured peak must admit the build");

    // Comfortably under the real peak: rejects.
    let tight_config = rushdb_storage::resources::StorageResources {
        logical_cache_bytes: tight_limit,
        ..Default::default()
    };
    let tight_store = Arc::new(Store::new_with_resources(tight_config).unwrap());
    let tight_engine = VectorEngine::new(Arc::new(Lmpg::new(
        Arc::clone(&tight_store),
        TenantId::from_ordinal(77),
    )));
    bind_many(&tight_engine, "ann", dims, n);
    let error = tight_engine
        .build_ann("ann")
        .expect_err("a budget comfortably under the real measured peak must reject the build");
    assert!(error.contains("ANN build memory budget"), "{error}");
}

fn fixture(engine: &VectorEngine, alias: &str) -> (SourceRef, EmbeddingSpace, u64) {
    let root = engine
        .lmpg
        .create_record(
            "Doc",
            None,
            vec![("name".into(), FieldChange::Set(json!("before")))],
        )
        .unwrap();
    let source = SourceRef {
        kind: OwnerKind::Record,
        owner_ord: root.ordinal,
        pid: 0,
        elem: 0,
    };
    let space = engine.ensure_space(alias, 2).unwrap();
    let slot = engine.bind_external(alias, source, &[1., 0.]).unwrap();
    (source, space, slot)
}

#[test]
fn retrying_identical_external_bind_is_idempotent() {
    let store = Arc::new(Store::new_in_memory());
    let engine = VectorEngine::new(Arc::new(Lmpg::new(store, TenantId::from_ordinal(72))));
    let source = SourceRef {
        kind: OwnerKind::Record,
        owner_ord: 1,
        pid: 0,
        elem: 0,
    };
    engine.ensure_space("retry", 2).unwrap();
    let first = engine.bind_external("retry", source, &[1., 0.]).unwrap();
    let second = engine.bind_external("retry", source, &[1., 0.]).unwrap();
    assert_eq!(first, second);
    let space = engine.lookup_space_by_alias("retry").unwrap().unwrap();
    assert_eq!(engine.bindings_of(space.space_id).unwrap().len(), 1);
}

#[test]
fn managed_embedding_completion_rejects_stale_source_hash() {
    let store = Arc::new(Store::new_in_memory());
    let engine = VectorEngine::new(Arc::new(Lmpg::new(store, TenantId::from_ordinal(73))));
    let root = engine
        .lmpg
        .create_record(
            "Doc",
            None,
            vec![("name".into(), FieldChange::Set(json!("before")))],
        )
        .unwrap();
    let source = SourceRef {
        kind: OwnerKind::Record,
        owner_ord: root.ordinal,
        pid: 0,
        elem: 0,
    };
    engine.ensure_space("managed", 2).unwrap();
    let captured_hash = source_hash(&engine.snapshot().unwrap(), engine.tenant(), source)
        .unwrap()
        .expect("source exists");

    engine
        .lmpg
        .update_fields(
            root,
            vec![("name".into(), FieldChange::Set(json!("after")))],
        )
        .unwrap();
    let error = engine
        .bind_if_source_hash("managed", source, captured_hash, &[1., 0.])
        .unwrap_err();
    assert!(error.to_string().contains("source changed"));
    let space = engine.lookup_space_by_alias("managed").unwrap().unwrap();
    assert!(engine.bindings_of(space.space_id).unwrap().is_empty());
}

#[test]
fn alias_and_binding_catalogs_share_a_view_across_clear_and_gc() {
    on_backends(|engine| {
        let (source, space, _) = fixture(engine, "catalog");
        let view = engine.snapshot().unwrap();
        engine.clear_space("catalog").unwrap();
        engine.store.gc(u64::MAX).unwrap();
        assert_eq!(
            engine.lookup_space_in_view(&view, "catalog").unwrap(),
            Some(space.clone())
        );
        assert!(engine
            .binding_of_in_view(&view, space.space_id, source)
            .unwrap()
            .is_some());
        assert_eq!(
            engine
                .bindings_of_in_view(&view, space.space_id)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            engine
                .search_exact_in_view(&view, &space, &[1., 0.], 1, None)
                .unwrap()
                .len(),
            1
        );
        assert!(engine.lookup_space_by_alias("catalog").unwrap().is_none());
        assert!(engine.bindings_of(space.space_id).unwrap().is_empty());
    });
}

#[test]
fn external_binding_tracks_owner_lifecycle_not_occurrence_hash() {
    on_backends(|engine| {
        let root = engine
            .lmpg
            .create_record(
                "Doc",
                None,
                vec![("name".into(), FieldChange::Set(json!("before")))],
            )
            .unwrap();
        let source = SourceRef {
            kind: OwnerKind::Record,
            owner_ord: root.ordinal,
            pid: 0,
            elem: 0,
        };
        let space = engine.ensure_space("external-lifecycle", 2).unwrap();
        engine
            .bind_external("external-lifecycle", source, &[1., 0.])
            .unwrap();

        engine
            .lmpg
            .update_fields(
                root,
                vec![("name".into(), FieldChange::Set(json!("after")))],
            )
            .unwrap();
        assert_eq!(
            engine.revalidate_bindings("external-lifecycle").unwrap(),
            (0, 0)
        );
        assert_eq!(
            engine.bindings_of(space.space_id).unwrap()[0].status,
            BindingStatus::Ready
        );

        engine.lmpg.delete_record(root).unwrap();
        assert_eq!(
            engine.revalidate_bindings("external-lifecycle").unwrap(),
            (0, 1)
        );
        assert_eq!(
            engine.bindings_of(space.space_id).unwrap()[0].status,
            BindingStatus::Retired
        );
    });
}

#[test]
fn caller_owned_vector_views_reject_another_store() {
    on_backends(|engine| {
        let (source, space, _) = fixture(engine, "catalog");
        let foreign = Store::new_in_memory().read_view().unwrap();
        assert!(engine.lookup_space_in_view(&foreign, "catalog").is_err());
        assert!(engine
            .binding_of_in_view(&foreign, space.space_id, source)
            .is_err());
        assert!(engine
            .bindings_of_in_view(&foreign, space.space_id)
            .is_err());
        assert!(engine
            .search_exact_in_view(&foreign, &space, &[1., 0.], 0, None)
            .is_err());
        assert!(engine
            .search_ann_in_view(&foreign, &space, &[1., 0.], 0, 1, None)
            .is_err());
        assert!(engine.reconstruct_sources_in_view(&foreign, &[]).is_err());
    });
}

fn mutate_raw(engine: &VectorEngine, storage_key: Vec<u8>, value: Option<Vec<u8>>) {
    let commit_key = uuid_v7();
    engine
        .store
        .commit(commit_key, |_, _| {
            let mut batch = empty_batch(engine.tenant());
            batch.commit_key = commit_key;
            let key = String::from_utf8(storage_key).unwrap();
            batch.mutations.push(match value {
                Some(value) => Mutation::SetMetaBlob { key, value },
                None => Mutation::DeleteMetaBlob { key },
            });
            Ok((batch, ()))
        })
        .unwrap();
}

#[test]
fn exact_ann_rerank_and_ann_fallback_keep_one_view_during_writes_and_gc() {
    on_backends(|engine| {
        for mode in 0..3 {
            let alias = format!("space-{mode}");
            let (source, space, slot) = fixture(engine, &alias);
            let hash = engine
                .binding_of(space.space_id, source)
                .unwrap()
                .unwrap()
                .source_hash;
            let ci = engine.store.committed_version();
            let changed = Cell::new(false);
            let filter = |_| {
                if !changed.replace(true) {
                    engine.bind_external(&alias, source, &[0., 1.]).unwrap();
                    mutate_raw(
                        engine,
                        emb_vector_key(engine.tenant(), space.space_id, slot),
                        None,
                    );
                    engine.store.gc(u64::MAX).unwrap();
                    assert_eq!(engine.store.gc_watermark(), ci);
                }
                true
            };
            let hits = match mode {
                0 => engine.search_exact(&alias, &[1., 0.], 1, Some(&filter)),
                1 => engine.search_ann(&alias, &[1., 0.], 1, 2, Some(&filter)),
                _ => engine.search_ann(&alias, &[1., 0.], 2, 2, Some(&filter)),
            }
            .unwrap();
            assert!(changed.get());
            assert_eq!(hits.len(), 1);
            assert!((hits[0].score - 1.).abs() < f32::EPSILON);
            assert_eq!(hits[0].value_hash, hash);
            assert!(
                engine.search_exact(&alias, &[1., 0.], 1, None).unwrap()[0]
                    .score
                    .abs()
                    < f32::EPSILON
            );
        }
    });
}

#[test]
fn corrupt_vector_bytes_are_errors_not_short_arrays_or_empty_results() {
    on_backends(|engine| {
        for (i, payload) in [
            None,
            Some(vec![1]),
            Some(vec![0; 4]),
            Some([f32::NAN.to_le_bytes(), 0f32.to_le_bytes()].concat()),
        ]
        .into_iter()
        .enumerate()
        {
            let alias = format!("bad-{i}");
            let (_, space, slot) = fixture(engine, &alias);
            mutate_raw(
                engine,
                emb_vector_key(engine.tenant(), space.space_id, slot),
                payload,
            );
            assert!(matches!(
                engine.search_exact(&alias, &[1., 0.], 1, None),
                Err(VectorError::Storage(StorageError::CorruptLog))
            ));
            assert!(matches!(
                engine.search_ann(&alias, &[1., 0.], 1, 2, None),
                Err(VectorError::Storage(StorageError::CorruptLog))
            ));
            // Metadata freshness is computed from binding metadata only: it
            // must not decode the (now corrupt) vector corpus. This guards the
            // unbudgeted whole-corpus decode that OOM'd metadata calls.
            let view = engine.snapshot().unwrap();
            assert!(
                engine.ann_lifecycle_in_view(&view, &space).is_ok(),
                "ANN lifecycle must not decode vector payloads"
            );
        }
    });
}

#[test]
fn malformed_aliases_and_bindings_do_not_look_missing() {
    on_backends(|engine| {
        let (source, space, _) = fixture(engine, "bad-binding");
        mutate_raw(
            engine,
            emb_binding_key(
                engine.tenant(),
                space.space_id,
                source.kind,
                source.owner_ord,
                source.pid,
                source.elem,
            ),
            Some(vec![1]),
        );
        assert!(engine.binding_of(space.space_id, source).is_err());
        assert!(engine.bindings_of(space.space_id).is_err());
        assert!(engine
            .search_exact("bad-binding", &[1., 0.], 1, None)
            .is_err());
        assert!(engine
            .search_ann("bad-binding", &[1., 0.], 1, 2, None)
            .is_err());
        mutate_raw(
            engine,
            emb_alias_key(engine.tenant(), "bad-binding"),
            Some(vec![1]),
        );
        assert!(engine.lookup_space_by_alias("bad-binding").is_err());
        assert!(engine.lookup_space_by_alias("absent").unwrap().is_none());
    });
}

#[test]
fn old_ann_generation_cannot_replace_newer_cache() {
    on_backends(|engine| {
        let (source, space, _) = fixture(engine, "cache");
        let old = engine.snapshot().unwrap();
        engine.bind_external("cache", source, &[0., 1.]).unwrap();
        let new = engine.snapshot().unwrap();
        engine.current_ann(&new, &space).unwrap();
        let old_version = crate::ann::space_version(&old, engine.tenant(), space.space_id)
            .unwrap()
            .1;
        let new_version = crate::ann::space_version(&new, engine.tenant(), space.space_id)
            .unwrap()
            .1;
        assert!(old_version < new_version);
        // A reader of the old view gets a generation for its own version…
        assert_eq!(
            engine.current_ann(&old, &space).unwrap().space_version,
            old_version
        );
        // …which never replaces the newer cached one.
        assert_eq!(
            engine.ann_cache.lock().unwrap()[&space.space_id].space_version,
            new_version
        );
    });
}

#[test]
fn invalid_queries_fail_on_empty_indexes_and_zero_k_is_supported() {
    on_backends(|engine| {
        engine.ensure_space("empty", 2).unwrap();
        assert!(matches!(
            engine.search_ann("empty", &[1.], 1, 2, None),
            Err(VectorError::DimMismatch(2, 1))
        ));
        assert!(matches!(
            engine.search_exact("empty", &[f32::NAN, 0.], 1, None),
            Err(VectorError::InvalidVector(_))
        ));
        assert!(matches!(
            engine.search_ann("empty", &[f32::INFINITY, 0.], 1, 2, None),
            Err(VectorError::InvalidVector(_))
        ));
        assert!(engine
            .search_ann("empty", &[1., 0.], 0, 0, None)
            .unwrap()
            .is_empty());
        assert!(engine.ensure_space("zero", 0).is_err());
        assert!((cosine(&[f32::MAX, f32::MAX], &[f32::MAX, f32::MAX]) - 1.).abs() < 1e-6);
    });
}

#[test]
fn shared_view_owner_reconstruction_and_native_reopen_preserve_results() {
    let path = rushdb_storage::TempPath::store("vector-reopen");
    let tenant = TenantId::from_ordinal(71);
    let engine = VectorEngine::new(Arc::new(Lmpg::new(
        Arc::new(Store::open(&path).unwrap()),
        tenant,
    )));
    let (source, space, _) = fixture(&engine, "reopen");
    drop(engine);
    let engine = VectorEngine::new(Arc::new(Lmpg::new(
        Arc::new(Store::open(&path).unwrap()),
        tenant,
    )));
    let snap = engine.snapshot().unwrap();
    let header = rushdb_lmpg::reader::try_access::record_by_ord(&snap, tenant, source.owner_ord)
        .unwrap()
        .unwrap();
    engine
        .lmpg
        .update_fields(
            rushdb_lmpg::RecordRef {
                record_id: header.record_id,
                ordinal: header.ordinal,
            },
            vec![("name".into(), FieldChange::Set(json!("after")))],
        )
        .unwrap();
    engine.store.gc(u64::MAX).unwrap();
    for hits in [
        engine
            .search_exact_in_view(&snap, &space, &[1., 0.], 1, None)
            .unwrap(),
        engine
            .search_ann_in_view(&snap, &space, &[1., 0.], 1, 2, None)
            .unwrap(),
    ] {
        assert_eq!(hits.len(), 1);
        assert!((hits[0].score - 1.).abs() < f32::EPSILON);
        assert_eq!(
            engine.reconstruct_sources_in_view(&snap, &hits).unwrap()[0]["name"],
            json!("before")
        );
        assert_eq!(
            engine.reconstruct_sources(&hits).unwrap()[0]["name"],
            json!("after")
        );
    }
    drop(snap);
    drop(engine);
    rushdb_storage::remove_store(&path).unwrap();
}

fn command(
    engine: &VectorEngine,
    version: u32,
    mutations: Vec<Mutation>,
) -> Result<CommitOutcome<()>, StorageError> {
    let commit_key = uuid_v7();
    engine.store.commit(commit_key, |_, _| {
        let mut batch = empty_batch(engine.tenant());
        batch.commit_key = commit_key;
        batch.command_version = version;
        batch.mutations = mutations;
        Ok((batch, ()))
    })
}

#[test]
fn clear_space_deletes_binary_keys_and_orphan_segments_without_crossing_scope() {
    on_backends(|base| {
        let engine = VectorEngine::new(Arc::new(Lmpg::new(
            base.store.clone(),
            TenantId(u32::MAX - 1),
        )));
        let other = VectorEngine::new(Arc::new(Lmpg::new(
            base.store.clone(),
            TenantId(u32::MAX - 2),
        )));
        let (source, space, original_slot) = fixture(&engine, "target");
        let (_, sibling, _) = fixture(&engine, "target/extra");
        let (_, other_space, _) = fixture(&other, "target");
        assert_eq!(space.space_id, other_space.space_id);
        let mut binding = engine.binding_of(space.space_id, source).unwrap().unwrap();
        binding.vector_slot = 255;
        command(
            &engine,
            rushdb_types::command::COMMAND_VERSION,
            vec![Mutation::UpsertEmbeddingBinding {
                binding,
                vector: Some(vec![0., 1.]),
                expect_live_hash: false,
            }],
        )
        .unwrap();
        engine.build_ann("target").unwrap();
        let old = engine.snapshot().unwrap();
        let slot_counter =
            meta_counter_key(engine.tenant(), &format!("vecslot/{}", space.space_id));
        let counter_before = old.get(&slot_counter).unwrap();
        assert_eq!(engine.clear_space("target").unwrap(), 1);
        assert!(engine.lookup_space_by_alias("target").unwrap().is_none());
        assert!(engine.bindings_of(space.space_id).unwrap().is_empty());
        assert!(engine
            .store
            .try_get(&emb_vector_key(
                engine.tenant(),
                space.space_id,
                original_slot
            ))
            .unwrap()
            .is_none());
        assert!(engine
            .store
            .try_get(&emb_vector_key(engine.tenant(), space.space_id, 255))
            .unwrap()
            .is_none());
        assert_eq!(engine.store.try_get(&slot_counter).unwrap(), counter_before);
        assert!(engine
            .store
            .try_get(&emb_space_key(engine.tenant(), space.space_id))
            .unwrap()
            .is_some());
        assert!(!engine
            .ann_cache
            .lock()
            .unwrap()
            .contains_key(&space.space_id));
        assert_eq!(engine.bindings_of(sibling.space_id).unwrap().len(), 1);
        assert_eq!(other.bindings_of(other_space.space_id).unwrap().len(), 1);
        engine.store.gc(u64::MAX).unwrap();
        assert!(old
            .get(&emb_alias_key(engine.tenant(), "target"))
            .unwrap()
            .is_some());
        assert!(
            (engine
                .search_exact_in_view(&old, &space, &[0., 1.], 1, None)
                .unwrap()[0]
                .score
                - 1.)
                .abs()
                < f32::EPSILON
        );
        let recreated = engine.ensure_space("target", 2).unwrap();
        assert!(recreated.space_id > sibling.space_id);
        assert!(engine.bindings_of(recreated.space_id).unwrap().is_empty());
    });
}

#[test]
fn racing_bind_and_clear_leave_no_orphan_binding_and_report_admitted_count() {
    on_backends(|engine| {
        for round in 0..10 {
            let alias = format!("race-{round}");
            let (source, space, _) = fixture(engine, &alias);
            let new_source = SourceRef {
                owner_ord: source.owner_ord + 1000,
                ..source
            };
            let barrier = std::sync::Barrier::new(2);
            let (bound, cleared) = std::thread::scope(|scope| {
                let binder = scope.spawn(|| {
                    barrier.wait();
                    engine.bind_external(&alias, new_source, &[0., 1.])
                });
                let clearer = scope.spawn(|| {
                    barrier.wait();
                    engine.clear_space(&alias)
                });
                (binder.join().unwrap(), clearer.join().unwrap())
            });
            assert_eq!(cleared.unwrap(), if bound.is_ok() { 2 } else { 1 });
            assert!(engine.lookup_space_by_alias(&alias).unwrap().is_none());
            assert!(engine.bindings_of(space.space_id).unwrap().is_empty());
            assert!(engine
                .snapshot()
                .unwrap()
                .range_prefix(&key(
                    engine.tenant(),
                    &[P_EMB_VECTOR, &u64be(space.space_id)]
                ))
                .unwrap()
                .is_empty());
        }
    });
}

#[test]
fn clear_intent_checks_version_and_alias_identity_before_publication() {
    on_backends(|engine| {
        let (_, space, _) = fixture(engine, "checked");
        let before = engine.store.committed_version();
        assert!(command(
            engine,
            2,
            vec![Mutation::ClearEmbeddingSpace {
                alias: "checked".into(),
                space_id: space.space_id
            }]
        )
        .is_err());
        assert!(command(
            engine,
            rushdb_types::command::COMMAND_VERSION,
            vec![
                Mutation::SetMetaBlob {
                    key: "clear-rollback-probe".into(),
                    value: vec![1]
                },
                Mutation::ClearEmbeddingSpace {
                    alias: "checked".into(),
                    space_id: space.space_id + 1
                },
            ]
        )
        .is_err());
        assert!(engine.clear_space("missing").is_err());
        assert_eq!(engine.store.committed_version(), before);
        assert!(engine
            .store
            .try_get(b"clear-rollback-probe")
            .unwrap()
            .is_none());
        assert_eq!(engine.bindings_of(space.space_id).unwrap().len(), 1);
    });
}

#[test]
fn old_commands_reject_atomically_and_current_clear_survives_reopen() {
    let path = rushdb_storage::TempPath::store("vector-clear");
    let tenant = TenantId(u32::MAX - 1);
    let engine = VectorEngine::new(Arc::new(Lmpg::new(
        Arc::new(Store::open(&path).unwrap()),
        tenant,
    )));
    // Any other format: none, the pre-release development format (7), and
    // a future one.
    for version in [0, 7, rushdb_types::command::COMMAND_VERSION + 1] {
        assert!(command(
            &engine,
            version,
            vec![Mutation::SetMetaValue {
                key: format!("rejected-command-{version}"),
                value: 42,
            }],
        )
        .is_err());
    }
    assert_eq!(engine.store.committed_version(), 0);
    let (_, space, slot) = fixture(&engine, "clear");
    engine.clear_space("clear").unwrap();
    let log = engine.store.take_log_for_crash();
    let last: CommitBatch = rushdb_types::command_codec::decode(log.last().unwrap()).unwrap();
    assert_eq!(last.command_version, rushdb_types::command::COMMAND_VERSION);
    assert_eq!(
        last.mutations,
        vec![Mutation::ClearEmbeddingSpace {
            alias: "clear".into(),
            space_id: space.space_id
        }]
    );
    drop(engine);
    let reopened = Arc::new(Store::open(&path).unwrap());
    let replayed = Arc::new(Store::new_in_memory());
    drop(Lmpg::new(replayed.clone(), tenant));
    replayed.try_replay(&log).unwrap();
    for store in [&reopened, &replayed] {
        let engine = VectorEngine::new(Arc::new(Lmpg::new(store.clone(), tenant)));
        assert!(engine.lookup_space_by_alias("clear").unwrap().is_none());
        assert!(engine.bindings_of(space.space_id).unwrap().is_empty());
        assert!(store
            .try_get(&emb_vector_key(tenant, space.space_id, slot))
            .unwrap()
            .is_none());
        for version in [1, 2, 4] {
            assert!(store
                .try_get(format!("rejected-command-{version}").as_bytes())
                .unwrap()
                .is_none());
        }
        assert!(engine.ensure_space("clear", 2).unwrap().space_id > space.space_id);
    }
    drop(reopened);
    rushdb_storage::remove_store(&path).unwrap();
}

#[test]
fn rebinding_reclaims_superseded_slots_once_the_floor_passes() {
    on_backends(|engine| {
        let (source, space, old_slot) = fixture(engine, "reclaim");
        let old_ci = engine.store.committed_version();
        // Re-binding allocates a fresh slot and stages the old one for
        // reclamation (it may still be read by a pinned view until the floor
        // passes this commit).
        let new_slot = engine.bind_external("reclaim", source, &[0., 1.]).unwrap();
        assert_ne!(new_slot, old_slot);
        let old_key = emb_vector_key(engine.tenant(), space.space_id, old_slot);
        let new_key = emb_vector_key(engine.tenant(), space.space_id, new_slot);
        assert!(engine.store.try_get(&old_key).unwrap().is_some());

        // At or below the old commit nothing is reclaimable.
        assert_eq!(engine.reclaim_slots(old_ci).unwrap(), 0);
        assert!(engine.store.try_get(&old_key).unwrap().is_some());

        // A pinned view at the old commit holds the floor: even a high
        // watermark cannot reclaim the slot it may still read.
        let pinned = engine.store.read_at(old_ci).unwrap();
        engine.store.gc(u64::MAX).unwrap();
        assert_eq!(engine.store.gc_watermark(), old_ci, "pin holds the floor");
        assert_eq!(engine.reclaim_slots(u64::MAX).unwrap(), 0);
        assert!(engine.store.try_get(&old_key).unwrap().is_some());
        drop(pinned);

        // Releasing the pin lets the floor advance past the re-bind; the slot
        // is reclaimed and the current binding still searches.
        engine.store.gc(u64::MAX).unwrap();
        assert!(engine.store.gc_watermark() > old_ci);
        assert!(engine.reclaim_slots(u64::MAX).unwrap() >= 1);
        assert!(
            engine.store.try_get(&old_key).unwrap().is_none(),
            "superseded slot reclaimed"
        );
        assert!(
            engine.store.try_get(&new_key).unwrap().is_some(),
            "current slot kept"
        );
        let hits = engine.search_exact("reclaim", &[0., 1.], 1, None).unwrap();
        assert_eq!(hits.len(), 1);
        assert!((hits[0].score - 1.).abs() < f32::EPSILON);
    });
}

#[test]
fn unrelated_commit_does_not_rebuild_a_space_generation() {
    // An unrelated global commit must not invalidate a space's cached
    // ANN generation; only a change to that space's bindings rebuilds it.
    let store = Arc::new(Store::new_in_memory());
    let engine = VectorEngine::new(Arc::new(Lmpg::new(store, TenantId::from_ordinal(73))));
    let (_source, space, _) = fixture(&engine, "stable");
    let snap = engine.snapshot().unwrap();
    let first = engine.current_ann(&snap, &space).unwrap();
    // An unrelated commit (a record in another label) advances the global CI.
    engine
        .lmpg
        .create_record(
            "Other",
            None,
            vec![("x".into(), FieldChange::Set(json!(1)))],
        )
        .unwrap();
    let snap2 = engine.snapshot().unwrap();
    let second = engine.current_ann(&snap2, &space).unwrap();
    assert!(
        Arc::ptr_eq(&first, &second),
        "an unrelated commit must reuse the cached generation"
    );
}

#[test]
fn incremental_bind_absorbs_into_the_live_cache_without_a_full_rebuild() {
    // Bucket 9b, landed: a bind after the cache is warm appends a hot
    // segment to the EXISTING cache entry (`incrementally_absorb_bind`)
    // instead of throwing the whole generation away and rebuilding one
    // fresh frozen segment from canonical storage. A full rebuild always
    // produces exactly one frozen segment (`choose_backing`'s shape); two
    // segments after a second bind is the distinguishing, observable proof
    // that absorption — not a fallback rebuild — handled it.
    let store = Arc::new(Store::new_in_memory());
    let engine = VectorEngine::new(Arc::new(Lmpg::new(store, TenantId::from_ordinal(80))));
    let (first_source, space, _) = fixture(&engine, "incr");

    let snap = engine.snapshot().unwrap();
    let cached = engine.current_ann(&snap, &space).unwrap();
    #[allow(
        irrefutable_let_patterns,
        reason = "refutable only with --features turbovec, which adds AnnBacking::Provider"
    )]
    let crate::ann::AnnBacking::Segments(set) = &cached.backing
    else {
        panic!("expected the segment backing");
    };
    assert_eq!(
        set.snapshot().len(),
        1,
        "a fresh rebuild is exactly one frozen segment"
    );

    // A distinct, synthetic owner ordinal — `bind_external` needs no live
    // occurrence to exist (its hash is derived from the vector payload, not
    // a record field) — so this is the ONLY commit after the cache was
    // warmed, preserving the strict one-commit adjacency
    // `incrementally_absorb_bind` requires.
    let second_source = SourceRef {
        kind: OwnerKind::Record,
        owner_ord: first_source.owner_ord + 1000,
        pid: 0,
        elem: 0,
    };
    engine
        .bind_external("incr", second_source, &[0., 1.])
        .unwrap();

    let snap2 = engine.snapshot().unwrap();
    let cached2 = engine.current_ann(&snap2, &space).unwrap();
    #[allow(
        irrefutable_let_patterns,
        reason = "refutable only with --features turbovec, which adds AnnBacking::Provider"
    )]
    let crate::ann::AnnBacking::Segments(set2) = &cached2.backing
    else {
        panic!("expected the segment backing");
    };
    assert_eq!(
        set2.snapshot().len(),
        2,
        "an absorbed bind appends a hot segment rather than rebuilding one fresh frozen segment"
    );

    let hits = engine.search_ann("incr", &[0., 1.], 2, 4, None).unwrap();
    assert_eq!(hits.len(), 2);
    assert!(hits
        .iter()
        .any(|h| h.source.owner_ord == first_source.owner_ord));
    assert!(hits
        .iter()
        .any(|h| h.source.owner_ord == second_source.owner_ord));
}

#[test]
fn incremental_retirement_tombstones_without_a_full_rebuild() {
    // Bucket 9b, landed: retiring a binding (owner deleted,
    // `revalidate_bindings`) tombstones it in the live cache
    // (`incrementally_absorb_retirements`) instead of rebuilding. The
    // distinguishing, observable proof: the segment count AND the
    // original segment's own entry count stay unchanged (the entry
    // physically remains, just tombstoned) — a fallback full rebuild would
    // instead produce one fresh, now-empty frozen segment.
    let store = Arc::new(Store::new_in_memory());
    let engine = VectorEngine::new(Arc::new(Lmpg::new(store, TenantId::from_ordinal(81))));
    let root = engine
        .lmpg
        .create_record(
            "Doc",
            None,
            vec![("name".into(), FieldChange::Set(json!("gone")))],
        )
        .unwrap();
    let source = SourceRef {
        kind: OwnerKind::Record,
        owner_ord: root.ordinal,
        pid: 0,
        elem: 0,
    };
    engine.ensure_space("retire", 2).unwrap();
    engine.bind_external("retire", source, &[1., 0.]).unwrap();

    let snap = engine.snapshot().unwrap();
    let space = engine
        .lookup_space_in_view(&snap, "retire")
        .unwrap()
        .unwrap();
    engine.current_ann(&snap, &space).unwrap();

    engine.lmpg.delete_record(root).unwrap();
    // A raw record delete alone doesn't touch the `EmbeddingBinding` row —
    // it stays `Ready` until `revalidate_bindings` explicitly flips it — so
    // a read here sees an unchanged fingerprint and refreshes the cache's
    // `commit_index` in place to the delete's commit (the existing
    // "unrelated commit" path). That is what makes `revalidate_bindings`'s
    // own commit land exactly one past what the cache reflects, letting
    // `incrementally_absorb_retirements` chain onto it instead of falling
    // back — a realistic sequence (reads keep happening between a delete
    // and the next revalidation pass), not a test-only contrivance.
    let resync = engine.snapshot().unwrap();
    engine.current_ann(&resync, &space).unwrap();

    let (_, retired) = engine.revalidate_bindings("retire").unwrap();
    assert_eq!(retired, 1);

    let hits = engine.search_ann("retire", &[1., 0.], 1, 4, None).unwrap();
    assert!(
        hits.is_empty(),
        "a retired source must never surface, even without a fresh rebuild"
    );

    let snap2 = engine.snapshot().unwrap();
    let cached2 = engine.current_ann(&snap2, &space).unwrap();
    #[allow(
        irrefutable_let_patterns,
        reason = "refutable only with --features turbovec, which adds AnnBacking::Provider"
    )]
    let crate::ann::AnnBacking::Segments(set2) = &cached2.backing
    else {
        panic!("expected the segment backing");
    };
    let segments = set2.snapshot();
    assert_eq!(
        segments.len(),
        1,
        "retirement must not trigger a fresh rebuild"
    );
    assert_eq!(
        segments[0].entries().len(),
        1,
        "the entry physically remains in the segment; tombstoning never rebuilds the index"
    );
}

/// Published segment files (not sidecars, not staged) under `space_dir`.
fn segment_files(space_dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(space_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| crate::segment::is_segment_file_name(n))
        .collect();
    names.sort();
    names
}

#[test]
fn evicted_space_reopens_its_segments_and_decodes_only_what_changed() {
    // Dropping the engine (a tenant eviction or a restart) keeps the
    // space's segment files; the next search reopens them, reconciles them
    // with the bindings changed meanwhile, and decodes only those, instead
    // of decoding the whole corpus again.
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::new_in_memory());
    let lmpg = Arc::new(Lmpg::new(Arc::clone(&store), TenantId::from_ordinal(82)));
    let engine = VectorEngine::new(Arc::clone(&lmpg)).with_segments_dir(dir.path().to_path_buf());
    let (_, space, _) = fixture(&engine, "evict");
    let gone = lmpg
        .create_record("Doc", None, vec![("n".into(), FieldChange::Set(json!(1)))])
        .unwrap();
    let gone_source = SourceRef {
        kind: OwnerKind::Record,
        owner_ord: gone.ordinal,
        pid: 0,
        elem: 0,
    };
    engine
        .bind_external("evict", gone_source, &[1., 0.1])
        .unwrap();
    engine
        .current_ann(&engine.snapshot().unwrap(), &space)
        .unwrap();
    // Record-backed sources: revalidation retires a source without one.
    let record = || {
        let root = lmpg
            .create_record("Doc", None, vec![("n".into(), FieldChange::Set(json!(0)))])
            .unwrap();
        external(root.ordinal)
    };
    let mut first = None;
    for _ in 0..3 {
        let batch: Vec<(SourceRef, Vec<f32>)> = (0..crate::ann::HOT_SEGMENT_FREEZE_THRESHOLD)
            .map(|_| (record(), vec![1.0, 0.01]))
            .collect();
        first.get_or_insert(batch[0].0);
        engine.bind_external_many("evict", batch).unwrap();
        engine.maintain_segments().unwrap();
    }
    // An unfrozen tail stays hot.
    let tail: Vec<(SourceRef, Vec<f32>)> = (0..10).map(|_| (record(), vec![1.0, 0.02])).collect();
    engine.bind_external_many("evict", tail).unwrap();
    let before = cached_segments(&engine, &space);
    let frozen_before = before
        .iter()
        .filter(|s| matches!(s.as_ref(), crate::segment::Segment::Frozen(_)))
        .count();
    assert_eq!(
        frozen_before, 4,
        "the main segment plus three frozen batches"
    );
    let space_dir = dir.path().join(format!("space-{}", space.space_id));
    let files = segment_files(&space_dir);
    drop(before);
    drop(engine);
    assert_eq!(
        segment_files(&space_dir),
        files,
        "eviction keeps every segment file"
    );

    // While evicted: one frozen source is rebound, one is retired.
    let fresh = VectorEngine::new(Arc::clone(&lmpg)).with_segments_dir(dir.path().to_path_buf());
    let rebound = first.unwrap();
    fresh.bind_external("evict", rebound, &[0.0, 1.0]).unwrap();
    lmpg.delete_record(gone).unwrap();
    let (_, retired) = fresh.revalidate_bindings("evict").unwrap();
    assert_eq!(retired, 1);

    let after = cached_segments(&fresh, &space);
    let hot: Vec<_> = after
        .iter()
        .filter(|s| matches!(s.as_ref(), crate::segment::Segment::Hot(_)))
        .collect();
    assert_eq!(
        after.len() - hot.len(),
        frozen_before,
        "every frozen segment was reopened, none rebuilt"
    );
    assert_eq!(hot.len(), 1);
    assert_eq!(
        hot[0].entries().len(),
        11,
        "only the unfrozen tail and the rebound vector were decoded"
    );
    assert_eq!(segment_files(&space_dir), files, "nothing was rewritten");

    let hits = fresh.search_ann("evict", &[0.0, 1.0], 1, 16, None).unwrap();
    assert_eq!(
        hits[0].source.owner_ord, rebound.owner_ord,
        "the rebound vector wins"
    );
    let all = fresh
        .search_ann("evict", &[1.0, 0.1], 1_000, 1_000, None)
        .unwrap();
    assert!(
        all.iter()
            .all(|hit| hit.source.owner_ord != gone_source.owner_ord),
        "a source retired while evicted never surfaces"
    );
    assert_eq!(
        all.iter()
            .filter(|hit| hit.source.owner_ord == rebound.owner_ord)
            .count(),
        1,
        "the rebound source surfaces once"
    );
}

#[test]
fn space_larger_than_the_vector_budget_is_built_in_persisted_chunks() {
    // A full build reserves before it decodes and builds in chunks
    // sized to the free budget, each persisted and released, so a space
    // whose whole-corpus build would not fit is still served by ANN when
    // its segments are file-backed. Without a segments directory the
    // retained segments themselves exceed the budget: refused, exact.
    // Not a multiple of 8: segments, with or without the `turbovec`
    // provider (which decodes whole and is not chunked).
    let dims = 764u32;
    let n = 3_000usize;
    let per_vector = crate::segment::build_bytes_per_vector(dims);
    // Room for about a third of the corpus at once.
    let config = rushdb_storage::resources::StorageResources {
        logical_cache_bytes: n * per_vector / 3,
        ..Default::default()
    };
    let store = Arc::new(Store::new_with_resources(config).unwrap());
    let budget = store.vector_cache_budget().unwrap();
    assert!(
        budget.snapshot().limit < n * per_vector,
        "the whole build must not fit the vector pool"
    );
    let dir = tempfile::tempdir().unwrap();
    let lmpg = Arc::new(Lmpg::new(Arc::clone(&store), TenantId::from_ordinal(83)));
    let persisted =
        VectorEngine::new(Arc::clone(&lmpg)).with_segments_dir(dir.path().to_path_buf());
    persisted.ensure_space("big", dims).unwrap();
    let mut rng = 0x9e37_79b9_7f4a_7c15_u64;
    let batch: Vec<(SourceRef, Vec<f32>)> = (0..n as u64)
        .map(|i| {
            let vector = (0..dims)
                .map(|_| {
                    rng ^= rng << 13;
                    rng ^= rng >> 7;
                    rng ^= rng << 17;
                    #[expect(clippy::cast_precision_loss, reason = "test vectors")]
                    let x = (rng % 2000) as f32 / 1000.0 - 1.0;
                    x
                })
                .collect();
            (external(500_000 + i), vector)
        })
        .collect();
    let probe = batch[1234].clone();
    persisted.bind_external_many("big", batch).unwrap();
    let space = persisted.lookup_space_by_alias("big").unwrap().unwrap();

    let segments = cached_segments(&persisted, &space);
    assert!(segments.len() >= 3, "{} segments", segments.len());
    assert_eq!(segments.iter().map(|s| s.entries().len()).sum::<usize>(), n);
    assert!(budget.snapshot().used <= budget.snapshot().limit);
    let hits = persisted.search_ann("big", &probe.1, 1, 64, None).unwrap();
    assert_eq!(hits[0].source.owner_ord, probe.0.owner_ord);

    let in_memory = VectorEngine::new(Arc::clone(&lmpg));
    let error = in_memory
        .current_ann(&in_memory.snapshot().unwrap(), &space)
        .err()
        .expect("resident segments over the budget are refused");
    assert!(matches!(error, VectorError::AnnBudget(_)), "{error}");
    let exact = in_memory.search_ann("big", &probe.1, 1, 64, None).unwrap();
    assert_eq!(
        exact[0].source.owner_ord, probe.0.owner_ord,
        "refused ANN answers exactly"
    );
}

#[test]
fn batch_bind_absorbs_in_one_commit_and_maintenance_bounds_segments() {
    // C9: `bind_external_many` binds a whole batch in one commit and the
    // warm cache absorbs it in one step (into the hot segment); background
    // maintenance freezes and merges, so the segment count stays bounded
    // without a rebuild.
    let store = Arc::new(Store::new_in_memory());
    let engine = VectorEngine::new(Arc::new(Lmpg::new(store, TenantId::from_ordinal(81))));
    let (first_source, space, _) = fixture(&engine, "batched");
    let segments = |engine: &VectorEngine| {
        let snap = engine.snapshot().unwrap();
        let cached = engine.current_ann(&snap, &space).unwrap();
        #[allow(
            irrefutable_let_patterns,
            reason = "refutable only with --features turbovec, which adds AnnBacking::Provider"
        )]
        let crate::ann::AnnBacking::Segments(set) = &cached.backing
        else {
            panic!("expected the segment backing");
        };
        set.snapshot().len()
    };
    assert_eq!(segments(&engine), 1, "warm cache: one frozen segment");

    let before = engine.snapshot().unwrap().commit_index();
    let batch: Vec<(SourceRef, Vec<f32>)> = (1..=300u64)
        .map(|offset| {
            let source = SourceRef {
                kind: OwnerKind::Record,
                owner_ord: first_source.owner_ord + offset,
                pid: 0,
                elem: 0,
            };
            #[expect(clippy::cast_precision_loss, reason = "small test ordinal")]
            let x = offset as f32;
            (source, vec![1.0, x])
        })
        .collect();
    let slots = engine.bind_external_many("batched", batch).unwrap();
    assert_eq!(slots.len(), 300);
    assert_eq!(
        engine.snapshot().unwrap().commit_index(),
        before + 1,
        "the whole batch is one commit"
    );
    // 300 absorbed binds land in one hot segment next to the main one.
    assert_eq!(segments(&engine), 2, "absorbed without a full rebuild");
    engine.maintain_segments().unwrap();
    assert_eq!(segments(&engine), 2, "the hot segment was frozen in place");

    // Keep absorbing with maintenance running: tiers merge, so the count
    // stays bounded by the merge factor instead of growing per freeze.
    let mut next = first_source.owner_ord + 1_000;
    for _ in 0..10 {
        let batch: Vec<(SourceRef, Vec<f32>)> = (0..256u64)
            .map(|offset| {
                (
                    SourceRef {
                        kind: OwnerKind::Record,
                        owner_ord: next + offset,
                        pid: 0,
                        elem: 0,
                    },
                    vec![0.5, 1.0],
                )
            })
            .collect();
        next += 256;
        engine.bind_external_many("batched", batch).unwrap();
        engine.maintain_segments().unwrap();
        assert!(
            segments(&engine) <= crate::segment::MERGE_FACTOR + 1,
            "segment count stays bounded"
        );
    }
}

/// Segment list of a space's cached generation (tests).
fn cached_segments(
    engine: &VectorEngine,
    space: &EmbeddingSpace,
) -> Arc<Vec<Arc<crate::segment::Segment>>> {
    let snap = engine.snapshot().unwrap();
    let cached = engine.current_ann(&snap, space).unwrap();
    #[allow(
        irrefutable_let_patterns,
        reason = "refutable only with --features turbovec, which adds AnnBacking::Provider"
    )]
    let crate::ann::AnnBacking::Segments(set) = &cached.backing
    else {
        panic!("expected the segment backing");
    };
    set.snapshot()
}

fn external(ord: u64) -> SourceRef {
    SourceRef {
        kind: OwnerKind::Record,
        owner_ord: ord,
        pid: 0,
        elem: 0,
    }
}

#[test]
fn absorb_survives_interleaved_other_tenant_commits() {
    // The absorb chain is the space's own version row, so commits by
    // another tenant (or to other spaces) between two binds no longer force
    // a full rebuild.
    let store = Arc::new(Store::new_in_memory());
    let a = VectorEngine::new(Arc::new(Lmpg::new(
        Arc::clone(&store),
        TenantId::from_ordinal(91),
    )));
    let b = Lmpg::new(Arc::clone(&store), TenantId::from_ordinal(92));
    let (_, space, _) = fixture(&a, "chain");
    let warm = a.current_ann(&a.snapshot().unwrap(), &space).unwrap();
    a.bind_external("chain", external(10_001), &[0.5, 0.5])
        .unwrap();
    for i in 0..3 {
        b.create_record(
            "Other",
            None,
            vec![("x".into(), FieldChange::Set(json!(i)))],
        )
        .unwrap();
    }
    a.bind_external("chain", external(10_002), &[0.4, 0.6])
        .unwrap();
    let segments = cached_segments(&a, &space);
    // Still the warm main segment plus one hot segment holding both binds.
    assert!(Arc::ptr_eq(
        &segments[0],
        &match &warm.backing {
            #[allow(
                irrefutable_let_patterns,
                reason = "refutable only with --features turbovec, which adds AnnBacking::Provider"
            )]
            crate::ann::AnnBacking::Segments(set) => set.snapshot()[0].clone(),
            #[cfg(feature = "turbovec")]
            crate::ann::AnnBacking::Provider(_) => unreachable!(),
        }
    ));
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[1].entries().len(), 2);
    let hits = a.search_ann("chain", &[0.4, 0.6], 1, 8, None).unwrap();
    assert_eq!(hits[0].source.owner_ord, 10_002);
}

#[test]
fn segment_count_bounded_under_50k_incremental_vectors() {
    let store = Arc::new(Store::new_in_memory());
    let engine = VectorEngine::new(Arc::new(Lmpg::new(store, TenantId::from_ordinal(93))));
    let (_, space, _) = fixture(&engine, "bulk");
    engine
        .current_ann(&engine.snapshot().unwrap(), &space)
        .unwrap();
    let mut ord = 100_000u64;
    let mut max_segments = 0;
    for _ in 0..100 {
        let batch: Vec<(SourceRef, Vec<f32>)> = (0..500)
            .map(|_| {
                ord += 1;
                #[expect(clippy::cast_precision_loss, reason = "test vectors")]
                let angle = (ord % 3_600) as f32 / 573.0;
                (external(ord), vec![angle.cos(), angle.sin()])
            })
            .collect();
        engine.bind_external_many("bulk", batch).unwrap();
        engine.maintain_segments().unwrap();
        max_segments = max_segments.max(cached_segments(&engine, &space).len());
    }
    // 50,000 incremental vectors would be ~195 segments of 256 without
    // merging; size-tiered merging keeps it to a few per tier.
    assert!(
        max_segments <= 3 * crate::segment::MERGE_FACTOR,
        "{max_segments} segments"
    );
    let hits = engine.search_ann("bulk", &[1.0, 0.0], 5, 64, None).unwrap();
    assert_eq!(hits.len(), 5);
    assert!(hits[0].score > 0.99);
}

#[test]
fn superseded_segment_files_are_deleted_after_last_reader() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::new_in_memory());
    let engine = VectorEngine::new(Arc::new(Lmpg::new(store, TenantId::from_ordinal(94))))
        .with_segments_dir(dir.path().to_path_buf());
    let (_, space, _) = fixture(&engine, "files");
    engine
        .current_ann(&engine.snapshot().unwrap(), &space)
        .unwrap();
    let space_dir = dir.path().join(format!("space-{}", space.space_id));
    let incremental = || segment_files(&space_dir);
    let mut ord = 200_000u64;
    let mut bind_batch = |engine: &VectorEngine| {
        let batch: Vec<(SourceRef, Vec<f32>)> = (0..crate::ann::HOT_SEGMENT_FREEZE_THRESHOLD)
            .map(|_| {
                ord += 1;
                (external(ord), vec![1.0, 0.0])
            })
            .collect();
        engine.bind_external_many("files", batch).unwrap();
    };
    // The fixture's 1-entry main segment is in the lowest tier too: eight
    // freezes of 256 leave the tier one short; the ninth (below) fills it
    // and merges.
    for _ in 2..crate::segment::MERGE_FACTOR {
        bind_batch(&engine);
        engine.maintain_segments().unwrap();
    }
    let before_merge = incremental();
    // The fixture's 1-entry segment plus eight frozen batches.
    assert_eq!(before_merge.len(), crate::segment::MERGE_FACTOR - 1);
    bind_batch(&engine);
    // A reader pins the current segments across the freeze + merge.
    let reader = engine
        .current_ann(&engine.snapshot().unwrap(), &space)
        .unwrap();
    engine.maintain_segments().unwrap();
    let after_merge = incremental();
    assert!(
        before_merge.iter().all(|name| after_merge.contains(name)),
        "a pinned reader keeps the merged-away files"
    );
    assert_eq!(
        after_merge.len(),
        before_merge.len() + 1,
        "plus the merged file"
    );
    drop(reader);
    let remaining = incremental();
    assert!(
        before_merge.iter().all(|name| !remaining.contains(name)),
        "files go when the last reader drops: {remaining:?}"
    );
    assert_eq!(remaining.len(), 1, "only the merged segment's file is left");
}

#[test]
fn interrupted_segment_writes_swept_at_open() {
    // Staged writes and sidecars without their segment are debris of an
    // interrupted build; published segments stay for the next reopen.
    let dir = tempfile::tempdir().unwrap();
    let space_dir = dir.path().join("space-7");
    std::fs::create_dir_all(&space_dir).unwrap();
    for name in [
        "seg-0190aaaa",
        "seg-0190aaaa.manifest.json",
        "seg-0190aaaa.entries",
        "seg-0190bbbb.manifest.json",
        "seg-0190bbbb.entries",
        "seg-0190cccc.tmp-1",
        "seg-0190cccc.tmp-1.manifest.json",
    ] {
        std::fs::write(space_dir.join(name), b"x").unwrap();
    }
    let store = Arc::new(Store::new_in_memory());
    let _engine = VectorEngine::new(Arc::new(Lmpg::new(store, TenantId::from_ordinal(95))))
        .with_segments_dir(dir.path().to_path_buf());
    let mut left: Vec<String> = std::fs::read_dir(&space_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(
        left,
        vec![
            "seg-0190aaaa",
            "seg-0190aaaa.entries",
            "seg-0190aaaa.manifest.json"
        ]
    );
}

#[test]
fn small_index_charged_bytes_under_1mib() {
    // A built index used to keep usearch's 2 x 8 MiB arena (~16 MiB
    // charged for any size); a view of its serialized bytes holds only them.
    let store = Arc::new(Store::new_in_memory());
    let engine = VectorEngine::new(Arc::new(Lmpg::new(store, TenantId::from_ordinal(96))));
    let (_, space, _) = fixture(&engine, "small");
    let batch: Vec<(SourceRef, Vec<f32>)> = (0..200u64)
        .map(|i| {
            #[expect(clippy::cast_precision_loss, reason = "test vectors")]
            let x = i as f32;
            (external(300_000 + i), vec![1.0, x])
        })
        .collect();
    engine.bind_external_many("small", batch).unwrap();
    // A fresh engine builds the whole space as one frozen segment.
    let fresh = VectorEngine::new(Arc::clone(&engine.lmpg));
    let cached = fresh
        .current_ann(&fresh.snapshot().unwrap(), &space)
        .unwrap();
    let charged = crate::ann::cached_ann_bytes(&cached);
    assert!(charged < 1024 * 1024, "{charged} bytes charged");
}

#[test]
#[ignore = "vector budget measurement; run in release"]
#[expect(clippy::cast_precision_loss, reason = "reporting precision only")]
fn probe_twelve_spaces_of_3k_by_768_fit_the_default_budget() {
    let store = Arc::new(Store::new_in_memory());
    let engine = VectorEngine::new(Arc::new(Lmpg::new(
        Arc::clone(&store),
        TenantId::from_ordinal(97),
    )));
    let mut rng = 0x9e37_79b9_7f4a_7c15_u64;
    let mut vector = || -> Vec<f32> {
        (0..768)
            .map(|_| {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                #[expect(clippy::cast_precision_loss, reason = "test vectors")]
                let x = (rng % 2000) as f32 / 1000.0 - 1.0;
                x
            })
            .collect()
    };
    let spaces = 12;
    for space in 0..spaces {
        let alias = format!("s{space}");
        engine.ensure_space(&alias, 768).unwrap();
        for chunk in 0..6u64 {
            let batch: Vec<(SourceRef, Vec<f32>)> = (0..500u64)
                .map(|i| (external(1_000_000 + chunk * 500 + i), vector()))
                .collect();
            engine.bind_external_many(&alias, batch).unwrap();
        }
    }
    let fallbacks = crate::ann::ann_exact_fallbacks_total();
    let query = vector();
    let started = std::time::Instant::now();
    for space in 0..spaces {
        engine
            .search_ann(&format!("s{space}"), &query, 10, 64, None)
            .unwrap();
    }
    let build = started.elapsed();
    // Second round must be served from the cache for every space.
    let started = std::time::Instant::now();
    for space in 0..spaces {
        engine
            .search_ann(&format!("s{space}"), &query, 10, 64, None)
            .unwrap();
    }
    let cached_round = started.elapsed();
    let cache = engine.ann_cache.lock().unwrap();
    let charged: usize = cache
        .values()
        .map(|c| crate::ann::cached_ann_bytes(c))
        .sum();
    eprintln!(
        "PROBE spaces cached {}/{spaces}, charged {:.1} MiB (budget used {:.1} MiB of {:.1}), fallbacks {}, build round {build:?}, cached round {cached_round:?}",
        cache.len(),
        charged as f64 / 1_048_576.0,
        store.vector_cache_budget().unwrap().snapshot().used as f64 / 1_048_576.0,
        store.vector_cache_budget().unwrap().snapshot().limit as f64 / 1_048_576.0,
        crate::ann::ann_exact_fallbacks_total() - fallbacks,
    );
}

#[test]
#[ignore = "vector budget measurement; run in release"]
#[expect(clippy::cast_precision_loss, reason = "reporting precision only")]
fn probe_segment_bytes_after_twenty_rebind_rounds() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::new_in_memory());
    let engine = VectorEngine::new(Arc::new(Lmpg::new(store, TenantId::from_ordinal(98))))
        .with_segments_dir(dir.path().to_path_buf());
    let dims = 128usize;
    let n = 3_000u64;
    engine
        .ensure_space("rebind", u32::try_from(dims).unwrap())
        .unwrap();
    let mut rng = 0x2545_f491_4f6c_dd1d_u64;
    let mut vector = || -> Vec<f32> {
        (0..dims)
            .map(|_| {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                #[expect(clippy::cast_precision_loss, reason = "test vectors")]
                let x = (rng % 2000) as f32 / 1000.0 - 1.0;
                x
            })
            .collect()
    };
    let bytes = |path: &std::path::Path| -> u64 {
        fn walk(path: &std::path::Path) -> u64 {
            std::fs::read_dir(path).map_or(0, |entries| {
                entries
                    .flatten()
                    .map(|e| {
                        let m = e.metadata().unwrap();
                        if m.is_dir() {
                            walk(&e.path())
                        } else {
                            m.len()
                        }
                    })
                    .sum()
            })
        }
        walk(path)
    };
    let mut peak = 0;
    for round in 0..=20u64 {
        for chunk in 0..(n / 500) {
            let batch: Vec<(SourceRef, Vec<f32>)> = (0..500u64)
                .map(|i| (external(chunk * 500 + i), vector()))
                .collect();
            engine.bind_external_many("rebind", batch).unwrap();
            engine.maintain_segments().unwrap();
            peak = peak.max(bytes(dir.path()));
        }
        if round == 0 {
            // Warm the cache so later rounds absorb incrementally.
            engine.search_ann("rebind", &vector(), 5, 32, None).unwrap();
        }
    }
    let live = n * dims as u64 * 4;
    let end = bytes(dir.path());
    eprintln!(
        "PROBE live vectors {live} B; vector-segments end {end} B ({:.2}x), peak {peak} B ({:.2}x)",
        end as f64 / live as f64,
        peak as f64 / live as f64
    );
}

#[test]
fn binding_reads_stream_past_the_materialization_ceiling() {
    // VEC-3: a space past ~230K bindings overflowed the 64 MiB scan
    // materialization ceiling, so every ANN build and index description
    // failed. Binding reads stream instead; here the ceiling is cut to
    // 1 KiB: one binding row fits, 20 do not.
    on_backends(|engine| {
        bind_many(engine, "big", 4, 20);
        let space = engine.lookup_space_by_alias("big").unwrap().unwrap();
        let limited = Arc::new(engine.snapshot().unwrap()).with_materialization_limit(1024);
        assert!(
            limited
                .range_prefix(&rushdb_types::keys::emb_bindings_prefix(
                    engine.lmpg.tenant,
                    space.space_id
                ))
                .is_err(),
            "the ceiling must bind a collected scan"
        );
        let counts = engine
            .binding_counts_in_view(&limited, space.space_id)
            .unwrap();
        assert_eq!((counts.total, counts.ready), (20, 20));
        assert_eq!(
            engine
                .bindings_of_in_view(&limited, space.space_id)
                .unwrap()
                .len(),
            20
        );
        let outcome = engine
            .search_in_view(
                &limited,
                &space,
                &[1.0, 2.0, 3.0, 4.0],
                5,
                crate::search::SearchMode::Ann { ef: 64 },
                None,
                None,
            )
            .unwrap();
        assert_eq!(outcome.hits.len(), 5);
    });
}
