//! 4B — vector filter-semantics fixtures. Exact search is the recall oracle:
//! a vector query with a predicate must return exactly the records that match
//! the predicate (filtered recall == 1.0), never a top-k-ranked record outside
//! the filter.

use rushdb_authz::{AuthorizationMode, DataAccessPolicy};
use rushdb_lmpg::plan::FieldChange;
use rushdb_protocol::{
    Operation, Query, RequestContext, RequestEnvelope, Response, VectorIndexCandidatesQuery,
    VectorIndexDefinition, VectorItem, VectorQuery,
};
use rushdb_server::{Node, ServerConfig};

fn node() -> Node {
    let mut cfg = ServerConfig::default();
    Node::bootstrap(&mut cfg).unwrap()
}

fn create_vector_record(n: &Node, label: &str, tag: &str, vector: &[f32]) -> String {
    n.scope()
        .create_record(
            label,
            &serde_json::json!({ "tag": tag, "embedding": vector }),
            &[],
            &serde_json::json!({}),
        )
        .unwrap()
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string()
}

fn vector_find(n: &Node, query: &VectorQuery) -> Vec<(String, String)> {
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("vector-fixture".into()),
        context: RequestContext::default(),
        operation: Operation::Query {
            query: Query::Vector {
                query: query.clone(),
            },
        },
    };
    let res = rushdb_server::protocol::ProtocolExecutor::new(n, None)
        .execute(envelope)
        .unwrap();
    let Response::Records { records, .. } = res.response else {
        panic!("expected records, got {:?}", res.response);
    };
    records
        .into_iter()
        .filter_map(|r| {
            let obj = r.as_object()?;
            let id = obj.get("id").and_then(|v| v.as_str())?.to_string();
            let tag = obj
                .get("properties")
                .and_then(|p| p.get("tag"))
                .and_then(|v| v.as_str())?
                .to_string();
            Some((id, tag))
        })
        .collect()
}

#[test]
fn policy_masks_vector_record_payloads() {
    let n = node();
    let alias = "emb__DOC__embedding__cosine__2";
    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(alias.into()),
            label: "DOC".into(),
            property: "embedding".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    let id = n
        .scope()
        .create_record(
            "DOC",
            &serde_json::json!({"title": "visible", "secret": "hidden"}),
            &[],
            &serde_json::json!({}),
        )
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    n.scope()
        .upsert_vector_index(
            alias,
            &[VectorItem {
                record_id: id.clone(),
                vector: vec![1.0, 0.0],
                source_hash: None,
                index_generation: None,
            }],
        )
        .unwrap();
    n.scope()
        .security_put_policy(
            &DataAccessPolicy {
                allowed_labels: vec!["DOC".into()],
                visible_properties: std::collections::BTreeMap::from([(
                    "DOC".into(),
                    vec!["title".into()],
                )]),
                ..Default::default()
            },
            AuthorizationMode::PolicyEnforced,
        )
        .unwrap();
    let query = VectorQuery {
        labels: vec!["DOC".into()],
        property: "embedding".into(),
        vector: vec![1.0, 0.0],
        similarity: "cosine".into(),
        index: Some(alias.into()),
        predicate: serde_json::json!({}),
        offset: 0,
        limit: 1,
        ef: None,
        mode: None,
        score_threshold: None,
    };
    let response = rushdb_server::protocol::ProtocolExecutor::new(&n, None)
        .execute(RequestEnvelope {
            protocol_version: 1,
            request_id: Some("policy-vector".into()),
            context: RequestContext::default(),
            operation: Operation::Query {
                query: Query::Vector { query },
            },
        })
        .unwrap()
        .response;
    let Response::Records { records, .. } = response else {
        panic!("expected vector records");
    };
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["id"], id);
    assert_eq!(records[0]["properties"]["title"], "visible");
    assert!(records[0]["properties"].get("secret").is_none());
}

