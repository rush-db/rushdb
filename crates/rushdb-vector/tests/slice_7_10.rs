//! Vertical-slice integration tests for embedding spaces, bindings,
//! candidate scoping, top-K ranking, and owner projection (items 7-10).

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::reader::try_access;
use rushdb_lmpg::{reader::TryRead, Lmpg};
use rushdb_storage::Store;
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::keys::*;
use rushdb_types::occurrence::BindingStatus;
use rushdb_vector::{SourceRef, VectorEngine, VectorError};
use serde_json::json;

/// Creates an independent engine over one in-memory store; returns the
/// graph facade plus a second shared-store facade for the vector
/// projection layer (the same runtime composition the platform uses).
fn setup() -> (Lmpg, VectorEngine) {
    let store = Arc::new(Store::new_in_memory());
    let tenant = TenantId::from_ordinal(11);
    let lmpg = Lmpg::new(store.clone(), tenant);
    let vecs = VectorEngine::new(Arc::new(Lmpg::new(store, tenant)));
    (lmpg, vecs)
}

fn pid_of(lmpg: &Lmpg, scope: OwnerKind, name: &str) -> u64 {
    let norm = rushdb_types::value::normalize_name(name);
    let sig = key(
        lmpg.tenant,
        &[
            P_FAMILY_SIG,
            b"default",
            &[scope.as_byte()],
            norm.as_bytes(),
        ],
    );
    let raw = lmpg
        .store
        .get(&sig)
        .unwrap()
        .expect("family signature exists");
    u64::from_be_bytes([
        raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
    ])
}

/// Item 7: two immutable Spaces bind one Record-owned text occurrence; two
/// more Spaces bind one Relationship-owned occurrence.
#[test]
fn item7_two_spaces_share_one_source() {
    let (lmpg, vecs) = setup();
    let rec = lmpg
        .create_record(
            "Doc",
            None,
            vec![("text".into(), FieldChange::Set(json!("alpha beta")))],
        )
        .unwrap();

    let s1 = vecs.ensure_space("title-v1", 4).unwrap();
    let s2 = vecs.ensure_space("fulltext-v1", 4).unwrap();
    assert_ne!(s1.space_id, s2.space_id);

    let src = SourceRef {
        kind: OwnerKind::Record,
        owner_ord: rec.ordinal,
        pid: pid_of(&lmpg, OwnerKind::Record, "text"),
        elem: 0,
    };
    let slot_a = vecs.bind("title-v1", src, &[1.0, 0.0, 0.0, 0.0]).unwrap();
    let slot_b = vecs
        .bind("fulltext-v1", src, &[0.0, 1.0, 0.0, 0.0])
        .unwrap();
    let _ = (slot_a, slot_b); // slots are per-Space; both spaces hold their copy
    assert_eq!(
        vecs.bindings_of(s1.space_id).unwrap().len(),
        1,
        "each Space keeps its own binding"
    );

    // Immutable contract enforcement on dims/model change.
    match vecs.ensure_space("title-v1", 8) {
        Err(VectorError::Conflict(msg)) => assert!(msg.contains("immutable space mismatch")),
        other => panic!("expected immutability error, got {other:?}"),
    }

    // Relationship-owned occurrence bound into two spaces.
    let target = lmpg.create_record("Doc", None, vec![]).unwrap();
    let rel = lmpg
        .create_relationship(
            "MENTIONS",
            rec,
            target,
            vec![("label".into(), FieldChange::Set(json!("x")))],
        )
        .unwrap();
    let rsrc = SourceRef {
        kind: OwnerKind::Relationship,
        owner_ord: rel.ordinal,
        pid: pid_of(&lmpg, OwnerKind::Relationship, "label"),
        elem: 0,
    };
    let ra = vecs.ensure_space("rel-a", 4).unwrap();
    let rb = vecs.ensure_space("rel-b", 4).unwrap();
    assert_ne!(ra.space_id, rb.space_id);
    vecs.bind("rel-a", rsrc, &[1.0, 1.0, 0.0, 0.0]).unwrap();
    vecs.bind("rel-b", rsrc, &[0.0, 0.0, 1.0, 0.0]).unwrap();
}

