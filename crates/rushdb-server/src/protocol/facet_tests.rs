//! Facets counted from occurrence rows must agree with the records response
//! for the same query document: labels, per-label property support and array
//! shape, and property values with their counts.

use super::*;
use rushdb_protocol::{Describe, PropertyValuesMode, Query, RequestContext, RequestEnvelope};
use serde_json::json;
use std::collections::BTreeMap;

fn run(node: &Node, operation: Operation) -> Response {
    ProtocolExecutor::new(node, None)
        .execute(RequestEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: None,
            context: RequestContext::default(),
            operation,
        })
        .unwrap()
        .response
}

fn document(labels: &[&str], predicate: &serde_json::Value) -> QueryDocument {
    QueryDocument {
        labels: labels.iter().map(ToString::to_string).collect(),
        predicate: predicate.clone(),
        ..QueryDocument::default()
    }
}

fn seeded_node() -> Node {
    let mut cfg = super::tests::config(false);
    let node = Node::bootstrap(&mut cfg).unwrap();
    let rows = [
        (
            "Person",
            json!({"name": "ann", "age": 31, "score": 1, "tags": ["x", "y"], "city": "Riga"}),
        ),
        (
            "Person",
            json!({"name": "bob", "age": 25, "score": 1.5, "tags": [], "city": "Oslo"}),
        ),
        (
            "Person",
            json!({"name": "cat", "age": 40, "score": 1, "nick": null, "city": "Riga"}),
        ),
        (
            "Person",
            json!({"name": "dan", "age": 31, "tags": ["y"], "born": "2020-01-02T03:04:05Z"}),
        ),
        (
            "Company",
            json!({"name": "acme", "age": 99, "score": 7, "city": "Riga"}),
        ),
        (
            "Company",
            json!({"name": "init", "city": "Oslo", "tags": ["x"]}),
        ),
        (
            "Company",
            json!({"name": "void", "score": null, "city": "Riga"}),
        ),
    ];
    for (label, properties) in rows {
        node.scope()
            .create_record(label, &properties, &[], &json!({}))
            .unwrap();
    }
    node
}

/// `(label, properties)` of every record the records response returns.
fn oracle(
    node: &Node,
    query: &QueryDocument,
) -> Vec<(String, serde_json::Map<String, serde_json::Value>)> {
    let Response::Records { records, .. } = run(
        node,
        Operation::Query {
            query: Query::Records {
                query: query.clone(),
            },
        },
    ) else {
        panic!("expected records")
    };
    records
        .into_iter()
        .map(|record| {
            (
                record["label"].as_str().unwrap().to_string(),
                record["properties"].as_object().unwrap().clone(),
            )
        })
        .collect()
}

type Matched = [(String, serde_json::Map<String, serde_json::Value>)];

/// Labels and schema label counts against the matched records.
fn check_labels(node: &Node, query: &QueryDocument, expected: &Matched) {
    let context = format!("{:?} {}", query.labels, query.predicate);
    let Response::Labels { labels: mut got } = run(
        node,
        Operation::Describe {
            describe: Describe::Labels {
                query: query.clone(),
            },
        },
    ) else {
        panic!("expected labels")
    };
    got.sort();
    let mut want: Vec<String> = expected.iter().map(|(label, _)| label.clone()).collect();
    want.sort();
    want.dedup();
    assert_eq!(got, want, "labels for {context}");
    let Response::Schema { schema } = run(node, schema_request(query)) else {
        panic!("expected schema")
    };
    let mut want_counts: BTreeMap<String, u64> = BTreeMap::new();
    for (label, _) in expected {
        *want_counts.entry(label.clone()).or_default() += 1;
    }
    let got_counts: BTreeMap<String, u64> = schema
        .labels
        .iter()
        .map(|label| (label.name.clone(), label.records_count))
        .collect();
    assert_eq!(got_counts, want_counts, "schema labels for {context}");
}

fn schema_request(query: &QueryDocument) -> Operation {
    Operation::Describe {
        describe: Describe::Schema {
            query: query.clone(),
            detail: None,
            skip: None,
            limit: None,
        },
    }
}

/// Support per (label, name), summed over typed siblings, and array shape.
fn check_properties(node: &Node, query: &QueryDocument, expected: &Matched) {
    let mut want: BTreeMap<(String, String), (u64, bool)> = BTreeMap::new();
    for (label, properties) in expected {
        for (name, value) in properties {
            // A null is support, on a typed property and on one that is only
            // ever null (described with type `null`).
            let slot = want.entry((label.clone(), name.clone())).or_default();
            slot.0 += 1;
            slot.1 |= value.is_array();
        }
    }
    let Response::Schema { schema } = run(node, schema_request(query)) else {
        panic!("expected schema")
    };
    let mut got: BTreeMap<(String, String), (u64, bool)> = BTreeMap::new();
    for entry in &schema.properties {
        let slot = got
            .entry((entry.label.clone().unwrap(), entry.name.clone()))
            .or_default();
        slot.0 += entry.records_count;
        slot.1 |= entry.is_array;
    }
    assert_eq!(
        got, want,
        "properties for {:?} {}",
        query.labels, query.predicate
    );
}

/// First-seen distinct values, distinct count and top-k counts.
fn check_values(node: &Node, query: &QueryDocument, expected: &Matched, property: &str) {
    let context = format!("{property} for {:?} {}", query.labels, query.predicate);
    let mut want: Vec<(serde_json::Value, u64)> = Vec::new();
    for (_, properties) in expected {
        let Some(value) = properties.get(property) else {
            continue;
        };
        match want.iter_mut().find(|(seen, _)| seen == value) {
            Some(slot) => slot.1 += 1,
            None => want.push((value.clone(), 1)),
        }
    }
    let ask = |mode| {
        let Response::PropertyValues {
            values,
            distinct_count,
            counts,
            ..
        } = run(
            node,
            Operation::Query {
                query: Query::PropertyValues {
                    property: property.into(),
                    query: query.clone(),
                    mode,
                    limit: None,
                    search: None,
                },
            },
        )
        else {
            panic!("expected property values")
        };
        (values, distinct_count, counts)
    };
    let (values, distinct, _) = ask(None);
    let scoped = !(query.labels.is_empty() && query.predicate.as_object().unwrap().is_empty());
    if scoped {
        // By descending record count, ties by the value's JSON text.
        let mut ranked = want.clone();
        ranked.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.0.to_string().cmp(&b.0.to_string()))
        });
        let order: Vec<_> = ranked.into_iter().map(|(value, _)| value).collect();
        assert_eq!(values, order, "values {context}");
    }
    assert_eq!(distinct, Some(want.len() as u64), "distinct {context}");
    // Top-k counts: from the dictionaries when unfiltered (also under a
    // label), else from the match set.
    let (top, _, counts) = ask(Some(PropertyValuesMode::TopK));
    assert_eq!(top.len(), counts.len());
    for (value, count) in top.iter().zip(&counts) {
        let expected_count = want
            .iter()
            .find(|(seen, _)| seen == value)
            .unwrap_or_else(|| panic!("top-k value {value} not in records: {context}"))
            .1;
        assert_eq!(*count, expected_count, "top-k count of {value}: {context}");
    }
}