#[test]
fn vector_search_filter_recall_matches_exact_oracle() {
    let n = node();
    let alias = "emb__FRUIT__embedding__cosine__2";
    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(alias.into()),
            label: "FRUIT".into(),
            property: "embedding".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    // Records with a `kind` tag + a 2-d vector. All vectors are near [1,0] so
    // the query vector ranks them all; the predicate must be the ONLY filter.
    let apple = create_vector_record(&n, "FRUIT", "apple", &[0.99, 0.05]);
    let banana = create_vector_record(&n, "FRUIT", "banana", &[0.98, 0.1]);
    let cherry = create_vector_record(&n, "FRUIT", "cherry", &[0.97, 0.15]);
    let _ = apple;
    n.scope()
        .upsert_vector_index(
            alias,
            &[
                VectorItem {
                    record_id: apple,
                    vector: vec![0.99, 0.05],
                    source_hash: None,
                    index_generation: None,
                },
                VectorItem {
                    record_id: banana,
                    vector: vec![0.98, 0.1],
                    source_hash: None,
                    index_generation: None,
                },
                VectorItem {
                    record_id: cherry,
                    vector: vec![0.97, 0.15],
                    source_hash: None,
                    index_generation: None,
                },
            ],
        )
        .unwrap();

    // Predicate: only banana. Exact-oracle expectation: exactly banana.
    let hit = vector_find(
        &n,
        &VectorQuery {
            labels: vec!["FRUIT".to_string()],
            property: "embedding".to_string(),
            vector: vec![1.0, 0.0],
            similarity: "cosine".to_string(),
            index: Some(alias.into()),
            predicate: serde_json::json!({ "tag": "banana" }),
            offset: 0,
            limit: 10,
            ef: None,
            mode: None,
            score_threshold: None,
        },
    );
    assert_eq!(hit.len(), 1, "filtered recall must be exact: {hit:?}");
    assert_eq!(hit[0].1, "banana");

    // Predicate: apple or cherry (2 records). The query vector still ranks all
    // three, so a post-rank filter could return banana — but the engine filters
    // BEFORE top-k, so it must return exactly the two matching.
    let hit2 = vector_find(
        &n,
        &VectorQuery {
            labels: vec!["FRUIT".to_string()],
            property: "embedding".to_string(),
            vector: vec![1.0, 0.0],
            similarity: "cosine".to_string(),
            index: Some(alias.into()),
            predicate: serde_json::json!({ "tag": { "$in": ["apple", "cherry"] } }),
            offset: 0,
            limit: 10,
            ef: None,
            mode: None,
            score_threshold: None,
        },
    );
    let mut tags: Vec<&str> = hit2.iter().map(|(_, t)| t.as_str()).collect();
    tags.sort_unstable();
    assert_eq!(
        tags,
        vec!["apple", "cherry"],
        "must not leak banana: {hit2:?}"
    );
}

#[test]
fn ann_ef_query_uses_the_index_and_matches_exact_top_hit() {
    let n = node();
    let alias = "emb__ANNEF__embedding__cosine__2";
    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(alias.into()),
            label: "ANNEF".into(),
            property: "embedding".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    let mut items = Vec::new();
    for i in 0..32u16 {
        let angle = f32::from(i) * 0.1;
        let vector = vec![angle.cos(), angle.sin()];
        let record_id = create_vector_record(&n, "ANNEF", &format!("r{i}"), &vector);
        items.push(VectorItem {
            record_id,
            vector,
            source_hash: None,
            index_generation: None,
        });
    }
    n.scope().upsert_vector_index(alias, &items).unwrap();

    let query = |ef: Option<usize>| VectorQuery {
        labels: vec!["ANNEF".into()],
        property: "embedding".into(),
        vector: vec![1.0, 0.0],
        similarity: "cosine".into(),
        index: Some(alias.into()),
        predicate: serde_json::json!({}),
        offset: 0,
        limit: 5,
        ef,
        mode: None,
        score_threshold: None,
    };
    let exact = vector_find(&n, &query(None));
    let approximate = vector_find(&n, &query(Some(32)));
    assert!(!exact.is_empty(), "exact search must return hits");
    assert_eq!(
        approximate, exact,
        "ANN (ef=32) with exact rerank must match the exact top-k on this corpus"
    );
}

