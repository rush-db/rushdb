//! Tier 2: what a client does, in the order it does it, against a running
//! server with its own background workers. Each feature a client reaches
//! through background work (an index built over existing records, an
//! applier catching up) has its client sequence here: the in-process suite
//! drives that work by hand and cannot see an interleaving only the
//! workers produce (a text index created after a load stayed empty in a
//! build whose in-process suite was green).
mod common;

use common::TestServer;
use serde_json::{json, Value};

fn ingest(server: &TestServer, label: &str, documents: &Value) -> Value {
    server.execute(
        &json!({"kind": "ingest", "ingest": {"kind": "documents", "label": label,
        "documents": documents, "options": {"returnResult": true}}}),
    )
}

fn create_index(server: &TestServer, definition: &Value) -> Value {
    server.execute(&json!({"kind": "index", "index": {"kind": "create", "definition": definition}}))
}

fn docs() -> Value {
    json!([
        {"doc_id": 1, "title": "quantum world", "body": "machines learn quantum mechanics",
         "$vectors": [{"propertyName": "embedding", "vector": [1.0, 0.0, 0.0, 0.0], "index": "docs-embedding"}]},
        {"doc_id": 2, "title": "dragons", "body": "magic dragon call",
         "$vectors": [{"propertyName": "embedding", "vector": [0.0, 1.0, 0.0, 0.0], "index": "docs-embedding"}]},
        {"doc_id": 3, "title": "gardens", "body": "roses and tulips in spring",
         "$vectors": [{"propertyName": "embedding", "vector": [0.0, 0.0, 1.0, 0.0], "index": "docs-embedding"}]}
    ])
}

fn doc_ids(records: &Value) -> Vec<i64> {
    records
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|record| record["properties"]["doc_id"].as_i64())
        .collect()
}

#[test]
fn documents_ingested_then_text_indexed_are_found() {
    // A text index created over existing records lost them when the
    // applier, woken by the index's own creation, judged coverage by the
    // definition from before the build. The timing varies, so the sequence
    // is repeated, with the index declared before and after the load.
    let server = TestServer::start();
    for round in 0..4 {
        for (property, index_first) in [("body", false), ("*", false), ("title", true)] {
            let label = format!("docs_{round}_{}", property.replace('*', "all"));
            let index = format!("{label}_text");
            let definition =
                json!({"name": index, "label": label, "property": property, "kind": "text"});
            if index_first {
                create_index(&server, &definition);
            }
            ingest(
                &server,
                &label,
                &json!([
                    {"doc_id": 1, "title": "quantum world", "body": "machines learn quantum mechanics"},
                    {"doc_id": 2, "title": "dragons", "body": "magic dragon call"}
                ]),
            );
            if !index_first {
                create_index(&server, &definition);
            }
            let found = server.settled(
                &json!({"kind": "query", "query": {"kind": "text", "index": index, "query": "quantum", "limit": 5}}),
            );
            assert_eq!(
                found["rows"].as_array().map(Vec::len),
                Some(1),
                "round {round}, property {property}, index first {index_first}: {found}"
            );
        }
    }
}

#[test]
fn hybrid_search_fuses_a_text_index_created_after_the_load() {
    // The benchmark's order: a vector index, documents with vectors, the
    // text index after the load. The vector ranks documents 3, 2, 1 and the
    // text finds document 1 alone, so fused document 1 leads; without the
    // text side documents 3 and 2 would.
    let server = TestServer::start();
    server.execute(
        &json!({"kind": "index", "index": {"kind": "createVector", "definition": {
        "name": "docs-embedding", "label": "docs", "property": "embedding", "dimensions": 4,
        "similarity": "cosine"}}}),
    );
    ingest(&server, "docs", &docs());
    create_index(
        &server,
        &json!({"name": "docs_text", "label": "docs", "property": "*", "kind": "text"}),
    );
    server.settled(
        &json!({"kind": "query", "query": {"kind": "text", "index": "docs_text", "query": "quantum", "limit": 5}}),
    );
    let fused = server.execute(&json!({"kind": "query", "query": {"kind": "hybrid", "query": {
        "labels": ["docs"],
        "vector": {"property": "embedding", "vector": [0.0, 0.5, 1.0, 0.0], "index": "docs-embedding"},
        "text": {"index": "docs_text", "query": "quantum"}, "limit": 2, "fusion": "rrf", "rrfK": 60}}}));
    assert_eq!(doc_ids(&fused["records"]).first(), Some(&1), "{fused}");
}

