//! Vector search mode: `auto` answers exactly while the search could score
//! at most `resources.vector-exact-max-pool` vectors (a predicate's matching
//! owners, or the whole index) and on the ANN graph above it, never waiting
//! for a build; `exact`/`ann` force a path, though a forced `ann` over a
//! small filtered pool is answered exactly; `scoreThreshold` drops weak
//! hits; the response and `explain` report the path and the pool.

use rushdb_protocol::{
    Describe, ErrorCode, Operation, Query, RequestContext, RequestEnvelope, Response,
    VectorIndexDefinition, VectorItem, VectorQuery, VectorSearchMode, VectorSearchReport,
};
use rushdb_server::{Node, ServerConfig};

const EXACT_MAX_POOL: u64 = 4;

fn node() -> Node {
    node_with_pool(EXACT_MAX_POOL)
}

fn node_with_pool(exact_max_pool: u64) -> Node {
    let mut cfg = ServerConfig::default();
    cfg.resources.vector_exact_max_pool = exact_max_pool;
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

/// `count` records spread around the unit circle, each with one vector and
/// a `tenant` from `tenant_of(i)`.
#[expect(
    clippy::cast_precision_loss,
    reason = "a small record index mapped into f32 radians"
)]
fn bind_tenants(n: &Node, name: &str, label: &str, count: usize, tenant_of: fn(usize) -> u64) {
    let items: Vec<VectorItem> = (0..count)
        .map(|i| {
            let angle = i as f32 * 6.2 / count as f32;
            let vector = vec![angle.cos(), angle.sin()];
            let id = n
                .scope()
                .create_record(
                    label,
                    &serde_json::json!({ "embedding": vector, "tenant": tenant_of(i) }),
                    &[],
                    &serde_json::json!({}),
                )
                .unwrap()["id"]
                .as_str()
                .unwrap()
                .to_string();
            VectorItem {
                record_id: id,
                vector,
                source_hash: None,
                index_generation: None,
            }
        })
        .collect();
    n.scope().upsert_vector_index(name, &items).unwrap();
}

