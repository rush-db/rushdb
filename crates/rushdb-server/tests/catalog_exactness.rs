//! Catalog facts an agent reads as exact are exact (LMPG catalog audit,
//! 2026-10-24): describe ranges cover every record, not the first 10k;
//! type splits are reported on the label that has them; a name stored as
//! text on one label does not hide another label's numeric range; counts
//! and top-k over long texts fit the working set; and a name spelled
//! exactly ranks first among equal suggestions.

use std::sync::Arc;

use rushdb_protocol::{EnvelopeHandler, RequestEnvelope};
use rushdb_server::{Node, NodeEnvelopeHandler, ServerConfig};
use serde_json::{json, Value};

fn node(query_memory_max: usize) -> NodeEnvelopeHandler {
    let resources = rushdb_server::resource_config::ResourceConfig {
        query_memory_max: rushdb_server::resource_config::ByteSize(query_memory_max),
        ..Default::default()
    };
    let mut cfg = ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: "t".into(),
        data_dir: None,
        multitenant: false,
        capability_secret: None,
        raft: None,
        resources,
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages: rushdb_protocol::LanguageSet::default(),
        max_request_bytes: 64 * 1024 * 1024,
        listen_addr: "127.0.0.1".into(),
        tls_cert: None,
        tls_key: None,
        maintenance: false,
        allow_insecure_listen: false,
        require_index_warmup: false,
        request_timeout_ms: 0,
        max_concurrent_requests_per_tenant: 0,
        explicit_flags: std::collections::BTreeSet::new(),
    };
    NodeEnvelopeHandler::new(Arc::new(Node::bootstrap(&mut cfg).expect("bootstrap")))
}

fn call(handler: &NodeEnvelopeHandler, operation: &Value) -> Value {
    let envelope: RequestEnvelope =
        serde_json::from_value(json!({ "protocolVersion": 1, "operation": operation }))
            .expect("envelope");
    let response = handler.execute(envelope, None).expect("request succeeds");
    serde_json::to_value(response.response).expect("response json")
}

fn ingest(handler: &NodeEnvelopeHandler, label: &str, documents: &[Value]) {
    for chunk in documents.chunks(2000) {
        call(
            handler,
            &json!({ "kind": "ingest", "ingest": {
                "kind": "documents", "label": label, "documents": chunk,
                "options": { "suggestTypes": true } } }),
        );
    }
}

fn properties(handler: &NodeEnvelopeHandler, label: &str) -> Vec<Value> {
    let out = call(
        handler,
        &json!({ "kind": "describe", "describe": {
            "kind": "properties", "query": { "labels": [label] } } }),
    );
    out["properties"].as_array().cloned().unwrap_or_default()
}

fn property<'a>(properties: &'a [Value], name: &str) -> &'a Value {
    properties
        .iter()
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("no property {name} in {properties:?}"))
}

#[test]
fn describe_ranges_cover_every_record_not_a_sample() {
    let handler = node(256 * 1024 * 1024);
    // The extremes arrive after the first 10,000 records.
    let mut documents: Vec<Value> = (0..12_000)
        .map(|i| json!({ "zip": 1000 + (i % 500), "at": "2020-01-02" }))
        .collect();
    documents.push(json!({ "zip": 99_950, "at": "2031-05-06T07:08:09Z" }));
    documents.push(json!({ "zip": 501, "at": "1999-12-31" }));
    ingest(&handler, "zip_code", &documents);
    let properties = properties(&handler, "zip_code");
    let zip = property(&properties, "zip");
    assert_eq!(
        (zip["min"].clone(), zip["max"].clone()),
        (json!(501), json!(99_950))
    );
    let at = property(&properties, "at");
    assert_eq!(at["min"], json!("1999-12-31"));
    assert_eq!(at["max"], json!("2031-05-06T07:08:09.000Z"));
}

