//! End-to-end coverage of the documented query document
//! (docs.rushdb.com/learn/search-query/): MongoDB-style `where` operators,
//! logical composition, `$id`, label-key relationship blocks, `hops`
//! variable-length traversal, `$cycle` detection, and query-document `select`/groupBy
//! lowering.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{import, Lmpg};
use rushdb_query::document::parse_document;
use rushdb_query::{
    aggregate, aggregate_to_rows, aggregate_with_cancel, find_records, find_records_with_cancel,
    QueryError,
};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::{json, Value};

fn setup() -> Lmpg {
    let store = Arc::new(Store::new_in_memory());
    Lmpg::new(store, TenantId::from_ordinal(71))
}

fn run(lmpg: &Lmpg, document: &Value) -> Vec<Value> {
    let q = parse_document(document).expect("valid query document");
    if let Some(agg) = q.aggregate {
        let buckets = aggregate(lmpg, &agg).expect("aggregate");
        buckets
            .iter()
            .map(|b| Value::Object(b.keys.clone()))
            .collect()
    } else {
        find_records(lmpg, &q.search).expect("search").records
    }
}

#[test]
fn in_nin_type_ends_with_and_logical_ops() {
    let lmpg = setup();
    for (name, status, premium, free, n) in [
        ("alice", "active", true, false, 5u64),
        ("bob", "pending", false, true, 3),
        ("carol", "archived", true, true, 7),
        ("dave", "deleted", false, false, 2),
    ] {
        lmpg.create_record(
            "USER",
            None,
            vec![
                ("name".into(), FieldChange::Set(json!(name))),
                ("status".into(), FieldChange::Set(json!(status))),
                ("premium".into(), FieldChange::Set(json!(premium))),
                ("freeAccess".into(), FieldChange::Set(json!(free))),
                ("score".into(), FieldChange::Set(json!(n))),
            ],
        )
        .unwrap();
    }

    // $in
    let rows = run(
        &lmpg,
        &json!({"labels": ["USER"], "where": {"status": {"$in": ["active", "pending"]}}}),
    );
    assert_eq!(rows.len(), 2);

    // $nin
    let rows = run(
        &lmpg,
        &json!({"labels": ["USER"], "where": {"status": {"$nin": ["deleted", "archived"]}}}),
    );
    assert_eq!(rows.len(), 2);

    // $type string + $endsWith
    let rows = run(
        &lmpg,
        &json!({"labels": ["USER"], "where": {
            "$and": [
                {"name": {"$type": "string"}},
                {"name": {"$endsWith": "e"}}
            ]
        }}),
    );
    assert_eq!(rows.len(), 2); // alice, dave

    // $gt + $lte combined on one field (implicit and in operator block)
    let rows = run(
        &lmpg,
        &json!({"labels": ["USER"], "where": {"score": {"$gte": 3, "$lte": 5}}}),
    );
    assert_eq!(rows.len(), 2); // alice(5), bob(3)

    // $xor: exactly one of premium / freeAccess
    let rows = run(
        &lmpg,
        &json!({"labels": ["USER"], "where": {"$xor": [{"premium": true}, {"freeAccess": true}]}}),
    );
    assert_eq!(rows.len(), 2); // alice, bob

    // $nor: not active and not pending
    let rows = run(
        &lmpg,
        &json!({"labels": ["USER"], "where": {"$nor": [{"status": "active"}, {"status": "pending"}]}}),
    );
    assert_eq!(rows.len(), 2); // carol, dave

    // $not + $and nested
    let rows = run(
        &lmpg,
        &json!({"labels": ["USER"], "where": {"$and": [
            {"premium": true},
            {"$not": {"status": "archived"}}
        ]}}),
    );
    assert_eq!(rows.len(), 1); // alice
}

#[test]
fn datetime_component_integer_overflow_is_rejected() {
    let result = parse_document(&json!({
        "where": { "createdAt": { "$year": 18_446_744_073_709_551_615u64 } }
    }));
    match result {
        Ok(_) => panic!("overflowing component should be rejected"),
        Err(err) => assert!(err.to_string().contains("expected integer")),
    }
}

#[test]
fn id_filter_and_flat_equality() {
    let lmpg = setup();
    let r = lmpg
        .create_record(
            "DOC",
            None,
            vec![("title".into(), FieldChange::Set(json!("hello")))],
        )
        .unwrap();

    let snap = lmpg.store.read_view().unwrap();
    let json = rushdb_lmpg::reconstruct::reconstruct_record(&snap, lmpg.tenant, r.ordinal)
        .unwrap()
        .unwrap();
    let id = json["__id"].as_str().unwrap().to_string();

    let rows = run(&lmpg, &json!({"labels": ["DOC"], "where": {"$id": id}}));
    assert_eq!(rows.len(), 1);

    let dashed = uuid::Uuid::from_bytes(r.record_id).to_string();
    let rows = run(
        &lmpg,
        &json!({"labels": ["DOC"], "where": {"$id": {"$in": [dashed]}}}),
    );
    assert_eq!(rows.len(), 1);

    // flat equality: { title: "hello" }
    let rows = run(
        &lmpg,
        &json!({"labels": ["DOC"], "where": {"title": "hello"}}),
    );
    assert_eq!(rows.len(), 1);
    // missing field doesn't match
    let rows = run(&lmpg, &json!({"labels": ["DOC"], "where": {"nope": true}}));
    assert_eq!(rows.len(), 0);
}

