//! `query.text` reports the index's unapplied backlog and accepts the
//! all-terms and phrase modes; `rushdb_text_pending` covers every text index
//! in every tenancy mode.

use super::*;
use rushdb_protocol::{RequestContext, RequestEnvelope, StatementLanguage, TextSyntax};

fn execute(node: &Node, scope: Option<&str>, operation: Operation) -> Response {
    try_execute(node, scope, operation).unwrap()
}

fn try_execute(
    node: &Node,
    scope: Option<&str>,
    operation: Operation,
) -> Result<Response, rushdb_protocol::ProtocolError> {
    ProtocolExecutor::new(node, scope.map(str::to_owned))
        .execute(RequestEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: None,
            context: RequestContext {
                scope: scope.map(str::to_owned),
                deadline_ms: None,
                ..Default::default()
            },
            operation,
        })
        .map(|outcome| outcome.response)
}

/// An `eventual` whole-record text index `name` on `Doc`, built.
fn eventual_index(node: &Node, scope: Option<&str>, name: &str) {
    declare_eventual_index(node, scope, name);
    while node.step_index_builds(std::time::Duration::from_secs(5)) > 0 {}
}

/// An `eventual` whole-record text index `name` on `Doc`, declared but not
/// yet built.
fn declare_eventual_index(node: &Node, scope: Option<&str>, name: &str) {
    execute(
        node,
        scope,
        Operation::Mutate {
            mutation: Mutation::Statement {
                language: StatementLanguage::Mongo,
                statement: serde_json::json!({"createIndex": {"collection": "Doc",
                    "property": "*", "kind": "text", "name": name,
                    "consistency": "eventual"}})
                .to_string(),
                parameters: None,
            },
        },
    );
}

fn doc(node: &Node, scope: Option<&str>, body: &str) {
    execute(
        node,
        scope,
        Operation::Mutate {
            mutation: Mutation::PutRecord {
                mode: PutMode::Create,
                id: None,
                label: Some("Doc".into()),
                properties: serde_json::json!({ "body": body }),
                vectors: Vec::new(),
                options: serde_json::json!({}),
            },
        },
    );
}

/// `(hits, pending)` of one text search.
fn search(node: &Node, query: &str, all_terms: bool, phrase: bool) -> (u64, u64) {
    search_with(node, "docs", query, all_terms, phrase, TextSyntax::Terms)
}

/// `(hits, pending)` of one text search of `index` under `syntax`.
fn search_with(
    node: &Node,
    index: &str,
    query: &str,
    all_terms: bool,
    phrase: bool,
    syntax: TextSyntax,
) -> (u64, u64) {
    let response = try_search(node, index, query, all_terms, phrase, syntax)
        .unwrap_or_else(|error| panic!("search failed: {error:?}"));
    let Response::TextHits { total, pending, .. } = response else {
        panic!("expected text hits, got {response:?}");
    };
    (total, pending)
}

/// One text search as the protocol answers it.
fn try_search(
    node: &Node,
    index: &str,
    query: &str,
    all_terms: bool,
    phrase: bool,
    syntax: TextSyntax,
) -> Result<Response, rushdb_protocol::ProtocolError> {
    try_execute(
        node,
        None,
        Operation::Query {
            query: Query::Text {
                index: index.into(),
                query: query.into(),
                limit: 10,
                all_terms,
                phrase,
                syntax,
            },
        },
    )
}

#[test]
fn eventual_search_reports_its_backlog() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    eventual_index(&node, None, "docs");
    for body in ["quick brown fox", "quick red fox", "lazy brown dog"] {
        doc(&node, None, body);
    }
    // No applier has run: the search misses all three and says so.
    assert_eq!(search(&node, "fox", false, false), (0, 3));
    node.apply_text_indexes(std::time::Duration::from_secs(5));
    assert_eq!(search(&node, "fox", false, false), (2, 0));
}

#[test]
fn all_terms_and_phrase_modes_reach_the_index() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    eventual_index(&node, None, "docs");
    for body in [
        "quick brown fox",
        "brown quick fox",
        "quick red fox",
        "lazy brown dog",
    ] {
        doc(&node, None, body);
    }
    node.apply_text_indexes(std::time::Duration::from_secs(5));
    assert_eq!(search(&node, "quick brown", false, false).0, 4, "any term");
    assert_eq!(search(&node, "quick brown", true, false).0, 2, "every term");
    assert_eq!(
        search(&node, "quick brown", false, true).0,
        1,
        "adjacent, in order"
    );

    // The wire form: camelCase, omitted when false.
    let query: Query = serde_json::from_value(serde_json::json!({
        "kind": "text", "index": "docs", "query": "quick brown", "allTerms": true
    }))
    .unwrap();
    assert!(matches!(
        query,
        Query::Text {
            all_terms: true,
            phrase: false,
            ..
        }
    ));
}

