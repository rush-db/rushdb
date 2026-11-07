//! `updateRelationshipsById` rewrites the properties of existing
//! relationships between records named by id: `set` writes (a JSON `null` is
//! an explicit null), `unset` removes, a pair without the relationship is
//! skipped, and a bad request writes nothing.

use super::transaction_reference_tests::run;
use super::*;

fn record(node: &Node, label: &str, name: &str) -> String {
    node.scope()
        .create_record(
            label,
            &serde_json::json!({ "name": name }),
            &[],
            &serde_json::json!({}),
        )
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn mutate(
    node: &Node,
    kind: &str,
    command: &serde_json::Value,
) -> Result<serde_json::Value, ProtocolError> {
    run(
        node,
        &serde_json::json!({"kind": "mutate", "mutation": {"kind": kind, "command": command}}),
    )
}

fn update(node: &Node, command: &serde_json::Value) -> Result<u64, ProtocolError> {
    mutate(node, "updateRelationshipsById", command).map(|response| {
        assert_eq!(response["kind"], "mutation", "{response}");
        response["affected"].as_u64().unwrap()
    })
}

fn link(
    node: &Node,
    relation_type: &str,
    source: &str,
    target: &str,
    properties: &serde_json::Value,
) {
    mutate(
        node,
        "linkRecordsById",
        &serde_json::json!({"relationType": relation_type, "sourceIds": [source],
            "targetIds": [target], "properties": properties}),
    )
    .unwrap();
}

/// Properties of every relationship, keyed by `type:source->target`.
fn relationships(node: &Node) -> std::collections::BTreeMap<String, serde_json::Value> {
    run(
        node,
        &serde_json::json!({"kind": "query", "query": {"kind": "relationships", "query": {}}}),
    )
    .unwrap()["relationships"]
        .as_array()
        .unwrap()
        .iter()
        .map(|rel| {
            (
                format!(
                    "{}:{}->{}",
                    rel["type"].as_str().unwrap(),
                    rel["sourceId"].as_str().unwrap(),
                    rel["targetId"].as_str().unwrap()
                ),
                rel["properties"].clone(),
            )
        })
        .collect()
}

/// `describe.schema`'s relationship properties of `relationship_type`:
/// name -> (type, relationshipsCount).
fn schema_properties(
    node: &Node,
    relationship_type: &str,
) -> std::collections::BTreeMap<String, (String, u64)> {
    let schema = run(
        node,
        &serde_json::json!({"kind": "describe", "describe": {"kind": "schema", "query": {}}}),
    )
    .unwrap();
    schema["schema"]["relationships"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|shape| shape["type"] == relationship_type)
        .flat_map(|shape| shape["properties"].as_array().unwrap().clone())
        .map(|property| {
            (
                property["name"].as_str().unwrap().to_owned(),
                (
                    property["type"].as_str().unwrap().to_owned(),
                    property["relationshipsCount"].as_u64().unwrap(),
                ),
            )
        })
        .collect()
}

#[test]
fn set_overwrites_writes_nulls_and_unset_removes_and_the_schema_follows() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let ada = record(&node, "Customer", "Ada");
    let bob = record(&node, "Customer", "Bob");
    let order = record(&node, "Order", "o-1");
    link(
        &node,
        "PLACES",
        &ada,
        &order,
        &serde_json::json!({"channel": "store", "old": 1}),
    );
    link(
        &node,
        "PLACES",
        &bob,
        &order,
        &serde_json::json!({"channel": "store"}),
    );
    let ada_key = format!("PLACES:{ada}->{order}");
    let bob_key = format!("PLACES:{bob}->{order}");
    assert_eq!(
        schema_properties(&node, "PLACES")["old"],
        ("number".to_owned(), 1)
    );

    let affected = update(
        &node,
        &serde_json::json!({"relationType": "PLACES", "direction": "out",
            "sourceIds": [ada], "targetIds": [order],
            "set": {"channel": "web", "note": null, "priority": 2}, "unset": ["old"]}),
    )
    .unwrap();
    assert_eq!(affected, 1);
    let rels = relationships(&node);
    assert_eq!(
        rels[&ada_key],
        serde_json::json!({"channel": "web", "note": null, "priority": 2}),
        "overwritten, explicit null, new property, `old` removed"
    );
    assert_eq!(
        rels[&bob_key],
        serde_json::json!({"channel": "store"}),
        "a relationship outside the call is untouched"
    );

    let schema = schema_properties(&node, "PLACES");
    assert_eq!(schema["channel"].1, 2);
    assert_eq!(schema["priority"], ("number".to_owned(), 1), "{schema:?}");
    assert!(
        !schema.contains_key("old"),
        "unsetting the last occurrence removes the property: {schema:?}"
    );

    // Both sources at once (cross product); `unset` of the new property on
    // its only holder removes it from the schema again.
    let affected = update(
        &node,
        &serde_json::json!({"relationType": "PLACES", "sourceIds": [ada, bob],
            "targetIds": [order], "unset": ["priority"]}),
    )
    .unwrap();
    assert_eq!(affected, 2);
    assert!(!schema_properties(&node, "PLACES").contains_key("priority"));
}