#[test]
fn facets_agree_with_the_records_response() {
    let node = seeded_node();
    let filters = [
        (vec![], json!({})),
        (vec![], json!({"city": "Riga"})),
        (vec![], json!({"age": {"$gte": 31}})),
        (vec!["Person"], json!({})),
        (vec!["Person"], json!({"city": {"$in": ["Oslo", "Riga"]}})),
    ];
    for (labels, predicate) in &filters {
        let query = document(labels, predicate);
        let expected = oracle(&node, &query);
        check_labels(&node, &query, &expected);
        check_properties(&node, &query, &expected);
        for property in ["name", "age", "score", "tags", "nick", "born", "city"] {
            check_values(&node, &query, &expected, property);
        }
    }
}

#[test]
fn value_facets_honour_the_property_mask() {
    let mut cfg = super::tests::config(false);
    let node = Node::bootstrap(&mut cfg).unwrap();
    for (label, properties) in [
        ("Allowed", json!({"v": 1, "secret": "x"})),
        ("Denied", json!({"v": 2, "secret": "y"})),
    ] {
        node.scope()
            .create_record(label, &properties, &[], &json!({}))
            .unwrap();
    }
    node.scope()
        .security_put_policy(
            &rushdb_authz::DataAccessPolicy {
                allowed_labels: vec!["Allowed".into()],
                visible_properties: BTreeMap::from([("Allowed".into(), vec!["v".into()])]),
                ..Default::default()
            },
            rushdb_authz::AuthorizationMode::PolicyEnforced,
        )
        .unwrap();
    // As in the records response: the denied label is absent and the masked
    // property is hidden.
    for (property, want) in [("v", vec![json!(1)]), ("secret", vec![])] {
        let Response::PropertyValues { values, .. } = run(
            &node,
            Operation::Query {
                query: Query::PropertyValues {
                    property: property.into(),
                    query: QueryDocument::default(),
                    mode: None,
                    limit: None,
                    search: None,
                },
            },
        ) else {
            panic!("expected property values")
        };
        assert_eq!(values, want, "{property}");
    }
}

/// Ids of the records the records response returns for `query`.
fn matched_ids(node: &Node, query: &QueryDocument) -> std::collections::BTreeSet<String> {
    let Response::Records { records, .. } = run(
        node,
        Operation::Query {
            query: Query::Records {
                query: query.clone(),
            },
        },
    ) else {
        panic!("expected records")
    };
    records
        .iter()
        .map(|record| record["id"].as_str().unwrap().to_string())
        .collect()
}

type Shapes = BTreeMap<(String, String, String), (u64, BTreeMap<String, u64>)>;

/// Relationship shapes touching `ids` (either endpoint), from the
/// relationships response: count and per-property non-null counts.
fn shapes_touching(node: &Node, ids: &std::collections::BTreeSet<String>) -> Shapes {
    let Response::Relationships { relationships, .. } = run(
        node,
        Operation::Query {
            query: Query::Relationships {
                query: rushdb_protocol::RelationshipQuery {
                    limit: Some(10_000),
                    ..Default::default()
                },
            },
        },
    ) else {
        panic!("expected relationships")
    };
    let mut shapes = Shapes::new();
    for row in relationships {
        let touches = [&row["sourceId"], &row["targetId"]]
            .iter()
            .any(|end| end.as_str().is_some_and(|id| ids.contains(id)));
        if !touches {
            continue;
        }
        let key = (
            row["sourceLabel"].as_str().unwrap().to_string(),
            row["type"].as_str().unwrap().to_string(),
            row["targetLabel"].as_str().unwrap().to_string(),
        );
        let shape = shapes.entry(key).or_default();
        shape.0 += 1;
        for (name, value) in row["properties"].as_object().unwrap() {
            if !value.is_null() {
                *shape.1.entry(name.clone()).or_default() += 1;
            }
        }
    }
    shapes
}

fn link(node: &Node, rel_type: &str, from: &str, to: &str, properties: &serde_json::Value) {
    let command: rushdb_protocol::RelationshipMutation = serde_json::from_value(json!({
        "relationType": rel_type,
        "source": {"label": "Person", "predicate": {"name": from}},
        "target": {"label": "Person", "predicate": {"name": to}},
        "allowCartesian": true,
        "properties": properties,
    }))
    .unwrap();
    node.scope().link_records(&command).unwrap();
}

#[test]
fn relationship_facets_narrow_with_the_match_set() {
    let mut cfg = super::tests::config(false);
    let node = Node::bootstrap(&mut cfg).unwrap();
    for (name, city, employer) in [
        ("ann", "Riga", "acme"),
        ("bob", "Oslo", "init"),
        ("cat", "Riga", "acme"),
    ] {
        node.scope()
            .import_json_document(
                "Person",
                &json!({"name": name, "city": city, "employer": {"name": employer}}),
            )
            .unwrap();
    }
    link(&node, "KNOWS", "ann", "bob", &json!({"since": 2020}));
    link(&node, "KNOWS", "bob", "ann", &json!({}));
    link(&node, "SELF", "cat", "cat", &json!({"note": "x"}));
    let filters = [
        (vec![], json!({})),
        (vec!["Person"], json!({})),
        (vec![], json!({"city": "Riga"})),
        (vec!["Person"], json!({"name": "bob"})),
        (vec!["Person"], json!({"city": "Oslo"})),
        (vec![], json!({"name": "acme"})),
    ];
    for (labels, predicate) in &filters {
        let query = document(labels, predicate);
        let want = shapes_touching(&node, &matched_ids(&node, &query));
        let Response::Schema { schema } = run(&node, schema_request(&query)) else {
            panic!("expected schema")
        };
        let got: Shapes = schema
            .relationships
            .iter()
            .map(|shape| {
                (
                    (
                        shape.source_label.clone(),
                        shape.relationship_type.clone(),
                        shape.target_label.clone(),
                    ),
                    (
                        shape.count,
                        shape
                            .properties
                            .iter()
                            .map(|property| (property.name.clone(), property.relationships_count))
                            .collect(),
                    ),
                )
            })
            .collect();
        let filtered = !predicate.as_object().unwrap().is_empty();
        if filtered {
            assert_eq!(got, want, "relationships for {labels:?} {predicate}");
        } else {
            // The catalog reports property counts per relationship type, so
            // only the shapes and their counts are compared here.
            let counts = |shapes: &Shapes| -> BTreeMap<(String, String, String), u64> {
                shapes
                    .iter()
                    .map(|(key, (count, _))| (key.clone(), *count))
                    .collect()
            };
            assert_eq!(
                counts(&got),
                counts(&want),
                "relationships for {labels:?} {predicate}"
            );
        }
    }
}

