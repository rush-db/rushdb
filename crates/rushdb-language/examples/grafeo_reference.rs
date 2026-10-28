//! Plain Grafeo (its own in-memory store, no LMPG, no `RushDB` adapter) as a
//! reference for attributing openCypher TCK failures: a scenario that fails
//! on `RushDB` but passes here fails because of `RushDB`, not Grafeo.
//!
//! Speaks JSON lines on stdin/stdout, one response per command:
//! `{"op": "reset"}` → `{}`; `{"op": "run", "statement": "...",
//! "parameters": {...}}` → `{"rows": [...]}` or `{"error": "..."}`;
//! `{"op": "snapshot"}` → `{"nodes": [...], "relationships": [...]}`.
//! Values use the protocol's JSON shapes (`_id`, `__labels`, `type`,
//! `source`, `target`, paths as `{"nodes", "edges"}`, temporal ISO text).
//! Driven by `scripts/run-opencypher-tck.py --reference`.

use std::collections::HashMap;
use std::io::{BufRead, Write};

use grafeo_common::types::{EdgeId, LogicalType, NodeId, PropertyKey, Value};
use grafeo_engine::GrafeoDB;
use serde_json::{json, Map, Value as Json};

fn value_json(db: &GrafeoDB, kind: Option<&LogicalType>, value: &Value) -> Json {
    match (kind, value) {
        (Some(LogicalType::Node), Value::Int64(id)) => {
            node_json(db, NodeId::new(u64::try_from(*id).unwrap_or_default()))
        }
        (Some(LogicalType::Edge), Value::Int64(id)) => {
            edge_json(db, EdgeId::new(u64::try_from(*id).unwrap_or_default()))
        }
        (Some(LogicalType::List(inner)), Value::List(items)) => Json::Array(
            items
                .iter()
                .map(|item| value_json(db, Some(inner), item))
                .collect(),
        ),
        (_, Value::Map(map)) => {
            let id = map.get(&PropertyKey::new("_id"));
            if let (Some(Value::Int64(id)), true) =
                (id, map.contains_key(&PropertyKey::new("_labels")))
            {
                return node_json(db, NodeId::new(u64::try_from(*id).unwrap_or_default()));
            }
            if let (Some(Value::Int64(id)), true) =
                (id, map.contains_key(&PropertyKey::new("_type")))
            {
                return edge_json(db, EdgeId::new(u64::try_from(*id).unwrap_or_default()));
            }
            Json::Object(
                map.iter()
                    .map(|(key, value)| (key.to_string(), value_json(db, None, value)))
                    .collect(),
            )
        }
        (_, Value::Path { nodes, edges }) => json!({
            "nodes": nodes.iter().map(|n| value_json(db, Some(&LogicalType::Node), n)).collect::<Vec<_>>(),
            "edges": edges.iter().map(|e| value_json(db, Some(&LogicalType::Edge), e)).collect::<Vec<_>>(),
        }),
        (_, Value::Null) => Json::Null,
        (_, Value::Bool(b)) => Json::Bool(*b),
        (_, Value::Int64(i)) => json!(i),
        (_, Value::Float64(f)) => serde_json::Number::from_f64(*f).map_or(Json::Null, Json::Number),
        (_, Value::String(s)) => Json::String(s.to_string()),
        (_, Value::List(items)) => Json::Array(
            items
                .iter()
                .map(|item| value_json(db, None, item))
                .collect(),
        ),
        (_, Value::Date(d)) => Json::String(d.to_string()),
        (_, Value::Time(t)) => Json::String(t.to_string()),
        (_, Value::Duration(d)) => Json::String(d.to_string()),
        (_, Value::ZonedDatetime(z)) => Json::String(z.to_string()),
        (_, other) => serde_json::to_value(other).unwrap_or(Json::Null),
    }
}

fn properties(map: &mut Map<String, Json>, db: &GrafeoDB, props: Vec<(String, Value)>) {
    for (key, value) in props {
        map.insert(key, value_json(db, None, &value));
    }
}

fn node_json(db: &GrafeoDB, id: NodeId) -> Json {
    let Some(node) = db.get_node(id) else {
        return Json::Null;
    };
    let mut map = Map::new();
    map.insert("_id".into(), json!(format!("n{}", id.as_u64())));
    map.insert(
        "__labels".into(),
        Json::Array(
            node.labels
                .iter()
                .map(|l| Json::String(l.to_string()))
                .collect(),
        ),
    );
    properties(
        &mut map,
        db,
        node.properties_as_btree()
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    );
    Json::Object(map)
}