#[test]
fn revalidate_reports_ann_build_failure_without_failing_reconciliation() {
    // A tiny derived-cache budget makes the ANN build fail. Revalidation must
    // still succeed (the durable binding-status reconciliation happened) and
    // report the derived-build failure instead of returning an error over a
    // partial mutation.
    let mut cfg = ServerConfig::default();
    cfg.resources.logical_cache_bytes = rushdb_server::resource_config::ByteSize(64);
    let n = Node::bootstrap(&mut cfg).unwrap();
    let alias = "emb__REVAL__embedding__cosine__2";
    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(alias.into()),
            label: "REVAL".into(),
            property: "embedding".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    let record_id = create_vector_record(&n, "REVAL", "a", &[1.0, 0.0]);
    n.scope()
        .upsert_vector_index(
            alias,
            &[VectorItem {
                record_id,
                vector: vec![1.0, 0.0],
                source_hash: None,
                index_generation: None,
            }],
        )
        .unwrap();

    let result = n.scope().revalidate_vector_index(alias).unwrap();
    assert!(
        result["annRebuildError"].is_string(),
        "tiny cache must fail the derived ANN build: {result}"
    );
    assert!(result["stale"].is_number());
    assert!(result["retired"].is_number());
}

#[test]
fn zero_cache_budget_refuses_resident_ann_and_searches_exactly() {
    // With the logical cache budget disabled there is no bound on a resident
    // ANN build: the build is refused with an actionable error, and an `ef`
    // search answers exactly instead of failing.
    let mut cfg = ServerConfig::default();
    cfg.resources.logical_cache_bytes = rushdb_server::resource_config::ByteSize(0);
    let n = Node::bootstrap(&mut cfg).unwrap();
    let alias = "emb__ZERO__embedding__cosine__2";
    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(alias.into()),
            label: "ZERO".into(),
            property: "embedding".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    let record_id = create_vector_record(&n, "ZERO", "a", &[1.0, 0.0]);
    n.scope()
        .upsert_vector_index(
            alias,
            &[VectorItem {
                record_id,
                vector: vec![1.0, 0.0],
                source_hash: None,
                index_generation: None,
            }],
        )
        .unwrap();
    let result = n.scope().revalidate_vector_index(alias).unwrap();
    let error = result["annRebuildError"]
        .as_str()
        .expect("zero cache budget must fail the ANN build");
    assert!(
        error.contains("logical cache budget"),
        "error must name the missing budget: {error}"
    );
    let query = |ef: Option<usize>| VectorQuery {
        labels: vec!["ZERO".into()],
        property: "embedding".into(),
        vector: vec![1.0, 0.0],
        similarity: "cosine".into(),
        index: Some(alias.into()),
        predicate: serde_json::json!({}),
        offset: 0,
        limit: 1,
        ef,
        mode: None,
        score_threshold: None,
    };
    assert_eq!(
        vector_find(&n, &query(Some(16))),
        vector_find(&n, &query(None))
    );
}

#[test]
fn vector_query_without_labels_searches_the_index_label() {
    let n = node();
    let alias = "emb__NOLABEL__embedding__cosine__2";
    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(alias.into()),
            label: "NOLABEL".into(),
            property: "embedding".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    let record_id = create_vector_record(&n, "NOLABEL", "only", &[1.0, 0.0]);
    n.scope()
        .upsert_vector_index(
            alias,
            &[VectorItem {
                record_id,
                vector: vec![1.0, 0.0],
                source_hash: None,
                index_generation: None,
            }],
        )
        .unwrap();
    let hits = vector_find(
        &n,
        &VectorQuery {
            labels: vec![],
            property: "embedding".into(),
            vector: vec![1.0, 0.0],
            similarity: "cosine".into(),
            index: Some(alias.into()),
            predicate: serde_json::json!({}),
            offset: 0,
            limit: 5,
            ef: None,
            mode: None,
            score_threshold: None,
        },
    );
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].1, "only");
}

