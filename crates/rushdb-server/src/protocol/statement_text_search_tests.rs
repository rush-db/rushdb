//! Text indexes are searchable from every statement language, through the
//! node's one set of text indexes: SQL's `text_search` table function.

use super::*;
use rushdb_protocol::{RequestContext, RequestEnvelope, StatementLanguage};

fn run(node: &Node, operation: Operation) -> Result<Response, ProtocolError> {
    ProtocolExecutor::new(node, None)
        .execute(RequestEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: None,
            context: RequestContext::default(),
            operation,
        })
        .map(|envelope| envelope.response)
}

fn mutate(node: &Node, language: StatementLanguage, statement: &str) {
    run(
        node,
        Operation::Mutate {
            mutation: Mutation::Statement {
                language,
                statement: statement.into(),
                parameters: None,
            },
        },
    )
    .unwrap();
}

/// Rows of a read statement.
fn rows(node: &Node, language: StatementLanguage, statement: &str) -> Vec<serde_json::Value> {
    match run(
        node,
        Operation::Query {
            query: Query::Statement {
                language,
                statement: statement.into(),
                parameters: None,
            },
        },
    ) {
        Ok(Response::Rows { rows, .. }) => rows,
        other => panic!("{language:?} `{statement}`: {other:?}"),
    }
}

/// Articles with `immediate` text indexes on `title` and on every string.
fn articles() -> Node {
    let mut cfg = super::tests::config(false);
    cfg.languages.sql = true;
    cfg.languages.mongodb = true;
    cfg.languages.cypher = true;
    cfg.languages.gql = true;
    let node = Node::bootstrap(&mut cfg).unwrap();
    for (title, body) in [
        ("Graph databases", "nodes and edges in a graph"),
        ("Cooking pasta", "boil water, add salt"),
        ("Database internals", "storage engines and a graph of pages"),
    ] {
        node.scope()
            .create_record(
                "Article",
                &serde_json::json!({ "title": title, "body": body }),
                &[],
                &serde_json::json!({}),
            )
            .unwrap();
    }
    mutate(
        &node,
        StatementLanguage::Sql,
        "CREATE INDEX titles ON Article(title) USING TEXT IMMEDIATE",
    );
    mutate(
        &node,
        StatementLanguage::Mongo,
        r#"{"createIndex": {"collection": "Article", "property": "*", "kind": "text", "name": "everything", "consistency": "immediate"}}"#,
    );
    while node.step_index_builds(std::time::Duration::from_secs(5)) > 0 {}
    node
}

fn titles(rows: &[serde_json::Value]) -> Vec<&str> {
    rows.iter()
        .map(|row| row["title"].as_str().unwrap())
        .collect()
}

#[test]
fn sql_text_search_joins_ranked_hits_to_records() {
    let node = articles();
    let found = rows(
        &node,
        StatementLanguage::Sql,
        "SELECT a.title, t.score FROM text_search('everything', 'graph', 10) AS t \
         JOIN article a ON a.__id = t.id ORDER BY t.score DESC, a.title",
    );
    assert_eq!(
        titles(&found),
        vec!["Graph databases", "Database internals"]
    );
    assert!(found[0]["score"].as_f64().unwrap() >= found[1]["score"].as_f64().unwrap());

    // A property index sees only that property (the other article mentions
    // "graph" in its body); the limit defaults to 10.
    let found = rows(
        &node,
        StatementLanguage::Sql,
        "SELECT a.title FROM text_search('titles', 'graph') AS t JOIN article a ON a.__id = t.id",
    );
    assert_eq!(titles(&found), vec!["Graph databases"]);

    // Options reach the index: every term, or the exact phrase.
    let all_terms = rows(
        &node,
        StatementLanguage::Sql,
        r#"SELECT count(*) AS n FROM text_search('everything', 'graph storage', 10, '{"allTerms": true}')"#,
    );
    assert_eq!(all_terms[0]["n"], 1);
    let phrase = rows(
        &node,
        StatementLanguage::Sql,
        r#"SELECT count(*) AS n FROM text_search('everything', 'edges nodes', 10, '{"phrase": true}')"#,
    );
    assert_eq!(phrase[0]["n"], 0);
}

#[test]
fn sql_text_search_reports_bad_calls() {
    let node = articles();
    for (statement, expected) in [
        (
            "SELECT * FROM text_search('missing', 'graph')",
            "unknown text index `missing`",
        ),
        (
            "SELECT * FROM text_search('titles')",
            "takes 2 to 4 arguments",
        ),
        (
            "SELECT * FROM text_search('titles', 'graph', -1)",
            "must not be negative",
        ),
        (
            r#"SELECT * FROM text_search('titles', 'graph', 5, '{"fuzzy": true}')"#,
            "unknown text_search option `fuzzy`",
        ),
    ] {
        let error = run(
            &node,
            Operation::Query {
                query: Query::Statement {
                    language: StatementLanguage::Sql,
                    statement: statement.into(),
                    parameters: None,
                },
            },
        )
        .expect_err(statement);
        assert!(error.message.contains(expected), "{statement}: {error:?}");
    }
}

