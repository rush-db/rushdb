//! 2C — randomized property tests: the optimized LMPG query path must agree
//! with a simple O(n) reference evaluator over the plain record set. A seeded
//! PRNG drives record generation and query predicates; any divergence indicates
//! an optimization (index, pushdown, early-exit) changed the documented
//! semantics.

use rushdb_protocol::{
    Mutation, Operation, Query, QueryDocument, RequestContext, RequestEnvelope, Response,
};
use rushdb_server::{Node, ServerConfig};

// ── tiny deterministic PRNG (xorshift64) ──────────────────────────────────────
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the pick index is bounded by the slice length, so the u64-to-usize conversion is lossless on supported targets"
    )]
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }
}

const LABELS: [&str; 3] = ["ALPHA", "BETA", "GAMMA"];
const NAMES: [&str; 6] = ["ada", "bob", "carol", "dave", "erin", "frank"];

fn node() -> Node {
    let mut cfg = ServerConfig::default();
    Node::bootstrap(&mut cfg).unwrap()
}

/// Reference state: label + plain properties per generated record.
struct ReferenceState {
    records: Vec<(String, serde_json::Value)>,
}

fn put(node: &Node, label: &str, props: serde_json::Value) {
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("rnd-put".into()),
        context: RequestContext::default(),
        operation: Operation::Mutate {
            mutation: Mutation::PutRecord {
                mode: rushdb_protocol::PutMode::Create,
                id: None,
                label: Some(label.to_string()),
                properties: props,
                vectors: vec![],
                options: serde_json::json!({}),
            },
        },
    };
    rushdb_server::protocol::ProtocolExecutor::new(node, None)
        .execute(envelope)
        .unwrap();
}

fn find_contents(node: &Node, doc: QueryDocument) -> Vec<String> {
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("rnd-find".into()),
        context: RequestContext::default(),
        operation: Operation::Query {
            query: Query::Records { query: doc },
        },
    };
    let res = rushdb_server::protocol::ProtocolExecutor::new(node, None)
        .execute(envelope)
        .unwrap();
    let Response::Records { records, .. } = res.response else {
        panic!("expected records");
    };
    let mut contents = records
        .into_iter()
        .map(|record| {
            format!(
                "{}:{}",
                record.get("label").map_or("null".into(), canonical_json),
                record
                    .get("properties")
                    .map_or("null".into(), canonical_json)
            )
        })
        .collect::<Vec<_>>();
    contents.sort();
    contents
}

/// Naive reference evaluator: label membership + a single scalar predicate.
fn reference_eval(
    state: &ReferenceState,
    labels: &[String],
    predicate: &serde_json::Value,
) -> Vec<(String, serde_json::Value)> {
    state
        .records
        .iter()
        .filter(|(label, _props)| labels.is_empty() || labels.iter().any(|l| l == label))
        .filter(|(_, props)| eval_predicate(props, predicate))
        .cloned()
        .collect()
}

fn eval_predicate(props: &serde_json::Value, predicate: &serde_json::Value) -> bool {
    let Some(obj) = predicate.as_object() else {
        return false;
    };
    // Logical AND of all conditions.
    obj.iter().all(|(field, cond)| {
        let value = props.get(field);
        match cond {
            serde_json::Value::Null => value.is_some_and(serde_json::Value::is_null),
            serde_json::Value::Object(op) => {
                let (op_name, operand) = op.iter().next().unwrap();
                match op_name.as_str() {
                    "$gt" => value
                        .and_then(serde_json::Value::as_i64)
                        .is_some_and(|v| v > operand.as_i64().unwrap()),
                    "$gte" => value
                        .and_then(serde_json::Value::as_i64)
                        .is_some_and(|v| v >= operand.as_i64().unwrap()),
                    "$lt" => value
                        .and_then(serde_json::Value::as_i64)
                        .is_some_and(|v| v < operand.as_i64().unwrap()),
                    "$lte" => value
                        .and_then(serde_json::Value::as_i64)
                        .is_some_and(|v| v <= operand.as_i64().unwrap()),
                    "$ne" => value.is_some_and(|v| v != operand),
                    _ => false,
                }
            }
            other => value.is_some_and(|v| v == other),
        }
    })
}