#[test]
fn inline_aggregate_operands_feed_derived_math() {
    let lmpg = setup();
    for (name, price) in [("a", 10), ("a", 20), ("b", 8)] {
        lmpg.create_record(
            "ITEM",
            None,
            vec![
                ("name".into(), FieldChange::Set(json!(name))),
                ("price".into(), FieldChange::Set(json!(price))),
            ],
        )
        .unwrap();
    }
    let parsed = parse_document(&json!({
        "labels": ["ITEM"],
        "select": {
            "total": {"$count": "*"},
            "ratio": {"$divide": [
                {"$sum": "$record.price"},
                {"$avg": "$record.price", "$precision": 2}
            ]}
        },
        "groupBy": ["$record.name"]
    }))
    .unwrap();
    let aggregate_request = parsed.aggregate.unwrap();
    let buckets = aggregate(&lmpg, &aggregate_request).unwrap();
    let rows = aggregate_to_rows(&lmpg, &buckets, &aggregate_request).unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter().all(|row| row.get("ratio").is_some()),
        "{rows:?}"
    );
    assert!(rows.iter().all(|row| {
        row.as_object()
            .is_some_and(|obj| obj.keys().all(|key| !key.starts_with("__ratio_")))
    }));
}

#[test]
fn related_label_block_and_alias() {
    let lmpg = setup();
    let tech = import::import_document(&lmpg, "COMPANY", &json!({"name": "Tech Corp"})).unwrap();
    let eng = import::import_document(
        &lmpg,
        "DEPARTMENT",
        &json!({"name": "Engineering", "headcount": 50}),
    )
    .unwrap();
    import::import_document(
        &lmpg,
        "DEPARTMENT",
        &json!({"name": "Sales", "headcount": 10}),
    )
    .unwrap();
    lmpg.create_relationship("HAS_DEPARTMENT", tech, eng, vec![])
        .unwrap();

    // basic related block with property filter
    let rows = run(
        &lmpg,
        &json!({"labels": ["COMPANY"], "where": {
            "DEPARTMENT": {"name": "Engineering", "headcount": {"$gte": 20}}
        }}),
    );
    assert_eq!(rows.len(), 1);

    // related block with $relation direction out + no match
    let rows = run(
        &lmpg,
        &json!({"labels": ["COMPANY"], "where": {
            "DEPARTMENT": {"$relation": {"type": "HAS_DEPARTMENT", "direction": "out"},
                           "name": "Sales"}
        }}),
    );
    assert_eq!(rows.len(), 0); // Sales is not a department of Tech
}

#[test]
fn variable_length_hops_traversal() {
    let lmpg = setup();
    // chain: a -> b -> c -> d
    let a = import::import_document(&lmpg, "EMP", &json!({"name": "A"})).unwrap();
    let b = import::import_document(&lmpg, "EMP", &json!({"name": "Alice"})).unwrap();
    let c = import::import_document(&lmpg, "EMP", &json!({"name": "C"})).unwrap();
    let d = import::import_document(&lmpg, "EMP", &json!({"name": "D"})).unwrap();
    lmpg.create_relationship("REPORTS_TO", a, b, vec![])
        .unwrap();
    lmpg.create_relationship("REPORTS_TO", b, c, vec![])
        .unwrap();
    lmpg.create_relationship("REPORTS_TO", c, d, vec![])
        .unwrap();

    // a's management chain up to 4 hops contains Alice (at depth 1)
    let rows = run(
        &lmpg,
        &json!({"labels": ["EMP"], "where": {
            "EMP": {"$relation": {"type": "REPORTS_TO", "direction": "out",
                                  "hops": {"min": 1, "max": 4}},
                    "name": {"$contains": "Alice"}}
        }}),
    );
    // every node whose chain reaches Alice: a (depth1), and none beyond
    assert_eq!(rows.len(), 1);

    // a's management chain reaches D at exactly 3 hops (a->b->c->d)
    let rows = run(
        &lmpg,
        &json!({"labels": ["EMP"], "where": {
            "EMP": {"$relation": {"type": "REPORTS_TO", "direction": "out",
                                  "hops": 3},
                    "name": "D"}
        }}),
    );
    assert_eq!(rows.len(), 1);
}

#[test]
fn cycle_detection_finds_rings() {
    let lmpg = setup();
    let a = import::import_document(&lmpg, "ACCOUNT", &json!({"n": "a"})).unwrap();
    let b = import::import_document(&lmpg, "ACCOUNT", &json!({"n": "b"})).unwrap();
    let c = import::import_document(&lmpg, "ACCOUNT", &json!({"n": "c"})).unwrap();
    let d = import::import_document(&lmpg, "ACCOUNT", &json!({"n": "d"})).unwrap();
    // ring a->b->c->a ; d isolated
    lmpg.create_relationship("TRANSFERRED_TO", a, b, vec![])
        .unwrap();
    lmpg.create_relationship("TRANSFERRED_TO", b, c, vec![])
        .unwrap();
    lmpg.create_relationship("TRANSFERRED_TO", c, a, vec![])
        .unwrap();
    lmpg.create_relationship("TRANSFERRED_TO", d, d, vec![])
        .unwrap();

    let rows = run(
        &lmpg,
        &json!({"labels": ["ACCOUNT"], "where": {
            "$cycle": {"type": "TRANSFERRED_TO", "direction": "out",
                       "hops": {"min": 2, "max": 6}}
        }}),
    );
    assert_eq!(rows.len(), 3); // a, b, c on the ring; d's self-loop is a 1-hop
}