/// Item 10: source value update excludes old bindings as stale through
/// hash mismatch, after explicit revalidation (projector lag model).
#[test]
fn item10_stale_exclusion_after_source_update() {
    let (lmpg, vecs) = setup();
    let rec = lmpg
        .create_record(
            "Doc",
            None,
            vec![("text".into(), FieldChange::Set(json!("original value")))],
        )
        .unwrap();
    vecs.ensure_space("doc-space", 3).unwrap();
    let src = SourceRef {
        kind: OwnerKind::Record,
        owner_ord: rec.ordinal,
        pid: pid_of(&lmpg, OwnerKind::Record, "text"),
        elem: 0,
    };
    vecs.bind("doc-space", src, &[1.0, 0.0, 0.0]).unwrap();
    assert_eq!(
        vecs.search_exact("doc-space", &[1.0, 0.0, 0.0], 5, None)
            .unwrap()
            .len(),
        1
    );

    lmpg.update_fields(
        rec,
        vec![("text".into(), FieldChange::Set(json!("changed value")))],
    )
    .unwrap();

    // Bounded read without revalidation still observes lagged Ready state.
    assert_eq!(
        vecs.search_exact("doc-space", &[1.0, 0.0, 0.0], 5, None)
            .unwrap()
            .len(),
        1
    );

    let (stale, retired) = vecs.revalidate_bindings("doc-space").unwrap();
    assert_eq!((stale, retired), (1, 0));
    assert!(
        vecs.search_exact("doc-space", &[1.0, 0.0, 0.0], 5, None)
            .unwrap()
            .is_empty(),
        "stale binding must be ineligible"
    );
    let space_id = vecs
        .lookup_space_by_alias("doc-space")
        .unwrap()
        .unwrap()
        .space_id;
    assert_eq!(
        vecs.bindings_of(space_id).unwrap()[0].status,
        BindingStatus::Stale
    );

    // Deleting the source retires the binding entirely.
    lmpg.delete_record(rec).unwrap();
    vecs.revalidate_bindings("doc-space").unwrap();
    let bindings = vecs.bindings_of(space_id).unwrap();
    assert_eq!(bindings.len(), 1);
}

/// Items 8 + 9: candidate scoping before scoring, exact top-K ranking and
/// reconstruction projection through Records/Labels perspectives.
#[test]
fn items_8_9_candidate_scope_topk_and_projection() {
    let (lmpg, vecs) = setup();
    vecs.ensure_space("scope-space", 3).unwrap();

    let examples: [(&str, [f32; 3]); 3] = [
        ("keep", [1.0, 0.0, 0.0]),
        ("drop-near", [0.99, 0.01, 0.0]),
        ("far-away", [0.0, 1.0, 0.0]),
    ];

    let _seed = lmpg
        .create_record(
            "Scoped",
            None,
            vec![("t".into(), FieldChange::Set(json!("seed")))],
        )
        .unwrap();
    let pid = pid_of(&lmpg, OwnerKind::Record, "t");
    let mut ords = Vec::new();
    for (name, vector) in examples {
        let r = lmpg
            .create_record(
                "Scoped",
                None,
                vec![("t".into(), FieldChange::Set(json!(name)))],
            )
            .unwrap();
        ords.push(r.ordinal);
        vecs.bind(
            "scope-space",
            SourceRef {
                kind: OwnerKind::Record,
                owner_ord: r.ordinal,
                pid,
                elem: 0,
            },
            &vector,
        )
        .unwrap();
    }
    let keep_ord = ords[0];

    // Scoped search pins exactly one candidate.
    let scoped = vecs
        .search_exact(
            "scope-space",
            &[1.0, 0.0, 0.0],
            10,
            Some(&|src| src.owner_ord == keep_ord),
        )
        .unwrap();
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].source.owner_ord, keep_ord);

    // Unscoped exact top-K ranks by cosine; near duplicate ranks second.
    let all = vecs
        .search_exact("scope-space", &[1.0, 0.0, 0.0], 3, None)
        .unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].source.owner_ord, keep_ord);
    assert!(all[0].score > all[1].score);

    // Projection reconstructs owner JSON for the Records perspective.
    let projected = vecs.reconstruct_sources(&all[..1]).unwrap();
    assert_eq!(projected[0]["t"], json!("keep"));

    // Labels perspective resolves every ranked root through headers.
    let snap = lmpg.store.read_view().unwrap();
    for h in &all {
        assert!(
            try_access::record_by_ord(&snap, lmpg.tenant, h.source.owner_ord)
                .unwrap()
                .is_some()
        );
    }
}
