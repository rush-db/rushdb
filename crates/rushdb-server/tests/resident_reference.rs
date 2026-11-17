//! Storage is the reference for every resident layer (§B7 2026-11-16): a
//! request answered with resident entries must be the same bytes as the same
//! request answered from storage at the same snapshot.
//!
//! - The same-snapshot corpus: every kind of read (records, aggregates,
//!   indexed filters, ranges, traversals, paths, property values, describe,
//!   SQL, vector search) answered with layers on (after warming them) and
//!   off, byte for byte, on several tenants.
//! - The randomized differential: writers commit random changes (inserts,
//!   deletes, type drift, label changes, links, index create and drop,
//!   policy changes) to several tenants while a reader answers random
//!   requests on and off at one snapshot.
//! - Restart: a cold node answers like the warm one before it.
//!
//! Run with `RUSHDB_RESIDENT_CACHES=off` too: the comparisons then hold
//! trivially and the assertions about hits are skipped.

use std::sync::{Arc, RwLock};

use rushdb_protocol::{EnvelopeHandler, RequestEnvelope};
use rushdb_server::{Node, NodeEnvelopeHandler, ServerConfig};
use serde_json::{json, Value};

const TENANTS: [&str; 3] = ["tenant-a", "tenant-b", "tenant-c"];

struct Harness {
    node: Arc<Node>,
    handler: NodeEnvelopeHandler,
}

fn harness(data_dir: Option<std::path::PathBuf>) -> Harness {
    let mut cfg = ServerConfig {
        multitenant: true,
        data_dir,
        ..ServerConfig::default()
    };
    let node = Arc::new(Node::bootstrap(&mut cfg).expect("bootstrap"));
    let handler = NodeEnvelopeHandler::new(Arc::clone(&node), &cfg);
    Harness { node, handler }
}

impl Harness {
    /// `tenant` is empty on a single-tenant node.
    fn try_call(&self, tenant: &str, operation: &Value) -> Result<Value, String> {
        let context = if tenant.is_empty() {
            json!({})
        } else {
            json!({ "scope": tenant })
        };
        let envelope: RequestEnvelope = serde_json::from_value(json!({
            "protocolVersion": 1,
            "context": context,
            "operation": operation,
        }))
        .map_err(|error| format!("envelope: {error}"))?;
        self.handler
            .execute(envelope, (!tenant.is_empty()).then(|| tenant.to_owned()))
            .map(|response| serde_json::to_value(response.response).expect("response json"))
            .map_err(|error| format!("{error:?}"))
    }

    fn call(&self, tenant: &str, operation: &Value) -> Value {
        self.try_call(tenant, operation)
            .unwrap_or_else(|error| panic!("{operation}: {error}"))
    }

    fn residents(&self) -> Arc<rushdb_storage::resident::Residents> {
        self.node.residents()
    }

    /// The bytes of `operation` answered from storage only, at the current
    /// snapshot.
    fn reference(&self, tenant: &str, operation: &Value) -> String {
        self.residents().set_enabled(false);
        let out = self.try_call(tenant, operation);
        self.residents()
            .set_enabled(rushdb_storage::resident::residents_enabled());
        render(out)
    }

    /// The bytes of `operation` with layers warmed by earlier runs and
    /// background builds, at the current snapshot.
    fn resident(&self, tenant: &str, operation: &Value) -> String {
        for _ in 0..2 {
            let _ = self.try_call(tenant, operation);
            self.residents().wait_idle();
        }
        render(self.try_call(tenant, operation))
    }

    /// Answer `operation` on and off at one snapshot and require the same
    /// bytes.
    fn assert_same(&self, tenant: &str, operation: &Value) {
        let before = self.node.committed_version();
        let resident = self.resident(tenant, operation);
        let reference = self.reference(tenant, operation);
        assert_eq!(
            self.node.committed_version(),
            before,
            "a commit landed between the two answers"
        );
        assert_eq!(
            resident, reference,
            "tenant {tenant}: resident answer differs from storage for {operation}"
        );
        // A malformed request would compare two equal parse errors; refusals
        // computed from the data (overflow, policy) are compared like answers.
        assert!(
            !reference.contains("envelope:") && !reference.contains("field `"),
            "{operation}: {reference}"
        );
    }

    fn hits(&self, layer: &str) -> u64 {
        self.residents()
            .snapshots()
            .iter()
            .filter(|snapshot| snapshot.layer == layer)
            .map(|snapshot| snapshot.hits)
            .sum()
    }
}