#[test]
fn direction_in_swaps_the_endpoints() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let ada = record(&node, "Customer", "Ada");
    let order = record(&node, "Order", "o-1");
    link(&node, "PLACES", &ada, &order, &serde_json::json!({}));

    let from_order = |direction: &str| {
        update(
            &node,
            &serde_json::json!({"relationType": "PLACES", "direction": direction,
                "sourceIds": [order], "targetIds": [ada], "set": {"channel": "web"}}),
        )
        .unwrap()
    };
    assert_eq!(from_order("out"), 0, "no PLACES points from the order");
    assert_eq!(
        relationships(&node)[&format!("PLACES:{ada}->{order}")],
        serde_json::json!({})
    );
    assert_eq!(from_order("in"), 1, "`in` names the target as source");
    assert_eq!(
        relationships(&node)[&format!("PLACES:{ada}->{order}")],
        serde_json::json!({"channel": "web"})
    );
}

#[test]
fn pairs_without_the_relationship_are_skipped() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let ada = record(&node, "Customer", "Ada");
    let first = record(&node, "Order", "o-1");
    let second = record(&node, "Order", "o-2");
    let third = record(&node, "Order", "o-3");
    link(&node, "PLACES", &ada, &first, &serde_json::json!({}));
    // Another type between the same records is not updated.
    link(&node, "RETURNS", &ada, &second, &serde_json::json!({}));

    let affected = update(
        &node,
        &serde_json::json!({"relationType": "PLACES", "sourceIds": [ada],
            "targetIds": [first, second, third, first], "set": {"channel": "web"}}),
    )
    .unwrap();
    assert_eq!(affected, 1, "one PLACES relationship, counted once");
    let rels = relationships(&node);
    assert_eq!(
        rels[&format!("PLACES:{ada}->{first}")],
        serde_json::json!({"channel": "web"})
    );
    assert_eq!(
        rels[&format!("RETURNS:{ada}->{second}")],
        serde_json::json!({})
    );
    assert_eq!(rels.len(), 2);
}

#[test]
fn a_bad_request_writes_nothing() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let ada = record(&node, "Customer", "Ada");
    let order = record(&node, "Order", "o-1");
    link(
        &node,
        "PLACES",
        &ada,
        &order,
        &serde_json::json!({"channel": "store"}),
    );
    let before = relationships(&node);

    // A dangling id fails the call, though the other pair exists.
    let dangling = update(
        &node,
        &serde_json::json!({"relationType": "PLACES", "sourceIds": [ada],
            "targetIds": [order, "00000000-0000-7000-8000-000000000000"],
            "set": {"channel": "web"}}),
    )
    .unwrap_err();
    assert_eq!(dangling.code, ErrorCode::NotFound, "{dangling:?}");

    let conflict = update(
        &node,
        &serde_json::json!({"relationType": "PLACES", "sourceIds": [ada], "targetIds": [order],
            "set": {"channel": "web"}, "unset": ["channel"]}),
    )
    .unwrap_err();
    assert_eq!(conflict.code, ErrorCode::InvalidRequest, "{conflict:?}");
    assert!(
        conflict.message.contains("both set and unset"),
        "{conflict:?}"
    );

    let nested = update(
        &node,
        &serde_json::json!({"relationType": "PLACES", "sourceIds": [ada], "targetIds": [order],
            "set": {"channel": {"web": true}}}),
    )
    .unwrap_err();
    assert_eq!(nested.code, ErrorCode::InvalidRequest, "{nested:?}");

    assert_eq!(relationships(&node), before);
}

