use rushdb_protocol::CsvParseConfig;
use serde_json::{json, Value};

use super::{decode, render};

/// Papa Parse 5.4 and the platform's clean-up on the same inputs; see
/// `testdata/papaparse-csv-oracle.cjs`.
const ORACLE: &str = include_str!("testdata/papaparse-csv-oracle.json");

fn oracle() -> serde_json::Map<String, Value> {
    serde_json::from_str::<Value>(ORACLE)
        .expect("oracle is JSON")
        .as_object()
        .expect("oracle is an object")
        .clone()
}

#[test]
fn decoding_reproduces_the_platform_import_case_by_case() {
    let mut checked = 0;
    for (name, case) in oracle() {
        if name == "__export" {
            continue;
        }
        let input = &case["input"];
        let data = input["data"].as_str().expect("data");
        let config: CsvParseConfig =
            serde_json::from_value(input["parseConfig"].clone()).expect("parse config");
        let suggest_types = input["options"]["suggestTypes"].as_bool().unwrap_or(false);
        let decoded = decode(data, &config, suggest_types);
        match (case.get("error"), decoded) {
            (Some(expected), Err(actual)) => assert_eq!(
                expected.as_str().expect("error text"),
                format!("CSV parse error: {actual}"),
                "case {name}"
            ),
            (None, Ok(rows)) => assert_eq!(case["rows"], Value::Array(rows), "case {name}"),
            (expected, actual) => panic!("case {name}: expected {expected:?}, got {actual:?}"),
        }
        checked += 1;
    }
    assert!(checked >= 59, "only {checked} oracle cases ran");
}

#[test]
fn export_reproduces_the_platform_export() {
    let oracle = oracle();
    let export = &oracle["__export"];
    let records: Vec<Value> = export["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| {
            let mut properties = row.as_object().expect("row").clone();
            let id = properties.remove("__id").expect("__id");
            let label = properties.remove("__label").expect("__label");
            json!({ "id": id, "label": label, "properties": properties })
        })
        .collect();
    assert_eq!(render(&records), export["csv"].as_str().expect("csv"));
    assert_eq!(render(&[]), export["empty"].as_str().expect("empty"));
}

#[test]
fn configuration_outside_the_contract_is_rejected() {
    let reject = |config: Value| {
        let config: CsvParseConfig = serde_json::from_value(config).expect("config");
        decode("a,b\n1,2\n", &config, false).expect_err("rejected")
    };
    assert!(reject(json!({ "quoteChar": "''" })).contains("quoteChar"));
    assert!(reject(json!({ "escapeChar": "\\\\" })).contains("escapeChar"));
    assert!(reject(json!({ "delimiter": "" })).contains("delimiter"));
    assert!(reject(json!({ "delimiter": "\n" })).contains("delimiter"));
    assert!(reject(json!({ "delimiter": "\"" })).contains("delimiter"));
    assert!(serde_json::from_value::<CsvParseConfig>(json!({ "comments": "#" })).is_err());
    assert!(serde_json::from_value::<CsvParseConfig>(json!({ "skipEmptyLines": "yes" })).is_err());
}

#[test]
fn dynamic_typing_follows_the_suggest_types_option_unless_set() {
    let config = CsvParseConfig::default();
    assert_eq!(
        decode("a\n1\n", &config, true).unwrap(),
        vec![json!({ "a": 1 })]
    );
    assert_eq!(
        decode("a\n1\n", &config, false).unwrap(),
        vec![json!({ "a": "1" })]
    );
    let explicit: CsvParseConfig =
        serde_json::from_value(json!({ "dynamicTyping": false })).expect("config");
    assert_eq!(
        decode("a\n1\n", &explicit, true).unwrap(),
        vec![json!({ "a": "1" })]
    );
}