/// The response, or the error, as comparable bytes (errors must match too).
fn render(out: Result<Value, String>) -> String {
    match out {
        Ok(value) => serde_json::to_string(&value).expect("json"),
        Err(error) => format!("error: {error}"),
    }
}

fn records(query: &Value) -> Value {
    json!({ "kind": "query", "query": { "kind": "records", "query": query } })
}

fn ingest(label: &str, documents: &Value) -> Value {
    json!({ "kind": "ingest", "ingest": { "kind": "documents", "label": label,
        "documents": documents,
        "options": { "relationshipTypes": { "VENDOR": "OPERATED_BY", "STOP": "STOPS_AT" } } } })
}

/// Trips with every numeric edge the aggregates must keep bit for bit, a
/// relationship graph, and type drift on one property.
fn trips(seed: u64, n: u64) -> Value {
    // `k % m` as a float, exactly (every `m` below fits a u32).
    let small = |k: u64, m: u64| f64::from(u32::try_from(k % m).expect("below m"));
    let index = |k: u64, len: usize| usize::try_from(k % len as u64).expect("below len");
    let specials = [
        json!(-0.0),
        json!(0.0),
        json!(1e-310),
        json!(-1e-310),
        json!(f64::MAX),
        json!(9_007_199_254_740_993_i64),
        json!(i64::MAX),
        json!(i64::MIN),
        json!(null),
        json!(0.1),
    ];
    let docs: Vec<Value> = (0..n)
        .map(|i| {
            let k = i.wrapping_mul(2_654_435_761).wrapping_add(seed);
            let payment = ["card", "cash", "dispute", "no charge"][index(k, 4)];
            let mut doc = json!({
                "payment": payment,
                "vendor": k % 3 + 1,
                "pickup_hour": k % 24,
                "passengers": k % 6,
                "fare": small(k, 9_973) / 7.0,
                "tip": small(k, 101) / 3.0,
                "distance": small(k, 503) / 11.0,
                "embedding": [small(k, 7), small(k, 5) + 0.5, small(k, 3), 1.0],
            });
            // Type drift: one name holding ints, floats, strings and booleans.
            doc["mixed"] = match k % 5 {
                0 => json!(k),
                1 => json!(small(k, 1_000_003) + 0.25),
                2 => json!(format!("s{}", k % 13)),
                3 => json!(k % 2 == 0),
                _ => json!(null),
            };
            if k % 11 == 0 {
                doc["edge"] = specials[index(k / 11, specials.len())].clone();
            }
            if k % 7 == 0 {
                doc.as_object_mut().unwrap().remove("tip");
            }
            if k % 13 == 0 {
                doc["VENDOR"] = json!({ "name": format!("v{}", k % 3) });
                doc["STOP"] = json!([{ "zone": k % 17 }, { "zone": (k + 1) % 17 }]);
            }
            doc
        })
        .collect();
    Value::Array(docs)
}

fn setup(h: &Harness, tenant: &str, seed: u64) {
    h.call(tenant, &ingest("Trip", &trips(seed, 600)));
    for (name, property) in [
        ("trip_vendor", "vendor"),
        ("trip_payment", "payment"),
        ("trip_fare", "fare"),
    ] {
        h.call(
            tenant,
            &json!({ "kind": "index", "index": { "kind": "create", "definition": {
                "name": name, "label": "Trip", "property": property, "kind": "ordered" } } }),
        );
    }
    let _ = h.try_call(
        tenant,
        &json!({ "kind": "index", "index": { "kind": "createVector", "definition": {
            "label": "Trip", "property": "embedding", "dimensions": 4,
            "similarity": "cosine", "name": "trip_embedding" } } }),
    );
    wait_indexes_ready(h, tenant);
}