fn edge_json(db: &GrafeoDB, id: EdgeId) -> Json {
    let Some(edge) = db.get_edge(id) else {
        return Json::Null;
    };
    let mut map = Map::new();
    map.insert("_id".into(), json!(format!("e{}", id.as_u64())));
    map.insert("type".into(), Json::String(edge.edge_type.to_string()));
    map.insert("source".into(), json!(format!("n{}", edge.src.as_u64())));
    map.insert("target".into(), json!(format!("n{}", edge.dst.as_u64())));
    properties(
        &mut map,
        db,
        edge.properties_as_btree()
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    );
    Json::Object(map)
}

fn parameters(value: &Json) -> Option<HashMap<String, Value>> {
    fn convert(value: &Json) -> Value {
        match value {
            Json::Null => Value::Null,
            Json::Bool(b) => Value::Bool(*b),
            Json::Number(n) => n
                .as_i64()
                .map(Value::Int64)
                .or_else(|| n.as_f64().map(Value::Float64))
                .unwrap_or(Value::Null),
            Json::String(s) => Value::from(s.as_str()),
            Json::Array(items) => Value::List(items.iter().map(convert).collect::<Vec<_>>().into()),
            Json::Object(map) => Value::Map(std::sync::Arc::new(
                map.iter()
                    .map(|(k, v)| (k.as_str().into(), convert(v)))
                    .collect(),
            )),
        }
    }
    value
        .as_object()
        .map(|map| map.iter().map(|(k, v)| (k.clone(), convert(v))).collect())
}

fn run(db: &GrafeoDB, statement: &str, params: &Json) -> Json {
    let session = db.session();
    match session.execute_language(statement, "cypher", parameters(params)) {
        Ok(result) => {
            let columns = result.columns.clone();
            let types = result.column_types.clone();
            // The same row convention the RushDB adapter applies, so the two
            // differ only below it: an updating statement without RETURN, or
            // one projecting only anonymous ids, returns no rows.
            let anonymous = !columns.is_empty() && columns.iter().all(|c| c.starts_with("_anon_"));
            let has_return = statement
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .any(|word| word.eq_ignore_ascii_case("RETURN"));
            if anonymous || !has_return {
                return json!({ "rows": [] });
            }
            let rows: Vec<Json> = result
                .into_rows()
                .into_iter()
                .map(|row| {
                    Json::Object(
                        row.iter()
                            .enumerate()
                            .map(|(i, value)| {
                                (
                                    columns.get(i).cloned().unwrap_or_default(),
                                    value_json(db, types.get(i), value),
                                )
                            })
                            .collect(),
                    )
                })
                .collect();
            json!({ "rows": rows })
        }
        Err(error) => json!({ "error": error.to_string() }),
    }
}

fn main() {
    let mut db = GrafeoDB::new_in_memory();
    let stdout = std::io::stdout();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(command) = serde_json::from_str::<Json>(&line) else {
            continue;
        };
        let response = match command["op"].as_str() {
            Some("reset") => {
                db = GrafeoDB::new_in_memory();
                json!({})
            }
            Some("run") => {
                // A statement that never ends must not stall the attribution:
                // the caller treats a missing answer as a failure.
                let statement = command["statement"].as_str().unwrap_or_default();
                std::thread::scope(|scope| {
                    let worker = std::thread::Builder::new()
                        .stack_size(256 * 1024 * 1024)
                        .spawn_scoped(scope, || run(&db, statement, &command["parameters"]))
                        .expect("thread");
                    worker
                        .join()
                        .unwrap_or_else(|_| json!({"error": "panicked"}))
                })
            }
            Some("snapshot") => {
                let nodes = run(&db, "MATCH (n) RETURN n", &Json::Null);
                let rels = run(&db, "MATCH ()-[r]->() RETURN r", &Json::Null);
                json!({
                    "nodes": nodes["rows"].as_array().cloned().unwrap_or_default().into_iter().map(|r| r["n"].clone()).collect::<Vec<_>>(),
                    "relationships": rels["rows"].as_array().cloned().unwrap_or_default().into_iter().map(|r| r["r"].clone()).collect::<Vec<_>>(),
                })
            }
            _ => json!({ "error": "unknown op" }),
        };
        let mut out = stdout.lock();
        writeln!(out, "{response}").ok();
        out.flush().ok();
    }
}