#[test]
fn cypher_and_gql_filter_and_rank_with_text_functions() {
    let node = articles();
    for language in [StatementLanguage::Cypher, StatementLanguage::Gql] {
        // A boolean filter on a property index.
        let found = rows(
            &node,
            language,
            "MATCH (a:Article) WHERE text_match(a.title, 'graph') RETURN a.title AS title",
        );
        assert_eq!(titles(&found), vec!["Graph databases"], "{language:?}");
        // Ranked top-k on the whole-record index: `*` names it.
        let ranked = rows(
            &node,
            language,
            "MATCH (a:Article) RETURN a.title AS title, text_score(a.`*`, 'graph') AS score \
             ORDER BY text_score(a.`*`, 'graph') DESC LIMIT 2",
        );
        assert_eq!(
            titles(&ranked),
            vec!["Graph databases", "Database internals"],
            "{language:?}: {ranked:?}"
        );
        assert!(ranked[0]["score"].as_f64().unwrap() > 0.0, "{language:?}");
    }
}

#[test]
fn a_text_function_needs_an_index_and_a_parsable_query() {
    let node = articles();
    // `body` has no index of its own (only the `*` one covers it).
    for (statement, expected) in [
        (
            "MATCH (a:Article) WHERE text_match(a.body, 'graph') RETURN a.title AS title",
            "no ready text index on :Article(body)",
        ),
        (
            "MATCH (a:Article) WHERE text_match(a.title, 'title:(') RETURN a",
            "text search failed",
        ),
    ] {
        let error = run(
            &node,
            Operation::Query {
                query: Query::Statement {
                    language: StatementLanguage::Cypher,
                    statement: statement.into(),
                    parameters: None,
                },
            },
        )
        .expect_err(statement);
        assert_eq!(error.code, ErrorCode::InvalidRequest, "{error:?}");
        assert!(error.message.contains(expected), "{statement}: {error:?}");
    }
}

/// One `immediate` text index on `Doc`, for Mongo's one-per-collection rule.
fn documents() -> Node {
    let node = articles();
    for (title, year) in [
        ("graph basics", 2019),
        ("graph graph graph", 2022),
        ("cooking", 2023),
    ] {
        node.scope()
            .create_record(
                "Doc",
                &serde_json::json!({ "title": title, "year": year }),
                &[],
                &serde_json::json!({}),
            )
            .unwrap();
    }
    mutate(
        &node,
        StatementLanguage::Mongo,
        r#"{"createIndex": {"collection": "Doc", "property": "title", "kind": "text", "name": "doc_titles", "consistency": "immediate"}}"#,
    );
    while node.step_index_builds(std::time::Duration::from_secs(5)) > 0 {}
    node
}

#[test]
fn mongo_text_filters_scores_and_sorts() {
    let node = documents();
    let found = rows(
        &node,
        StatementLanguage::Mongo,
        r#"{"collection": "Doc", "filter": {"$text": {"$search": "graph"}},
            "projection": {"score": {"$meta": "textScore"}},
            "sort": {"score": {"$meta": "textScore"}}}"#,
    );
    assert_eq!(titles(&found), vec!["graph graph graph", "graph basics"]);
    let (best, next) = (
        found[0]["score"].as_f64().unwrap(),
        found[1]["score"].as_f64().unwrap(),
    );
    assert!(best > next && next > 0.0, "{found:?}");
    // Every field is still returned next to the projected score.
    assert_eq!(found[0]["year"], 2022);

    // The rest of the filter, skip and limit apply to the matches.
    let found = rows(
        &node,
        StatementLanguage::Mongo,
        r#"{"collection": "Doc", "filter": {"$text": {"$search": "graph"}, "year": {"$lt": 2020}}}"#,
    );
    assert_eq!(titles(&found), vec!["graph basics"]);
    let found = rows(
        &node,
        StatementLanguage::Mongo,
        r#"{"collection": "Doc", "filter": {"$text": {"$search": "graph"}},
            "sort": {"score": {"$meta": "textScore"}}, "skip": 1, "limit": 1}"#,
    );
    assert_eq!(titles(&found), vec!["graph basics"]);
    let unmatched = rows(
        &node,
        StatementLanguage::Mongo,
        r#"{"collection": "Doc", "filter": {"$text": {"$search": "nowhere"}}}"#,
    );
    assert!(unmatched.is_empty(), "{unmatched:?}");

    // A pipeline's first `$match` restricts what it groups.
    let counted = rows(
        &node,
        StatementLanguage::Mongo,
        r#"{"collection": "Doc", "aggregate": [
            {"$match": {"$text": {"$search": "graph"}}}, {"$count": "n"}]}"#,
    );
    assert_eq!(counted[0]["n"], 2, "{counted:?}");
}

#[test]
fn mongo_text_reports_what_it_cannot_answer() {
    let node = documents();
    for (statement, expected) in [
        (
            r#"{"collection": "Note", "filter": {"$text": {"$search": "graph"}}}"#,
            "requires a ready text index on `Note`",
        ),
        (
            r#"{"collection": "Article", "filter": {"$text": {"$search": "graph"}}}"#,
            "exactly one text index on `Article`",
        ),
        (
            r#"{"collection": "Doc", "filter": {"$text": {"$search": "graph", "$caseSensitive": true}}}"#,
            "`$text.$caseSensitive` is unsupported",
        ),
        (
            r#"{"collection": "Doc", "sort": {"score": {"$meta": "textScore"}}}"#,
            "requires a `$text` filter",
        ),
        (
            r#"{"collection": "Doc", "aggregate": [{"$limit": 5}, {"$match": {"$text": {"$search": "graph"}}}]}"#,
            "`$text` must be in the first `$match`",
        ),
    ] {
        let error = run(
            &node,
            Operation::Query {
                query: Query::Statement {
                    language: StatementLanguage::Mongo,
                    statement: statement.into(),
                    parameters: None,
                },
            },
        )
        .expect_err(statement);
        assert!(error.message.contains(expected), "{statement}: {error:?}");
    }
}