fn wait_indexes_ready(h: &Harness, tenant: &str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        h.node.step_index_builds(std::time::Duration::from_secs(5));
        let out = h.call(
            tenant,
            &json!({ "kind": "describe", "describe": { "kind": "indexes" } }),
        );
        let text = out.to_string();
        if !text.contains("building") {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "indexes never became ready: {text}"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Every kind of read, over the setup's data.
#[expect(
    clippy::too_many_lines,
    reason = "one literal list of requests; splitting it would scatter the corpus"
)]
fn corpus(path_ends: Option<(String, String)>) -> Vec<Value> {
    let agg = |predicate: Value, select: Value, group_by: Value| {
        records(
            &json!({ "labels": ["Trip"], "predicate": predicate, "select": select, "groupBy": group_by }),
        )
    };
    let mut out = vec![
        // Grouped sum, avg, min and max over one or two keys, with and without a filter.
        agg(
            json!({}),
            json!({ "n": { "$count": "*" }, "s": { "$sum": "$record.fare" } }),
            json!(["$record.payment"]),
        ),
        agg(
            json!({}),
            json!({ "a": { "$avg": "$record.tip" } }),
            json!(["$record.pickup_hour"]),
        ),
        agg(
            json!({}),
            json!({ "a": { "$avg": "$record.distance" }, "m": { "$max": "$record.fare" } }),
            json!(["$record.vendor"]),
        ),
        agg(
            json!({ "payment": "card", "passengers": { "$gte": 2 } }),
            json!({ "n": { "$count": "*" }, "s": { "$sum": "$record.fare" } }),
            json!(["$record.vendor"]),
        ),
        agg(
            json!({}),
            json!({ "n": { "$count": "*" }, "m": { "$min": "$record.fare" } }),
            json!(["$record.vendor", "$record.payment"]),
        ),
        // Numeric edges, drift, missing values and nulls.
        agg(
            json!({}),
            json!({ "s": { "$sum": "$record.edge" }, "a": { "$avg": "$record.edge" },
            "lo": { "$min": "$record.edge" }, "hi": { "$max": "$record.edge" } }),
            json!(["$record.vendor"]),
        ),
        agg(
            json!({}),
            json!({ "lo": { "$min": "$record.edge" }, "hi": { "$max": "$record.edge" },
            "n": { "$count": "$record.edge" } }),
            json!(["$record.payment"]),
        ),
        agg(
            json!({}),
            json!({ "s": { "$sum": "$record.mixed" }, "lo": { "$min": "$record.mixed" },
            "hi": { "$max": "$record.mixed" }, "n": { "$count": "$record.mixed" } }),
            json!(["$record.payment"]),
        ),
        agg(
            json!({}),
            json!({ "n": { "$count": "*" } }),
            json!(["$record.mixed"]),
        ),
        agg(
            json!({}),
            json!({ "n": { "$count": "*" } }),
            json!(["$record.tip"]),
        ),
        agg(
            json!({ "vendor": 99 }),
            json!({ "s": { "$sum": "$record.fare" } }),
            json!(["$record.payment"]),
        ),
        agg(
            json!({}),
            json!({ "s": { "$sum": "$record.fare" }, "n": { "$count": "*" } }),
            json!([]),
        ),
        agg(
            json!({}),
            json!({ "n": { "$count": "*" } }),
            json!(["$record.payment"]),
        ),
        // Indexed filters (owner sets) and ranges (planner statistics).
        records(
            &json!({ "labels": ["Trip"], "predicate": { "vendor": 2 }, "limit": 20, "order": { "fare": "asc" } }),
        ),
        records(
            &json!({ "labels": ["Trip"], "predicate": { "payment": { "$in": ["cash", "dispute"] } }, "limit": 5 }),
        ),
        records(
            &json!({ "labels": ["Trip"], "predicate": { "fare": { "$gte": 500, "$lt": 900 } }, "limit": 10 }),
        ),
        records(
            &json!({ "labels": ["Trip"], "predicate": { "fare": { "$gt": 1000 }, "vendor": 1 },
            "select": { "n": { "$count": "*" } } }),
        ),
        records(
            &json!({ "labels": ["Trip"], "predicate": { "tip": { "$exists": false } }, "limit": 3 }),
        ),
        // Traversals (resident graphs).
        records(
            &json!({ "labels": ["Trip"], "predicate": { "VENDOR": { "$relation": { "type": "OPERATED_BY" }, "name": "v1" } }, "limit": 10 }),
        ),
        records(
            &json!({ "labels": ["STOP"], "predicate": { "Trip": { "$relation": { "type": "STOPS_AT", "direction": "in" }, "vendor": 2 } }, "limit": 10 }),
        ),
        records(
            &json!({ "labels": ["Trip"], "predicate": { "STOP": { "$relation": { "type": "STOPS_AT" }, "zone": { "$gte": 10 } } },
            "select": { "n": { "$count": "*" } } }),
        ),
        json!({ "kind": "query", "query": { "kind": "relationships", "query": { "type": "STOPS_AT" } } }),
        // Property values, catalogs, describe.
        json!({ "kind": "query", "query": { "kind": "propertyValues", "property": "payment", "query": { "labels": ["Trip"] }, "mode": "count" } }),
        json!({ "kind": "query", "query": { "kind": "propertyValues", "property": "fare", "query": { "labels": ["Trip"] }, "mode": "range" } }),
        json!({ "kind": "query", "query": { "kind": "propertyValues", "property": "mixed", "query": { "labels": ["Trip"] }, "mode": "values", "limit": 20 } }),
        json!({ "kind": "describe", "describe": { "kind": "schema", "query": {} } }),
        json!({ "kind": "describe", "describe": { "kind": "properties", "query": {} } }),
        json!({ "kind": "describe", "describe": { "kind": "properties", "query": { "labels": ["Trip"] } } }),
        json!({ "kind": "describe", "describe": { "kind": "properties", "query": { "labels": ["Trip"] }, "detail": "catalog" } }),
        json!({ "kind": "describe", "describe": { "kind": "properties", "query": { "labels": ["Trip", "STOP"] }, "skip": 2, "limit": 3 } }),
        json!({ "kind": "describe", "describe": { "kind": "schema", "query": { "labels": ["Trip"] } } }),
        json!({ "kind": "describe", "describe": { "kind": "labels", "query": {} } }),
        json!({ "kind": "describe", "describe": { "kind": "schemaSummary", "labels": ["Trip"] } }),
        // SQL and vectors.
        json!({ "kind": "query", "query": { "kind": "statement", "language": "sql",
            "statement": "SELECT payment, count(*) AS n, sum(fare) AS s FROM trip GROUP BY payment ORDER BY payment" } }),
        json!({ "kind": "query", "query": { "kind": "vector", "query": { "labels": ["Trip"], "property": "embedding",
            "vector": [1.0, 2.0, 0.5, 1.0], "limit": 7, "mode": "exact" } } }),
        json!({ "kind": "query", "query": { "kind": "vector", "query": { "labels": ["Trip"], "property": "embedding",
            "vector": [0.0, 1.0, 2.0, 1.0], "limit": 5, "predicate": { "vendor": 3 } } } }),
    ];
    if let Some((from, to)) = path_ends {
        out.push(
            json!({ "kind": "query", "query": { "kind": "path", "query": {
            "from": from, "to": to, "direction": "both", "maxHops": 6 } } }),
        );
    }
    out
}

