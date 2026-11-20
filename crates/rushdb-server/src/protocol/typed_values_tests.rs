//! Typed values (dates, times, local date-times, datetimes with their
//! offset, durations, points) written once read back the same through every
//! surface: records, filters, aggregates, describe, SQL, Cypher and Mongo.
//! None of them shows the engine's internal tagged form.

use super::transaction_reference_tests::run;
use super::*;
use serde_json::{json, Value};

fn node() -> Node {
    let mut cfg = super::tests::config(false);
    cfg.languages = rushdb_protocol::LanguageSet {
        sql: true,
        gql: true,
        cypher: true,
        gremlin: true,
        sql_pgq: true,
        mongodb: true,
    };
    Node::bootstrap(&mut cfg).unwrap()
}

/// Every value written, in its public form.
fn event() -> Value {
    json!({
        "name": "launch",
        "day": "2024-05-01",
        "opens": "09:30:00",
        "local": "2024-05-01T09:30:00.5",
        "at": "2024-05-01T09:30:00.000001+02:00",
        "lasts": "P1DT2H",
        "where": {"type": "Point", "coordinates": [13.4, 52.5]},
        "days": ["2024-05-01", "2024-05-02"],
    })
}

/// The type each property of [`event`] is stored as.
fn types() -> Value {
    json!({
        "name": "string", "day": "date", "opens": "localtime", "local": "localdatetime",
        "at": "datetime", "lasts": "duration", "where": "point", "days": "date",
    })
}

fn assert_public(response: &Value) {
    let text = response.to_string();
    for tag in [
        "__date",
        "__localtime",
        "__localdatetime",
        "__datetime",
        "__duration",
        "__point",
    ] {
        assert!(!text.contains(tag), "{tag} leaked: {text}");
    }
}

fn query(node: &Node, query: &Value) -> Value {
    let response = run(
        node,
        &json!({"kind": "query", "query": {"kind": "records", "query": query}}),
    )
    .unwrap();
    assert_public(&response);
    response
}

fn statement(node: &Node, language: &str, statement: &str) -> Vec<Value> {
    let response = run(
        node,
        &json!({"kind": "query", "query": {"kind": "statement", "language": language,
            "statement": statement}}),
    )
    .unwrap_or_else(|error| panic!("{language} `{statement}`: {error:?}"));
    assert_public(&response);
    response["rows"].as_array().cloned().unwrap_or_default()
}