#[test]
fn stale_index_generation_rejects_late_vector_completion() {
    let n = node();
    let definition = VectorIndexDefinition {
        name: Some("generation-index".into()),
        label: "DOC".into(),
        property: "embedding".into(),
        dimensions: 2,
        similarity: "cosine".into(),
        model: Some("model-v1".into()),
        owner: None,
    };
    let first = n.scope().create_vector_index(&definition).unwrap();
    let old_generation = first["generation"].as_str().unwrap().to_owned();
    n.scope().drop_vector_index("generation-index").unwrap();
    let second = n.scope().create_vector_index(&definition).unwrap();
    let new_generation = second["generation"].as_str().unwrap();
    assert_ne!(old_generation, new_generation);

    let id = create_vector_record(&n, "DOC", "late", &[1.0, 0.0]);
    let current = create_vector_record(&n, "DOC", "current", &[0.0, 1.0]);
    let error = n
        .scope()
        .upsert_vector_index(
            "generation-index",
            &[
                VectorItem {
                    record_id: current,
                    vector: vec![0.0, 1.0],
                    source_hash: None,
                    index_generation: Some(new_generation.to_owned()),
                },
                VectorItem {
                    record_id: id,
                    vector: vec![1.0, 0.0],
                    source_hash: None,
                    index_generation: Some(old_generation),
                },
            ],
        )
        .expect_err("old generation completion");
    assert!(error.contains("stale vector index generation"));
    assert_eq!(
        n.scope().vector_index_stats("generation-index").unwrap()["indexedRecords"],
        0
    );
}

#[test]
fn vector_upsert_rejects_wrong_dimensions_before_binding_any_item() {
    let n = node();
    let alias = "dimension-index";
    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(alias.into()),
            label: "DOC".into(),
            property: "embedding".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    let first = create_vector_record(&n, "DOC", "first", &[1.0, 0.0]);
    let second = create_vector_record(&n, "DOC", "second", &[0.0, 1.0]);
    let error = n
        .scope()
        .upsert_vector_index(
            alias,
            &[
                VectorItem {
                    record_id: first,
                    vector: vec![1.0, 0.0],
                    source_hash: None,
                    index_generation: None,
                },
                VectorItem {
                    record_id: second,
                    vector: vec![0.0, 1.0, 0.0],
                    source_hash: None,
                    index_generation: None,
                },
            ],
        )
        .expect_err("wrong-dimension vectors must fail closed");
    assert!(error.contains("does not match index dimension"));
    assert_eq!(
        n.scope().vector_index_stats(alias).unwrap()["indexedRecords"],
        0
    );
}

#[test]
fn explain_vector_reports_access_path_and_candidate_count() {
    use rushdb_protocol::{Describe, Operation, RequestContext, RequestEnvelope, Response};
    let n = node();
    let alias = "emb__FRUIT__embedding__cosine__2";
    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(alias.into()),
            label: "FRUIT".into(),
            property: "embedding".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    for (tag, vec) in [("apple", vec![0.99, 0.05]), ("banana", vec![0.98, 0.1])] {
        let id = n
            .scope()
            .create_record(
                "FRUIT",
                &serde_json::json!({ "tag": tag, "embedding": vec }),
                &[],
                &serde_json::json!({}),
            )
            .unwrap()
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();
        n.scope()
            .upsert_vector_index(
                alias,
                &[VectorItem {
                    record_id: id,
                    vector: vec,
                    source_hash: None,
                    index_generation: None,
                }],
            )
            .unwrap();
    }
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("explain-vector".into()),
        context: RequestContext::default(),
        operation: Operation::Describe {
            describe: Describe::ExplainVector {
                query: VectorQuery {
                    labels: vec!["FRUIT".to_string()],
                    property: "embedding".to_string(),
                    vector: vec![1.0, 0.0],
                    similarity: "cosine".to_string(),
                    index: None,
                    predicate: serde_json::json!({ "tag": "banana" }),
                    offset: 0,
                    limit: 10,
                    ef: None,
                    mode: None,
                    score_threshold: None,
                },
            },
        },
    };
    let res = rushdb_server::protocol::ProtocolExecutor::new(&n, None)
        .execute(envelope)
        .unwrap();
    let Response::ExplainVector { plan } = res.response else {
        panic!("expected explain vector, got {:?}", res.response);
    };
    assert_eq!(plan.access_path, "vector_index");
    assert_eq!(plan.label, "FRUIT");
    assert_eq!(
        plan.candidate_count, 1,
        "banana predicate resolves 1 candidate"
    );
    assert_eq!(plan.k, 10);
    assert_eq!(plan.predicate, serde_json::json!({ "tag": "banana" }));
}