/// Value search: matching distinct values ranked by owner count, from
/// the dictionaries when unfiltered and from the match set otherwise.
#[test]
fn value_search_filters_distinct_values() {
    use rushdb_protocol::{ValueMatch, ValueSearch};
    let node = seeded_node();
    let ask = |property: &str,
               query: QueryDocument,
               mode: Option<PropertyValuesMode>,
               text: &str,
               matching: ValueMatch,
               case_sensitive: bool| {
        let Response::PropertyValues {
            values,
            distinct_count,
            counts,
            ..
        } = run(
            &node,
            Operation::Query {
                query: Query::PropertyValues {
                    property: property.into(),
                    query,
                    mode,
                    limit: None,
                    search: Some(ValueSearch {
                        text: text.into(),
                        matching,
                        case_sensitive,
                        domain_on_miss: None,
                        min_similarity: None,
                    }),
                },
            },
        )
        else {
            panic!("expected property values")
        };
        (values, distinct_count, counts)
    };
    let all = || document(&[], &json!({}));
    let person = || document(&["Person"], &json!({}));
    // Unscoped, case-insensitive contains, with exact owner counts.
    let (values, distinct, counts) = ask(
        "city",
        all(),
        Some(PropertyValuesMode::TopK),
        "RI",
        ValueMatch::Contains,
        false,
    );
    assert_eq!(
        (values, distinct, counts),
        (vec![json!("Riga")], Some(1), vec![4])
    );
    // Under a label: ties rank by value.
    let (values, ..) = ask("name", person(), None, "a", ValueMatch::Contains, false);
    assert_eq!(values, vec![json!("ann"), json!("cat"), json!("dan")]);
    let (values, ..) = ask("name", all(), None, "A", ValueMatch::Prefix, false);
    assert_eq!(values, vec![json!("acme"), json!("ann")]);
    let (_, distinct, _) = ask(
        "city",
        all(),
        Some(PropertyValuesMode::Count),
        "riga",
        ValueMatch::Exact,
        true,
    );
    assert_eq!(distinct, Some(0));
    // Numbers and datetimes match as the response renders them.
    let (values, _, counts) = ask(
        "age",
        person(),
        Some(PropertyValuesMode::TopK),
        "3",
        ValueMatch::Prefix,
        false,
    );
    assert_eq!((values, counts), (vec![json!(31)], vec![2]));
    let (values, ..) = ask("born", all(), None, "2020-01", ValueMatch::Contains, false);
    assert_eq!(values, vec![json!("2020-01-02T03:04:05.000Z")]);
    // A predicate forces the match-set scan; the same filter applies.
    let (values, ..) = ask(
        "name",
        document(&["Person"], &json!({"age": 31})),
        None,
        "a",
        ValueMatch::Contains,
        false,
    );
    assert_eq!(values, vec![json!("ann"), json!("dan")]);
    // Array values (scan path) match when any element does.
    let (values, distinct, _) = ask("tags", all(), None, "x", ValueMatch::Exact, false);
    assert_eq!(distinct, Some(2));
    assert!(values.contains(&json!(["x", "y"])) && values.contains(&json!(["x"])));
}

/// A property with values too long for the dictionary is searched by the
/// record scan, which keeps only matching values.
#[test]
fn value_search_scans_values_too_long_for_the_dictionary() {
    use rushdb_protocol::{ValueMatch, ValueSearch};
    let node = seeded_node();
    let long = |marker: &str| {
        format!(
            "{marker} {}",
            "x".repeat(rushdb_types::keys::DICT_MAX_VALUE_BYTES * 2)
        )
    };
    for body in [long("needle"), long("hay"), long("hay")] {
        run(
            &node,
            Operation::Ingest {
                ingest: rushdb_protocol::Ingest::Documents {
                    label: Some("Post".into()),
                    documents: json!([{ "body": body }]),
                    options: json!({}),
                },
            },
        );
    }
    let Response::PropertyValues {
        values,
        distinct_count,
        counts,
        ..
    } = run(
        &node,
        Operation::Query {
            query: Query::PropertyValues {
                property: "body".into(),
                query: document(&["Post"], &json!({})),
                mode: Some(PropertyValuesMode::TopK),
                limit: None,
                search: Some(ValueSearch {
                    text: "HAY".into(),
                    matching: ValueMatch::Prefix,
                    case_sensitive: false,
                    domain_on_miss: None,
                    min_similarity: None,
                }),
            },
        },
    )
    else {
        panic!("expected property values")
    };
    assert_eq!(
        (values, distinct_count, counts),
        (vec![json!(long("hay"))], Some(1), vec![2])
    );
}

/// A value search that matches nothing answers with the property's whole
/// domain when it is small (`domainOnMiss`), and a date in another layout
/// matches the stored date.
#[test]
fn value_search_shows_the_domain_on_a_miss_and_matches_dates() {
    use rushdb_protocol::{ValueMatch, ValueSearch};
    let node = seeded_node();
    run(
        &node,
        Operation::Ingest {
            ingest: rushdb_protocol::Ingest::Documents {
                label: Some("Lab".into()),
                documents: json!([
                    {"kct": "+", "taken": "1993-11-12T00:00:00Z", "month": 201_306},
                    {"kct": "-", "taken": "1994-01-05T00:00:00Z", "month": 201_307},
                    {"kct": "-"}
                ]),
                options: json!({}),
            },
        },
    );
    let ask = |property: &str, text: &str, domain_on_miss: Option<u64>| {
        let Response::PropertyValues {
            values,
            distinct_count,
            counts,
            search_missed,
            ..
        } = run(
            &node,
            Operation::Query {
                query: Query::PropertyValues {
                    property: property.into(),
                    query: document(&["Lab"], &json!({})),
                    mode: Some(PropertyValuesMode::TopK),
                    limit: None,
                    search: Some(ValueSearch {
                        text: text.into(),
                        matching: ValueMatch::Contains,
                        case_sensitive: false,
                        domain_on_miss,
                        min_similarity: None,
                    }),
                },
            },
        )
        else {
            panic!("expected property values")
        };
        (values, distinct_count, counts, search_missed)
    };
    // A miss on a small domain answers with every stored value and its count.
    assert_eq!(
        ask("kct", "positive", Some(20)),
        (vec![json!("-"), json!("+")], Some(2), vec![2, 1], true)
    );
    // A domain larger than the bound, or no bound: the empty search result.
    assert_eq!(
        ask("kct", "positive", Some(1)),
        (vec![], Some(0), vec![], false)
    );
    assert_eq!(
        ask("kct", "positive", None),
        (vec![], Some(0), vec![], false)
    );
    // A hit is answered as before.
    assert_eq!(
        ask("kct", "+", Some(20)),
        (vec![json!("+")], Some(1), vec![1], false)
    );
    // Dates written in another layout match the stored value.
    let (values, ..) = ask("taken", "1993/11/12", None);
    assert_eq!(values.len(), 1);
    assert!(values[0].as_str().unwrap().starts_with("1993-11-12"));
    assert_eq!(ask("month", "2013-06", None).0, vec![json!(201_306)]);
}

