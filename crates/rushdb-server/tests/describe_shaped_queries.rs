//! `Describe::Labels/Schema` must not 500 on output-shaping queries: label
//! discovery uses the raw matched-root search, so select/groupBy queries that
//! route through the aggregate path return Rows (no record label) instead of
//! panicking with "entered unreachable code".

use std::sync::Arc;

use rushdb_protocol::{Describe, EnvelopeHandler, Operation, RequestContext, RequestEnvelope};
use rushdb_server::{Node, NodeEnvelopeHandler, ServerConfig};

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "single linear operation/reader; splitting it would scatter shared borrows and hurt auditability"
)]
fn describe_labels_and_schema_handle_shaped_queries() {
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
        resources: rushdb_server::resource_config::ResourceConfig::default(),
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
    let node = Arc::new(Node::bootstrap(&mut cfg).expect("bootstrap"));
    let handler = NodeEnvelopeHandler::new(node.clone());

    // seed a small planet -> battle -> starship graph at the lmpg level
    {
        let lmpg = node.scope().lmpg();
        let p = rushdb_lmpg::import::import_document(
            &lmpg,
            "PLANET",
            &serde_json::json!({ "name": "Endor" }),
        )
        .unwrap();
        let b = rushdb_lmpg::import::import_document(
            &lmpg,
            "BATTLE",
            &serde_json::json!({ "name": "Endor battle" }),
        )
        .unwrap();
        let s = rushdb_lmpg::import::import_document(
            &lmpg,
            "STARSHIP",
            &serde_json::json!({ "name": "s1", "firepower_score": 90 }),
        )
        .unwrap();
        let _ = lmpg.create_relationship("RELATED", p, b, vec![]);
        let _ = lmpg.create_relationship("RELATED", b, s, vec![]);
    }

    // the failing label API shape: labels + predicate + select (multi-hop collect)
    let env = RequestEnvelope {
        protocol_version: 1,
        request_id: None,
        context: RequestContext {
            scope: None,
            deadline_ms: None,
            ..Default::default()
        },
        operation: Operation::Describe {
            describe: Describe::Labels {
                query: rushdb_protocol::QueryDocument {
                    labels: vec!["PLANET".into()],
                    predicate: serde_json::json!({
                        "BATTLE": {
                            "STARSHIP": { "$alias": "$starship", "firepower_score": { "$gte": 60 } }
                        }
                    }),
                    select: Some(serde_json::json!({
                        "planetName": "$record.name",
                        "highFirepowerStarships": {
                            "$collect": {
                                "from": "$starship",
                                "select": { "name": "$starship.name", "firepower": "$starship.firepower_score" },
                                "orderBy": { "firepower": "desc" },
                                "limit": 10
                            }
                        }
                    })),
                    group_by: vec![],
                    order: None,
                    offset: 0,
                    limit: None,
                    cursor_ttl_ms: None,
                },
            },
        },
    };
    let resp = handler.execute(env, None).expect("describe labels");
    match resp.response {
        rushdb_protocol::Response::Labels { labels } => assert_eq!(labels, vec!["PLANET"]),
        other => panic!("expected Labels, got {other:?}"),
    }

    // Describe::Schema had the same unreachable; verify it also returns records.
    let env = RequestEnvelope {
        protocol_version: 1,
        request_id: None,
        context: RequestContext {
            scope: None,
            deadline_ms: None,
            ..Default::default()
        },
        operation: Operation::Describe {
            describe: Describe::Schema {
                query: rushdb_protocol::QueryDocument {
                    labels: vec!["PLANET".into()],
                    predicate: serde_json::json!({}),
                    select: Some(serde_json::json!({
                        "planetName": "$record.name"
                    })),
                    group_by: vec![],
                    order: None,
                    offset: 0,
                    limit: None,
                    cursor_ttl_ms: None,
                },
                detail: None,
                skip: None,
                limit: None,
            },
        },
    };
    let resp = handler.execute(env, None).expect("describe schema");
    match resp.response {
        rushdb_protocol::Response::Schema { schema } => {
            assert_eq!(
                schema.labels,
                vec![rushdb_protocol::LabelDescription {
                    name: "PLANET".into(),
                    records_count: 1
                }]
            );
        }
        other => panic!("expected Schema, got {other:?}"),
    }
}