/// A custom-named index (two same-shape embedding models) must bind and search
/// through its explicit name, not a re-derived legacy alias.
#[test]
fn custom_named_index_binds_and_searches_by_name() {
    let n = node();
    let custom = "openai.text-embedding-3-small.v2";
    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(custom.into()),
            label: "FRUIT".into(),
            property: "embedding".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    let apple = create_vector_record(&n, "FRUIT", "apple", &[1.0, 0.0]);
    let banana = create_vector_record(&n, "FRUIT", "banana", &[0.9, 0.1]);
    let result = n
        .scope()
        .upsert_vector_index(
            custom,
            &[
                VectorItem {
                    record_id: apple,
                    vector: vec![1.0, 0.0],
                    source_hash: None,
                    index_generation: None,
                },
                VectorItem {
                    record_id: banana,
                    vector: vec![0.9, 0.1],
                    source_hash: None,
                    index_generation: None,
                },
            ],
        )
        .unwrap();
    assert_eq!(result["requested"], 2);
    assert_eq!(
        result["updated"], 2,
        "a custom-named index must accept vectors: {result}"
    );

    let hits = vector_find(
        &n,
        &VectorQuery {
            labels: vec!["FRUIT".into()],
            property: "embedding".into(),
            vector: vec![1.0, 0.0],
            similarity: "cosine".into(),
            index: Some(custom.into()),
            predicate: serde_json::json!({}),
            offset: 0,
            limit: 10,
            ef: None,
            mode: None,
            score_threshold: None,
        },
    );
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].1, "apple");
}

/// Two indexes with identical `(label, property, dims, similarity)` are
/// distinct spaces when explicitly named: vectors never cross-talk.
#[test]
fn same_shape_spaces_do_not_cross_talk() {
    let n = node();
    for name in ["space_a", "space_b"] {
        n.scope()
            .create_vector_index(&VectorIndexDefinition {
                name: Some(name.into()),
                label: "FRUIT".into(),
                property: "embedding".into(),
                dimensions: 2,
                similarity: "cosine".into(),
                model: None,
                owner: None,
            })
            .unwrap();
    }
    let apple = create_vector_record(&n, "FRUIT", "apple", &[1.0, 0.0]);
    let banana = create_vector_record(&n, "FRUIT", "banana", &[0.0, 1.0]);
    n.scope()
        .upsert_vector_index(
            "space_a",
            &[VectorItem {
                record_id: apple,
                vector: vec![1.0, 0.0],
                source_hash: None,
                index_generation: None,
            }],
        )
        .unwrap();
    n.scope()
        .upsert_vector_index(
            "space_b",
            &[VectorItem {
                record_id: banana,
                vector: vec![1.0, 0.0],
                source_hash: None,
                index_generation: None,
            }],
        )
        .unwrap();

    let query = |index: &str| VectorQuery {
        labels: vec!["FRUIT".into()],
        property: "embedding".into(),
        vector: vec![1.0, 0.0],
        similarity: "cosine".into(),
        index: Some(index.into()),
        predicate: serde_json::json!({}),
        offset: 0,
        limit: 10,
        ef: None,
        mode: None,
        score_threshold: None,
    };
    let a: Vec<String> = vector_find(&n, &query("space_a"))
        .into_iter()
        .map(|(_, tag)| tag)
        .collect();
    let b: Vec<String> = vector_find(&n, &query("space_b"))
        .into_iter()
        .map(|(_, tag)| tag)
        .collect();
    assert_eq!(a, vec!["apple"]);
    assert_eq!(b, vec!["banana"]);
}