fn ingest(node: &Node, label: &str, documents: serde_json::Value) {
    run(
        node,
        Operation::Ingest {
            ingest: rushdb_protocol::Ingest::Documents {
                label: Some(label.into()),
                documents,
                options: json!({}),
            },
        },
    );
}

/// Records shaped like the Core platform's relationship-analysis fixtures.
fn joins_node() -> Node {
    let node = seeded_node();
    ingest(
        &node,
        "Folder",
        json!([
            {"id": "f1", "title": "root"},
            {"id": "f2", "parentId": "f1", "title": "a"},
            {"id": "f3", "parentId": "f1", "title": "b"},
            {"id": "f4", "parentId": "f2", "title": "c"}
        ]),
    );
    ingest(
        &node,
        "TRANSACTION",
        json!([
            {"transaction_id": "t1", "sender_account": "acc_1", "receiver_account": "acc_2",
             "status": "posted", "previous_status": "pending"},
            {"transaction_id": "t2", "sender_account": "acc_2", "receiver_account": "acc_3",
             "status": "pending", "previous_status": "created"}
        ]),
    );
    ingest(
        &node,
        "CHARACTER",
        json!([{"id": "c1", "name": "Spider-Man"}, {"id": "c2", "name": "Yoda"}]),
    );
    ingest(
        &node,
        "BATTLE",
        json!([{"title": "Endor", "commander_character_ids": ["c1", "c2"]}]),
    );
    let clients: Vec<serde_json::Value> = (1..=20)
        .map(|i| json!({"client_id": i, "rating": i % 5 + 1}))
        .collect();
    ingest(&node, "client", json!(clients));
    let disps: Vec<serde_json::Value> = (1..=20)
        .map(|i| json!({"disp_id": i, "client_id": (i % 12) + 1}))
        .collect();
    ingest(&node, "disp", json!(disps));
    node
}

fn describe(node: &Node, describe: Describe) -> Response {
    run(node, Operation::Describe { describe })
}

fn end(label: &str, property: &str) -> rushdb_protocol::JoinEnd {
    rushdb_protocol::JoinEnd {
        label: label.into(),
        property: property.into(),
    }
}

/// REL-1: value overlap per pair, from the exact dictionaries: evidence
/// only, in request order, with missing properties and arrays reported as
/// not compared.
#[test]
fn value_overlap_reports_evidence_per_pair() {
    let node = joins_node();
    let pair = |source: (&str, &str), target: (&str, &str)| rushdb_protocol::JoinPair {
        source: end(source.0, source.1),
        target: end(target.0, target.1),
    };
    let Response::ValueOverlap { pairs } = describe(
        &node,
        Describe::ValueOverlap {
            pairs: vec![
                pair(("Folder", "parentId"), ("Folder", "id")),
                pair(("disp", "client_id"), ("client", "client_id")),
                pair(("client", "rating"), ("client", "client_id")),
                pair(("CHARACTER", "name"), ("Folder", "title")),
                pair(("BATTLE", "commander_character_ids"), ("CHARACTER", "id")),
                pair(("Folder", "nope"), ("Folder", "id")),
            ],
        },
    ) else {
        panic!("expected value overlap")
    };
    assert_eq!(pairs.len(), 6);
    // A self-reference: every parentId is a Folder id, and ids are unique.
    let folder = &pairs[0];
    assert_eq!(
        (
            folder.matched_distinct,
            folder.coverage,
            folder.target_uniqueness
        ),
        (Some(2), Some(1.0), Some(1.0))
    );
    assert!(folder.source_uniqueness.unwrap() < 1.0);
    // A reference across labels: 12 distinct client ids, held by 20 records.
    let disp = &pairs[1];
    assert_eq!(
        (disp.matched_distinct, disp.matched_records, disp.coverage),
        (Some(12), Some(20), Some(1.0))
    );
    // Small codes inside an id range: full coverage, a small numeric span.
    let rating = &pairs[2];
    assert_eq!(rating.coverage, Some(1.0));
    assert!(rating.numeric_span.unwrap() < 0.3);
    // No overlap; arrays and missing properties are not compared.
    assert_eq!(pairs[3].matched_distinct, Some(0));
    assert_eq!(pairs[4].not_compared.as_deref(), Some("array values"));
    assert_eq!(pairs[5].not_compared.as_deref(), Some("no such property"));
}

/// CAT-3: labels and properties spelled like a wrong name.
#[test]
fn names_resolve_case_plural_and_separators() {
    let node = joins_node();
    let names = |text: &str| {
        let Response::Names { names } = describe(
            &node,
            Describe::Names {
                text: text.into(),
                limit: None,
            },
        ) else {
            panic!("expected names")
        };
        names
    };
    let top = &names("characters")[0];
    assert_eq!(
        (top.label.as_str(), top.property.as_deref()),
        ("CHARACTER", None)
    );
    let top = &names("commanderCharacterId")[0];
    assert_eq!(
        (top.label.as_str(), top.property.as_deref()),
        ("BATTLE", Some("commander_character_ids"))
    );
    let top = &names("folders.parent_id")[0];
    assert_eq!(
        (top.label.as_str(), top.property.as_deref()),
        ("Folder", Some("parentId"))
    );
    assert!(names("zzzz").is_empty());
}

/// CAT-4: every label with properties, counts and few values in one read.
#[test]
fn schema_summary_lists_labels_properties_and_few_values() {
    let node = joins_node();
    let Response::SchemaSummary { labels } = describe(
        &node,
        Describe::SchemaSummary {
            labels: vec!["Folder".into(), "client".into()],
            few_values: Some(5),
        },
    ) else {
        panic!("expected a schema summary")
    };
    assert_eq!(
        labels.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(),
        ["Folder", "client"]
    );
    let folder = &labels[0];
    assert_eq!(folder.records_count, 4);
    let id = folder.properties.iter().find(|p| p.name == "id").unwrap();
    assert_eq!((id.distinct_count, id.values.len()), (Some(4), 4));
    let client = &labels[1];
    let client_id = client
        .properties
        .iter()
        .find(|p| p.name == "client_id")
        .unwrap();
    // 20 distinct values: too many to list; a numeric range instead.
    assert!(client_id.values.is_empty());
    assert_eq!(
        (client_id.min.clone(), client_id.max.clone()),
        (Some(json!(1)), Some(json!(20)))
    );
}