#[test]
fn a_denied_relationship_type_is_unauthorized_and_writes_nothing() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let ada = record(&node, "Customer", "Ada");
    let order = record(&node, "Order", "o-1");
    link(
        &node,
        "PLACES",
        &ada,
        &order,
        &serde_json::json!({"channel": "store"}),
    );
    node.scope()
        .security_put_policy(
            &rushdb_authz::DataAccessPolicy {
                allowed_labels: vec!["*".into()],
                denied_rel_types: vec!["PLACES".into()],
                ..Default::default()
            },
            rushdb_authz::AuthorizationMode::PolicyEnforced,
        )
        .unwrap();
    let lmpg = node.scope().lmpg();
    let before = lmpg.store.committed_version();

    let denied = update(
        &node,
        &serde_json::json!({"relationType": "PLACES", "sourceIds": [ada], "targetIds": [order],
            "set": {"channel": "web"}}),
    )
    .unwrap_err();
    assert_eq!(denied.code, ErrorCode::Unauthorized, "{denied:?}");
    // Classified as linkRecordsById is: a write of the relationship type.
    let link_denied = mutate(
        &node,
        "linkRecordsById",
        &serde_json::json!({"relationType": "PLACES", "sourceIds": [order], "targetIds": [ada]}),
    )
    .unwrap_err();
    assert_eq!(link_denied.code, ErrorCode::Unauthorized, "{link_denied:?}");
    assert_eq!(lmpg.store.committed_version(), before, "nothing committed");
}

/// A graph-language `SET` on a relationship stages through the same batch
/// operation and commits with its transaction.
#[test]
fn a_relationship_set_staged_in_a_transaction_commits() {
    let mut cfg = super::tests::config(false);
    cfg.languages.cypher = true;
    let node = Node::bootstrap(&mut cfg).unwrap();
    let ada = record(&node, "Customer", "Ada");
    let order = record(&node, "Order", "o-1");
    link(
        &node,
        "PLACES",
        &ada,
        &order,
        &serde_json::json!({"channel": "store"}),
    );

    let begun = run(
        &node,
        &serde_json::json!({"kind": "transaction", "transaction": {"kind": "begin"}}),
    )
    .unwrap();
    let transaction_id = begun["transactionId"].as_str().unwrap().to_owned();
    run(
        &node,
        &serde_json::json!({"kind": "transaction", "transaction": {"kind": "stage",
            "transactionId": transaction_id, "mutation": {"kind": "statement",
            "language": "cypher",
            "statement": "MATCH (:Customer)-[r:PLACES]->(:Order) SET r.channel = 'web'"}}}),
    )
    .unwrap();
    let key = format!("PLACES:{ada}->{order}");
    assert_eq!(
        relationships(&node)[&key],
        serde_json::json!({"channel": "store"}),
        "invisible before commit"
    );
    run(
        &node,
        &serde_json::json!({"kind": "transaction", "transaction": {"kind": "commit",
            "transactionId": transaction_id}}),
    )
    .unwrap();
    assert_eq!(
        relationships(&node)[&key],
        serde_json::json!({"channel": "web"})
    );
}

/// Every relationship row of `query.relationships`, keyed as
/// [`relationships`]: `(properties, propertyTypes)`.
fn typed_relationships(
    node: &Node,
) -> std::collections::BTreeMap<String, (serde_json::Value, serde_json::Value)> {
    run(
        node,
        &serde_json::json!({"kind": "query", "query": {"kind": "relationships", "query": {}}}),
    )
    .unwrap()["relationships"]
        .as_array()
        .unwrap()
        .iter()
        .map(|rel| {
            (
                format!(
                    "{}:{}->{}",
                    rel["type"].as_str().unwrap(),
                    rel["sourceId"].as_str().unwrap(),
                    rel["targetId"].as_str().unwrap()
                ),
                (rel["properties"].clone(), rel["propertyTypes"].clone()),
            )
        })
        .collect()
}