#[test]
fn query_document_select_groupby_orderby() {
    let lmpg = setup();
    import::import_document(&lmpg, "ORDER", &json!({"status": "paid", "total": 100})).unwrap();
    import::import_document(&lmpg, "ORDER", &json!({"status": "paid", "total": 50})).unwrap();
    import::import_document(&lmpg, "ORDER", &json!({"status": "pending", "total": 25})).unwrap();

    // dimensional groupBy with $sum/$count, orderBy desc
    let q = parse_document(&json!({
        "labels": ["ORDER"],
        "select": {"count": {"$count": "*"}, "revenue": {"$sum": "$record.total"}},
        "groupBy": ["$record.status"],
        "orderBy": {"revenue": "desc"}
    }))
    .unwrap();
    let buckets = aggregate(&lmpg, q.aggregate.as_ref().unwrap()).unwrap();
    assert_eq!(buckets.len(), 2);
    assert_eq!(buckets[0].sums.get("revenue"), Some(&json!(150.0)));
    assert_eq!(buckets[0].count_roots, 2);

    // self-group KPI: groupBy on select key collapses to one global bucket
    let q = parse_document(&json!({
        "labels": ["ORDER"],
        "select": {"totalRevenue": {"$sum": "$record.total"}},
        "groupBy": ["totalRevenue"],
        "orderBy": {"totalRevenue": "asc"}
    }))
    .unwrap();
    let buckets = aggregate(&lmpg, q.aggregate.as_ref().unwrap()).unwrap();
    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0].sums.get("totalRevenue"), Some(&json!(175.0)));
}

#[test]
fn plain_select_projects_each_matching_record_with_aliases() {
    let lmpg = setup();
    import::import_document(
        &lmpg,
        "STARSHIP",
        &json!({
            "name": "Millennium Falcon",
            "pilot_character_ids": ["character_han_solo", "character_chewbacca"]
        }),
    )
    .unwrap();
    import::import_document(
        &lmpg,
        "STARSHIP",
        &json!({
            "name": "X-wing",
            "pilot_character_ids": ["character_rey"]
        }),
    )
    .unwrap();

    let query = parse_document(&json!({
        "labels": ["STARSHIP"],
        "where": { "name": { "$contains": "falcon" } },
        "select": {
            "starship": "$record.name",
            "pilot_character_ids": "$record.pilot_character_ids"
        },
        "limit": 100
    }))
    .unwrap();
    let aggregate_request = query
        .aggregate
        .expect("select lowers to projection execution");
    let buckets = aggregate(&lmpg, &aggregate_request).unwrap();
    let rows = aggregate_to_rows(&lmpg, &buckets, &aggregate_request).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("starship"), Some(&json!("Millennium Falcon")));
    assert_eq!(
        rows[0].get("pilot_character_ids"),
        Some(&json!(["character_han_solo", "character_chewbacca"]))
    );
    assert!(rows.iter().all(|row| row
        .as_object()
        .is_some_and(|object| !object.contains_key("__rushdb_projection_root"))));
}