/// Explicit aliases are validated and a declared model is immutable under an
/// existing name, so incomparable spaces cannot be mixed silently.
#[test]
fn index_alias_validation_and_model_provenance() {
    let n = node();
    let def = |name: Option<&str>, model: Option<&str>| VectorIndexDefinition {
        label: "FRUIT".into(),
        property: "embedding".into(),
        dimensions: 2,
        similarity: "cosine".into(),
        name: name.map(str::to_string),
        model: model.map(str::to_string),
        owner: None,
    };

    for bad in ["", "has space", "slash/name", "emb__reserved__x"] {
        let err = n
            .scope()
            .create_vector_index(&def(Some(bad), None))
            .unwrap_err();
        assert!(err.contains("vector index name"), "name `{bad}`: {err}");
    }
    let long = "a".repeat(65);
    assert!(n
        .scope()
        .create_vector_index(&def(Some(&long), None))
        .is_err());

    let created = n
        .scope()
        .create_vector_index(&def(
            Some("space.model.v1"),
            Some("openai/text-embedding-3-small"),
        ))
        .unwrap();
    assert_eq!(created["nameSource"], "explicit");
    assert_eq!(created["model"], "openai/text-embedding-3-small");

    let err = n
        .scope()
        .create_vector_index(&def(Some("space.model.v1"), Some("qwen/other")))
        .unwrap_err();
    assert!(err.contains("different model"), "{err}");
    n.scope()
        .create_vector_index(&def(
            Some("space.model.v1"),
            Some("openai/text-embedding-3-small"),
        ))
        .unwrap();

    // Implicit derivation itself (opt-out of the now-mandatory gate) is a
    // separate concern from the alias/model validation above, so it gets its
    // own node with the gate explicitly disabled.
    let mut off_cfg = ServerConfig::default();
    off_cfg.resources.require_vector_index_name = false;
    let off_node = Node::bootstrap(&mut off_cfg).unwrap();
    let derived = off_node
        .scope()
        .create_vector_index(&def(None, None))
        .unwrap();
    assert_eq!(derived["nameSource"], "derived");
}

/// Relationship-owned vector indexes: an index over a relationship type binds
/// and searches relationships (not records), with the same exact filter oracle.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one linear fixture-build -> index -> search -> assert flow; splitting it would scatter shared setup"
)]
fn relationship_owned_vector_index_binds_and_searches() {
    let n = node();
    let lmpg = n.scope().lmpg();
    let a = lmpg
        .create_record(
            "PERSON",
            None,
            vec![("name".into(), FieldChange::Set(serde_json::json!("a")))],
        )
        .unwrap();
    let b = lmpg
        .create_record(
            "PERSON",
            None,
            vec![("name".into(), FieldChange::Set(serde_json::json!("b")))],
        )
        .unwrap();
    let rel = lmpg
        .create_relationship(
            "KNOWS",
            a,
            b,
            vec![(
                "embedding".into(),
                FieldChange::Set(serde_json::json!("friend")),
            )],
        )
        .unwrap();
    let rel_hex = rel
        .relationship_id
        .iter()
        .fold(String::with_capacity(32), |mut out, byte| {
            use std::fmt::Write as _;
            let _ = write!(out, "{byte:02x}");
            out
        });

    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            label: "KNOWS".into(),
            property: "embedding".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            name: Some("rel-index".into()),
            model: None,
            owner: Some("relationship".into()),
        })
        .unwrap();
    // The backfill helper lists the unbound relationship before it is bound.
    let (candidates, total) = n
        .scope()
        .vector_index_candidates(&VectorIndexCandidatesQuery {
            label: "KNOWS".into(),
            property: "embedding".into(),
            index: "rel-index".into(),
            skip: 0,
            limit: 10,
        })
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0]["value"], serde_json::json!("friend"));

    let updated = n
        .scope()
        .upsert_vector_index(
            "rel-index",
            &[VectorItem {
                record_id: rel_hex,
                vector: vec![1.0, 0.0],
                source_hash: None,
                index_generation: None,
            }],
        )
        .unwrap();
    assert_eq!(updated["updated"], 1, "{updated}");

    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("rel-vector".into()),
        context: RequestContext::default(),
        operation: Operation::Query {
            query: Query::Vector {
                query: VectorQuery {
                    labels: vec!["KNOWS".into()],
                    property: "embedding".into(),
                    vector: vec![1.0, 0.0],
                    similarity: "cosine".into(),
                    index: Some("rel-index".into()),
                    predicate: serde_json::json!({}),
                    offset: 0,
                    limit: 10,
                    ef: None,
                    mode: None,
                    score_threshold: None,
                },
            },
        },
    };
    let res = rushdb_server::protocol::ProtocolExecutor::new(&n, None)
        .execute(envelope)
        .unwrap();
    let Response::Records { records, .. } = res.response else {
        panic!("expected records, got {:?}", res.response);
    };
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["__type"], serde_json::json!("KNOWS"));
    assert_eq!(records[0]["embedding"], serde_json::json!("friend"));
}