#[test]
fn typed_set_writes_datetimes_read_back_as_iso_strings_with_property_types() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let ada = record(&node, "Customer", "Ada");
    let order = record(&node, "Order", "o-1");
    link(
        &node,
        "PLACES",
        &ada,
        &order,
        &serde_json::json!({"channel": "store"}),
    );

    let affected = update(
        &node,
        &serde_json::json!({"relationType": "PLACES", "sourceIds": [ada], "targetIds": [order],
            "set": {"placedAt": "2024-03-01T09:30:00.250+02:00", "day": "2024-03-01",
                "untyped": "2024-03-01", "count": 3, "tags": ["a", "b"], "cleared": null},
            "types": {"placedAt": "datetime", "day": "datetime", "count": "number",
                "tags": "string", "cleared": "datetime"}}),
    )
    .unwrap();
    assert_eq!(affected, 1);

    let (properties, types) = typed_relationships(&node)[&format!("PLACES:{ada}->{order}")].clone();
    assert_eq!(
        properties,
        serde_json::json!({"channel": "store", "placedAt": "2024-03-01T07:30:00.250Z",
            "day": "2024-03-01", "untyped": "2024-03-01", "count": 3, "tags": ["a", "b"],
            "cleared": null}),
        "datetimes render as ISO strings; an untyped date string stays a string"
    );
    assert_eq!(
        types,
        serde_json::json!({"channel": "string", "placedAt": "datetime", "day": "datetime",
            "untyped": "string", "count": "number", "tags": "string"}),
        "a null has no type, whatever was declared"
    );
    let schema = schema_properties(&node, "PLACES");
    assert_eq!(schema["placedAt"].0, "datetime", "{schema:?}");
    assert_eq!(schema["untyped"].0, "string", "{schema:?}");

    // The relationship predicate still filters the stored datetime.
    let filtered = run(
        &node,
        &serde_json::json!({"kind": "query", "query": {"kind": "relationships", "query": {
            "predicate": {"day": {"$gte": "2024-02-01"}}}}}),
    )
    .unwrap();
    assert_eq!(filtered["total"], 1, "{filtered}");
}

#[test]
fn by_id_links_take_types_too() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let ada = record(&node, "Customer", "Ada");
    let first = record(&node, "Order", "o-1");
    let second = record(&node, "Order", "o-2");
    mutate(
        &node,
        "linkRecordsById",
        &serde_json::json!({"relationType": "PLACES", "sourceIds": [ada], "targetIds": [first],
            "properties": {"at": "2024-03-01T10:00:00Z", "note": "2024-03-01"},
            "types": {"at": "datetime"}}),
    )
    .unwrap();
    mutate(
        &node,
        "linkRecordsPairs",
        &serde_json::json!({"relationType": "PLACES",
            "pairs": [{"sourceId": ada, "targetId": second}],
            "properties": {"at": "2024-03-02"}, "types": {"at": "datetime"}}),
    )
    .unwrap();
    let rels = typed_relationships(&node);
    assert_eq!(
        rels[&format!("PLACES:{ada}->{first}")],
        (
            serde_json::json!({"at": "2024-03-01T10:00:00.000Z", "note": "2024-03-01"}),
            serde_json::json!({"at": "datetime", "note": "string"})
        )
    );
    assert_eq!(
        rels[&format!("PLACES:{ada}->{second}")],
        (
            serde_json::json!({"at": "2024-03-02"}),
            serde_json::json!({"at": "datetime"})
        )
    );

    // A bad declaration fails the link before anything is written.
    let third = record(&node, "Order", "o-3");
    let lmpg = node.scope().lmpg();
    let committed = lmpg.store.committed_version();
    for (kind, command) in [
        (
            "linkRecordsById",
            serde_json::json!({"relationType": "PLACES", "sourceIds": [ada],
                "targetIds": [third], "properties": {"at": "soon"}, "types": {"at": "datetime"}}),
        ),
        (
            "linkRecordsPairs",
            serde_json::json!({"relationType": "PLACES",
                "pairs": [{"sourceId": ada, "targetId": third}],
                "properties": {}, "types": {"at": "datetime"}}),
        ),
    ] {
        let error = mutate(&node, kind, &command).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidRequest, "{kind}: {error:?}");
        assert!(
            error.message.starts_with(&format!("invalid {kind}: ")),
            "{error:?}"
        );
    }
    assert_eq!(
        lmpg.store.committed_version(),
        committed,
        "nothing committed"
    );
}