#[test]
fn count_star_order_and_aggregate_column_order_match_query_contract() {
    let lmpg = setup();
    for faction in [
        "faction_rebel_alliance",
        "faction_galactic_republic",
        "faction_rebel_alliance",
        "faction_sith",
        "faction_rebel_alliance",
        "faction_galactic_republic",
        "faction_rebel_alliance",
    ] {
        import::import_document(&lmpg, "BATTLE", &json!({ "winner_faction_id": faction })).unwrap();
    }

    let parsed = parse_document(&json!({
        "labels": ["BATTLE"],
        "select": {
            "faction": "$record.winner_faction_id",
            "battles_won": { "$count": "*" }
        },
        "groupBy": ["$record.winner_faction_id"],
        "orderBy": { "battles_won": "desc" }
    }))
    .unwrap();
    let request = parsed.aggregate.unwrap();
    let buckets = aggregate(&lmpg, &request).unwrap();
    let rows = aggregate_to_rows(&lmpg, &buckets, &request).unwrap();

    assert_eq!(
        rows.iter()
            .map(|row| row["battles_won"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![4, 2, 1]
    );
    assert_eq!(
        rows[0].as_object().unwrap().keys().collect::<Vec<_>>(),
        vec!["winner_faction_id", "battles_won"]
    );
}

/// A group key keeps the property name as `groupBy` spells it, so a
/// camelCase property comes back under the name the caller used, and ordering
/// by that name still resolves.
#[test]
fn group_keys_keep_the_requested_property_spelling() {
    let lmpg = setup();
    for driver in ["driver_ayla", "driver_bo", "driver_ayla"] {
        import::import_document(&lmpg, "RACE", &json!({ "winningDriverRef": driver })).unwrap();
    }
    let parsed = parse_document(&json!({
        "labels": ["RACE"],
        "select": { "driver": "$record.winningDriverRef", "wins": { "$count": "*" } },
        "groupBy": ["$record.winningDriverRef"],
        "orderBy": { "winningDriverRef": "desc" }
    }))
    .unwrap();
    let request = parsed.aggregate.unwrap();
    let buckets = aggregate(&lmpg, &request).unwrap();
    let rows = aggregate_to_rows(&lmpg, &buckets, &request).unwrap();
    assert_eq!(
        rows,
        vec![
            json!({ "winningDriverRef": "driver_bo", "wins": 1 }),
            json!({ "winningDriverRef": "driver_ayla", "wins": 2 }),
        ]
    );
}

#[test]
fn id_ordering_uses_uuidv7_creation_order() {
    let lmpg = setup();
    for name in ["first", "second", "third"] {
        import::import_document(&lmpg, "ITEM", &json!({ "name": name })).unwrap();
    }

    let descending = run(&lmpg, &json!({ "labels": ["ITEM"], "orderBy": "desc" }));
    let descending_ids = descending
        .iter()
        .map(|record| record["__id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(descending_ids.windows(2).all(|pair| pair[0] > pair[1]));

    let ascending = run(
        &lmpg,
        &json!({ "labels": ["ITEM"], "orderBy": { "id": "asc" } }),
    );
    let ascending_ids = ascending
        .iter()
        .map(|record| record["__id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(ascending_ids.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn record_and_aggregate_scans_stop_cooperatively_when_cancelled() {
    let lmpg = setup();
    for value in 0..32 {
        import::import_document(&lmpg, "ITEM", &json!({ "value": value })).unwrap();
    }

    let parsed = parse_document(&json!({ "labels": ["ITEM"] })).unwrap();
    let probes = std::cell::Cell::new(0usize);
    let cancelled = || {
        probes.set(probes.get() + 1);
        probes.get() > 4
    };
    assert!(matches!(
        find_records_with_cancel(&lmpg, &parsed.search, &cancelled),
        Err(QueryError::Cancelled)
    ));

    let parsed = parse_document(&json!({
        "labels": ["ITEM"],
        "select": { "count": { "$count": "*" } },
        "groupBy": ["$record.value"]
    }))
    .unwrap();
    let probes = std::cell::Cell::new(0usize);
    let cancelled = || {
        probes.set(probes.get() + 1);
        probes.get() > 6
    };
    assert!(matches!(
        aggregate_with_cancel(&lmpg, parsed.aggregate.as_ref().unwrap(), &cancelled),
        Err(QueryError::Cancelled)
    ));
}

#[test]
fn datetime_iso_and_component_operators() {
    let lmpg = setup();
    // created stored as epoch-ms Int64 values:
    // 2023-01-01T00:00:00Z = 1672531200000
    // 2023-06-15T12:00:00Z = 1686823200000
    // 2024-01-01T00:00:00Z = 1704067200000
    for ms in [1_672_531_200_000i64, 1_686_823_200_000, 1_704_067_200_000] {
        lmpg.create_record(
            "EVENT",
            None,
            vec![("created".into(), FieldChange::Set(json!(ms)))],
        )
        .unwrap();
    }

    // $gte ISO string: on or after 2023-06-15
    let rows = run(
        &lmpg,
        &json!({"labels": ["EVENT"], "where": {"created": {"$gte": "2023-06-15T00:00:00Z"}}}),
    );
    assert_eq!(rows.len(), 2);

    // $lt component object (year 2024 boundary)
    let rows = run(
        &lmpg,
        &json!({"labels": ["EVENT"], "where": {"created": {"$lt": {"$year": 2024, "$month": 1, "$day": 1}}}}),
    );
    assert_eq!(rows.len(), 2);

    // $year component: records created in 2023
    let rows = run(
        &lmpg,
        &json!({"labels": ["EVENT"], "where": {"created": {"$year": 2023}}}),
    );
    assert_eq!(rows.len(), 2);

    // $year + $month: June 2023
    let rows = run(
        &lmpg,
        &json!({"labels": ["EVENT"], "where": {"$and": [
            {"created": {"$year": 2023}},
            {"created": {"$month": 6}}
        ]}}),
    );
    assert_eq!(rows.len(), 1);
}

/// ORDER BY must be a total order even when a field mixes types or is missing
/// on some records. A comparator returning `Equal` in a non-transitive way
/// (e.g. from a NaN `partial_cmp`) previously panicked `sort_by` with
/// "user-provided comparison function does not correctly implement a total
/// order".
#[test]
fn order_by_handles_nan_and_mixed_types_as_total_order() {
    let lmpg = setup();
    // a real number, a string, a missing value, and a large number on one field
    lmpg.create_record(
        "ITEM",
        None,
        vec![("birth_year_bby".into(), FieldChange::Set(json!(2000)))],
    )
    .unwrap();
    lmpg.create_record(
        "ITEM",
        None,
        vec![("birth_year_bby".into(), FieldChange::Set(json!("c")))],
    )
    .unwrap();
    lmpg.create_record("ITEM", None, vec![]).unwrap();
    lmpg.create_record(
        "ITEM",
        None,
        vec![("birth_year_bby".into(), FieldChange::Set(json!(100)))],
    )
    .unwrap();

    for order_by in [
        json!({"birth_year_bby": "asc"}),
        json!({"birth_year_bby": "desc"}),
    ] {
        let rows = run(
            &lmpg,
            &json!({"labels": ["ITEM"], "where": {}, "orderBy": order_by, "limit": 100}),
        );
        assert_eq!(rows.len(), 4, "orderBy {order_by} must not panic");
    }
}

/// Regression: `$count` over a related-record alias must count the distinct
/// related records, not be treated as a root-record field (all-zeros bug).
/// Query shape mirrors the Star Wars fixture: FACTION <-[:`HAS_WINNER_FACTION_ID`]-(BATTLE).
#[test]
fn count_over_related_alias_counts_distinct_related_records() {
    let lmpg = setup();
    // Two factions; a wins twice, b wins once, via BATTLE -> FACTION edges.
    let a = import::import_document(&lmpg, "FACTION", &json!({"name": "Jedi Order"})).unwrap();
    let b = import::import_document(&lmpg, "FACTION", &json!({"name": "Sith Order"})).unwrap();
    for _ in 0..2 {
        let battle = import::import_document(&lmpg, "BATTLE", &json!({"name": "b"})).unwrap();
        lmpg.create_relationship("HAS_WINNER_FACTION_ID", battle, a, vec![])
            .unwrap();
    }
    {
        let battle = import::import_document(&lmpg, "BATTLE", &json!({"name": "b"})).unwrap();
        lmpg.create_relationship("HAS_WINNER_FACTION_ID", battle, b, vec![])
            .unwrap();
    }

    let parsed = parse_document(&json!({
        "labels": ["FACTION"],
        "where": {
            "BATTLE": {
                "$alias": "$battle",
                "$relation": { "type": "HAS_WINNER_FACTION_ID", "direction": "in" }
            }
        },
        "select": {
            "faction": "$record.name",
            "wins": { "$count": "$battle" }
        },
        "groupBy": ["$record.name"],
        "orderBy": { "wins": "desc" }
    }))
    .unwrap();
    let req = parsed.aggregate.unwrap();
    let buckets = aggregate(&lmpg, &req).unwrap();
    let rows = aggregate_to_rows(&lmpg, &buckets, &req).unwrap();

    assert_eq!(
        rows.len(),
        2,
        "both factions have at least one winning battle"
    );
    assert_eq!(rows[0]["name"], json!("Jedi Order"));
    assert_eq!(rows[0]["wins"], json!(2), "Jedi Order won two battles");
    assert_eq!(rows[1]["name"], json!("Sith Order"));
    assert_eq!(rows[1]["wins"], json!(1), "Sith Order won one battle");
}

#[test]
fn collect_from_alias_gathers_related_records() {
    let lmpg = setup();
    let company = import::import_document(&lmpg, "Company", &json!({"name": "TechCorp"})).unwrap();
    let e1 = import::import_document(
        &lmpg,
        "Employee",
        &json!({"name": "Jane", "salary": 600_000}),
    )
    .unwrap();
    let e2 = import::import_document(
        &lmpg,
        "Employee",
        &json!({"name": "John", "salary": 550_000}),
    )
    .unwrap();
    lmpg.create_relationship("EMPLOYS", company, e1, vec![])
        .unwrap();
    lmpg.create_relationship("EMPLOYS", company, e2, vec![])
        .unwrap();

    let rows = run(
        &lmpg,
        &json!({
            "labels": ["Company"],
            "where": { "Employee": { "$alias": "$employee" } },
            "select": {
                "employeeNames": {
                    "$collect": {
                        "from": "$employee",
                        "select": { "name": "$employee.name" },
                        "orderBy": { "name": "asc" },
                        "limit": 10
                    }
                }
            }
        }),
    );
    let names: Vec<String> = rows[0]["employeeNames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["Jane".to_string(), "John".to_string()]);
}

#[test]
fn nested_label_collect_produces_hierarchical_output() {
    let lmpg = setup();
    let company = import::import_document(&lmpg, "Company", &json!({"name": "TechCorp"})).unwrap();
    let dept = import::import_document(&lmpg, "Department", &json!({"name": "Eng"})).unwrap();
    let project = import::import_document(&lmpg, "Project", &json!({"name": "Platform"})).unwrap();
    let employee = import::import_document(
        &lmpg,
        "Employee",
        &json!({"name": "Jane", "salary": 620_000}),
    )
    .unwrap();
    lmpg.create_relationship("HAS_DEPT", company, dept, vec![])
        .unwrap();
    lmpg.create_relationship("HAS_PROJECT", dept, project, vec![])
        .unwrap();
    lmpg.create_relationship("HAS_EMPLOYEE", project, employee, vec![])
        .unwrap();

    let rows = run(
        &lmpg,
        &json!({
            "labels": ["Company"],
            "select": {
                "departments": {
                    "$collect": {
                        "label": "Department",
                        "select": {
                            "name": "$self.name",
                            "projects": {
                                "$collect": {
                                    "label": "Project",
                                    "select": {
                                        "name": "$self.name",
                                        "employees": {
                                            "$collect": {
                                                "label": "Employee",
                                                "select": { "name": "$self.name" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }),
    );
    assert_eq!(rows.len(), 1, "one company row");
    eprintln!(
        "[dbg] nested rows: {}",
        serde_json::to_string_pretty(&rows[0]).unwrap()
    );
    let departments = rows[0]["departments"].as_array().unwrap();
    assert_eq!(departments.len(), 1);
    assert_eq!(departments[0]["name"], json!("Eng"));
    let projects = departments[0]["projects"].as_array().unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0]["name"], json!("Platform"));
    let employees = projects[0]["employees"].as_array().unwrap();
    assert_eq!(employees.len(), 1);
    assert_eq!(employees[0]["name"], json!("Jane"));
}

#[test]
fn label_collect_where_filter_and_slice() {
    let lmpg = setup();
    let company = import::import_document(&lmpg, "Company", &json!({"name": "TechCorp"})).unwrap();
    for name in ["Alice", "Bob", "Carol"] {
        let e =
            import::import_document(&lmpg, "Employee", &json!({"name": name, "salary": 100_000}))
                .unwrap();
        lmpg.create_relationship("EMPLOYS", company, e, vec![])
            .unwrap();
    }
    let rows = run(
        &lmpg,
        &json!({
            "labels": ["Company"],
            "select": {
                "senior": {
                    "$collect": {
                        "label": "Employee",
                        "where": { "name": { "$startsWith": "A" } },
                        "select": { "name": "$self.name" }
                    }
                }
            }
        }),
    );
    let names: Vec<String> = rows[0]["senior"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["Alice".to_string()]);
}

#[test]
fn aggregate_with_select_aliases_and_collect_per_root() {
    let lmpg = setup();
    let tech = import::import_document(&lmpg, "Company", &json!({"name": "TechCorp"})).unwrap();
    let other = import::import_document(&lmpg, "Company", &json!({"name": "OldCorp"})).unwrap();
    let e1 = import::import_document(
        &lmpg,
        "Employee",
        &json!({"name": "Jane", "salary": 600_000}),
    )
    .unwrap();
    let e2 = import::import_document(
        &lmpg,
        "Employee",
        &json!({"name": "John", "salary": 550_000}),
    )
    .unwrap();
    let e3 = import::import_document(
        &lmpg,
        "Employee",
        &json!({"name": "Grace", "salary": 480_000}),
    )
    .unwrap();
    lmpg.create_relationship("EMPLOYS", tech, e1, vec![])
        .unwrap();
    lmpg.create_relationship("EMPLOYS", tech, e2, vec![])
        .unwrap();
    lmpg.create_relationship("EMPLOYS", other, e3, vec![])
        .unwrap();

    let q = parse_document(&json!({
        "labels": ["Company"],
        "where": { "Employee": { "$alias": "$employee" } },
        "select": {
            "companyName": "$record.name",
            "headcount": { "$count": "$employee" },
            "totalWage": { "$sum": "$employee.salary" },
            "minSalary": { "$min": "$employee.salary" },
            "maxSalary": { "$max": "$employee.salary" },
            "employeeNames": {
                "$collect": {
                    "from": "$employee",
                    "select": { "name": "$employee.name" },
                    "orderBy": { "name": "asc" }
                }
            }
        }
    }))
    .unwrap();
    let req = q.aggregate.unwrap();
    let buckets = aggregate(&lmpg, &req).unwrap();
    let rows = aggregate_to_rows(&lmpg, &buckets, &req).unwrap();

    // Per-root rows: one per company (no groupBy).
    assert_eq!(rows.len(), 2, "one row per root company");
    let tech_row = rows
        .iter()
        .find(|r| r["companyName"] == json!("TechCorp"))
        .expect("tech row");
    assert_eq!(tech_row["headcount"], json!(2));
    assert_eq!(tech_row["totalWage"], json!(1_150_000.0));
    assert_eq!(tech_row["minSalary"], json!(550_000.0));
    assert_eq!(tech_row["maxSalary"], json!(600_000.0));
    let names: Vec<String> = tech_row["employeeNames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["Jane".to_string(), "John".to_string()]);
    let old_row = rows
        .iter()
        .find(|r| r["companyName"] == json!("OldCorp"))
        .expect("old row");
    assert_eq!(old_row["headcount"], json!(1));
    assert_eq!(old_row["totalWage"], json!(480_000.0));
    assert_eq!(old_row["employeeNames"][0]["name"], json!("Grace"));
}

#[test]
fn self_group_divide_ref_matches_prod() {
    let lmpg = setup();
    // 26 military ships with crew_capacity (mix).
    for i in 0..26 {
        import::import_document(
            &lmpg,
            "STARSHIP",
            &json!({ "is_military": true, "crew_capacity": 200_000 + i * 3000 }),
        )
        .unwrap();
    }
    let q = parse_document(&json!({
        "labels": ["STARSHIP"],
        "where": { "is_military": true },
        "select": {
            "totalCrewCapacity": { "$sum": "$record.crew_capacity" },
            "shipCount": { "$count": "*" },
            "crewPerShip": { "$divide": [ { "$ref": "totalCrewCapacity" }, { "$ref": "shipCount" } ] }
        },
        "groupBy": ["totalCrewCapacity", "shipCount", "crewPerShip"],
        "orderBy": { "crewPerShip": "desc" }
    }))
    .unwrap();
    let req = q.aggregate.unwrap();
    let (buckets, matched) = aggregate_with_cancel(&lmpg, &req, &|| false).unwrap();
    let rows = aggregate_to_rows(&lmpg, &buckets, &req).unwrap();
    assert_eq!(matched, 26, "total = matched roots");
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    // 26 ships at 200000 + i*3000: sum = 6175000, per-ship = 6175000/26 = 237500
    assert_eq!(row["shipCount"], json!(26));
    assert_eq!(row["totalCrewCapacity"], json!(6_175_000.0));
    let crew_per_ship = row["crewPerShip"].as_f64().unwrap();
    assert!(
        (crew_per_ship - 237_500.0).abs() < 0.01,
        "crewPerShip={crew_per_ship}"
    );
    assert!(
        row.get("totalcrewcapacity").is_none(),
        "no junk self-group keys"
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one scenario asserts several alias-metric results that share fixture setup"
)]
fn alias_metrics_apply_level_filter_and_traverse_paths() {
    // q1 pattern: alias $count/$sum/$avg must apply the alias-level `where`
    // filter (is_military) — only military ships are counted/summed.
    let lmpg = setup();
    let f1 = import::import_document(&lmpg, "FACTION", &json!({ "id": "f1", "name": "Rebels" }))
        .unwrap();
    for (name, military, crew, firepower) in [
        ("military ship", true, 100.0, 80.0),
        ("civilian ship", false, 200.0, 40.0),
    ] {
        let s = import::import_document(
            &lmpg,
            "STARSHIP",
            &json!({ "id": name, "is_military": military, "crew_capacity": crew, "firepower_score": firepower }),
        )
        .unwrap();
        let _ = lmpg.create_relationship("OWNS", f1, s, vec![]);
    }
    let q = parse_document(&json!({
        "labels": ["FACTION"],
        "where": { "STARSHIP": { "$alias": "$starship", "is_military": true } },
        "select": {
            "name": "$record.name",
            "militaryShipCount": { "$count": "$starship" },
            "totalCrewCapacity": { "$sum": "$starship.crew_capacity" },
            "avgFirepower": { "$avg": "$starship.firepower_score", "$precision": 0 }
        },
        "groupBy": ["$record.name"]
    }))
    .unwrap();
    let req = q.aggregate.unwrap();
    let (buckets, _) = aggregate_with_cancel(&lmpg, &req, &|| false).unwrap();
    let rows = aggregate_to_rows(&lmpg, &buckets, &req).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["militaryShipCount"], json!(1));
    assert_eq!(rows[0]["totalCrewCapacity"], json!(100.0));
    assert_eq!(rows[0]["avgFirepower"], json!(80.0));

    // q2 pattern: multi-hop alias $collect (planet -> battle -> starship) with
    // a per-level filter; only firepower>=60 ships appear, ordered by the
    // output alias (firepower) desc.
    let p1 =
        import::import_document(&lmpg, "PLANET", &json!({ "id": "p1", "name": "Endor" })).unwrap();
    let b1 = import::import_document(
        &lmpg,
        "BATTLE",
        &json!({ "id": "b1", "name": "Endor battle" }),
    )
    .unwrap();
    let _ = lmpg.create_relationship("HOSTED", p1, b1, vec![]);
    for (name, fp, crew) in [("s3", 90.0, 50.0), ("s4", 70.0, 30.0), ("s5", 20.0, 10.0)] {
        let s = import::import_document(
            &lmpg,
            "STARSHIP",
            &json!({ "id": name, "name": name, "firepower_score": fp, "crew_capacity": crew }),
        )
        .unwrap();
        let _ = lmpg.create_relationship("INVOLVED", b1, s, vec![]);
    }
    let q2 = parse_document(&json!({
        "labels": ["PLANET"],
        "where": { "BATTLE": { "STARSHIP": { "$alias": "$starship", "firepower_score": { "$gte": 60 } } } },
        "select": {
            "planetName": "$record.name",
            "highFirepowerStarships": {
                "$collect": {
                    "from": "$starship",
                    "select": {
                        "name": "$starship.name",
                        "firepower": "$starship.firepower_score",
                        "crewCapacity": "$starship.crew_capacity"
                    },
                    "orderBy": { "firepower": "desc" }
                }
            }
        }
    }))
    .unwrap();
    let req2 = q2.aggregate.unwrap();
    let (buckets2, _) = aggregate_with_cancel(&lmpg, &req2, &|| false).unwrap();
    let rows2 = aggregate_to_rows(&lmpg, &buckets2, &req2).unwrap();
    assert_eq!(rows2.len(), 1);
    let ships = rows2[0]["highFirepowerStarships"].as_array().unwrap();
    assert_eq!(ships.len(), 2, "non-matching firepower ship excluded");
    assert_eq!(
        ships[0]["name"],
        json!("s3"),
        "orderBy output alias (firepower) desc"
    );
    assert_eq!(ships[1]["name"], json!("s4"));
    assert_eq!(ships[0]["firepower"], json!(90.0));

    // q7 pattern: $avg with $precision 0 truncates toward zero (toInteger),
    // matching prod: (100 + 101) / 2 = 100.5 -> 100, not 101.
    let q7 = parse_document(&json!({
        "labels": ["BATTLE"],
        "select": {
            "count": { "$count": "*" },
            "avgCasualties": { "$avg": "$record.estimated_military_casualties", "$precision": 0 }
        },
        "groupBy": ["$record.era"]
    }))
    .unwrap();
    for era in [0u64, 1] {
        let b = import::import_document(
            &lmpg,
            "BATTLE",
            &json!({ "era": "era_a", "estimated_military_casualties": if era == 0 { 100 } else { 101 } }),
        )
        .unwrap();
        let _ = b;
    }
    let req7 = q7.aggregate.unwrap();
    let (buckets7, _) = aggregate_with_cancel(&lmpg, &req7, &|| false).unwrap();
    let rows7 = aggregate_to_rows(&lmpg, &buckets7, &req7).unwrap();
    let era_a = rows7
        .iter()
        .find(|r| r.get("era").and_then(|v| v.as_str()) == Some("era_a"))
        .unwrap();
    assert_eq!(era_a["count"], json!(2));
    assert_eq!(
        era_a["avgCasualties"],
        json!(100.0),
        "precision 0 truncates"
    );
}

#[test]
fn nested_aggregate_math_rejects_lossy_integer_literals() {
    let result = parse_document(&json!({
        "select": {
            "value": {"$sum": {"$add": [9_007_199_254_740_993i64, 1]}}
        }
    }));
    let Err(error) = result else {
        panic!("nested aggregate literal must not round through f64");
    };
    assert!(error.to_string().contains("not exactly representable"));
}

#[test]
fn aggregate_derived_math_errors_are_not_discarded() {
    let lmpg = setup();
    lmpg.create_record(
        "ITEM",
        None,
        vec![("price".into(), FieldChange::Set(json!(10)))],
    )
    .unwrap();
    let parsed = parse_document(&json!({
        "labels": ["ITEM"],
        "select": {
            "ratio": {"$divide": [{"$sum": "$record.price"}, 0]}
        }
    }))
    .unwrap();
    let error = aggregate(&lmpg, &parsed.aggregate.unwrap()).expect_err("divide by zero");
    assert!(error.to_string().contains("divide by zero"));
}

/// C8 (cloud-capacity F9): the protocol always forwards the query predicate as
/// `where`, and an empty predicate (`{}` or `null`) means "no filter". It must
/// parse to no `where` at all, otherwise every `$count`/`total` misses the
/// O(1) catalog path and reconstructs every record of the label.
#[test]
fn empty_where_parses_to_no_filter_so_count_takes_the_catalog_path() {
    for empty in [json!({}), Value::Null] {
        let parsed = parse_document(&json!({
            "labels": ["User"],
            "where": empty,
            "select": {"n": {"$count": "*"}}
        }))
        .expect("valid query document");
        let aggregate = parsed.aggregate.expect("count is an aggregate");
        assert!(aggregate.where_.is_none(), "empty where must be no filter");
        assert!(parsed.search.where_.is_none());
    }
    // A real predicate is still a filter.
    let parsed = parse_document(&json!({"labels": ["User"], "where": {"age": 3}})).unwrap();
    assert!(parsed.search.where_.is_some());
}

#[test]
fn unknown_operators_and_directions_are_refused() {
    for document in [
        json!({"where": {"name": {"$regx": "a"}}}),
        json!({"where": {"$regx": {"name": "a"}}}),
        json!({"where": {"DEPARTMENT": {"name": {"$regx": "a"}}}}),
        json!({"where": {"DEPARTMENT": {"$relation": "X", "$exists": true}}}),
        json!({"where": {"DEPARTMENT": {"$relation": {"type": "X", "direction": "sideways"}}}}),
    ] {
        assert!(parse_document(&document).is_err(), "{document}");
    }
}

#[test]
fn relation_type_is_matched_verbatim() {
    let lmpg = setup();
    let company = import::import_document(&lmpg, "COMPANY", &json!({"name": "Tech"})).unwrap();
    let dept = import::import_document(&lmpg, "DEPARTMENT", &json!({"name": "Eng"})).unwrap();
    lmpg.create_relationship("hasDepartment", company, dept, vec![])
        .unwrap();
    let rows = run(
        &lmpg,
        &json!({"labels": ["COMPANY"], "where": {
            "DEPARTMENT": {"$relation": {"type": "hasDepartment", "direction": "out"}}
        }}),
    );
    assert_eq!(rows.len(), 1, "a lower-case relationship type is found");
}

#[test]
fn type_never_matches_a_record_without_the_property() {
    let lmpg = setup();
    import::import_document(&lmpg, "ITEM", &json!({"name": "with", "price": 3})).unwrap();
    import::import_document(&lmpg, "ITEM", &json!({"name": "without"})).unwrap();
    let rows = run(
        &lmpg,
        &json!({"labels": ["ITEM"], "where": {"price": {"$type": "number"}}}),
    );
    assert_eq!(rows.len(), 1, "{rows:?}");
}

#[test]
fn unindexed_walks_above_the_threshold_are_counted() {
    let lmpg = setup();
    let mut batch = lmpg.begin_batch();
    let records = usize::try_from(rushdb_query::UNINDEXED_WALK_THRESHOLD).unwrap() + 1;
    for i in 0..records {
        batch.ops.push(rushdb_lmpg::BatchOp::CreateRecord {
            label: "ROW".into(),
            explicit_id: None,
            fields: vec![("n".into(), FieldChange::Set(json!(i % 7)))],
        });
    }
    lmpg.run_batch(batch).unwrap();
    let before = rushdb_query::unindexed_walks_total();
    let rows = run(
        &lmpg,
        &json!({"labels": ["ROW"], "where": {"n": 3}, "limit": 5}),
    );
    assert_eq!(rows.len(), 5);
    assert!(rushdb_query::unindexed_walks_total() > before);
}