fn reference_contents(
    state: &ReferenceState,
    labels: &[String],
    predicate: &serde_json::Value,
) -> Vec<String> {
    let mut contents = reference_eval(state, labels, predicate)
        .into_iter()
        .map(|(label, properties)| {
            format!(
                "{}:{}",
                canonical_json(&serde_json::json!(label)),
                canonical_json(&properties)
            )
        })
        .collect::<Vec<_>>();
    contents.sort();
    contents
}

fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(object) => {
            let mut fields = object
                .iter()
                .map(|(key, value)| format!("{}:{}", key, canonical_json(value)))
                .collect::<Vec<_>>();
            fields.sort();
            format!("{{{}}}", fields.join(","))
        }
        serde_json::Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        other => other.to_string(),
    }
}

#[test]
#[expect(
    clippy::cast_possible_wrap,
    reason = "the generator draws values well below i64::MAX, so the u64-to-i64 conversion cannot wrap"
)]
fn randomized_queries_agree_with_reference_evaluator() {
    let mut rng = Rng(0x5EED_CAFE_1234_5678);
    let n = node();
    let mut state = ReferenceState {
        records: Vec::new(),
    };

    // Generate a deterministic random state.
    for i in 0..120 {
        let label = *rng.pick(&LABELS);
        let props = serde_json::json!({
            "n": (rng.below(40) as i64) - 10,          // -10..=29
            "s": rng.pick(&NAMES),
            "b": rng.below(2) == 0,
            "idx": i,
        });
        put(&n, label, props.clone());
        state.records.push((label.to_string(), props));
    }

    // Random queries: random label subset + a random scalar predicate.
    for _ in 0..400 {
        let use_subset = rng.below(2) == 0;
        let labels: Vec<String> = if use_subset {
            vec![(*rng.pick(&LABELS)).to_string()]
        } else {
            vec![]
        };
        let predicate = match rng.below(5) {
            0 => serde_json::json!({ "n": (rng.below(40) as i64) - 10 }),
            1 => serde_json::json!({ "n": { "$gt": rng.below(30) as i64 } }),
            2 => serde_json::json!({ "n": { "$lt": rng.below(30) as i64 } }),
            3 => serde_json::json!({ "b": rng.below(2) == 0 }),
            _ => serde_json::json!({ "s": rng.pick(&NAMES) }),
        };

        let doc = QueryDocument {
            labels: labels.clone(),
            predicate: predicate.clone(),
            limit: Some(1000),
            ..Default::default()
        };
        let optimized = find_contents(&n, doc);
        let reference = reference_contents(&state, &labels, &predicate);

        assert_eq!(
            optimized, reference,
            "matched content diverged for {labels:?} {predicate}"
        );
    }
}

#[test]
#[expect(
    clippy::cast_possible_wrap,
    reason = "the generator draws values well below i64::MAX, so the u64-to-i64 conversion cannot wrap"
)]
fn randomized_gt_queries_match_reference_exactly() {
    // Focused property: `$gt` boundaries must match the reference precisely.
    let mut rng = Rng(0xABCD_EF01_2345_6789);
    let n = node();
    let mut state = ReferenceState {
        records: Vec::new(),
    };
    for i in 0..200 {
        let props = serde_json::json!({ "n": (rng.below(100) as i64) });
        put(&n, "A", props.clone());
        state.records.push(("A".to_string(), props));
        let _ = i;
    }
    for threshold in [0i64, 25, 50, 99, 100, -1] {
        let predicate = serde_json::json!({ "n": { "$gt": threshold } });
        let doc = QueryDocument {
            labels: vec!["A".to_string()],
            predicate: predicate.clone(),
            limit: Some(1000),
            ..Default::default()
        };
        let optimized = find_contents(&n, doc);
        let reference = reference_contents(&state, &["A".to_string()], &predicate);
        assert_eq!(optimized, reference, "diverged at threshold {threshold}");
    }
}