/// Two record ids joined through the graph, for the path query.
fn path_ends(h: &Harness, tenant: &str) -> Option<(String, String)> {
    let out = h.call(
        tenant,
        &records(&json!({ "labels": ["STOP"], "limit": 2, "order": { "zone": "asc" } })),
    );
    let ids: Vec<String> = out["records"]
        .as_array()?
        .iter()
        .filter_map(|record| record["__id"].as_str().map(str::to_owned))
        .collect();
    (ids.len() == 2).then(|| (ids[0].clone(), ids[1].clone()))
}

#[test]
fn every_read_is_the_same_bytes_with_and_without_resident_layers() {
    let h = harness(None);
    for (seed, tenant) in TENANTS.iter().enumerate() {
        setup(&h, tenant, seed as u64 * 7_919);
    }
    for tenant in TENANTS {
        for operation in corpus(path_ends(&h, tenant)) {
            h.assert_same(tenant, &operation);
        }
    }
    if rushdb_storage::resident::residents_enabled() {
        for layer in [
            "owner-sets",
            "label-catalog",
            "property-dictionary",
            "planner-stats",
            "graph",
            "tenant-directory",
        ] {
            assert!(h.hits(layer) > 0, "layer {layer} was never used");
        }
    }
}

/// A seeded xorshift, replayable from the printed seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn index(&mut self, len: usize) -> usize {
        usize::try_from(self.below(len as u64)).expect("below len")
    }
}