/// CAT-2: a value search over every property, and near matches.
#[test]
fn search_values_across_properties_and_similar_spellings() {
    use rushdb_protocol::{ValueMatch, ValueSearch};
    let node = joins_node();
    let search = |text: &str, matching: ValueMatch| ValueSearch {
        text: text.into(),
        matching,
        case_sensitive: false,
        domain_on_miss: None,
        min_similarity: None,
    };
    let Response::ValueMatches { matches, total, .. } = run(
        &node,
        Operation::Query {
            query: Query::SearchValues {
                search: search("f1", ValueMatch::Exact),
                labels: vec!["Folder".into()],
                limit: None,
            },
        },
    ) else {
        panic!("expected value matches")
    };
    assert_eq!(total, 2);
    // parentId holds f1 twice, id once.
    assert_eq!(
        matches
            .iter()
            .map(|m| (m.property.as_str(), m.count))
            .collect::<Vec<_>>(),
        [("parentId", 2), ("id", 1)]
    );
    // "spiderman" is not contained in "Spider-Man", but is spelled like it.
    let names = |matching| {
        let Response::PropertyValues { values, .. } = run(
            &node,
            Operation::Query {
                query: Query::PropertyValues {
                    property: "name".into(),
                    query: document(&["CHARACTER"], &json!({})),
                    mode: Some(PropertyValuesMode::TopK),
                    limit: None,
                    search: Some(search("spiderman", matching)),
                },
            },
        ) else {
            panic!("expected property values")
        };
        values
    };
    assert!(names(ValueMatch::Contains).is_empty());
    assert_eq!(names(ValueMatch::Similar), vec![json!("Spider-Man")]);
}

fn try_run(node: &Node, operation: Operation) -> Result<Response, crate::protocol::ProtocolError> {
    ProtocolExecutor::new(node, None)
        .execute(RequestEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: None,
            context: RequestContext::default(),
            operation,
        })
        .map(|outcome| outcome.response)
}

fn sql(node: &Node, statement: &str) -> Result<serde_json::Value, String> {
    try_run(
        node,
        Operation::Query {
            query: Query::Statement {
                language: rushdb_protocol::StatementLanguage::Sql,
                statement: statement.into(),
                parameters: None,
            },
        },
    )
    .map(|response| serde_json::to_value(response).unwrap())
    .map_err(|error| error.to_string())
}

/// POL-2: under an enforced policy, the catalog reads and SQL show the
/// allowed labels, their visible properties and the allowed relationship
/// types only; hidden ones are never named, counted or suggested.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one fixture checked through every catalog read and SQL"
)]
fn enforced_policy_scopes_the_catalog_and_sql() {
    let node = seeded_node();
    let create = |label: &str, properties: serde_json::Value| {
        let v = properties["v"].clone();
        node.scope()
            .create_record(label, &properties, &[], &json!({}))
            .unwrap();
        matched_ids(&node, &document(&[label], &json!({ "v": v })))
            .into_iter()
            .next()
            .unwrap()
    };
    let a1 = create("Allowed", json!({"v": 1, "secret": "x"}));
    let a2 = create("Allowed", json!({"v": 2, "secret": "y"}));
    let d = create("Denied", json!({"v": 3, "secret": "z"}));
    for (rel, source, target) in [("OK", &a1, &a2), ("OK", &a1, &d), ("HIDDEN", &a2, &a1)] {
        node.scope()
            .link_records_by_id(&rushdb_protocol::RelationshipIdsMutation {
                relation_type: rel.into(),
                direction: rushdb_protocol::Direction::default(),
                source_ids: vec![source.clone()],
                target_ids: vec![target.clone()],
                properties: json!({}),
            })
            .unwrap();
    }
    node.scope()
        .security_put_policy(
            &rushdb_authz::DataAccessPolicy {
                allowed_labels: vec!["Allowed".into()],
                denied_rel_types: vec!["HIDDEN".into()],
                visible_properties: BTreeMap::from([("Allowed".into(), vec!["v".into()])]),
                ..Default::default()
            },
            rushdb_authz::AuthorizationMode::PolicyEnforced,
        )
        .unwrap();

    let Response::Labels { labels } = describe(
        &node,
        Describe::Labels {
            query: QueryDocument::default(),
        },
    ) else {
        panic!("expected labels")
    };
    assert_eq!(labels, vec!["Allowed".to_string()]);
    let Response::Schema { schema } = describe(
        &node,
        Describe::Schema {
            query: QueryDocument::default(),
            detail: Some(rushdb_protocol::PropertyDetail::Catalog),
            skip: None,
            limit: None,
        },
    ) else {
        panic!("expected a schema")
    };
    assert_eq!(
        schema
            .labels
            .iter()
            .map(|l| l.name.as_str())
            .collect::<Vec<_>>(),
        ["Allowed"]
    );
    assert_eq!(
        schema
            .properties
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["v"]
    );
    assert_eq!(
        schema
            .relationships
            .iter()
            .map(|r| (
                r.relationship_type.as_str(),
                r.target_label.as_str(),
                r.count
            ))
            .collect::<Vec<_>>(),
        [("OK", "Allowed", 1)]
    );
    let Response::SchemaSummary { labels } = describe(
        &node,
        Describe::SchemaSummary {
            labels: Vec::new(),
            few_values: None,
        },
    ) else {
        panic!("expected a summary")
    };
    assert_eq!(labels.len(), 1);
    assert_eq!(
        labels[0]
            .properties
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["v"]
    );
    assert_eq!(labels[0].relationships.len(), 1);
    for text in ["secret", "Denied", "Person"] {
        let Response::Names { names } = describe(
            &node,
            Describe::Names {
                text: text.into(),
                limit: None,
            },
        ) else {
            panic!("expected names")
        };
        assert!(names.is_empty(), "{text}: {names:?}");
    }
    let Response::ValueMatches { matches, .. } = run(
        &node,
        Operation::Query {
            query: Query::SearchValues {
                search: rushdb_protocol::ValueSearch {
                    text: "x".into(),
                    matching: rushdb_protocol::ValueMatch::Exact,
                    case_sensitive: false,
                    domain_on_miss: None,
                    min_similarity: None,
                },
                labels: Vec::new(),
                limit: None,
            },
        },
    ) else {
        panic!("expected value matches")
    };
    assert!(matches.is_empty());

    // SQL sees the allowed table and its visible column only.
    let rows = sql(&node, "SELECT v FROM allowed ORDER BY v").unwrap();
    assert_eq!(rows["rows"], json!([{"v": 1}, {"v": 2}]), "{rows}");
    let all = sql(&node, "SELECT * FROM allowed").unwrap();
    assert!(!all.to_string().contains("secret"), "{all}");
    for hidden in [
        "SELECT secret FROM allowed",
        "SELECT * FROM denied",
        "SELECT * FROM person",
        "SELECT * FROM rushdb_edges.hidden",
    ] {
        let error = sql(&node, hidden).unwrap_err();
        assert!(
            !error.contains("Denied") && !error.contains("secret "),
            "{hidden}: {error}"
        );
    }
    // Edges between allowed labels only.
    let edges = sql(&node, "SELECT COUNT(*) AS c FROM rushdb_edges.ok").unwrap();
    assert_eq!(edges["rows"], json!([{"c": 1}]), "{edges}");
    // Still closed: other statement languages and explain.
    assert!(try_run(
        &node,
        Operation::Query {
            query: Query::Statement {
                language: rushdb_protocol::StatementLanguage::Cypher,
                statement: "MATCH (n) RETURN n".into(),
                parameters: None,
            },
        },
    )
    .is_err());
}

