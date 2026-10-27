//! Vector search mode: `auto` answers exactly up to
//! `resources.vector-exact-max-pool` vectors and on the ANN graph above it,
//! never waiting for a build; `exact`/`ann` force a path; `scoreThreshold`
//! drops weak hits; the response reports the path and the index's pool.

use rushdb_protocol::{
    ErrorCode, Operation, Query, RequestContext, RequestEnvelope, Response, VectorIndexDefinition,
    VectorItem, VectorQuery, VectorSearchMode, VectorSearchReport,
};
use rushdb_server::{Node, ServerConfig};

const EXACT_MAX_POOL: u64 = 4;

fn node() -> Node {
    let mut cfg = ServerConfig::default();
    cfg.resources.vector_exact_max_pool = EXACT_MAX_POOL;
    Node::bootstrap(&mut cfg).unwrap()
}

fn index(n: &Node, name: &str, label: &str, similarity: &str) {
    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some(name.into()),
            label: label.into(),
            property: "embedding".into(),
            dimensions: 2,
            similarity: similarity.into(),
            model: None,
            owner: None,
        })
        .unwrap();
}

/// Bind one vector per angle (radians from `[1, 0]`) to a new record.
fn bind(n: &Node, name: &str, label: &str, angles: &[f32]) -> Vec<String> {
    let mut ids = Vec::new();
    let items: Vec<VectorItem> = angles
        .iter()
        .map(|angle| {
            let vector = vec![angle.cos(), angle.sin()];
            let id = n
                .scope()
                .create_record(
                    label,
                    &serde_json::json!({ "embedding": vector }),
                    &[],
                    &serde_json::json!({}),
                )
                .unwrap()["id"]
                .as_str()
                .unwrap()
                .to_string();
            ids.push(id.clone());
            VectorItem {
                record_id: id,
                vector,
                source_hash: None,
                index_generation: None,
            }
        })
        .collect();
    n.scope().upsert_vector_index(name, &items).unwrap();
    ids
}

fn query(name: &str, label: &str) -> VectorQuery {
    VectorQuery {
        labels: vec![label.into()],
        property: "embedding".into(),
        vector: vec![1.0, 0.0],
        similarity: "cosine".into(),
        index: Some(name.into()),
        predicate: serde_json::json!({}),
        offset: 0,
        // Below the index size: an ANN gather shorter than `min(limit, ef)`
        // is answered exactly (`lowConfidence`), which a tiny index always is.
        limit: 2,
        mode: None,
        ef: None,
        score_threshold: None,
    }
}

fn search(n: &Node, query: &VectorQuery) -> Result<(usize, VectorSearchReport), ErrorCode> {
    let response = rushdb_server::protocol::ProtocolExecutor::new(n, None)
        .execute(RequestEnvelope {
            protocol_version: 1,
            request_id: None,
            context: RequestContext::default(),
            operation: Operation::Query {
                query: Query::Vector {
                    query: query.clone(),
                },
            },
        })
        .map_err(|error| error.code)?;
    let Response::Records {
        records,
        vector_search,
        ..
    } = response.response
    else {
        panic!("expected records")
    };
    Ok((records.len(), vector_search.expect("vector search report")))
}

fn report(mode: &str, ef: Option<usize>, fallback: Option<&str>, pool: u64) -> VectorSearchReport {
    VectorSearchReport {
        mode: mode.into(),
        ef,
        fallback: fallback.map(Into::into),
        pool,
    }
}