#[test]
fn mistyped_values_and_undeclared_names_are_invalid_and_write_nothing() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let ada = record(&node, "Customer", "Ada");
    let order = record(&node, "Order", "o-1");
    link(
        &node,
        "PLACES",
        &ada,
        &order,
        &serde_json::json!({"channel": "store"}),
    );
    let lmpg = node.scope().lmpg();
    let before = lmpg.store.committed_version();
    let cases = [
        (
            serde_json::json!({"channel": "web"}),
            serde_json::json!({"placedAt": "datetime"}),
            "invalid updateRelationshipsById: types names `placedAt`, which is not in set",
        ),
        (
            serde_json::json!({"count": "3"}),
            serde_json::json!({"count": "number"}),
            "invalid updateRelationshipsById: property `count` is typed number but holds a \
             string value",
        ),
        (
            serde_json::json!({"flag": 1}),
            serde_json::json!({"flag": "boolean"}),
            "invalid updateRelationshipsById: property `flag` is typed boolean but holds a \
             number value",
        ),
        (
            serde_json::json!({"tags": ["a", 1]}),
            serde_json::json!({"tags": "string"}),
            "invalid updateRelationshipsById: property `tags` is typed string but holds an \
             array with a number element",
        ),
        (
            serde_json::json!({"at": "next tuesday"}),
            serde_json::json!({"at": "datetime"}),
            "invalid updateRelationshipsById: property `at` is typed datetime but its string \
             is not an ISO 8601 date or datetime",
        ),
        (
            serde_json::json!({"at": 1_709_251_200_000_i64}),
            serde_json::json!({"at": "datetime"}),
            "invalid updateRelationshipsById: property `at` is typed datetime but holds a \
             number value",
        ),
        (
            serde_json::json!({"at": ["2024-03-01", "2024-03-02"]}),
            serde_json::json!({"at": "datetime"}),
            "invalid updateRelationshipsById: property `at` is typed datetime but holds an \
             array; datetime arrays are not supported",
        ),
    ];
    for (set, types, message) in cases {
        let error = update(
            &node,
            &serde_json::json!({"relationType": "PLACES", "sourceIds": [ada],
                "targetIds": [order], "set": set, "types": types}),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidRequest, "{error:?}");
        assert_eq!(error.message, message);
    }
    // A name only in `unset` is not in `set` either.
    let unset_only = update(
        &node,
        &serde_json::json!({"relationType": "PLACES", "sourceIds": [ada], "targetIds": [order],
            "unset": ["channel"], "types": {"channel": "string"}}),
    )
    .unwrap_err();
    assert_eq!(unset_only.code, ErrorCode::InvalidRequest, "{unset_only:?}");
    assert_eq!(lmpg.store.committed_version(), before, "nothing committed");
    assert_eq!(
        relationships(&node)[&format!("PLACES:{ada}->{order}")],
        serde_json::json!({"channel": "store"})
    );
}

#[test]
fn a_typed_datetime_survives_retype_into_a_new_and_an_existing_relationship() {
    let node = Node::bootstrap(&mut super::tests::config(false)).unwrap();
    let ada = record(&node, "Customer", "Ada");
    let first = record(&node, "Order", "o-1");
    let second = record(&node, "Order", "o-2");
    for order in [&first, &second] {
        mutate(
            &node,
            "linkRecordsById",
            &serde_json::json!({"relationType": "PLACES", "sourceIds": [ada],
                "targetIds": [order], "properties": {"at": "2024-03-01T10:00:00Z"},
                "types": {"at": "datetime"}}),
        )
        .unwrap();
    }
    // `second` already has an ORDERED relationship: the retype merges into
    // it (an update) rather than creating one.
    link(
        &node,
        "ORDERED",
        &ada,
        &second,
        &serde_json::json!({"channel": "web"}),
    );
    mutate(
        &node,
        "retypeRelations",
        &serde_json::json!({"sourceLabel": "Customer", "targetLabel": "Order",
            "sourceType": "PLACES", "targetType": "ORDERED"}),
    )
    .unwrap();
    let rels = typed_relationships(&node);
    assert_eq!(rels.len(), 2, "{rels:?}");
    assert_eq!(
        rels[&format!("ORDERED:{ada}->{first}")],
        (
            serde_json::json!({"at": "2024-03-01T10:00:00.000Z"}),
            serde_json::json!({"at": "datetime"})
        )
    );
    assert_eq!(
        rels[&format!("ORDERED:{ada}->{second}")],
        (
            serde_json::json!({"channel": "web", "at": "2024-03-01T10:00:00.000Z"}),
            serde_json::json!({"channel": "string", "at": "datetime"})
        )
    );
}
