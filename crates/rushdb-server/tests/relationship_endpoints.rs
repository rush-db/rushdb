//! `query.relationships` with endpoint predicates: one record's relationships
//! come from its own links, outgoing for a source filter and incoming for a
//! target filter, without listing the store's other relationships.

use rushdb_protocol::{RelationshipIdsMutation, RelationshipQuery};
use rushdb_server::{Node, ServerConfig};
use serde_json::{json, Value};

fn node() -> Node {
    let mut cfg = ServerConfig::default();
    Node::bootstrap(&mut cfg).unwrap()
}

fn record(n: &Node, name: &str) -> String {
    n.scope()
        .create_record("N", &json!({ "name": name }), &[], &json!({}))
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn link(n: &Node, source: &str, target: &str) {
    let command: RelationshipIdsMutation = serde_json::from_value(json!({
        "relationType": "FLOW", "sourceIds": [source], "targetIds": [target]
    }))
    .unwrap();
    n.scope().link_records_by_id(&command).unwrap();
}

/// `(source, target)` id pairs of the matching relationships, and the total.
fn relationships(n: &Node, query: Value) -> (Vec<(String, String)>, u64) {
    let query: RelationshipQuery = serde_json::from_value(query).unwrap();
    let result = n.scope().scan_relationships(&query).unwrap();
    let pairs = result["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["sourceId"].as_str().unwrap().to_string(),
                row["targetId"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    (pairs, result["total"].as_u64().unwrap())
}

#[test]
fn a_records_relationships_come_from_its_own_links() {
    let db = node();
    let [alpha, beta, gamma, delta] = ["a", "b", "c", "d"].map(|name| record(&db, name));
    link(&db, &alpha, &beta);
    link(&db, &gamma, &alpha);
    link(&db, &gamma, &delta);
    // Traffic among other records that must not appear.
    let others: Vec<String> = (0..50).map(|i| record(&db, &format!("o{i}"))).collect();
    for pair in others.windows(2) {
        link(&db, &pair[0], &pair[1]);
    }

    assert_eq!(
        relationships(&db, json!({ "sourcePredicate": { "$id": alpha } })),
        (vec![(alpha.clone(), beta.clone())], 1)
    );
    assert_eq!(
        relationships(&db, json!({ "targetPredicate": { "$id": alpha } })),
        (vec![(gamma.clone(), alpha)], 1)
    );
    assert_eq!(
        relationships(
            &db,
            json!({ "sourcePredicate": { "$id": gamma }, "targetPredicate": { "$id": delta } })
        ),
        (vec![(gamma, delta)], 1)
    );
    // `beta` only receives; as a source it has nothing.
    assert_eq!(
        relationships(&db, json!({ "sourcePredicate": { "$id": beta } })),
        (vec![], 0)
    );
    // A record predicate, not just an id: every source named `c`.
    assert_eq!(
        relationships(&db, json!({ "sourcePredicate": { "name": "c" } })).1,
        2
    );
}

#[test]
fn an_unfiltered_listing_pages_with_an_exact_total() {
    let n = node();
    let records: Vec<String> = (0..30).map(|i| record(&n, &format!("r{i}"))).collect();
    for pair in records.windows(2) {
        link(&n, &pair[0], &pair[1]);
    }
    let (page, total) = relationships(&n, json!({ "offset": 25, "limit": 10 }));
    assert_eq!(total, 29);
    assert_eq!(page.len(), 4);
}