/// Inline `$vectors` with an explicit `index` binds to that space; a missing
/// name fails before any record is created.
#[test]
fn inline_vectors_bind_to_the_explicit_index() {
    let n = node();
    let custom = "custom-inline";
    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(custom.into()),
            label: "FRUIT".into(),
            property: "embedding".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    n.scope()
        .create_record(
            "FRUIT",
            &serde_json::json!({ "tag": "apple" }),
            &[serde_json::json!({
                "propertyName": "embedding",
                "vector": [1.0, 0.0],
                "index": custom,
            })],
            &serde_json::json!({}),
        )
        .unwrap();
    let hits = vector_find(
        &n,
        &VectorQuery {
            labels: vec!["FRUIT".into()],
            property: "embedding".into(),
            vector: vec![1.0, 0.0],
            similarity: "cosine".into(),
            index: Some(custom.into()),
            predicate: serde_json::json!({}),
            offset: 0,
            limit: 10,
            ef: None,
            mode: None,
            score_threshold: None,
        },
    );
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].1, "apple");

    let err = n
        .scope()
        .create_record(
            "FRUIT",
            &serde_json::json!({ "tag": "ghost" }),
            &[serde_json::json!({
                "propertyName": "embedding",
                "vector": [1.0, 0.0],
                "index": "does-not-exist",
            })],
            &serde_json::json!({}),
        )
        .unwrap_err();
    assert!(
        err.contains("no embedding index named"),
        "missing explicit index must fail loudly, got: {err}"
    );
}

/// C9 (cloud-capacity F10): inline (external) vectors are not stored
/// properties, so an index whose every record is bound must report `ready`
/// with honest counts — not `awaiting_vectors` with `totalRecords: 0`.
#[test]
fn inline_vector_index_stats_report_ready_with_bound_counts() {
    let n = node();
    let alias = "inline-stats";
    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(alias.into()),
            label: "DOC".into(),
            property: "emb".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    for i in 0..5u8 {
        n.scope()
            .create_record(
                "DOC",
                &serde_json::json!({ "n": i }),
                &[serde_json::json!({
                    "propertyName": "emb",
                    "vector": [1.0, f32::from(i)],
                    "index": alias,
                })],
                &serde_json::json!({}),
            )
            .unwrap();
    }
    let stats = n.scope().vector_index_stats(alias).unwrap();
    assert_eq!(stats["indexedRecords"], 5, "{stats}");
    assert_eq!(stats["totalRecords"], 5, "{stats}");
    assert_eq!(stats["unindexedRecords"], 0, "{stats}");
    assert_eq!(stats["status"], "ready", "{stats}");
}

/// C9: a managed index counts records carrying the property from the derived
/// support catalog — bound or not — exactly like the old reconstruction did.
#[test]
fn managed_vector_index_total_counts_records_carrying_the_property() {
    let n = node();
    let alias = "managed-stats";
    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(alias.into()),
            label: "FRUIT".into(),
            property: "embedding".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    for (tag, vector) in [("a", [1.0, 0.0]), ("b", [0.0, 1.0]), ("c", [1.0, 1.0])] {
        create_vector_record(&n, "FRUIT", tag, &vector);
    }
    // A record of the label without the property is not counted.
    n.scope()
        .create_record(
            "FRUIT",
            &serde_json::json!({ "tag": "bare" }),
            &[],
            &serde_json::json!({}),
        )
        .unwrap();
    let stats = n.scope().vector_index_stats(alias).unwrap();
    assert_eq!(stats["totalRecords"], 3, "{stats}");
}