fn filtered(name: &str, label: &str, tenant: u64, mode: Option<VectorSearchMode>) -> VectorQuery {
    VectorQuery {
        predicate: serde_json::json!({ "tenant": tenant }),
        mode,
        limit: 3,
        ..query(name, label)
    }
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

fn execute(
    n: &Node,
    query: Query,
) -> Result<(Vec<serde_json::Value>, VectorSearchReport), ErrorCode> {
    let response = rushdb_server::protocol::ProtocolExecutor::new(n, None)
        .execute(RequestEnvelope {
            protocol_version: 1,
            request_id: None,
            context: RequestContext::default(),
            operation: Operation::Query { query },
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
    Ok((records, vector_search.expect("vector search report")))
}

fn ranked(
    n: &Node,
    query: &VectorQuery,
) -> Result<(Vec<serde_json::Value>, VectorSearchReport), ErrorCode> {
    execute(
        n,
        Query::Vector {
            query: query.clone(),
        },
    )
}

fn search(n: &Node, query: &VectorQuery) -> Result<(usize, VectorSearchReport), ErrorCode> {
    ranked(n, query).map(|(records, report)| (records.len(), report))
}

/// The route `explain` reports for `query`.
fn explain(n: &Node, query: &VectorQuery) -> rushdb_protocol::ExplainVectorPlan {
    let response = rushdb_server::protocol::ProtocolExecutor::new(n, None)
        .execute(RequestEnvelope {
            protocol_version: 1,
            request_id: None,
            context: RequestContext::default(),
            operation: Operation::Describe {
                describe: Describe::ExplainVector {
                    query: query.clone(),
                },
            },
        })
        .unwrap();
    let Response::ExplainVector { plan } = response.response else {
        panic!("expected an explain plan")
    };
    plan
}

/// Search unfiltered until `auto` answers on the resident ANN graph.
fn warm(n: &Node, name: &str, label: &str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while search(n, &query(name, label)).unwrap().1.mode != "ann" {
        assert!(
            std::time::Instant::now() < deadline,
            "the graph never warmed"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
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
    // `ef` is the server default (128), at least `offset + limit`.
    assert_eq!(warmed, (2, report("ann", Some(128), None, 6)));
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
fn the_reported_pool_is_what_the_search_could_score() {
    let n = node();
    index(&n, "docs", "DOC", "cosine");
    let ids = bind(&n, "docs", "DOC", &[0.0, 0.1, 0.2]);
    // Unfiltered: the index's ready vectors.
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

    // Filtered: the owners matching the predicate, never more than the
    // index holds.
    index(&n, "tagged", "TAG", "cosine");
    bind_tenants(&n, "tagged", "TAG", 9, |i| u64::from(i % 3 == 0));
    assert_eq!(search(&n, &query("tagged", "TAG")).unwrap().1.pool, 9);
    for (tenant, pool) in [(1, 3), (0, 6), (7, 0)] {
        let filtered = filtered("tagged", "TAG", tenant, Some(VectorSearchMode::Exact));
        assert_eq!(
            search(&n, &filtered).unwrap().1.pool,
            pool,
            "tenant {tenant}"
        );
    }
}

#[test]
fn auto_searches_a_narrow_filter_exactly_on_an_index_above_the_pool() {
    let n = node();
    index(&n, "docs", "DOC", "cosine");
    // 40 vectors (pool 4): tenant 0 holds 4 of them, spread over the circle.
    bind_tenants(&n, "docs", "DOC", 40, |i| u64::from(i % 10 != 0));
    warm(&n, "docs", "DOC");

    let (auto, auto_report) = ranked(&n, &filtered("docs", "DOC", 0, None)).unwrap();
    assert_eq!(auto_report, report("exact", None, None, 4));
    // Recall 1.0: the exact oracle's page, record for record.
    let (oracle, _) = ranked(
        &n,
        &filtered("docs", "DOC", 0, Some(VectorSearchMode::Exact)),
    )
    .unwrap();
    assert_eq!(auto.len(), 3);
    assert_eq!(auto, oracle);

    // A broad filter is above the pool and stays on the graph.
    let broad = search(&n, &filtered("docs", "DOC", 1, None)).unwrap().1;
    assert_eq!(broad, report("ann", Some(128), None, 36));
}

#[test]
fn a_forced_ann_over_a_small_candidate_set_is_answered_exactly() {
    let n = node();
    index(&n, "docs", "DOC", "cosine");
    bind_tenants(&n, "docs", "DOC", 40, |i| u64::from(i % 10 != 0));
    let ann = Some(VectorSearchMode::Ann);
    let (forced, forced_report) = ranked(&n, &filtered("docs", "DOC", 0, ann)).unwrap();
    assert_eq!(
        forced_report,
        report("exact", None, Some("smallCandidateSet"), 4)
    );
    let (oracle, _) = ranked(
        &n,
        &filtered("docs", "DOC", 0, Some(VectorSearchMode::Exact)),
    )
    .unwrap();
    assert_eq!(forced, oracle);
    // Above the pool a forced `ann` walks the graph.
    assert_eq!(
        search(&n, &filtered("docs", "DOC", 1, ann)).unwrap().1,
        report("ann", Some(128), None, 36)
    );
}

#[test]
fn explain_reports_the_route_the_search_takes() {
    let n = node();
    // The index is found by its name, not by the alias derived from the
    // label and property.
    index(&n, "docs", "DOC", "cosine");
    bind_tenants(&n, "docs", "DOC", 40, |i| u64::from(i % 10 != 0));
    warm(&n, "docs", "DOC");
    for (tenant, mode) in [
        (0, None),
        (1, None),
        (0, Some(VectorSearchMode::Ann)),
        (1, Some(VectorSearchMode::Ann)),
        (1, Some(VectorSearchMode::Exact)),
    ] {
        let query = filtered("docs", "DOC", tenant, mode);
        let plan = explain(&n, &query);
        assert_eq!(plan.alias, "docs");
        assert_eq!(plan.candidate_count, if tenant == 0 { 4 } else { 36 });
        assert_eq!(
            plan.search,
            search(&n, &query).unwrap().1,
            "{tenant} {mode:?}"
        );
    }
    let unfiltered = explain(&n, &query("docs", "DOC"));
    assert_eq!(unfiltered.candidate_count, 40);
    assert_eq!(unfiltered.search, report("ann", Some(128), None, 40));
}

#[test]
fn a_hybrid_vector_leg_routes_like_a_vector_search() {
    let n = node();
    index(&n, "docs", "DOC", "cosine");
    bind_tenants(&n, "docs", "DOC", 40, |i| u64::from(i % 10 != 0));
    warm(&n, "docs", "DOC");
    let hybrid = |tenant: u64, mode: &str| {
        let query = serde_json::from_value(serde_json::json!({
            "labels": ["DOC"],
            "predicate": { "tenant": tenant },
            "limit": 3,
            "vector": {
                "property": "embedding",
                "vector": [1.0, 0.0],
                "index": "docs",
                "mode": mode,
            },
        }))
        .unwrap();
        execute(&n, Query::Hybrid { query }).unwrap().1
    };
    assert_eq!(hybrid(0, "auto"), report("exact", None, None, 4));
    assert_eq!(
        hybrid(0, "ann"),
        report("exact", None, Some("smallCandidateSet"), 4)
    );
    assert_eq!(hybrid(1, "auto").mode, "ann");
    assert_eq!(hybrid(1, "auto").pool, 36);
}

/// Backend reads of the vector index's own rows (bindings and vectors) for
/// one search.
fn index_reads(n: &Node, query: &VectorQuery) -> (u64, VectorSearchReport) {
    rushdb_storage::read_accounting::enable(true);
    let _ = rushdb_storage::read_accounting::take_local();
    let report = search(n, query).unwrap().1;
    let reads = rushdb_storage::read_accounting::take_local()
        .iter()
        .filter(|((_, family), _)| family == "legacy/emb-binding" || family == "legacy/vec")
        .map(|(_, count)| count)
        .sum();
    (reads, report)
}

#[test]
fn a_filtered_search_reads_its_candidates_not_the_index() {
    // The same 50 matching records inside an index of 500 and of 5,000
    // vectors (the default pool, 5,000, keeps the 50 exact). Read
    // accounting counts the on-disk backend's reads.
    let reads = |total: usize| {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = ServerConfig {
            data_dir: Some(dir.path().to_path_buf()),
            ..ServerConfig::default()
        };
        let n = Node::bootstrap(&mut cfg).unwrap();
        index(&n, "docs", "DOC", "cosine");
        let tenant_of: fn(usize) -> u64 = if total == 500 {
            |i| u64::from(i % 10 != 0)
        } else {
            |i| u64::from(i % 100 != 0)
        };
        bind_tenants(&n, "docs", "DOC", total, tenant_of);
        let exact = index_reads(
            &n,
            &filtered("docs", "DOC", 0, Some(VectorSearchMode::Exact)),
        );
        assert_eq!(exact.1, report("exact", None, None, 50));
        let ann = index_reads(&n, &filtered("docs", "DOC", 0, Some(VectorSearchMode::Ann)));
        assert_eq!(ann.1, report("exact", None, Some("smallCandidateSet"), 50));
        (exact.0, ann.0)
    };
    let (small_exact, small_ann) = reads(500);
    let (large_exact, large_ann) = reads(5_000);
    for (small, large) in [(small_exact, large_exact), (small_ann, large_ann)] {
        assert!(small > 0);
        assert!(
            large <= small + small / 10,
            "reads grew with the index: {small} at 500 vectors, {large} at 5,000"
        );
    }
}

/// `count` records around the unit circle in an order unrelated to their
/// creation (a fixed permutation), each with one vector.
#[expect(
    clippy::cast_precision_loss,
    reason = "a small record index mapped into f32 radians"
)]
fn bind_scattered(n: &Node, name: &str, label: &str, count: usize) {
    let items: Vec<VectorItem> = (0..count)
        .map(|i| {
            let angle = ((i * 2_654_435_761) % count) as f32 * 6.2 / count as f32;
            let vector = vec![angle.cos(), angle.sin()];
            let id = n
                .scope()
                .create_record(label, &serde_json::json!({}), &[], &serde_json::json!({}))
                .unwrap()["id"]
                .as_str()
                .unwrap()
                .to_string();
            VectorItem {
                record_id: id,
                vector,
                source_hash: None,
                index_generation: None,
            }
        })
        .collect();
    n.scope().upsert_vector_index(name, &items).unwrap();
}

/// An unfiltered search checks label membership only for the hits that
/// would enter its top `k` (in an order unrelated to the scores, about
/// `k * ln(n / k)` of them), not for every vector it scores.
#[test]
fn an_unfiltered_exact_search_checks_membership_only_for_kept_hits() {
    let reads = |total: usize| {
        let n = node_with_pool(10_000);
        index(&n, "docs", "DOC", "cosine");
        bind_scattered(&n, "docs", "DOC", total);
        rushdb_storage::read_accounting::enable(true);
        let _ = rushdb_storage::read_accounting::take_local();
        let (found, report) = search(
            &n,
            &VectorQuery {
                limit: 3,
                mode: Some(VectorSearchMode::Exact),
                ..query("docs", "DOC")
            },
        )
        .unwrap();
        assert_eq!((found, report), (3, report_exact(total)));
        let counts = rushdb_storage::read_accounting::take_local();
        let family = |name: &str| -> u64 {
            counts
                .iter()
                .filter(|((_, family), _)| family == name)
                .map(|(_, count)| count)
                .sum()
        };
        (family("label-member"), family("legacy/vec"))
    };
    for total in [400usize, 4_000] {
        let (checks, vectors) = reads(total);
        assert_eq!(vectors, total as u64, "every vector is scored once");
        assert!(
            checks * 20 < total as u64,
            "{checks} membership checks for {total} vectors"
        );
    }
}

fn report_exact(pool: usize) -> VectorSearchReport {
    report("exact", None, None, pool as u64)
}