/// POL-2 small-count threshold: under an enforced policy with `minCount`,
/// catalog reads withhold labels, properties, values and relationship shapes
/// counted over fewer records, and drop samples and extremes.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one fixture checked through every catalog read"
)]
fn enforced_policy_withholds_small_counts() {
    let node = seeded_node();
    for properties in [
        json!({"v": 1}),
        json!({"v": 1}),
        json!({"v": 2, "rare": "x"}),
    ] {
        node.scope()
            .create_record("Allowed", &properties, &[], &json!({}))
            .unwrap();
    }
    node.scope()
        .create_record("Small", &json!({"v": 1}), &[], &json!({}))
        .unwrap();
    let ids: Vec<String> = matched_ids(&node, &document(&["Allowed"], &json!({})))
        .into_iter()
        .collect();
    node.scope()
        .link_records_by_id(&rushdb_protocol::RelationshipIdsMutation {
            relation_type: "ONCE".into(),
            direction: rushdb_protocol::Direction::default(),
            source_ids: vec![ids[0].clone()],
            target_ids: vec![ids[1].clone()],
            properties: json!({}),
        })
        .unwrap();
    let Response::Policy { .. } = run(
        &node,
        Operation::Policy {
            policy: rushdb_protocol::PolicyOperation::Put {
                policy: json!({"allowedLabels": ["Allowed", "Small"], "minCount": 2}),
                enforced: true,
            },
        },
    ) else {
        panic!("policy installed")
    };
    let Response::Policy {
        policy: Some(stored),
        ..
    } = run(
        &node,
        Operation::Policy {
            policy: rushdb_protocol::PolicyOperation::Get,
        },
    )
    else {
        panic!("policy read")
    };
    assert_eq!(stored["minCount"], 2);

    // A label with one record is withheld.
    let Response::Labels { labels } = describe(
        &node,
        Describe::Labels {
            query: QueryDocument::default(),
        },
    ) else {
        panic!("expected labels")
    };
    assert_eq!(labels, vec!["Allowed".to_string()]);
    // A property on one record and a one-edge shape are withheld; samples and
    // extremes are dropped.
    let Response::Schema { schema } = describe(
        &node,
        Describe::Schema {
            query: document(&["Allowed"], &json!({})),
            detail: None,
            skip: None,
            limit: None,
        },
    ) else {
        panic!("expected a schema")
    };
    assert_eq!(
        schema
            .properties
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["v"]
    );
    assert!(schema
        .properties
        .iter()
        .all(|p| p.sample_values.is_empty() && p.min.is_none() && p.max.is_none()));
    assert!(schema.relationships.is_empty());
    // A value held by one record is withheld, in every mode.
    let values = |mode| {
        let Response::PropertyValues {
            values,
            distinct_count,
            min,
            max,
            ..
        } = run(
            &node,
            Operation::Query {
                query: Query::PropertyValues {
                    property: "v".into(),
                    query: document(&["Allowed"], &json!({})),
                    mode,
                    limit: None,
                    search: None,
                },
            },
        )
        else {
            panic!("expected property values")
        };
        (values, distinct_count, min, max)
    };
    assert_eq!(values(None).0, vec![json!(1)]);
    assert_eq!(values(Some(PropertyValuesMode::Count)).1, Some(1));
    let (_, _, min, max) = values(Some(PropertyValuesMode::Range));
    assert_eq!((min, max), (Some(json!(1)), Some(json!(1))));
    // The summary and name suggestions follow.
    let Response::SchemaSummary { labels } = describe(
        &node,
        Describe::SchemaSummary {
            labels: Vec::new(),
            few_values: None,
        },
    ) else {
        panic!("expected a summary")
    };
    assert_eq!(labels.len(), 1);
    let v = &labels[0].properties[0];
    assert_eq!((v.name.as_str(), v.values.clone()), ("v", vec![json!(1)]));
    assert!(v.min.is_none() && v.max.is_none());
    assert!(labels[0].relationships.is_empty());
    for text in ["rare", "Small"] {
        let Response::Names { names } = describe(
            &node,
            Describe::Names {
                text: text.into(),
                limit: None,
            },
        ) else {
            panic!("expected names")
        };
        assert!(names.is_empty(), "{text}: {names:?}");
    }
}

/// `similar` also works on values too long for the dictionary (record scan):
/// a misspelled phrase finds the long text that contains it.
#[test]
fn similar_search_reaches_long_values() {
    use rushdb_protocol::{ValueMatch, ValueSearch};
    let node = seeded_node();
    let long = |text: &str| {
        format!(
            "{text} {}",
            "lorem ".repeat(rushdb_types::keys::DICT_MAX_VALUE_BYTES)
        )
    };
    for body in [long("Orange County, Virginia"), long("Kings County")] {
        ingest(&node, "Note", json!([{ "body": body }]));
    }
    let Response::PropertyValues { values, .. } = run(
        &node,
        Operation::Query {
            query: Query::PropertyValues {
                property: "body".into(),
                query: document(&["Note"], &json!({})),
                mode: Some(PropertyValuesMode::TopK),
                limit: None,
                search: Some(ValueSearch {
                    text: "orange cuonty virginia".into(),
                    matching: ValueMatch::Similar,
                    case_sensitive: false,
                    domain_on_miss: None,
                    min_similarity: None,
                }),
            },
        },
    ) else {
        panic!("expected property values")
    };
    assert_eq!(values, vec![json!(long("Orange County, Virginia"))]);
}

/// Clients discover the catalog reads by capability flag, not by error.
#[test]
fn catalog_reads_are_advertised() {
    let node = seeded_node();
    let Response::Capabilities { capabilities } = describe(&node, Describe::Capabilities) else {
        panic!("expected capabilities")
    };
    for flag in [
        "valueSearch",
        "searchValues",
        "describeNames",
        "schemaSummary",
        "valueOverlap",
    ] {
        assert!(capabilities.iter().any(|c| c == flag), "{flag}");
    }
}

fn put_policy(node: &Node, policy: serde_json::Value) {
    let Response::Policy { .. } = run(
        node,
        Operation::Policy {
            policy: rushdb_protocol::PolicyOperation::Put {
                policy,
                enforced: true,
            },
        },
    ) else {
        panic!("policy installed")
    };
}

fn search_values(
    node: &Node,
    context: RequestContext,
    text: &str,
    matching: rushdb_protocol::ValueMatch,
) -> Result<Response, crate::protocol::ProtocolError> {
    ProtocolExecutor::new(node, None)
        .execute(RequestEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: None,
            context,
            operation: Operation::Query {
                query: Query::SearchValues {
                    search: rushdb_protocol::ValueSearch {
                        text: text.into(),
                        matching,
                        case_sensitive: false,
                        domain_on_miss: None,
                        min_similarity: None,
                    },
                    labels: Vec::new(),
                    limit: None,
                },
            },
        })
        .map(|outcome| outcome.response)
}