/// C9 (cloud-capacity F11): an unfiltered search checks label membership per
/// candidate instead of materializing every member up front. A record deleted
/// after binding must still never be returned, by exact or ANN search.
#[test]
fn unfiltered_vector_search_excludes_deleted_records() {
    let n = node();
    let alias = "unfiltered";
    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(alias.into()),
            label: "FRUIT".into(),
            property: "embedding".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    let mut ids = Vec::new();
    for (tag, vector) in [
        ("keep", [1.0, 0.0]),
        ("gone", [0.99, 0.01]),
        ("far", [0.0, 1.0]),
    ] {
        let id = n
            .scope()
            .create_record(
                "FRUIT",
                &serde_json::json!({ "tag": tag }),
                &[serde_json::json!({ "propertyName": "embedding", "vector": vector, "index": alias })],
                &serde_json::json!({}),
            )
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        ids.push(id);
    }
    n.scope().delete_record(&ids[1]).unwrap();
    for ef in [None, Some(64)] {
        let hits = vector_find(
            &n,
            &VectorQuery {
                labels: vec!["FRUIT".into()],
                property: "embedding".into(),
                vector: vec![1.0, 0.0],
                similarity: "cosine".into(),
                index: Some(alias.into()),
                predicate: serde_json::json!({}),
                offset: 0,
                limit: 10,
                ef,
                mode: None,
                score_threshold: None,
            },
        );
        let tags: Vec<&str> = hits.iter().map(|(_, tag)| tag.as_str()).collect();
        assert_eq!(tags, ["keep", "far"], "ef={ef:?}");
    }
}

/// C9 (cloud-capacity F10): the ANN graph is a cache. When the vector budget
/// cannot admit a build, an ANN query must answer exactly instead of failing.
#[test]
fn ann_search_falls_back_to_exact_when_the_budget_cannot_admit_a_build() {
    let mut cfg = ServerConfig::default();
    // Room for the store's own needs, far too little for any ANN build.
    cfg.resources.logical_cache_bytes = rushdb_server::resource_config::ByteSize(64 * 1024);
    let n = Node::bootstrap(&mut cfg).unwrap();
    let alias = "tiny-budget";
    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(alias.into()),
            label: "FRUIT".into(),
            property: "embedding".into(),
            dimensions: 64,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    for i in 0..300u16 {
        let mut vector = vec![0.0_f32; 64];
        vector[usize::from(i % 64)] = 1.0;
        vector[0] += f32::from(i) / 1000.0;
        n.scope()
            .create_record(
                "FRUIT",
                &serde_json::json!({ "tag": format!("t{i}") }),
                &[serde_json::json!({ "propertyName": "embedding", "vector": vector, "index": alias })],
                &serde_json::json!({}),
            )
            .unwrap();
    }
    let mut query = vec![0.0_f32; 64];
    query[5] = 1.0;
    let search = |ef| {
        vector_find(
            &n,
            &VectorQuery {
                labels: vec!["FRUIT".into()],
                property: "embedding".into(),
                vector: query.clone(),
                similarity: "cosine".into(),
                index: Some(alias.into()),
                predicate: serde_json::json!({}),
                offset: 0,
                limit: 5,
                ef,
                mode: None,
                score_threshold: None,
            },
        )
    };
    assert_eq!(
        search(Some(64)),
        search(None),
        "ANN must equal exact when it falls back"
    );
}

/// Vector indexes are built from the current snapshot, never replayed from the
/// WAL, so declaring, binding and revalidating one must not pin WAL retention.
#[test]
fn vector_index_lifecycle_never_pins_wal_retention() {
    let n = node();
    let alias = "no-retention-pin";
    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(alias.into()),
            label: "DOC".into(),
            property: "emb".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    n.scope()
        .create_record(
            "DOC",
            &serde_json::json!({ "n": 1 }),
            &[serde_json::json!({ "propertyName": "emb", "vector": [1.0, 0.0], "index": alias })],
            &serde_json::json!({}),
        )
        .unwrap();
    n.scope().vector_index_stats(alias).unwrap();
    n.scope().revalidate_vector_index(alias).unwrap();
    assert_eq!(n.protected_retention_floor(), None);
}