#[test]
fn auto_searches_small_indexes_exactly_and_large_ones_on_a_warmed_graph() {
    let n = node();
    index(&n, "docs", "DOC", "cosine");
    bind(&n, "docs", "DOC", &[0.0, 0.1, 0.2]);
    assert_eq!(
        search(&n, &query("docs", "DOC")).unwrap(),
        (2, report("exact", None, None, 3))
    );

    bind(&n, "docs", "DOC", &[0.3, 0.4, 0.5]);
    // Above the threshold the graph is not resident yet: answered exactly
    // while it is built in the background.
    assert_eq!(
        search(&n, &query("docs", "DOC")).unwrap(),
        (2, report("exact", None, Some("annWarming"), 6))
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let warmed = loop {
        let (count, report) = search(&n, &query("docs", "DOC")).unwrap();
        if report.mode == "ann" || std::time::Instant::now() > deadline {
            break (count, report);
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    // `ef` is the server default (64), at least `offset + limit`.
    assert_eq!(warmed, (2, report("ann", Some(64), None, 6)));
    // VEC-3: every search is counted by the path that answered it, so an
    // ANN-sized pool answered exactly is visible.
    let rendered = n.metrics_render();
    let searches = |path: &str, reason: &str| {
        let prefix =
            format!("rushdb_vector_searches_total{{path=\"{path}\",reason=\"{reason}\"}} ");
        rendered
            .lines()
            .find_map(|line| line.strip_prefix(&prefix))
            .and_then(|value| value.trim().parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    assert!(searches("exact", "none") >= 1.0, "{rendered}");
    assert!(searches("exact", "annWarming") >= 1.0, "{rendered}");
    assert!(searches("ann", "none") >= 1.0, "{rendered}");
}

#[test]
fn forced_modes_are_honoured_and_ann_needs_a_cosine_index() {
    let n = node();
    index(&n, "docs", "DOC", "cosine");
    bind(&n, "docs", "DOC", &[0.0, 0.1, 0.2]);

    let mut exact = query("docs", "DOC");
    exact.mode = Some(VectorSearchMode::Exact);
    assert_eq!(
        search(&n, &exact).unwrap().1,
        report("exact", None, None, 3)
    );

    let mut ann = query("docs", "DOC");
    ann.mode = Some(VectorSearchMode::Ann);
    ann.ef = Some(1);
    // A forced ANN search builds the graph itself; `ef` is at least `k`.
    assert_eq!(search(&n, &ann).unwrap().1, report("ann", Some(2), None, 3));

    index(&n, "dist", "DIST", "euclidean");
    bind(&n, "dist", "DIST", &[0.0, 0.1, 0.2, 0.3, 0.4, 0.5]);
    let mut forced = query("dist", "DIST");
    forced.mode = Some(VectorSearchMode::Ann);
    assert_eq!(search(&n, &forced).unwrap_err(), ErrorCode::InvalidRequest);
    // `auto` over a large euclidean index stays exact: it has no graph.
    assert_eq!(
        search(&n, &query("dist", "DIST")).unwrap().1,
        report("exact", None, None, 6)
    );
}

#[test]
fn score_threshold_drops_weak_hits_on_both_paths() {
    let n = node();
    index(&n, "docs", "DOC", "cosine");
    // cos(0) = 1, cos(0.3) ≈ 0.955, cos(1.2) ≈ 0.362, cos(1.5) ≈ 0.071.
    bind(&n, "docs", "DOC", &[0.0, 0.3, 1.2, 1.5]);
    for mode in [VectorSearchMode::Exact, VectorSearchMode::Ann] {
        let mut strict = query("docs", "DOC");
        strict.mode = Some(mode);
        strict.score_threshold = Some(0.9);
        strict.limit = 3;
        assert_eq!(search(&n, &strict).unwrap().0, 2, "{mode:?}");
    }
}

#[test]
fn the_reported_pool_follows_ready_vectors() {
    let n = node();
    index(&n, "docs", "DOC", "cosine");
    let ids = bind(&n, "docs", "DOC", &[0.0, 0.1, 0.2]);
    assert_eq!(search(&n, &query("docs", "DOC")).unwrap().1.pool, 3);

    // A deleted record's binding is retired by revalidation.
    n.scope().delete_record(&ids[0]).unwrap();
    n.scope().revalidate_vector_index("docs").unwrap();
    assert_eq!(search(&n, &query("docs", "DOC")).unwrap().1.pool, 2);

    // A dropped index starts again from zero.
    n.scope().drop_vector_index("docs").unwrap();
    index(&n, "docs", "DOC", "cosine");
    bind(&n, "docs", "DOC", &[0.4]);
    assert_eq!(search(&n, &query("docs", "DOC")).unwrap().1.pool, 1);
}