/// POL-2: a policy whose allowed labels hold no records leaves nothing to
/// read; label-scoped catalog reads answer empty without reading.
#[test]
fn an_enforced_policy_with_nothing_visible_reads_empty() {
    let node = seeded_node();
    put_policy(&node, json!({"allowedLabels": ["Nobody"]}));
    for query in [QueryDocument::default(), document(&["Person"], &json!({}))] {
        let Response::Labels { labels } = describe(
            &node,
            Describe::Labels {
                query: query.clone(),
            },
        ) else {
            panic!("expected labels")
        };
        assert!(labels.is_empty(), "{labels:?}");
        let Response::Schema { schema } = describe(
            &node,
            Describe::Schema {
                query: query.clone(),
                detail: None,
                skip: None,
                limit: None,
            },
        ) else {
            panic!("expected a schema")
        };
        assert!(schema.labels.is_empty() && schema.properties.is_empty());
        assert!(schema.relationships.is_empty());
        let Response::Properties { properties } = describe(
            &node,
            Describe::Properties {
                query,
                detail: None,
                skip: None,
                limit: None,
            },
        ) else {
            panic!("expected properties")
        };
        assert!(properties.is_empty(), "{properties:?}");
    }
}

/// CAT-2 under POL-2: `searchValues` reads an allowed label's dictionaries
/// directly (exact per label) and scans its long values, showing visible
/// properties only and withholding values held by fewer than `minCount`
/// records.
#[test]
fn search_values_under_a_policy_reads_dictionaries_and_scans_long_values() {
    use rushdb_protocol::ValueMatch;
    let node = seeded_node();
    let long = |text: &str| {
        format!(
            "{text} {}",
            "lorem ".repeat(rushdb_types::keys::DICT_MAX_VALUE_BYTES)
        )
    };
    let orange = long("Orange County, Virginia");
    ingest(
        &node,
        "Note",
        json!([{ "body": orange }, { "body": orange }, { "body": long("Kings County") }]),
    );
    put_policy(
        &node,
        json!({
            "allowedLabels": ["Person", "Note"],
            "visibleProperties": {"Person": ["name", "city"]},
            "minCount": 2
        }),
    );
    let found = |text: &str, matching| {
        let Ok(Response::ValueMatches {
            matches, skipped, ..
        }) = search_values(&node, RequestContext::default(), text, matching)
        else {
            panic!("expected value matches")
        };
        assert!(skipped.is_empty(), "{skipped:?}");
        matches
            .into_iter()
            .map(|row| (row.label, row.property, row.value, row.count))
            .collect::<Vec<_>>()
    };
    // Riga: Person.city twice; Company is not allowed.
    assert_eq!(
        found("riga", ValueMatch::Exact),
        [("Person".into(), "city".into(), json!("Riga"), 2)]
    );
    // Oslo is held by one Person: withheld.
    assert!(found("oslo", ValueMatch::Exact).is_empty());
    // Long values are scanned: the twice-held body only.
    assert_eq!(
        found("orange county", ValueMatch::Contains),
        [("Note".into(), "body".into(), json!(orange), 2)]
    );
}

/// Catalog reads hold value dictionaries as query working set: over the
/// request's `queryMemoryMaxBytes`, value overlap and the schema summary
/// fail as resource exhausted, and value search skips the property.
#[test]
fn catalog_reads_stop_at_the_request_working_set() {
    let node = joins_node();
    let tiny = || RequestContext {
        query_memory_max_bytes: Some(256),
        ..Default::default()
    };
    let read = |context: RequestContext, describe: Describe| {
        ProtocolExecutor::new(&node, None)
            .execute(RequestEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: None,
                context,
                operation: Operation::Describe { describe },
            })
            .map(|outcome| outcome.response)
    };
    let overlap = || Describe::ValueOverlap {
        pairs: vec![rushdb_protocol::JoinPair {
            source: end("disp", "client_id"),
            target: end("client", "client_id"),
        }],
    };
    let summary = || Describe::SchemaSummary {
        labels: vec!["client".into()],
        few_values: None,
    };
    for describe in [overlap(), summary()] {
        let error = read(tiny(), describe.clone()).unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceExhausted, "{error:?}");
        assert!(read(RequestContext::default(), describe).is_ok());
    }
    // client_id holds 11 values containing "1": over the cap, so skipped;
    // the small ones still answer.
    let Ok(Response::ValueMatches {
        matches, skipped, ..
    }) = search_values(&node, tiny(), "1", rushdb_protocol::ValueMatch::Contains)
    else {
        panic!("expected value matches")
    };
    assert!(
        skipped.contains(&"client.client_id".to_string()),
        "{skipped:?}"
    );
    assert!(matches
        .iter()
        .any(|row| row.label == "client" && row.property == "rating"));
}

fn put_record(
    node: &Node,
    label: &str,
    properties: &serde_json::Value,
) -> Result<(), crate::protocol::ProtocolError> {
    let operation = serde_json::from_value(json!({"kind": "mutate", "mutation": {
        "kind": "putRecord", "mode": "create", "label": label,
        "properties": properties, "options": {}}}))
    .unwrap();
    try_run(node, operation).map(|_| ())
}

fn all_records(node: &Node) -> Result<Vec<serde_json::Value>, String> {
    match try_run(
        node,
        Operation::Query {
            query: Query::Records {
                query: QueryDocument::default(),
            },
        },
    ) {
        Ok(Response::Records { records, .. }) => Ok(records),
        Ok(other) => panic!("expected records, got {other:?}"),
        Err(error) => Err(error.to_string()),
    }
}

/// POL-2 `allowedLabels`: `[]` under an enforced policy locks the tenant's
/// data: reads fail, the catalog is empty, writes are refused.
#[test]
fn an_empty_allow_list_locks_reads_and_writes() {
    let node = seeded_node();
    put_policy(&node, json!({"allowedLabels": []}));
    assert!(all_records(&node).is_err());
    let Response::Labels { labels } = describe(
        &node,
        Describe::Labels {
            query: QueryDocument::default(),
        },
    ) else {
        panic!("expected labels")
    };
    assert!(labels.is_empty());
    let error = put_record(&node, "Person", &json!({"name": "eve"})).unwrap_err();
    assert_eq!(error.code, ErrorCode::Unauthorized, "{error:?}");
    assert!(error.message.contains("not authorized"), "{error:?}");
}