/// One random write to `tenant`: its kind (0..10) and whether it committed.
fn random_write(h: &Harness, rng: &mut Rng, tenant: &str) -> (usize, bool) {
    let k = rng.next();
    let kind = rng.below(10);
    let op = match kind {
        0..=2 => ingest("Trip", &trips(k, 1 + rng.below(20))),
        3 => json!({ "kind": "mutate", "mutation": { "kind": "deleteRecords",
            "query": { "labels": ["Trip"], "predicate": { "pickup_hour": rng.below(24) } } } }),
        4 => {
            // A label change: retype one record found by a filter, either way.
            let (from, to) = if rng.below(2) == 0 {
                ("Trip", "Ride")
            } else {
                ("Ride", "Trip")
            };
            let found = h.try_call(
                tenant,
                &records(&json!({ "labels": [from], "limit": 1,
                "predicate": { "vendor": 1 + rng.below(3) } })),
            );
            match found
                .ok()
                .and_then(|found| found["records"][0]["__id"].as_str().map(str::to_owned))
            {
                Some(id) => {
                    json!({ "kind": "mutate", "mutation": { "kind": "retypeRecord", "id": id,
                    "fromLabel": from, "toLabel": to } })
                }
                None => ingest("Trip", &trips(k, 1)),
            }
        }
        5 => ingest(
            "Trip",
            &json!([{ "fare": "free", "vendor": "x", "mixed": [1, 2], "payment": 7 }]),
        ),
        6 => json!({ "kind": "index", "index": { "kind": "drop", "name": "trip_vendor" } }),
        7 => json!({ "kind": "index", "index": { "kind": "create", "definition": {
            "name": "trip_vendor", "label": "Trip", "property": "vendor", "kind": "ordered" } } }),
        8 => {
            json!({ "kind": "policy", "policy": { "kind": "put", "enforced": rng.below(2) == 0, "policy": {
            "allowedLabels": ["Trip", "STOP"], "visibleProperties": { "Trip": ["payment", "vendor", "fare"] } } } })
        }
        _ => ingest(
            "Trip",
            &json!([{ "payment": "card", "vendor": 2, "fare": -0.0,
            "STOP": [{ "zone": rng.below(17) }] }]),
        ),
    };
    // Some writes are refused (dropping a missing index, a policy that
    // hides what a later write names); the test requires each kind to
    // commit at least once.
    let ok = h.try_call(tenant, &op).is_ok();
    if kind == 6 || kind == 7 {
        // Finish the drop or build, so the name can be created again.
        h.node.step_index_builds(std::time::Duration::from_secs(5));
    }
    (usize::try_from(kind).expect("below 10"), ok)
}