#[test]
fn text_pending_gauge_covers_every_index_in_multitenant_mode() {
    let node = Node::bootstrap(&mut super::tests::config(true)).unwrap();
    for (tenant, index, docs) in [("tenant-a", "notes", 2), ("tenant-b", "pages", 3)] {
        eventual_index(&node, Some(tenant), index);
        for i in 0..docs {
            doc(&node, Some(tenant), &format!("body {i}"));
        }
    }
    let rendered = node.metrics_render();
    for series in [
        "rushdb_text_pending{index=\"notes\"} 2",
        "rushdb_text_pending{index=\"pages\"} 3",
    ] {
        assert!(
            rendered.lines().any(|line| line == series),
            "missing `{series}` in:\n{}",
            rendered
                .lines()
                .filter(|line| line.contains("pending"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

/// Dropping a text index deletes its files once the drop completes.
#[test]
fn dropping_a_text_index_deletes_its_files() {
    let dir = rushdb_storage::TempPath::dir("text-drop-files");
    std::fs::create_dir_all(&dir).unwrap();
    let mut cfg = super::tests::config(false);
    cfg.data_dir = Some(dir.to_path_buf());
    cfg.languages.mongodb = true;
    let node = Node::bootstrap(&mut cfg).unwrap();
    doc(&node, None, "the quick fox");
    eventual_index(&node, None, "docs");
    // Applying the pending change writes the index's files.
    node.apply_text_indexes(std::time::Duration::from_secs(5));
    assert_eq!(search(&node, "fox", false, false).0, 1);
    let text_dir = dir.join("text");
    let files = || {
        std::fs::read_dir(&text_dir).map_or(0, |tenants| {
            tenants
                .flat_map(|tenant| std::fs::read_dir(tenant.unwrap().path()).unwrap())
                .count()
        })
    };
    assert_eq!(files(), 1, "one index directory");

    execute(
        &node,
        None,
        Operation::Index {
            index: IndexOperation::Drop {
                name: "docs".into(),
            },
        },
    );
    while node.step_index_builds(std::time::Duration::from_secs(5)) > 0 {}
    assert_eq!(files(), 0, "the dropped index's directory is gone");
    drop(node);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn plain_words_are_the_default_and_the_query_syntax_is_opt_in() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    eventual_index(&node, None, "docs");
    doc(
        &node,
        None,
        "Adhesion molecules and chemokines: relation to inflammation",
    );
    doc(&node, None, "the virus' genome");
    node.apply_text_indexes(std::time::Duration::from_secs(5));
    // Colons, apostrophes and the words and/or/not are text, as the
    // benchmark suite's natural-language queries send them.
    assert_eq!(
        search(&node, "inflammatory chemokines: role", false, false).0,
        1
    );
    assert_eq!(search(&node, "virus' genome", false, false).0, 1);
    // `and` is a word of the first document.
    assert_eq!(search(&node, "AND OR NOT", false, false).0, 1);
    assert_eq!(search(&node, "chemokines inflammation", true, false).0, 1);
    assert_eq!(search(&node, "chemokines absent", true, false).0, 0);
    // The query syntax is opt-in, and a query that does not parse says so.
    assert_eq!(
        search_with(
            &node,
            "docs",
            "genome -chemokines",
            false,
            false,
            TextSyntax::Query
        )
        .0,
        1
    );
    let error = try_search(&node, "docs", "title:(", false, false, TextSyntax::Query)
        .expect_err("an unparsable query must fail");
    assert_eq!(error.code, ErrorCode::InvalidRequest, "{error:?}");
    assert!(error.message.contains("cannot be parsed"), "{error:?}");
    // The index's internal owner column is not searchable.
    let error = try_search(&node, "docs", "owner:1", false, false, TextSyntax::Query)
        .expect_err("a field prefix must fail");
    assert_eq!(error.code, ErrorCode::InvalidRequest, "{error:?}");
}

#[test]
fn a_text_search_names_what_is_wrong_with_it() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let error = try_search(&node, "missing", "fox", false, false, TextSyntax::Terms)
        .expect_err("an unknown index must fail");
    assert_eq!(error.code, ErrorCode::NotFound, "{error:?}");
    assert!(error.message.contains("`missing`"), "{error:?}");
    declare_eventual_index(&node, None, "docs");
    let error = try_search(&node, "docs", "fox", false, false, TextSyntax::Terms)
        .expect_err("a building index must fail");
    assert_eq!(error.code, ErrorCode::Unavailable, "{error:?}");
    assert!(error.retryable, "{error:?}");
    assert!(error.message.contains("`docs`"), "{error:?}");
}