#[test]
fn typed_values_round_trip_through_every_surface() {
    let node = node();
    node.scope()
        .create_record("Event", &event(), &[], &json!({"suggestTypes": true}))
        .unwrap();

    // Records: every value as written, typed by its own type.
    let records = query(&node, &json!({"labels": ["Event"]}));
    let record = &records["records"][0];
    for (name, value) in event().as_object().unwrap() {
        assert_eq!(&record["properties"][name], value, "{name}: {record}");
    }
    assert_eq!(record["propertyTypes"], types(), "{record}");

    // Filters compare each value in its own type.
    for (filter, matches) in [
        (json!({"day": "2024-05-01"}), true),
        (
            json!({"day": {"$gt": "2024-04-30", "$lt": "2024-05-02"}}),
            true,
        ),
        (json!({"opens": {"$gte": "09:00"}}), true),
        (json!({"local": {"$lt": "2024-05-01T09:30:01"}}), true),
        // The same instant written in UTC.
        (json!({"at": "2024-05-01T07:30:00.000001Z"}), true),
        (json!({"at": {"$gt": "2024-05-01T07:30:00.000001Z"}}), false),
        (json!({"lasts": {"$gt": "PT25H"}}), true),
        (json!({"lasts": {"$gt": "P1D"}}), true),
        (json!({"days": "2024-05-02"}), true),
        (json!({"at": {"$hour": 7}}), true),
        (json!({"local": {"$microsecond": 500_000}}), true),
        (json!({"day": {"$type": "date"}}), true),
        (json!({"day": "2024-05-02"}), false),
    ] {
        let found = query(&node, &json!({"labels": ["Event"], "predicate": filter}));
        assert_eq!(found["total"] == 1, matches, "{filter}: {found}");
    }

    // Aggregates return values of their own type.
    let rows = query(
        &node,
        &json!({"labels": ["Event"], "select": {
            "first": {"$min": "$record.at"}, "last": {"$max": "$record.day"},
        }}),
    );
    assert_eq!(
        rows["rows"][0],
        json!({"first": "2024-05-01T09:30:00.000001+02:00", "last": "2024-05-01"}),
        "{rows}"
    );
    let buckets = query(
        &node,
        &json!({"labels": ["Event"], "select": {
            "month": {"$timeBucket": {"field": "$record.local", "unit": "month"}},
            "n": {"$count": "*"},
        }, "groupBy": ["month"]}),
    );
    assert_eq!(
        buckets["rows"][0]["month"],
        json!("2024-05-01T00:00:00"),
        "{buckets}"
    );

    // Describe names each type.
    let described = run(
        &node,
        &json!({"kind": "describe", "describe": {"kind": "schema", "query": {"labels": ["Event"]}}}),
    )
    .unwrap();
    assert_public(&described);
    let described = described.to_string();
    for ty in [
        "date",
        "localtime",
        "localdatetime",
        "datetime",
        "duration",
        "point",
    ] {
        assert!(
            described.contains(&format!("\"{ty}\"")),
            "{ty} not described: {described}"
        );
    }

    // SQL reads dates, times and timestamps as its own types (a datetime as
    // a timestamptz, in UTC), durations as ISO 8601 text and points as
    // PostgreSQL point text.
    let rows = statement(
        &node,
        "sql",
        "SELECT day, opens, at, lasts, \"where\" FROM \"Event\"",
    );
    assert_eq!(
        rows[0],
        json!({"day": "2024-05-01", "opens": "09:30:00",
            "at": "2024-05-01T07:30:00.000001Z", "lasts": "P1DT2H", "where": "(13.4,52.5)"}),
    );

    // Cypher reads Grafeo's typed values back in the same literal forms.
    let rows = statement(
        &node,
        "cypher",
        "MATCH (e:Event) RETURN e.day AS day, e.at AS at, e.lasts AS lasts, \
         e.day.year AS year, e.at > datetime('2024-05-01T07:00:00Z') AS later",
    );
    assert_eq!(
        rows[0],
        json!({"day": "2024-05-01", "at": "2024-05-01T09:30:00.000001+02:00",
            "lasts": "P1DT2H", "year": 2024, "later": true}),
    );

    // Mongo returns the record's values in their public form.
    let rows = statement(&node, "mongo", r#"{"collection": "Event", "find": {}}"#);
    assert_eq!(rows[0]["day"], json!("2024-05-01"), "{rows:?}");
    assert_eq!(rows[0]["where"], event()["where"], "{rows:?}");
}

#[test]
fn a_cypher_write_stores_typed_values() {
    let node = node();
    run(
        &node,
        &json!({"kind": "mutate", "mutation": {"kind": "statement", "language": "cypher",
            "statement": "CREATE (:Event {day: date('2024-05-01'), at: datetime('2024-05-01T09:30:00+02:00'), \
                lasts: duration('P1D'), local: localdatetime('2024-05-01T09:30:00')})"}}),
    )
    .unwrap();
    let records = query(
        &node,
        &json!({"labels": ["Event"], "predicate": {"day": {"$gte": "2024-05-01"}}}),
    );
    let record = &records["records"][0];
    assert_eq!(
        record["propertyTypes"],
        json!({"day": "date", "at": "datetime", "lasts": "duration", "local": "localdatetime"}),
        "{record}"
    );
    assert_eq!(
        record["properties"]["at"],
        json!("2024-05-01T09:30:00+02:00")
    );
}

#[test]
fn one_name_holding_integers_and_datetimes_renders_each_by_its_value() {
    let node = node();
    for value in [json!(5), json!("2024-05-01T10:00:00Z")] {
        node.scope()
            .create_record(
                "Mixed",
                &json!({"x": value}),
                &[],
                &json!({"suggestTypes": true}),
            )
            .unwrap();
    }
    let records = query(&node, &json!({"labels": ["Mixed"]}));
    let mut seen: Vec<(Value, Value)> = records["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| {
            (
                record["properties"]["x"].clone(),
                record["propertyTypes"]["x"].clone(),
            )
        })
        .collect();
    seen.sort_by_key(|(value, _)| value.to_string());
    assert_eq!(
        seen,
        [
            (json!("2024-05-01T10:00:00Z"), json!("datetime")),
            (json!(5), json!("number")),
        ]
    );
}

