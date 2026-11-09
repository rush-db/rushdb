//! The binary vector form, `{"f32": "<base64 of little-endian f32>"}`, is
//! accepted wherever a vector is (inline `$vectors`, `vectors.upsert`
//! items, `query.vector`, the hybrid vector leg) and carries the exact bits;
//! a malformed one is the caller's error.

use rushdb_protocol::vector_wire::encode_f32_base64;
use rushdb_protocol::{
    ErrorCode, Operation, Query, RequestContext, RequestEnvelope, Response, VectorIndexDefinition,
    VectorItem, PROTOCOL_VERSION,
};
use rushdb_server::{Node, ServerConfig};

/// Components no decimal round trip is guaranteed to keep: a subnormal,
/// a value one ulp above a short decimal, a tiny negative, pi.
fn components() -> [f32; 4] {
    [
        f32::from_bits(1),
        f32::from_bits(0.1f32.to_bits() + 1),
        -1.0e-30,
        std::f32::consts::PI,
    ]
}

fn node() -> Node {
    let n = Node::bootstrap(&mut ServerConfig::default()).unwrap();
    n.scope()
        .create_vector_index(&VectorIndexDefinition {
            name: Some("raw".into()),
            label: "DOC".into(),
            property: "embedding".into(),
            dimensions: 4,
            similarity: "dot".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    n
}

fn execute(n: &Node, operation: Operation) -> Result<Response, ErrorCode> {
    rushdb_server::protocol::ProtocolExecutor::new(n, None)
        .execute(RequestEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: None,
            context: RequestContext::default(),
            operation,
        })
        .map(|response| response.response)
        .map_err(|error| error.code)
}

fn one_hot(at: usize) -> serde_json::Value {
    let mut basis = [0.0f32; 4];
    basis[at] = 1.0;
    serde_json::json!({ "f32": encode_f32_base64(&basis) })
}

/// The score of the one record under a dot product with the `at`-th unit
/// vector — exactly its `at`-th stored component — sent as a binary
/// `query.vector` and as a binary hybrid vector leg.
fn stored_component(n: &Node, at: usize) -> (f32, f32) {
    let query = serde_json::from_value(serde_json::json!({
        "labels": ["DOC"],
        "property": "embedding",
        "vector": one_hot(at),
        "index": "raw",
        "limit": 1,
        "mode": "exact",
    }))
    .unwrap();
    let Response::Records { records, .. } = execute(
        n,
        Operation::Query {
            query: Query::Vector { query },
        },
    )
    .unwrap() else {
        panic!("expected records")
    };
    let hybrid = serde_json::from_value(serde_json::json!({
        "labels": ["DOC"],
        "limit": 1,
        "vector": { "property": "embedding", "vector": one_hot(at), "index": "raw", "mode": "exact" },
    }))
    .unwrap();
    let Response::Records { records: fused, .. } = execute(
        n,
        Operation::Query {
            query: Query::Hybrid { query: hybrid },
        },
    )
    .unwrap() else {
        panic!("expected records")
    };
    #[expect(
        clippy::cast_possible_truncation,
        reason = "an f32 score widened by JSON"
    )]
    let score = |record: &serde_json::Value, field: &str| record[field].as_f64().unwrap() as f32;
    (score(&records[0], "score"), score(&fused[0], "vectorScore"))
}

fn assert_bits(n: &Node) {
    for (at, expected) in components().into_iter().enumerate() {
        let (query, hybrid) = stored_component(n, at);
        assert_eq!(query.to_bits(), expected.to_bits(), "component {at}");
        assert_eq!(hybrid.to_bits(), expected.to_bits(), "component {at}");
    }
}

#[test]
fn inline_binary_vectors_round_trip_bit_exactly() {
    let n = node();
    n.scope()
        .create_record(
            "DOC",
            &serde_json::json!({ "title": "inline" }),
            &[serde_json::json!({
                "propertyName": "embedding",
                "vector": { "f32": encode_f32_base64(&components()) },
                "index": "raw",
            })],
            &serde_json::json!({}),
        )
        .unwrap();
    assert_bits(&n);
}

#[test]
fn upserted_binary_vectors_round_trip_bit_exactly() {
    let n = node();
    let id = n
        .scope()
        .create_record(
            "DOC",
            &serde_json::json!({ "title": "upsert" }),
            &[],
            &serde_json::json!({}),
        )
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let item: VectorItem = serde_json::from_value(serde_json::json!({
        "recordId": id,
        "vector": { "f32": encode_f32_base64(&components()) },
    }))
    .unwrap();
    n.scope().upsert_vector_index("raw", &[item]).unwrap();
    assert_bits(&n);
}

#[test]
fn a_malformed_binary_vector_is_an_invalid_request() {
    let n = node();
    let put = |vector: serde_json::Value| {
        execute(
            &n,
            serde_json::from_value(serde_json::json!({
                "kind": "mutate",
                "mutation": {
                    "kind": "putRecord",
                    "mode": "create",
                    "label": "DOC",
                    "properties": { "title": "bad" },
                    "vectors": [{ "propertyName": "embedding", "vector": vector, "index": "raw" }],
                },
            }))
            .unwrap(),
        )
    };
    for bad in [
        // 7 bytes: not a whole number of floats.
        serde_json::json!({ "f32": "AAAAAAAAAA==" }),
        serde_json::json!({ "f32": encode_f32_base64(&[1.0, f32::NAN, 0.0, 0.0]) }),
        serde_json::json!({ "f32": encode_f32_base64(&[f32::INFINITY, 0.0, 0.0, 0.0]) }),
        serde_json::json!({ "f32": "not base64!" }),
    ] {
        assert_eq!(put(bad.clone()), Err(ErrorCode::InvalidRequest), "{bad}");
    }
    // A request whose `query.vector` is malformed does not parse at all.
    let error = serde_json::from_value::<RequestEnvelope>(serde_json::json!({
        "protocolVersion": PROTOCOL_VERSION,
        "operation": { "kind": "query", "query": { "kind": "vector", "query": {
            "labels": ["DOC"],
            "property": "embedding",
            "vector": { "f32": "AAAAAAAAAA==" },
            "limit": 1,
        } } },
    }))
    .unwrap_err();
    assert!(error.to_string().contains("invalid vector"), "{error}");
    // The same write with a well-formed vector succeeds.
    assert!(put(serde_json::json!([1.0, 0.0, 0.0, 0.0])).is_ok());
}