#[test]
fn type_splits_and_ranges_are_per_label() {
    let handler = node(256 * 1024 * 1024);
    // `Date` is text on one label and a datetime on another; `mcmId` a
    // number on one label and text on another.
    ingest(
        &handler,
        "yearmonth",
        &[
            json!({ "Date": "201207", "mcmId": "16165" }),
            json!({ "Date": "201302", "mcmId": "7" }),
        ],
    );
    ingest(
        &handler,
        "transactions",
        &[
            json!({ "Date": "2012-08-24", "mcmId": 4 }),
            json!({ "Date": "2012-08-26", "mcmId": 3660 }),
        ],
    );
    let yearmonth = properties(&handler, "yearmonth");
    let transactions = properties(&handler, "transactions");
    for (properties, name) in [
        (&yearmonth, "Date"),
        (&yearmonth, "mcmId"),
        (&transactions, "Date"),
        (&transactions, "mcmId"),
    ] {
        let p = property(properties, name);
        assert!(
            p.get("siblingTypes")
                .is_none_or(|s| s.as_array().is_some_and(Vec::is_empty)),
            "{name}: no type split on this label: {p}"
        );
    }
    let summary = call(
        &handler,
        &json!({ "kind": "describe", "describe": { "kind": "schemaSummary", "fewValues": 10 } }),
    );
    let label = summary["labels"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["name"] == "transactions")
        .unwrap();
    let mcm = label["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "mcmId")
        .unwrap();
    assert_eq!(
        (mcm["min"].clone(), mcm["max"].clone()),
        (json!(4), json!(3660)),
        "{mcm}"
    );
}

#[test]
fn counts_and_top_values_of_long_texts_fit_the_working_set() {
    // 2,000 distinct texts of ~400 bytes need ~1.6 MB held as values; the
    // working set allows 512 KiB.
    let handler = node(512 * 1024);
    let documents: Vec<Value> = (0..2_000)
        .map(|i| json!({ "Text": format!("{i:05} {}", "lorem ipsum ".repeat(33)) }))
        .chain((0..5).map(|_| json!({ "Text": format!("common {}", "x".repeat(400)) })))
        .collect();
    ingest(&handler, "postHistory", &documents);
    let values = |mode: &str| {
        call(
            &handler,
            &json!({ "kind": "query", "query": {
                "kind": "propertyValues", "property": "Text",
                "query": { "labels": ["postHistory"] }, "mode": mode, "limit": 3 } }),
        )
    };
    assert_eq!(values("count")["distinctCount"], json!(2_001));
    let top = values("topK");
    assert_eq!(top["distinctCount"], json!(2_001));
    assert_eq!(top["counts"], json!([5, 1, 1]));
    assert_eq!(
        top["values"][0],
        json!(format!("common {}", "x".repeat(400)))
    );
    assert!(top["values"][1].as_str().unwrap().starts_with("00000 "));
}

#[test]
fn a_name_spelled_exactly_ranks_first_among_equal_scores() {
    let handler = node(256 * 1024 * 1024);
    ingest(
        &handler,
        "cards",
        &[json!({ "type": "Creature", "types": "Creature" })],
    );
    ingest(&handler, "tags", &[json!({ "TagName": "bayes" })]);
    ingest(&handler, "posts", &[json!({ "Tags": "<bayes>" })]);
    let names = |text: &str| {
        call(
            &handler,
            &json!({ "kind": "describe", "describe": { "kind": "names", "text": text, "limit": 3 } }),
        )["names"]
            .clone()
    };
    let first = names("cards.types");
    assert_eq!(
        (first[0]["label"].clone(), first[0]["property"].clone()),
        (json!("cards"), json!("types"))
    );
    let first = names("tags");
    assert_eq!(first[0]["label"], json!("tags"));
    assert!(
        first[0].get("property").is_none_or(Value::is_null),
        "{first}"
    );
}

fn matching(handler: &NodeEnvelopeHandler, label: &str, predicate: &Value) -> usize {
    let out = call(
        handler,
        &json!({ "kind": "query", "query": { "kind": "records", "query": {
            "labels": [label], "predicate": predicate, "limit": 1000 } } }),
    );
    out["records"].as_array().map_or(0, Vec::len)
}

fn values(handler: &NodeEnvelopeHandler, label: &str, property: &str, mode: &str) -> Value {
    call(
        handler,
        &json!({ "kind": "query", "query": { "kind": "propertyValues", "property": property,
            "query": { "labels": [label] }, "mode": mode, "limit": 100 } }),
    )
}

#[test]
fn integers_after_a_float_are_filtered_and_described_as_numbers() {
    let handler = node(256 * 1024 * 1024);
    // A float first, then integers (`hyperdrive_rating` in the LMPG JSON
    // audit: `$gte 1` matched 5 of 34 records and the range read 5e-324).
    ingest(
        &handler,
        "floatFirst",
        &[
            json!({ "x": 0.5 }),
            json!({ "x": 1 }),
            json!({ "x": 2 }),
            json!({ "x": 1.5 }),
            json!({ "x": 4 }),
        ],
    );
    // Integers first, then a float.
    ingest(
        &handler,
        "intFirst",
        &[json!({ "x": 1 }), json!({ "x": 1.1 }), json!({ "x": 3 })],
    );
    // Mixed number arrays.
    ingest(
        &handler,
        "arrays",
        &[json!({ "x": [1, 2.5] }), json!({ "x": [0.5, 3] })],
    );
    for (label, at_least_one, min, max) in [
        ("floatFirst", 4, json!(0.5), json!(4)),
        ("intFirst", 3, json!(1), json!(3)),
    ] {
        assert_eq!(
            matching(&handler, label, &json!({ "x": { "$gte": 1 } })),
            at_least_one,
            "{label}"
        );
        assert_eq!(
            matching(&handler, label, &json!({ "x": { "$lt": 1.25 } })),
            2, // 0.5 and 1; 1 and 1.1
            "{label}"
        );
        let range = values(&handler, label, "x", "range");
        assert_eq!(
            (range["min"].clone(), range["max"].clone()),
            (min, max),
            "{label}: {range}"
        );
        let p = property(&properties(&handler, label), "x").clone();
        assert_eq!(p["type"], json!("number"), "{label}: {p}");
        let top = values(&handler, label, "x", "topK");
        assert!(
            top["values"]
                .as_array()
                .unwrap()
                .iter()
                .all(|v| v.as_f64().is_some_and(|f| f >= 0.5)),
            "{label}: {top}"
        );
    }
    assert_eq!(
        matching(&handler, "arrays", &json!({ "x": { "$gte": 3 } })),
        1
    );
    assert_eq!(
        matching(&handler, "arrays", &json!({ "x": { "$lt": 1 } })),
        1
    );
}

#[test]
fn nulls_are_counted_once_as_the_null_entry_and_never_as_values() {
    let handler = node(256 * 1024 * 1024);
    // Nulls before and after the first value (LMPG JSON audit J1, J5:
    // `founder_character_id` read 31 of 34; `destroyed_at` string 22 /
    // datetime 17 for 2 strings, 12 datetimes and 25 nulls).
    ingest(
        &handler,
        "faction",
        &[
            json!({ "founder": null, "at": null }),
            json!({ "founder": null, "at": "2004-01-01T18:00:00Z" }),
            json!({ "founder": "palpatine", "at": null }),
            json!({ "founder": null, "at": "a long time ago" }),
            json!({ "founder": "dooku", "at": null }),
            json!({ "founder": "palpatine" }),
        ],
    );
    let described = properties(&handler, "faction");
    let entries = |name: &str| -> Vec<(String, u64)> {
        let mut out: Vec<(String, u64)> = described
            .iter()
            .filter(|p| p["name"] == name)
            .map(|p| {
                (
                    p["type"].as_str().unwrap().to_string(),
                    p["recordsCount"].as_u64().unwrap(),
                )
            })
            .collect();
        out.sort();
        out
    };
    assert_eq!(
        entries("founder"),
        [("null".to_string(), 3), ("string".to_string(), 3)]
    );
    assert_eq!(
        entries("at"),
        [
            ("datetime".to_string(), 1),
            ("null".to_string(), 3),
            ("string".to_string(), 1)
        ]
    );
    // J2: distinct counts and value lists agree; null is not a value.
    for mode in ["topK", "count"] {
        let r = values(&handler, "faction", "founder", mode);
        assert_eq!(r["distinctCount"], json!(2), "{mode}: {r}");
    }
    let top = values(&handler, "faction", "founder", "topK");
    assert_eq!(top["values"], json!(["palpatine", "dooku"]));
    assert_eq!(top["counts"], json!([2, 1]));
    let summary = call(
        &handler,
        &json!({ "kind": "describe", "describe": { "kind": "schemaSummary", "fewValues": 10 } }),
    );
    let founder: Vec<&Value> = summary["labels"][0]["properties"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["name"] == "founder" && p["type"] == "string")
        .collect();
    assert_eq!(founder[0]["distinctCount"], json!(2), "{founder:?}");
    assert_eq!(founder[0]["values"], json!(["palpatine", "dooku"]));
    // Querying nulls still finds every null.
    assert_eq!(
        matching(&handler, "faction", &json!({ "founder": null })),
        3
    );
}

fn summary_entry(summary: &Value, label: &str, name: &str, value_type: &str) -> Value {
    summary["labels"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["name"] == label)
        .and_then(|l| {
            l["properties"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["name"] == name && p["type"] == value_type)
                .cloned()
        })
        .unwrap_or_else(|| panic!("no {label}.{name} {value_type} in {summary}"))
}

#[test]
fn a_type_split_in_one_label_reports_each_type_on_its_own() {
    let handler = node(256 * 1024 * 1024);
    // J3: `battle.ended_at` text on 10 records, datetime on 12; the
    // datetime entry read min "-0001…" (a text) and the whole property's
    // distinct count.
    ingest(
        &handler,
        "battle",
        &[
            json!({ "ended": "a long time ago" }),
            json!({ "ended": "before the republic" }),
            json!({ "ended": "2004-01-01T18:00:00Z" }),
            json!({ "ended": "2019-12-20T10:00:00Z" }),
            json!({ "ended": "2019-12-20T10:00:00Z" }),
        ],
    );
    let summary = call(
        &handler,
        &json!({ "kind": "describe", "describe": { "kind": "schemaSummary", "fewValues": 10 } }),
    );
    let dates = summary_entry(&summary, "battle", "ended", "datetime");
    assert_eq!(dates["distinctCount"], json!(2), "{dates}");
    assert_eq!(dates["min"], json!("2004-01-01T18:00:00.000Z"));
    assert_eq!(dates["max"], json!("2019-12-20T10:00:00.000Z"));
    let texts = summary_entry(&summary, "battle", "ended", "string");
    assert_eq!(texts["distinctCount"], json!(2), "{texts}");
    assert_eq!(
        texts["values"],
        json!(["a long time ago", "before the republic"])
    );
    // A range covers one ordered class: the datetimes, by instant.
    let range = values(&handler, "battle", "ended", "range");
    assert_eq!(
        (range["min"].clone(), range["max"].clone()),
        (
            json!("2004-01-01T18:00:00.000Z"),
            json!("2019-12-20T10:00:00.000Z")
        ),
        "{range}"
    );
}

#[test]
fn signed_years_are_one_datetime_property_ordered_by_instant() {
    let handler = node(256 * 1024 * 1024);
    // J4: BBY dates (`-0032-…`) stayed text while `0000-…` parsed.
    ingest(
        &handler,
        "battle",
        &[
            json!({ "started": "-0032-01-01T08:00:00Z" }),
            json!({ "started": "-0001-01-01T08:00:00Z" }),
            json!({ "started": "0000-01-01T05:00:00Z" }),
            json!({ "started": "0035-01-01T16:00:00Z" }),
        ],
    );
    let started: Vec<Value> = properties(&handler, "battle")
        .into_iter()
        .filter(|p| p["name"] == "started")
        .collect();
    assert_eq!(started.len(), 1, "{started:?}");
    assert_eq!(started[0]["type"], json!("datetime"));
    assert_eq!(started[0]["recordsCount"], json!(4));
    assert_eq!(started[0]["min"], json!("-0032-01-01T08:00:00.000Z"));
    assert_eq!(started[0]["max"], json!("0035-01-01T16:00:00.000Z"));
    let summary = call(
        &handler,
        &json!({ "kind": "describe", "describe": { "kind": "schemaSummary", "fewValues": 10 } }),
    );
    let entry = summary_entry(&summary, "battle", "started", "datetime");
    assert_eq!(entry["min"], json!("-0032-01-01T08:00:00.000Z"), "{entry}");
    assert_eq!(
        matching(
            &handler,
            "battle",
            &json!({ "started": { "$lt": "0000-01-01T00:00:00Z" } })
        ),
        2
    );
}

#[test]
fn describe_rejects_what_it_does_not_take() {
    // J7: a `schemaSummary` sent a `query` summarized the whole tenant.
    let parsed: Result<RequestEnvelope, _> = serde_json::from_value(json!({
        "protocolVersion": 1,
        "operation": { "kind": "describe", "describe": {
            "kind": "schemaSummary", "query": { "predicate": { "era": "clone_wars" } } } }
    }));
    assert!(parsed.is_err());
    // J8: catalog detail with a predicate is a request error on
    // `describe.schema` too, not an internal one.
    let handler = node(256 * 1024 * 1024);
    ingest(&handler, "battle", &[json!({ "era": "clone_wars" })]);
    let envelope: RequestEnvelope = serde_json::from_value(json!({
        "protocolVersion": 1,
        "operation": { "kind": "describe", "describe": { "kind": "schema", "detail": "catalog",
            "query": { "predicate": { "era": "clone_wars" } } } }
    }))
    .unwrap();
    let error = handler.execute(envelope, None).unwrap_err();
    assert_eq!(
        error.code,
        rushdb_protocol::ErrorCode::InvalidRequest,
        "{error:?}"
    );
}

#[test]
fn values_list_by_record_count_then_value() {
    // Written in the order a, b, c, b, c, b: scan order would list a first.
    let handler = node(256 * 1024 * 1024);
    let docs: Vec<Value> = ["a", "b", "c", "b", "c", "b", "d"]
        .iter()
        .map(|v| json!({ "code": v }))
        .collect();
    ingest(&handler, "t", &docs);
    let all = values(&handler, "t", "code", "values");
    assert_eq!(all["values"], json!(["b", "c", "a", "d"]), "{all}");
    assert!(all.get("truncated").is_none(), "{all}"); // omitted when false
    let head = call(
        &handler,
        &json!({ "kind": "query", "query": { "kind": "propertyValues", "property": "code",
            "query": { "labels": ["t"] }, "mode": "values", "limit": 2 } }),
    );
    assert_eq!(head["values"], json!(["b", "c"]), "{head}");
    assert_eq!(head["truncated"], json!(true));
    assert_eq!(head["distinctCount"], json!(4));
}

#[test]
fn unfiltered_values_and_ranges_read_the_dictionary_as_the_scan_would() {
    // Unfiltered `values` and non-numeric `range` are answered from the value
    // dictionary; a predicate every record matches forces the match-set scan.
    // Both must agree on numbers, datetimes (each in its own rendering),
    // text, a type split, two labels and a value over the dictionary's size.
    const CODES: [&str; 4] = ["a", "b", "c", "b"];
    let handler = node(256 * 1024 * 1024);
    let long = "x".repeat(200);
    let docs: Vec<Value> = (0..40)
        .map(|i| {
            json!({
                "n": i % 7,
                "day": format!("2019-0{}-1{}", 1 + i % 9, i % 10),
                "at": format!("2010-07-19 19:15:5{}.0", i % 4),
                "code": CODES[i % 4],
                "mixed": if i % 3 == 0 { json!(i) } else { json!(format!("v{}", i % 5)) },
                "note": if i % 5 == 0 { json!(long.clone()) } else { json!(format!("n{}", i % 3)) },
            })
        })
        .collect();
    ingest(&handler, "t", &docs);
    ingest(
        &handler,
        "u",
        &[json!({ "day": "2021-01-01", "code": "z", "n": 99 })],
    );
    // Explicit nulls (one property seen only as null) are not dictionary
    // entries: reads that would include them keep scanning.
    ingest(
        &handler,
        "v",
        &[json!({ "code": null, "gone": null, "n": 5 })],
    );
    let read = |labels: &[&str], property: &str, mode: &str, filtered: bool, limit: Value| {
        let mut q = json!({ "labels": labels });
        if filtered {
            q["predicate"] = json!({ property: { "$exists": true } });
        }
        let mut r = call(
            &handler,
            &json!({ "kind": "query", "query": { "kind": "propertyValues", "property": property,
                "query": q, "mode": mode, "limit": limit } }),
        );
        r.as_object_mut().map(|o| o.remove("kind"));
        r
    };
    for property in ["n", "day", "at", "code", "mixed", "note", "gone"] {
        for labels in [&["t"][..], &["t", "u"][..], &["v"][..], &[][..]] {
            for (mode, limit) in [
                ("values", json!(null)),
                ("values", json!(3)),
                ("range", json!(null)),
            ] {
                let fast = read(labels, property, mode, false, limit.clone());
                let scan = read(labels, property, mode, true, limit);
                assert_eq!(fast, scan, "{property} {labels:?} {mode}");
            }
        }
    }
    let day = read(&["t"], "day", "range", false, json!(null));
    assert_eq!(
        (day["min"].clone(), day["max"].clone()),
        (json!("2019-01-10"), json!("2019-09-18")),
        "{day}"
    );
}

#[test]
fn unique_columns_are_described_and_listed_within_the_working_set() {
    // 20,000 unique ids would hold ~1.6 MB as a distinct set; the working
    // set allows 512 KiB. Under one label, counts come from counters, the
    // range from the dictionary's ends, and top-k/limited lists hold only
    // their head.
    let handler = node(512 * 1024);
    let documents: Vec<Value> = (0..20_000)
        .map(|i| json!({ "id": 7 + 3 * i, "score": f64::from(i % 50) / 4.0 - 3.0, "kind": if i % 2 == 0 { "a" } else { "b" } }))
        .collect();
    ingest(&handler, "trans", &documents);
    let id = property(&properties(&handler, "trans"), "id").clone();
    assert_eq!(
        (id["min"].clone(), id["max"].clone()),
        (json!(7), json!(7 + 3 * 19_999)),
        "{id}"
    );
    let values = |property: &str, mode: &str| {
        call(
            &handler,
            &json!({ "kind": "query", "query": { "kind": "propertyValues", "property": property,
                "query": { "labels": ["trans"] }, "mode": mode, "limit": 3 } }),
        )
    };
    assert_eq!(values("id", "count")["distinctCount"], json!(20_000));
    let range = values("id", "range");
    assert_eq!(
        (range["min"].clone(), range["max"].clone()),
        (json!(7), json!(60_004))
    );
    let top = values("id", "topK");
    // All counts tie at 1: ties by the value's JSON text.
    assert_eq!(top["values"], json!([10, 100, 1000]), "{top}");
    let list = values("id", "values");
    assert_eq!(
        (list["values"].clone(), list["truncated"].clone()),
        (json!([10, 100, 1000]), json!(true))
    );
    assert_eq!(list["distinctCount"], json!(20_000));
    let score = values("score", "range");
    assert_eq!(
        (score["min"].clone(), score["max"].clone()),
        (json!(-3.0), json!(9.25)),
        "{score}"
    );
}

#[test]
fn one_label_reads_agree_with_the_dictionary_walk() {
    // A second label without the property sends the same read down the
    // multi-label dictionary walk: both must answer alike.
    let handler = node(256 * 1024 * 1024);
    let docs: Vec<Value> = (0..300)
        .map(|i| {
            json!({ "n": (i * 37) % 101 - 50, "f": f64::from(i % 13) * 1.5, "code": format!("c{}", i % 17),
                    "day": format!("20{:02}-0{}-1{}", i % 30, 1 + i % 9, i % 10) })
        })
        .collect();
    ingest(&handler, "t", &docs);
    ingest(&handler, "w", &[json!({ "other": 1 })]);
    for property in ["n", "f", "code", "day"] {
        for (mode, limit) in [
            ("count", json!(null)),
            ("range", json!(null)),
            ("topK", json!(5)),
            ("values", json!(7)),
        ] {
            let read = |labels: Value| {
                let mut r = call(
                    &handler,
                    &json!({ "kind": "query", "query": { "kind": "propertyValues", "property": property,
                        "query": { "labels": labels }, "mode": mode, "limit": limit } }),
                );
                r.as_object_mut().map(|o| o.remove("kind"));
                r
            };
            assert_eq!(
                read(json!(["t"])),
                read(json!(["t", "w"])),
                "{property} {mode}"
            );
        }
    }
}