/// POL-2 `allowedLabels: ["*"]`: every label, including one created after
/// the policy, can be read and written; property masks still apply.
#[test]
fn a_star_allows_every_label_with_masks_still_applied() {
    let node = seeded_node();
    put_policy(
        &node,
        json!({
            "allowedLabels": ["*"],
            "visibleProperties": {"Person": ["name"]}
        }),
    );
    put_record(&node, "Invoice", &json!({"total": 5})).unwrap();
    let records = all_records(&node).unwrap();
    assert_eq!(records.len(), 8, "{records:?}");
    let person = records
        .iter()
        .find(|record| record["label"] == "Person")
        .unwrap();
    assert!(person["properties"].get("city").is_none(), "{person}");
    let Response::Labels { labels } = describe(
        &node,
        Describe::Labels {
            query: QueryDocument::default(),
        },
    ) else {
        panic!("expected labels")
    };
    assert_eq!(labels, ["Company", "Invoice", "Person"]);
    let rows = sql(&node, "SELECT total FROM invoice").unwrap();
    assert_eq!(rows["rows"], json!([{"total": 5}]), "{rows}");
}

/// Writes compare allowed labels as reads do: `note` allows `Note`.
#[test]
fn allowed_labels_match_writes_as_reads_do() {
    let node = seeded_node();
    put_policy(&node, json!({"allowedLabels": ["note"]}));
    put_record(&node, "Note", &json!({"title": "a"})).unwrap();
    let records = all_records(&node).unwrap();
    assert_eq!(records.len(), 1, "{records:?}");
    assert!(put_record(&node, "Person", &json!({"name": "eve"})).is_err());
}

fn chapter_node(max_chars: Option<usize>) -> (Node, Vec<String>) {
    let mut cfg = super::tests::config(false);
    if let Some(max) = max_chars {
        cfg.resources.introspection_value_max_chars = max;
    }
    let node = Node::bootstrap(&mut cfg).unwrap();
    // Two chapters longer than the value dictionary holds (96 bytes), one
    // of them twice, and a short value.
    let chapter = |n: usize| {
        format!(
            "Chapter {n}. {}",
            "It was a dark and stormy night. ".repeat(8)
        )
    };
    let bodies = vec![chapter(1), chapter(1), chapter(2), "short".to_string()];
    for body in &bodies {
        node.scope()
            .create_record("Chapter", &json!({"body": body}), &[], &json!({}))
            .unwrap();
    }
    (node, vec![chapter(1), chapter(2), "short".to_string()])
}

fn summary_of(node: &Node, label: &str) -> Vec<rushdb_protocol::PropertySummary> {
    let Response::SchemaSummary { mut labels } = describe(
        node,
        Describe::SchemaSummary {
            labels: vec![label.into()],
            few_values: Some(10),
        },
    ) else {
        panic!("expected a schema summary")
    };
    labels.remove(0).properties
}

/// A text column holding values too long for the value dictionary reports
/// its exact distinct count and few values in `schemaSummary` (from the
/// write-time long-value counters), ranked as the dictionary ranks them; introspection
/// shows each long text cut to `introspection-value-max-chars` (default 64)
/// with `…`, while `propertyValues` keeps values whole.
#[test]
fn schema_summary_counts_long_text_and_introspection_clips_it() {
    let (node, ranked) = chapter_node(None);
    let cut = |text: &str| format!("{}…", text.chars().take(64).collect::<String>());
    let body = summary_of(&node, "Chapter").remove(0);
    assert_eq!(body.distinct_count, Some(3));
    assert_eq!(
        body.values,
        [
            json!(cut(&ranked[0])),
            json!(cut(&ranked[1])),
            json!("short")
        ]
    );
    let Response::Properties { properties } = describe(
        &node,
        Describe::Properties {
            query: document(&["Chapter"], &json!({})),
            detail: None,
            skip: None,
            limit: None,
        },
    ) else {
        panic!("expected properties")
    };
    for value in properties[0]
        .sample_values
        .iter()
        .chain(properties[0].min.iter())
        .chain(properties[0].max.iter())
    {
        let text = value.as_str().unwrap();
        assert!(text.chars().count() <= 65, "{text}");
    }
    let Response::PropertyValues { values, .. } = run(
        &node,
        Operation::Query {
            query: Query::PropertyValues {
                property: "body".into(),
                query: document(&["Chapter"], &json!({})),
                mode: Some(PropertyValuesMode::TopK),
                limit: Some(1),
                search: None,
            },
        },
    ) else {
        panic!("expected property values")
    };
    assert_eq!(values, [json!(ranked[0])]);
}

/// Long-text counts are kept at write time (digest refcounts), so a delete
/// moves them without a scan; past `fewValues` the count stays and no
/// values are listed.
#[test]
fn schema_summary_long_text_counts_follow_writes() {
    let mut cfg = super::tests::config(false);
    let node = Node::bootstrap(&mut cfg).unwrap();
    let page = |n: usize| format!("Page {n}. {}", "Call me Ishmael. ".repeat(8));
    let mut ids = Vec::new();
    for n in 0..12 {
        let record = node
            .scope()
            .create_record("Page", &json!({"text": page(n)}), &[], &json!({}))
            .unwrap();
        ids.push(record.get("id").unwrap().as_str().unwrap().to_string());
    }
    let text = summary_of(&node, "Page").remove(0);
    assert_eq!((text.distinct_count, text.values.len()), (Some(12), 0));
    for id in &ids[..3] {
        node.scope().delete_record(id).unwrap();
    }
    let text = summary_of(&node, "Page").remove(0);
    assert_eq!(text.distinct_count, Some(9));
    assert_eq!(text.values.len(), 9);
    assert!(text.values[0].as_str().unwrap().starts_with("Page 10."));
}

/// `introspection-value-max-chars = 0` turns the cut off.
#[test]
fn introspection_value_cap_can_be_turned_off() {
    let (node, ranked) = chapter_node(Some(0));
    let body = summary_of(&node, "Chapter").remove(0);
    assert_eq!(
        body.values,
        [json!(ranked[0]), json!(ranked[1]), json!("short")]
    );
}

/// A type-split property's summary entries name their sibling types, as
/// `describe.properties` does.
#[test]
fn schema_summary_names_sibling_types() {
    let mut cfg = super::tests::config(false);
    let node = Node::bootstrap(&mut cfg).unwrap();
    for code in [json!(7), json!("A7")] {
        node.scope()
            .create_record("Item", &json!({"code": code}), &[], &json!({}))
            .unwrap();
    }
    let Response::Properties { properties } = describe(
        &node,
        Describe::Properties {
            query: document(&["Item"], &json!({})),
            detail: None,
            skip: None,
            limit: None,
        },
    ) else {
        panic!("expected properties")
    };
    let described: BTreeMap<String, Vec<String>> = properties
        .into_iter()
        .map(|p| (p.value_type, p.sibling_types))
        .collect();
    let summarized: BTreeMap<String, Vec<String>> = summary_of(&node, "Item")
        .into_iter()
        .map(|p| (p.value_type, p.sibling_types))
        .collect();
    assert_eq!(summarized, described);
    assert!(summarized.values().all(|siblings| !siblings.is_empty()));
}