/// `$within` (a radius and a box, one across the antimeridian) and
/// `$distance` order find exactly what a brute-force check of every point
/// finds, both by walking the property and through a declared index.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one scatter checked by both access paths"
)]
fn spatial_filters_and_distance_order_match_a_brute_force_oracle() {
    use rushdb_types::types::point;
    let node = node();
    // A fixed scatter over the globe, denser around the antimeridian.
    let mut seed = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        f64::from(u32::try_from(seed >> 40).unwrap()) / f64::from(1u32 << 24)
    };
    let mut points = Vec::new();
    for at in 0..400 {
        let (x, y) = if at % 4 == 0 {
            (
                179.0 + next() * 2.0 - if next() < 0.5 { 360.0 } else { 0.0 },
                next() * 20.0 - 10.0,
            )
        } else {
            (next() * 360.0 - 180.0, next() * 170.0 - 85.0)
        };
        let x = (x + 540.0).rem_euclid(360.0) - 180.0;
        let geojson = json!({"type": "Point", "coordinates": [x, y]});
        node.scope()
            .create_record(
                "Place",
                &json!({"n": at, "where": geojson}),
                &[],
                &json!({}),
            )
            .unwrap();
        points.push((at, point::parse_geojson(&geojson).unwrap()));
    }
    let geo = |x: f64, y: f64| json!({"type": "Point", "coordinates": [x, y]});
    let regions = [
        json!({"center": geo(13.4, 52.5), "meters": 2_500_000}),
        json!({"center": geo(179.5, 0.0), "meters": 300_000}),
        json!({"center": geo(-60.0, -30.0), "meters": 0}),
        json!({"box": [geo(-10.0, 30.0), geo(40.0, 60.0)]}),
        json!({"box": [geo(178.0, -5.0), geo(-178.0, 5.0)]}),
    ];
    let found = |predicate: &Value| {
        let mut found: Vec<i64> = query(
            &node,
            &json!({"labels": ["Place"], "predicate": predicate, "limit": 1000}),
        )["records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|record| record["properties"]["n"].as_i64().unwrap())
            .collect();
        found.sort_unstable();
        found
    };
    let check = |pass: &str| {
        for region in &regions {
            let parsed = rushdb_query::spatial::Region::parse(region).unwrap();
            let expected: Vec<i64> = points
                .iter()
                .filter(|(_, at)| parsed.contains(*at))
                .map(|(n, _)| i64::from(*n))
                .collect();
            assert_eq!(
                found(&json!({"where": {"$within": region}})),
                expected,
                "{pass}: {region}"
            );
        }
        // The five nearest to Berlin, nearest first.
        let berlin = point::parse_geojson(&geo(13.4, 52.5)).unwrap();
        let mut nearest: Vec<(f64, i64)> = points
            .iter()
            .map(|(n, at)| (point::distance(berlin, *at).unwrap(), i64::from(*n)))
            .collect();
        nearest.sort_by(|a, b| a.0.total_cmp(&b.0));
        let ordered: Vec<i64> = query(
            &node,
            &json!({"labels": ["Place"], "order": {"where": {"$distance": geo(13.4, 52.5)}},
                "limit": 5}),
        )["records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|record| record["properties"]["n"].as_i64().unwrap())
            .collect();
        assert_eq!(
            ordered,
            nearest.iter().take(5).map(|(_, n)| *n).collect::<Vec<_>>(),
            "{pass}"
        );
    };
    check("walk");
    node.scope()
        .create_index(&rushdb_protocol::IndexDefinition {
            name: "places".into(),
            label: "Place".into(),
            property: "where".into(),
            kind: "ordered".into(),
            consistency: None,
            dimensions: None,
        })
        .unwrap();
    while node.step_index_builds(std::time::Duration::from_secs(5)) > 0 {}
    check("index");

    let refused = run(
        &node,
        &json!({"kind": "query", "query": {"kind": "records", "query":
            {"labels": ["Place"], "predicate": {"where": {"$within": {"meters": 5}}}}}}),
    );
    assert!(refused.is_err(), "{refused:?}");
}

/// `$sum` and `$avg` over datetimes add their epoch milliseconds, as they
/// did when datetimes were stored as them; other typed values are not
/// numbers and are skipped.
#[test]
fn sums_and_averages_read_datetimes_as_epoch_milliseconds() {
    let node = node();
    for (at, lasts) in [
        ("1970-01-01T00:00:01Z", "PT1S"),
        ("1970-01-01T02:00:03+02:00", "PT2S"),
    ] {
        node.scope()
            .create_record("Tick", &json!({"at": at, "lasts": lasts}), &[], &json!({}))
            .unwrap();
    }
    let rows = query(
        &node,
        &json!({"labels": ["Tick"], "select": {
            "total": {"$sum": "$record.at"}, "mean": {"$avg": "$record.at"},
            "lasting": {"$sum": "$record.lasts"},
        }}),
    );
    assert_eq!(rows["rows"][0]["total"], json!(4_000.0), "{rows}");
    assert_eq!(rows["rows"][0]["mean"], json!(2_000.0), "{rows}");
    assert_eq!(rows["rows"][0]["lasting"], Value::Null, "{rows}");
}

/// SQL `INSERT` stores each typed literal as its type: a `TIMESTAMPTZ` is
/// an instant (it was stored as a local date-time), a `TIMESTAMP` a local
/// date-time, a `DATE` a date.
#[test]
fn sql_inserts_store_typed_literals_as_their_types() {
    let node = node();
    node.scope()
        .create_record("Shift", &json!({"name": "first"}), &[], &json!({}))
        .unwrap();
    run(
        &node,
        &json!({"kind": "mutate", "mutation": {"kind": "statement", "language": "sql",
            "statement": "INSERT INTO \"Shift\" (name, day, starts, at) VALUES ('x', DATE '2023-01-02', \
                TIMESTAMP '2023-01-02 08:00:00', TIMESTAMPTZ '2023-01-02T03:04:05+02:00')"}}),
    )
    .unwrap();
    let records = query(
        &node,
        &json!({"labels": ["Shift"], "predicate": {"name": "x"}}),
    );
    let record = &records["records"][0];
    assert_eq!(
        record["propertyTypes"],
        json!({"name": "string", "day": "date", "starts": "localdatetime", "at": "datetime"}),
        "{record}"
    );
    assert_eq!(record["properties"]["at"], json!("2023-01-02T01:04:05Z"));
    assert_eq!(record["properties"]["starts"], json!("2023-01-02T08:00:00"));
}

/// CHK-1: a query naming a label, property or value no record holds runs
/// and answers with warnings saying what it most likely meant; `strict`
/// refuses it; `off` and a query without misses add nothing.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "every surface checked over one fixture"
)]
fn queries_answer_with_warnings_for_names_and_values_no_record_holds() {
    let node = node();
    for (name, city) in [("Ada", "Berlin"), ("Grace", "Paris")] {
        node.scope()
            .create_record(
                "Person",
                &json!({"name": name, "city": city}),
                &[],
                &json!({}),
            )
            .unwrap();
    }
    node.scope()
        .create_record("Venue", &json!({"town": "berlin"}), &[], &json!({}))
        .unwrap();
    let envelope = |query: Value, check: Option<&str>| {
        let mut request = json!({"protocolVersion": 1,
            "operation": {"kind": "query", "query": {"kind": "records", "query": query}}});
        if let Some(check) = check {
            request["context"] = json!({"check": check});
        }
        crate::protocol::ProtocolExecutor::new(&node, None)
            .execute(serde_json::from_value(request).unwrap())
            .map(|envelope| serde_json::to_value(envelope).unwrap())
    };
    // No miss: no `warnings` field.
    let clean = envelope(
        json!({"labels": ["Person"], "predicate": {"city": "Berlin"}}),
        None,
    )
    .unwrap();
    assert!(clean.get("warnings").is_none(), "{clean}");
    // A value spelled otherwise, a property and a label that do not exist.
    let warned = envelope(
        json!({"labels": ["Person"], "predicate": {"city": "berlin", "nmae": "Ada"}}),
        None,
    )
    .unwrap();
    assert_eq!(warned["response"]["total"], json!(0), "{warned}");
    let warnings = warned["warnings"].as_array().unwrap();
    let value = warnings
        .iter()
        .find(|w| w["kind"] == "unknownValue")
        .unwrap();
    assert_eq!(value["value"], json!("berlin"));
    assert!(
        value["suggestions"]
            .as_array()
            .unwrap()
            .contains(&json!({"value": "Berlin", "reason": "spelling"})),
        "{value}"
    );
    let property = warnings
        .iter()
        .find(|w| w["kind"] == "unknownProperty")
        .unwrap();
    assert_eq!(property["property"], json!("nmae"));
    assert_eq!(
        property["suggestions"][0]["property"],
        json!("name"),
        "{property}"
    );
    let label = envelope(json!({"labels": ["Persn"]}), None).unwrap();
    assert_eq!(
        label["warnings"][0]["kind"],
        json!("unknownLabel"),
        "{label}"
    );
    assert_eq!(
        label["warnings"][0]["suggestions"][0]["label"],
        json!("Person")
    );
    // Strict refuses with the same warnings; off says nothing.
    let refused = envelope(json!({"labels": ["Persn"]}), Some("strict")).unwrap_err();
    assert_eq!(refused.code, rushdb_protocol::ErrorCode::InvalidRequest);
    assert_eq!(
        refused.details.unwrap()["warnings"][0]["label"],
        json!("Persn")
    );
    let off = envelope(json!({"labels": ["Persn"]}), Some("off")).unwrap();
    assert!(off.get("warnings").is_none(), "{off}");
    // Mongo: the collection and the filter.
    let request = json!({"protocolVersion": 1, "operation": {"kind": "query", "query": {
        "kind": "statement", "language": "mongo",
        "statement": r#"{"collection": "Person", "find": {"city": "berlin"}}"#}}});
    let mongo = crate::protocol::ProtocolExecutor::new(&node, None)
        .execute(serde_json::from_value(request).unwrap())
        .unwrap();
    assert_eq!(
        mongo.warnings[0].kind,
        rushdb_protocol::QueryWarningKind::UnknownValue
    );
    // SQL: texts compared by equality, placed by table and alias.
    for sql in [
        "SELECT name FROM \"Person\" WHERE city = 'berlin'",
        "SELECT p.name FROM \"Person\" p WHERE p.city IN ('Paris', 'berlin')",
    ] {
        let request = json!({"protocolVersion": 1, "operation": {"kind": "query", "query": {
            "kind": "statement", "language": "sql", "statement": sql}}});
        let answered = crate::protocol::ProtocolExecutor::new(&node, None)
            .execute(serde_json::from_value(request).unwrap())
            .unwrap();
        assert_eq!(answered.warnings.len(), 1, "{sql}: {:?}", answered.warnings);
        assert_eq!(
            answered.warnings[0].value.as_deref(),
            Some("berlin"),
            "{sql}"
        );
    }
    // The text stored in another label's property is suggested there.
    let venue = envelope(json!({"predicate": {"city": "berlin"}}), None).unwrap();
    assert!(
        venue["warnings"][0]["suggestions"]
            .as_array()
            .unwrap()
            .contains(
                &json!({"label": "Venue", "property": "town", "value": "berlin",
                "reason": "storedIn"})
            ),
        "{venue}"
    );
}
