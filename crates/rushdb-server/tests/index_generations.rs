//! 4A — durable index generations. Vector bindings and space metadata must
//! survive reopen and an index must be either correctly served or visibly
//! behind (never falsely ready) after a restart.

use rushdb_protocol::{VectorIndexCandidatesQuery, VectorIndexDefinition, VectorItem};
use rushdb_server::{Node, ServerConfig};

fn disk_node(dir: &std::path::Path) -> Node {
    let mut cfg = ServerConfig {
        data_dir: Some(dir.to_path_buf()),
        ..Default::default()
    };
    Node::bootstrap(&mut cfg).unwrap()
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "single linear operation/reader; splitting it would scatter shared borrows and hurt auditability"
)]
fn vector_index_serves_persisted_bindings_after_reopen() {
    let dir = rushdb_storage::TempPath::dir("idx");

    // The binding path uses the derived canonical alias.
    let alias = "emb__BOOK__embedding__cosine__3";
    {
        let node = disk_node(&dir);
        node.scope()
            .create_vector_index(&VectorIndexDefinition {
                name: Some(alias.into()),
                label: "BOOK".into(),
                property: "embedding".into(),
                dimensions: 3,
                similarity: "cosine".into(),
                model: None,
                owner: None,
            })
            .unwrap();
        let empty = node.scope().vector_index_stats(alias).unwrap();
        assert_eq!(empty["status"], "awaiting_vectors");
        // The records must exist for a binding to attach; capture the actual
        // generated record ids.
        let rec_a = node
            .scope()
            .create_record(
                "BOOK",
                &serde_json::json!({ "embedding": [1.0, 0.0, 0.0] }),
                &[],
                &serde_json::json!({}),
            )
            .unwrap()
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();
        let rec_b = node
            .scope()
            .create_record(
                "BOOK",
                &serde_json::json!({ "embedding": [0.0, 1.0, 0.0] }),
                &[],
                &serde_json::json!({}),
            )
            .unwrap()
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();
        node.scope()
            .upsert_vector_index(
                alias,
                &[
                    VectorItem {
                        record_id: rec_a.clone(),
                        vector: vec![1.0, 0.0, 0.0],
                        source_hash: None,
                        index_generation: None,
                    },
                    VectorItem {
                        record_id: rec_b.clone(),
                        vector: vec![0.0, 1.0, 0.0],
                        source_hash: None,
                        index_generation: None,
                    },
                ],
            )
            .unwrap();
        // The binding must be searchable on the live node (before reopen).
        let pre = node
            .scope()
            .vector_search(alias, &[1.0, 0.0, 0.0], 1)
            .unwrap();
        assert_eq!(pre.len(), 1, "binding must be served on the live node");
        let ready = node.scope().vector_index_stats(alias).unwrap();
        assert_eq!(ready["status"], "ready");
        assert_eq!(ready["indexedRecords"], 2);
        assert_eq!(ready["sourceTrackedBindings"], 0);
        assert_eq!(ready["externalBindings"], 2);
        assert_eq!(ready["annLifecycle"]["manifestState"], "rebuildRequired");
        let rebuilt = node.scope().revalidate_vector_index(alias).unwrap();
        assert_eq!(rebuilt["annLifecycle"]["manifestState"], "current");
        assert_eq!(rebuilt["annLifecycle"]["providerState"], "memoryCached");
        std::mem::forget(rec_a);
        std::mem::forget(rec_b);
    }

    // Reopen from the same data directory.
    let reopened = disk_node(&dir);
    // The persisted bindings are served — the index is correctly ready, not
    // silently empty after a restart.
    let hits = reopened
        .scope()
        .vector_search(alias, &[1.0, 0.0, 0.0], 1)
        .unwrap();
    assert_eq!(
        hits.len(),
        1,
        "persisted binding must be served after reopen"
    );
    assert_eq!(
        reopened.scope().vector_index_stats(alias).unwrap()["status"],
        "ready"
    );
    let after_restart = reopened.scope().vector_index_stats(alias).unwrap();
    assert_eq!(after_restart["annLifecycle"]["manifestState"], "current");
    assert_eq!(
        after_restart["annLifecycle"]["providerState"],
        "rebuildRequired"
    );

    // A record that carries the property but has no vector binding is a
    // candidate — the index is visibly behind, never falsely complete.
    let envelope = rushdb_protocol::RequestEnvelope {
        protocol_version: 1,
        request_id: Some("idx-record".into()),
        context: rushdb_protocol::RequestContext::default(),
        operation: rushdb_protocol::Operation::Mutate {
            mutation: rushdb_protocol::Mutation::PutRecord {
                mode: rushdb_protocol::PutMode::Create,
                id: None,
                label: Some("BOOK".into()),
                properties: serde_json::json!({ "embedding": [0.0, 0.0, 1.0], "title": "unembedded" }),
                vectors: vec![],
                options: serde_json::json!({}),
            },
        },
    };
    rushdb_server::protocol::ProtocolExecutor::new(&reopened, None)
        .execute(envelope)
        .unwrap();

    let (candidates, total) = reopened
        .scope()
        .vector_index_candidates(&VectorIndexCandidatesQuery {
            label: "BOOK".into(),
            property: "embedding".into(),
            index: alias.into(),
            skip: 0,
            limit: 10,
        })
        .unwrap();
    assert!(
        total >= 1,
        "an unembedded record must surface as a candidate (index visibly behind): {candidates:?}"
    );
    let behind = reopened.scope().vector_index_stats(alias).unwrap();
    assert_eq!(behind["status"], "behind");
    assert_eq!(behind["indexedRecords"], 2);
    assert_eq!(behind["unindexedRecords"], 1);

    let indexes = reopened.scope().list_vector_indexes().unwrap();
    assert_eq!(indexes["data"][0]["status"], "behind");

    // Re-declaring the managed index must preserve the live lifecycle state;
    // an existing binding does not make an index ready when coverage is behind.
    let redeclared = reopened
        .scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(alias.into()),
            label: "BOOK".into(),
            property: "embedding".into(),
            dimensions: 3,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    assert_eq!(redeclared["status"], "behind");

    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn deleted_record_binding_is_retired_and_no_longer_searchable() {
    let dir = rushdb_storage::TempPath::dir("idx-del");
    let alias = "emb__BOOK__embedding__cosine__3";
    let node = disk_node(&dir);
    node.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(alias.into()),
            label: "BOOK".into(),
            property: "embedding".into(),
            dimensions: 3,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    let rec_a = node
        .scope()
        .create_record(
            "BOOK",
            &serde_json::json!({ "embedding": [1.0, 0.0, 0.0] }),
            &[],
            &serde_json::json!({}),
        )
        .unwrap()
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();
    let rec_b = node
        .scope()
        .create_record(
            "BOOK",
            &serde_json::json!({ "embedding": [0.0, 1.0, 0.0] }),
            &[],
            &serde_json::json!({}),
        )
        .unwrap()
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();
    node.scope()
        .upsert_vector_index(
            alias,
            &[
                VectorItem {
                    record_id: rec_a.clone(),
                    vector: vec![1.0, 0.0, 0.0],
                    source_hash: None,
                    index_generation: None,
                },
                VectorItem {
                    record_id: rec_b.clone(),
                    vector: vec![0.0, 1.0, 0.0],
                    source_hash: None,
                    index_generation: None,
                },
            ],
        )
        .unwrap();

    let _pre_b = node
        .scope()
        .vector_search(alias, &[0.0, 1.0, 0.0], 1)
        .unwrap();
    // Delete rec_a, then revalidate: its binding must be retired (tombstone)
    // and the index must no longer return it.
    node.scope().delete_record(&rec_a).unwrap();
    let res = node.scope().revalidate_vector_index(alias).unwrap();
    assert!(res["retired"].as_u64().unwrap_or(0) >= 1, "{res:?}");
    let _post_b = node
        .scope()
        .vector_search(alias, &[0.0, 1.0, 0.0], 1)
        .unwrap();
    assert!(res["retired"].as_u64().unwrap_or(0) >= 1, "{res:?}");

    // The surviving binding still scores correctly.
    let hits_b = node
        .scope()
        .vector_search(alias, &[0.0, 1.0, 0.0], 1)
        .unwrap();
    assert_eq!(hits_b.len(), 1);
    // The deleted record is gone: it must not appear in any result. The
    // survivor (rec_b) is the only binding left, so a query returns it even at
    // score 0 — the tombstone is that rec_a is absent.
    let hits = node
        .scope()
        .vector_search(alias, &[1.0, 0.0, 0.0], 2)
        .unwrap();
    assert!(
        !hits.iter().any(|(id, _)| id == &rec_a),
        "deleted record must not be searchable: {hits:?}"
    );
    // Reopen: the Retired binding must be durable + still skipped.
    drop(node);
    let reopened = disk_node(&dir);
    let hits2 = reopened
        .scope()
        .vector_search(alias, &[1.0, 0.0, 0.0], 2)
        .unwrap();
    assert!(
        !hits2.iter().any(|(id, _)| id == &rec_a),
        "deleted record must not be searchable after reopen: {hits2:?}"
    );
    std::mem::forget(rec_a);
    std::mem::forget(rec_b);
    let _ = std::fs::remove_dir_all(&dir);
}

/// V-4b: `/ready` gating uses this signal. An index with no source values is
/// settled; an unbound source value leaves it behind; binding it warms it.
#[test]
fn readiness_reflects_vector_index_warmup() {
    let node = Node::bootstrap(&mut ServerConfig::default()).unwrap();
    let alias = "emb__BOOK__embedding__cosine__3";
    node.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(alias.into()),
            label: "BOOK".into(),
            property: "embedding".into(),
            dimensions: 3,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    assert!(
        node.indexes_warmed(),
        "an index with no source values is settled"
    );

    let record_id = node
        .scope()
        .create_record(
            "BOOK",
            &serde_json::json!({ "embedding": [1.0, 0.0, 0.0] }),
            &[],
            &serde_json::json!({}),
        )
        .unwrap()
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();
    assert!(
        !node.indexes_warmed(),
        "an unbound source value must leave the index behind"
    );

    node.scope()
        .upsert_vector_index(
            alias,
            &[VectorItem {
                record_id,
                vector: vec![1.0, 0.0, 0.0],
                source_hash: None,
                index_generation: None,
            }],
        )
        .unwrap();
    assert!(node.indexes_warmed(), "binding every source value warms it");
}

/// Bucket 9b, landed: on-disk ANN segments are a rebuildable cache keyed by
/// tenant, so `purge_tenant` must remove them wholesale alongside dropping
/// the in-memory vector-engine handle — not leave them orphaned on disk for
/// a future tenant id to collide with (even though a fingerprint mismatch
/// would still catch that safely, it shouldn't have to).
#[test]
fn purge_tenant_removes_persisted_ann_segments() {
    let dir = rushdb_storage::TempPath::dir("purge");
    let alias = "emb__BOOK__embedding__cosine__3";
    let node = disk_node(&dir);
    node.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(alias.into()),
            label: "BOOK".into(),
            property: "embedding".into(),
            dimensions: 3,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    let record_id = node
        .scope()
        .create_record(
            "BOOK",
            &serde_json::json!({ "embedding": [1.0, 0.0, 0.0] }),
            &[],
            &serde_json::json!({}),
        )
        .unwrap()
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();
    node.scope()
        .upsert_vector_index(
            alias,
            &[VectorItem {
                record_id,
                vector: vec![1.0, 0.0, 0.0],
                source_hash: None,
                index_generation: None,
            }],
        )
        .unwrap();
    // Forces a full ANN rebuild, which persists a frozen segment under the
    // configured data directory (see `vector_index_serves_persisted_bindings_
    // after_reopen` above for the same mechanism).
    node.scope().revalidate_vector_index(alias).unwrap();

    let tenant_hex = rushdb_types::keys::tenant_hex(rushdb_types::ids::TenantId::single());
    let segments_dir = dir.join("vector-segments").join(&tenant_hex);
    assert!(
        segments_dir.exists(),
        "a persisted segment directory must exist for the tenant before purge"
    );

    node.scope().purge_tenant().unwrap();
    assert!(
        !segments_dir.exists(),
        "purge_tenant must remove the tenant's on-disk ANN segments, not just the in-memory cache"
    );

    std::fs::remove_dir_all(&dir).ok();
}
