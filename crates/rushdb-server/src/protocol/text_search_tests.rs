//! `query.text` reports the index's unapplied backlog and accepts the
//! all-terms and phrase modes; `rushdb_text_pending` covers every text index
//! in every tenancy mode.

use super::*;
use rushdb_protocol::{RequestContext, RequestEnvelope, StatementLanguage};

fn execute(node: &Node, scope: Option<&str>, operation: Operation) -> Response {
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
        .unwrap()
        .response
}

/// An `eventual` whole-record text index `name` on `Doc`, built.
fn eventual_index(node: &Node, scope: Option<&str>, name: &str) {
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
    while node.step_index_builds(std::time::Duration::from_secs(5)) > 0 {}
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
    let response = execute(
        node,
        None,
        Operation::Query {
            query: Query::Text {
                index: "docs".into(),
                query: query.into(),
                limit: 10,
                all_terms,
                phrase,
            },
        },
    );
    let Response::TextHits { total, pending, .. } = response else {
        panic!("expected text hits, got {response:?}");
    };
    (total, pending)
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