#[test]
fn random_reads_under_interleaved_writes_match_storage_at_their_snapshot() {
    let rounds: u64 = std::env::var("RESIDENT_DIFF_ROUNDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(300);
    let seed: u64 = std::env::var("RESIDENT_DIFF_SEED")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0x005E_ED0F_2026);
    eprintln!("RESIDENT_DIFF_SEED={seed} RESIDENT_DIFF_ROUNDS={rounds}");
    let h = Arc::new(harness(None));
    for (n, tenant) in TENANTS.iter().enumerate() {
        setup(&h, tenant, seed.wrapping_add(n as u64));
    }
    // Writers commit under the shared side; a reader pair takes the
    // exclusive side, so no commit lands between its two answers.
    let gate = Arc::new(RwLock::new(()));
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let committed: Arc<[std::sync::atomic::AtomicU64; 10]> = Arc::new(Default::default());
    let writers: Vec<_> = (0..2u64)
        .map(|w| {
            let (h, gate, stop) = (Arc::clone(&h), Arc::clone(&gate), Arc::clone(&stop));
            let committed = Arc::clone(&committed);
            std::thread::spawn(move || {
                let mut rng = Rng(seed ^ (w + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    let tenant = TENANTS[rng.index(TENANTS.len())];
                    let _shared = gate.read().unwrap();
                    if let (kind, true) = random_write(&h, &mut rng, tenant) {
                        committed[kind].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();
    let mut refused = 0u64;
    let mut rng = Rng(seed | 1);
    for round in 0..rounds {
        let tenant = TENANTS[rng.index(TENANTS.len())];
        let corpus = corpus(None);
        let operation = &corpus[rng.index(corpus.len())];
        // Warm outside the exclusive side, so builds race the writers.
        let _ = h.try_call(tenant, operation);
        let _exclusive = gate.write().unwrap();
        let resident = render(h.try_call(tenant, operation));
        let reference = h.reference(tenant, operation);
        assert_eq!(
            resident, reference,
            "round {round} (seed {seed}), tenant {tenant}: {operation}"
        );
        refused += u64::from(reference.starts_with("error:"));
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    for writer in writers {
        writer.join().unwrap();
    }
    let committed: Vec<u64> = committed
        .iter()
        .map(|count| count.load(std::sync::atomic::Ordering::Relaxed))
        .collect();
    eprintln!("writes committed by kind: {committed:?}; reads refused: {refused} of {rounds}");
    for (kind, count) in committed.iter().enumerate() {
        assert!(*count > 0, "write kind {kind} never committed");
    }
    assert!(
        refused * 2 < rounds,
        "{refused} of {rounds} reads were refusals"
    );
}

#[test]
fn a_restarted_node_answers_like_the_warm_one_before_it() {
    let dir =
        std::env::temp_dir().join(format!("rushdb-resident-restart-{}", uuid::Uuid::now_v7()));
    let warm: Vec<String> = {
        let h = harness(Some(dir.clone()));
        setup(&h, "tenant-a", 11);
        corpus(None)
            .iter()
            .map(|operation| h.resident("tenant-a", operation))
            .collect()
    };
    let h = harness(Some(dir.clone()));
    for (operation, before) in corpus(None).iter().zip(&warm) {
        // Cold: the first answer comes from storage, then layers fill.
        assert_eq!(
            &render(h.try_call("tenant-a", operation)),
            before,
            "cold {operation}"
        );
        assert_eq!(
            &h.resident("tenant-a", operation),
            before,
            "rewarmed {operation}"
        );
    }
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Budgets so small that layers churn (LRU on every insert, most values
/// refused): answers still match storage.
#[test]
fn tiny_budgets_answer_like_storage() {
    let tiny = rushdb_server::resource_config::ByteSize(4 * 1024);
    let mut cfg = ServerConfig {
        multitenant: true,
        ..ServerConfig::default()
    };
    cfg.resources.owner_set_cache_bytes = tiny;
    cfg.resources.planner_stats_bytes = tiny;
    cfg.resources.graph_cache_bytes = tiny;
    cfg.resources.column_cache_bytes = tiny;
    cfg.resources.vector_cache_bytes = rushdb_server::resource_config::ByteSize(64 * 1024);
    // Catalog layers (labels, properties, describe answers) share it.
    cfg.resources.logical_cache_bytes = rushdb_server::resource_config::ByteSize(16 * 1024);
    let node = Arc::new(Node::bootstrap(&mut cfg).expect("bootstrap"));
    let h = Harness {
        handler: NodeEnvelopeHandler::new(Arc::clone(&node), &cfg),
        node,
    };
    setup(&h, "tenant-a", 3);
    for operation in corpus(path_ends(&h, "tenant-a")) {
        h.assert_same("tenant-a", &operation);
    }
}

/// A single-tenant node answers every read, describe included, with the
/// same bytes with and without resident layers.
#[test]
fn a_single_tenant_node_answers_like_storage() {
    let mut cfg = ServerConfig {
        multitenant: false,
        ..ServerConfig::default()
    };
    let node = Arc::new(Node::bootstrap(&mut cfg).expect("bootstrap"));
    let h = Harness {
        handler: NodeEnvelopeHandler::new(Arc::clone(&node), &cfg),
        node,
    };
    setup(&h, "", 5);
    for operation in corpus(path_ends(&h, "")) {
        h.assert_same("", &operation);
    }
    if rushdb_storage::resident::residents_enabled() {
        assert!(h.hits("describe") > 0);
    }
}

/// Lookups of `layer`: `(hits, misses)`.
fn lookups(h: &Harness, layer: &str) -> (u64, u64) {
    h.residents()
        .snapshots()
        .iter()
        .filter(|snapshot| snapshot.layer == layer)
        .fold((0, 0), |(hits, misses), snapshot| {
            (hits + snapshot.hits, misses + snapshot.misses)
        })
}

/// A `describe.properties` answer is kept until a write reaches what it
/// reads: the described label's rows or a property it holds, or a new
/// property. Writes to another label's own properties and to another
/// tenant keep it. Every answer is storage's, with and without a policy.
#[test]
fn describe_answers_are_kept_until_a_write_reaches_them() {
    let h = harness(None);
    setup(&h, "tenant-a", 1);
    setup(&h, "tenant-b", 2);
    h.call(
        "tenant-a",
        &ingest("Note", &json!([{ "memo": "a", "weight": 1 }])),
    );
    let trip = json!({ "kind": "describe", "describe": { "kind": "properties", "query": { "labels": ["Trip"] } } });
    let on = rushdb_storage::resident::residents_enabled();
    // The answer after `write`: storage's, and whether it came from the
    // entry the earlier read left.
    let after = |write: &dyn Fn()| -> (String, bool) {
        h.assert_same("tenant-a", &trip);
        write();
        let before = lookups(&h, "describe");
        let answer = render(h.try_call("tenant-a", &trip));
        let (hits, _) = lookups(&h, "describe");
        assert_eq!(answer, h.reference("tenant-a", &trip));
        (answer, hits > before.0)
    };
    let (warm, kept) = after(&|| {});
    assert_eq!(kept, on, "a repeated read is answered from the entry");
    let (_, kept) = after(&|| {
        h.call(
            "tenant-a",
            &ingest("Note", &json!([{ "memo": "b", "weight": 2 }])),
        );
    });
    assert_eq!(kept, on, "another label's own properties");
    let (_, kept) = after(&|| {
        h.call("tenant-b", &ingest("Trip", &trips(9, 3)));
    });
    assert_eq!(kept, on, "another tenant");
    let (_, kept) = after(&|| {
        h.call(
            "tenant-a",
            &ingest("Note", &json!([{ "memo": "c", "fare": 3.5 }])),
        );
    });
    assert!(!kept, "another label writing a property Trip holds");
    let (grown, kept) = after(&|| {
        h.call("tenant-a", &ingest("Trip", &trips(7, 2)));
    });
    assert!(!kept, "a write to the described label");
    assert_ne!(grown, warm);
    let (renamed, kept) = after(&|| {
        h.call(
            "tenant-a",
            &ingest("Trip", &json!([{ "vendor": 1, "late_field": true }])),
        );
    });
    assert!(!kept, "a new property");
    assert!(renamed.contains("late_field"));
    // A policy narrows the answer; the entry made without it never leaks
    // what it hides, and comes back when the policy is lifted.
    h.call(
        "tenant-a",
        &json!({ "kind": "policy", "policy": { "kind": "put", "enforced": true, "policy": {
            "allowedLabels": ["Trip"], "visibleProperties": { "Trip": ["payment", "vendor"] } } } }),
    );
    let masked = h.resident("tenant-a", &trip);
    assert_eq!(masked, h.reference("tenant-a", &trip));
    assert!(
        !masked.contains("\"fare\"") && masked.contains("\"vendor\""),
        "{masked}"
    );
    h.call(
        "tenant-a",
        &json!({ "kind": "policy", "policy": { "kind": "put", "enforced": false, "policy": {
            "allowedLabels": ["Trip"], "visibleProperties": { "Trip": ["payment", "vendor"] } } } }),
    );
    let lifted = h.resident("tenant-a", &trip);
    assert_eq!(lifted, h.reference("tenant-a", &trip));
    assert!(lifted.contains("\"fare\""));
}

/// Two tenants with the same labels and properties but different rows, and
/// a policy change in one: no entry made for one tenant or one policy ever
/// answers for another.
#[test]
fn no_entry_crosses_tenants_or_policies() {
    let h = harness(None);
    setup(&h, "tenant-a", 1);
    setup(&h, "tenant-b", 2);
    let probe = records(&json!({ "labels": ["Trip"], "predicate": { "vendor": 2 },
        "select": { "n": { "$count": "*" }, "s": { "$sum": "$record.fare" } }, "groupBy": ["$record.payment"] }));
    let schema = json!({ "kind": "describe", "describe": { "kind": "schema", "query": {} } });
    for operation in [&probe, &schema] {
        let a = h.resident("tenant-a", operation);
        let b = h.resident("tenant-b", operation);
        assert_ne!(a, b, "the tenants hold different rows");
        assert_eq!(a, h.reference("tenant-a", operation));
        assert_eq!(b, h.reference("tenant-b", operation));
    }
    // An enforced policy hiding `fare` and the relationship labels: the
    // warm entries of the unrestricted policy must not answer.
    let before = h.resident("tenant-a", &schema);
    h.call(
        "tenant-a",
        &json!({ "kind": "policy", "policy": { "kind": "put", "enforced": true, "policy": {
            "allowedLabels": ["Trip"], "visibleProperties": { "Trip": ["payment", "vendor"] } } } }),
    );
    for operation in corpus(None) {
        h.assert_same("tenant-a", &operation);
    }
    assert_ne!(
        h.resident("tenant-a", &schema),
        before,
        "the policy narrows the schema"
    );
    // The other tenant is untouched by the policy.
    h.assert_same("tenant-b", &schema);
}