#[test]
fn vectors_are_searchable_right_after_their_ingest() {
    // ANN and exact search straight after the load, while the index may
    // still be warming: answers, never a refusal, and the nearest first.
    let server = TestServer::start();
    server.execute(
        &json!({"kind": "index", "index": {"kind": "createVector", "definition": {
        "name": "docs-embedding", "label": "docs", "property": "embedding", "dimensions": 4,
        "similarity": "cosine"}}}),
    );
    ingest(&server, "docs", &docs());
    for mode in ["exact", "ann"] {
        let found = server.execute(
            &json!({"kind": "query", "query": {"kind": "vector", "query": {
            "labels": ["docs"], "property": "embedding", "index": "docs-embedding",
            "vector": [0.1, 0.9, 0.0, 0.0], "limit": 3, "mode": mode}}}),
        );
        assert_eq!(
            doc_ids(&found["records"]).first(),
            Some(&2),
            "{mode}: {found}"
        );
    }
}

#[test]
fn an_ordered_index_built_over_existing_records_answers_like_the_scan() {
    let server = TestServer::start();
    let rows: Vec<Value> = (0..3_000)
        .map(|n| json!({"n": n, "group": n % 7}))
        .collect();
    for chunk in rows.chunks(1_000) {
        ingest(&server, "rows", &Value::Array(chunk.to_vec()));
    }
    let query = json!({"kind": "query", "query": {"kind": "records", "query": {
        "labels": ["rows"], "predicate": {"n": {"$gte": 1_200, "$lt": 1_450}, "group": 3},
        "order": {"n": "asc"}, "limit": 1_000}}});
    let scanned = server.execute(&query);
    create_index(
        &server,
        &json!({"name": "rows_n", "label": "rows", "property": "n"}),
    );
    let indexed = server.settled(&query);
    let ns = |response: &Value| -> Vec<i64> {
        response["records"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|record| record["properties"]["n"].as_i64())
            .collect()
    };
    assert!(!ns(&scanned).is_empty());
    assert_eq!(ns(&indexed), ns(&scanned));
    assert_eq!(indexed["total"], scanned["total"]);
}

#[test]
fn a_unique_index_over_existing_values_refuses_a_duplicate_with_details() {
    let server = TestServer::start();
    let mut holder = String::new();
    for email in ["a@x", "b@x", "c@x"] {
        let created = server.execute(&json!({"kind": "mutate", "mutation": {"kind": "putRecord",
            "mode": "create", "label": "User", "properties": {"email": email}}}));
        if email == "b@x" {
            holder = created["record"]["id"].as_str().unwrap().to_owned();
        }
    }
    create_index(
        &server,
        &json!({"name": "user_email", "label": "User", "property": "email", "kind": "unique"}),
    );
    server.settled(&json!({"kind": "describe", "describe": {"kind": "indexes"}}));
    let refused = server.execute_raw(&json!({"kind": "mutate", "mutation": {"kind": "putRecord",
        "mode": "create", "label": "User", "properties": {"email": "b@x"}}}));
    assert_eq!(refused.status, 409, "{}", refused.body);
    assert_eq!(
        refused.body["error"]["code"], "conflict",
        "{}",
        refused.body
    );
    assert_eq!(
        refused.body["error"]["details"],
        json!({"label": "User", "property": "email", "recordId": holder})
    );
    assert!(
        !refused.body.to_string().contains("b@x"),
        "the value is never told"
    );

    // Declared over values two records hold: refused, nothing built.
    for _ in 0..2 {
        server.execute(&json!({"kind": "mutate", "mutation": {"kind": "putRecord",
            "mode": "create", "label": "Team", "properties": {"code": "same"}}}));
    }
    let declared = server.execute_raw(&json!({"kind": "index", "index": {"kind": "create",
        "definition": {"name": "team_code", "label": "Team", "property": "code", "kind": "unique"}}}));
    assert_ne!(declared.status, 200, "{}", declared.body);
}
