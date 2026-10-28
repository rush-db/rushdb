//! Comprehensive query-languages test suite.
//!
//! Every enabled language is validated for the full workflow: syntax/read,
//! aggregation, write, delete, and DDL (where applicable). The suite drives the
//! protocol executor directly (the single surface all languages share) with a
//! seeded fixture.

use rushdb_protocol::{
    Describe, ErrorCode, Mutation, Operation, Query, QueryDocument, QueryLanguageCapability,
    RequestContext, RequestEnvelope, Response, ResponseEnvelope, StatementLanguage,
};
use rushdb_server::{Node, ServerConfig};
use serde_json::{json, Value};

fn config() -> ServerConfig {
    ServerConfig {
        port: 0,
        pgwire_port: 0,
        grpc_port: 0,
        bolt_port: 0,
        token: "test-token".into(),
        data_dir: None,
        multitenant: false,
        capability_secret: None,
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages: rushdb_protocol::LanguageSet {
            sql: true,
            gql: true,
            cypher: true,
            gremlin: true,
            sql_pgq: true,
            mongodb: true,
        },
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
    }
}

fn bootstrap() -> Node {
    let mut cfg = config();
    Node::bootstrap(&mut cfg).unwrap()
}

fn seed(node: &Node) {
    for name in ["Jedi Order", "Sith Order", "Galactic Republic"] {
        node.scope().create_record(
            "FACTION",
            &serde_json::json!({ "name": name, "founded_year_bby": if name == "Jedi Order" { -100 } else { -50 } }),
            &[],
            &serde_json::json!({}),
        )
        .unwrap();
    }
}

fn read(node: &Node, language: StatementLanguage, statement: &str) -> Vec<Value> {
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some(format!("read-{language:?}")),
        context: RequestContext::default(),
        operation: Operation::Query {
            query: Query::Statement {
                language,
                statement: statement.to_string(),
                parameters: None,
            },
        },
    };
    let ResponseEnvelope { response, .. } =
        rushdb_server::protocol::ProtocolExecutor::new(node, None)
            .execute(envelope)
            .unwrap_or_else(|e| panic!("{language:?} read failed: {e}"));
    match response {
        Response::Rows { rows, .. } => rows,
        Response::Records { records, .. } => records,
        other => panic!("{language:?} read returned unexpected {other:?}"),
    }
}

fn write(node: &Node, language: StatementLanguage, statement: &str) -> u64 {
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some(format!("write-{language:?}")),
        context: RequestContext::default(),
        operation: Operation::Mutate {
            mutation: Mutation::Statement {
                language,
                statement: statement.to_string(),
                parameters: None,
            },
        },
    };
    let ResponseEnvelope { response, .. } =
        rushdb_server::protocol::ProtocolExecutor::new(node, None)
            .execute(envelope)
            .unwrap_or_else(|e| panic!("{language:?} write failed: {e}"));
    match response {
        Response::StatementMutation { affected, .. } => affected,
        other => panic!("{language:?} write returned unexpected {other:?}"),
    }
}

fn describe_languages(node: &Node) -> Vec<QueryLanguageCapability> {
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("describe-languages".into()),
        context: RequestContext::default(),
        operation: Operation::Describe {
            describe: Describe::QueryLanguages,
        },
    };
    let ResponseEnvelope { response, .. } =
        rushdb_server::protocol::ProtocolExecutor::new(node, None)
            .execute(envelope)
            .unwrap();
    match response {
        Response::QueryLanguages { languages } => languages,
        other => panic!("unexpected {other:?}"),
    }
}

fn label_count(node: &Node, label: &str) -> usize {
    let lmpg = node.scope().lmpg();
    let snap = lmpg.store.read_view().unwrap();
    rushdb_lmpg::reader::try_access::label_members(&snap, lmpg.tenant, label)
        .unwrap()
        .len()
}

// ── searchquery (native QueryDocument — always enabled) ─────────────────────

#[test]
fn searchquery_read_filter_projection_sort() {
    let node = bootstrap();
    seed(&node);
    let query = json!({
        "labels": ["FACTION"],
        "predicate": { "name": { "$ne": "Sith Order" } },
        "select": { "name": "$record.name" },
        "order": { "name": "asc" }
    });
    let rows = read_searchquery(&node, &query);
    assert_eq!(rows.len(), 2);
    for row in &rows {
        let row = row.as_object().unwrap();
        assert_eq!(row.len(), 1, "projected to `name` only: {row:?}");
        assert_ne!(row["name"], json!("Sith Order"));
    }
}

#[test]
fn searchquery_finite_page_preserves_total_and_order() {
    let node = bootstrap();
    seed(&node);
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("finite-native-page".into()),
        context: RequestContext::default(),
        operation: Operation::Query {
            query: Query::Records {
                query: QueryDocument {
                    labels: vec!["FACTION".into()],
                    order: Some(json!({ "name": "asc" })),
                    limit: Some(2),
                    ..Default::default()
                },
            },
        },
    };
    let response = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(envelope)
        .unwrap();
    let Response::Records { records, total, .. } = response.response else {
        panic!("expected a records response");
    };
    assert_eq!(total, 3, "total is pre-pagination");
    assert_eq!(records.len(), 2);
    assert_eq!(
        records
            .iter()
            .map(|record| record["properties"]["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["Galactic Republic", "Jedi Order"]
    );
}

#[test]
fn searchquery_finite_ordered_page_applies_offset_after_global_order() {
    let node = bootstrap();
    seed(&node);
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("finite-native-offset-page".into()),
        context: RequestContext::default(),
        operation: Operation::Query {
            query: Query::Records {
                query: QueryDocument {
                    labels: vec!["FACTION".into()],
                    order: Some(json!({ "name": "asc" })),
                    offset: 1,
                    limit: Some(1),
                    ..Default::default()
                },
            },
        },
    };
    let response = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(envelope)
        .unwrap();
    let Response::Records { records, total, .. } = response.response else {
        panic!("expected a records response");
    };
    assert_eq!(total, 3, "total is pre-pagination");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["properties"]["name"], "Jedi Order");
}

#[test]
fn searchquery_aggregate_group_by_and_count() {
    let node = bootstrap();
    seed(&node);
    let query = json!({
        "labels": ["FACTION"],
        "select": { "count": { "$count": "*" } },
        "groupBy": ["$record.name"]
    });
    let rows = read_searchquery(&node, &query);
    assert_eq!(rows.len(), 3, "one group per faction");
    assert!(rows.iter().all(|row| row["count"] == json!(1)), "{rows:?}");
}

#[test]
fn searchquery_write_delete_and_link() {
    let node = bootstrap();
    seed(&node);
    // PutRecord create
    let created = node
        .scope()
        .create_record(
            "FACTION",
            &json!({ "name": "Rebel Alliance" }),
            &[],
            &json!({}),
        )
        .unwrap();
    assert_eq!(
        created.get("label").and_then(Value::as_str),
        Some("FACTION")
    );
    // DeleteRecords via mutation
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: None,
        context: RequestContext::default(),
        operation: Operation::Mutate {
            mutation: Mutation::DeleteRecords {
                query: serde_json::from_value(json!({
                    "labels": ["FACTION"],
                    "predicate": { "name": "Rebel Alliance" }
                }))
                .unwrap(),
            },
        },
    };
    let _ = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(envelope)
        .unwrap();
    assert_eq!(
        label_count(&node, "FACTION"),
        3,
        "deleted the created record"
    );
}

fn read_searchquery(node: &Node, query: &Value) -> Vec<Value> {
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: None,
        context: RequestContext::default(),
        operation: Operation::Query {
            query: Query::Records {
                query: serde_json::from_value(query.clone()).unwrap(),
            },
        },
    };
    let ResponseEnvelope { response, .. } =
        rushdb_server::protocol::ProtocolExecutor::new(node, None)
            .execute(envelope)
            .unwrap();
    match response {
        Response::Records { records, .. } => records,
        Response::Rows { rows, .. } => rows,
        other => panic!("searchquery read returned {other:?}"),
    }
}

// ── sql ─────────────────────────────────────────────────────────────────────

#[test]
fn sql_read_and_aggregate() {
    let node = bootstrap();
    seed(&node);
    let rows = read(
        &node,
        StatementLanguage::Sql,
        "SELECT name FROM FACTION ORDER BY name",
    );
    assert_eq!(rows.len(), 3);
    let agg = read(
        &node,
        StatementLanguage::Sql,
        "SELECT founded_year_bby < 0 AS ancient, COUNT(*) AS n FROM FACTION GROUP BY ancient",
    );
    assert_eq!(agg.len(), 1);
}

#[test]
fn sql_write_update_delete() {
    let node = bootstrap();
    seed(&node);
    assert_eq!(
        write(
            &node,
            StatementLanguage::Sql,
            "INSERT INTO FACTION (name, founded_year_bby) VALUES ('New Republic', -25)",
        ),
        1
    );
    assert_eq!(label_count(&node, "FACTION"), 4);
    assert_eq!(
        write(
            &node,
            StatementLanguage::Sql,
            "UPDATE FACTION SET name = 'N Republic' WHERE name = 'New Republic'",
        ),
        1
    );
    assert_eq!(
        write(
            &node,
            StatementLanguage::Sql,
            "DELETE FROM FACTION WHERE name = 'N Republic'"
        ),
        1
    );
    assert_eq!(label_count(&node, "FACTION"), 3);
}

#[test]
fn sql_ddl_create_and_drop_vector_index() {
    let node = bootstrap();
    seed(&node);
    let alias = write(
        &node,
        StatementLanguage::Sql,
        "CREATE VECTOR INDEX _ddl ON FACTION(name) SIMILARITY cosine DIMENSIONS 4",
    );
    let _ = alias;
    let indexes = node.scope().list_vector_indexes().unwrap();
    assert_eq!(
        indexes.get("data").and_then(Value::as_array).map(Vec::len),
        Some(1)
    );
}

// ── cypher ──────────────────────────────────────────────────────────────────

#[test]
fn cypher_read_aggregate() {
    let node = bootstrap();
    seed(&node);
    let rows = read(
        &node,
        StatementLanguage::Cypher,
        "MATCH (n:FACTION) RETURN n.name AS name ORDER BY name",
    );
    assert_eq!(rows.len(), 3);
    let agg = read(
        &node,
        StatementLanguage::Cypher,
        "MATCH (n:FACTION) RETURN count(n) AS n",
    );
    assert_eq!(agg[0].get("n").and_then(Value::as_u64), Some(3));
}

#[test]
fn cypher_write_update_delete() {
    let node = bootstrap();
    seed(&node);
    assert_eq!(
        write(
            &node,
            StatementLanguage::Cypher,
            "CREATE (n:FACTION {name: 'Rebel Alliance'})"
        ),
        1
    );
    assert_eq!(label_count(&node, "FACTION"), 4);
    assert_eq!(
        write(
            &node,
            StatementLanguage::Cypher,
            "MATCH (n:FACTION {name: 'Rebel Alliance'}) SET n.name = 'Alliance'",
        ),
        1
    );
    assert_eq!(
        write(
            &node,
            StatementLanguage::Cypher,
            "MATCH (n:FACTION {name: 'Alliance'}) DELETE n"
        ),
        1
    );
    assert_eq!(label_count(&node, "FACTION"), 3);
}

// ── gql ─────────────────────────────────────────────────────────────────────

#[test]
fn gql_read_and_write() {
    let node = bootstrap();
    seed(&node);
    let rows = read(
        &node,
        StatementLanguage::Gql,
        "MATCH (n:FACTION) RETURN n.name AS name ORDER BY name",
    );
    assert_eq!(rows.len(), 3);
    assert_eq!(
        write(
            &node,
            StatementLanguage::Gql,
            "INSERT (:FACTION {name: 'Rebel Alliance'})"
        ),
        1
    );
    assert_eq!(label_count(&node, "FACTION"), 4);
}

// ── gremlin ─────────────────────────────────────────────────────────────────

#[test]
fn gremlin_read_and_write() {
    let node = bootstrap();
    seed(&node);
    let rows = read(
        &node,
        StatementLanguage::Gremlin,
        "g.V().hasLabel('FACTION').values('name')",
    );
    assert_eq!(rows.len(), 3);
    assert_eq!(
        write(
            &node,
            StatementLanguage::Gremlin,
            "g.addV('FACTION').property('name', 'Rebel Alliance')",
        ),
        1
    );
    assert_eq!(label_count(&node, "FACTION"), 4);
}

// ── sql-pgq (read-only) ─────────────────────────────────────────────────────

#[test]
fn sql_pgq_read_and_write_rejected() {
    let node = bootstrap();
    seed(&node);
    let rows = read(
        &node,
        StatementLanguage::SqlPgq,
        "SELECT * FROM GRAPH_TABLE (MATCH (n:FACTION) COLUMNS (n.name AS name)) AS g ORDER BY name",
    );
    assert_eq!(rows.len(), 3);
    // Writes are rejected as read-only.
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: None,
        context: RequestContext::default(),
        operation: Operation::Mutate {
            mutation: Mutation::Statement {
                language: StatementLanguage::SqlPgq,
                statement: "CREATE (:FACTION {name: 'X'})".into(),
                parameters: None,
            },
        },
    };
    let err = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(envelope)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidRequest);
}

// ── mongodb ─────────────────────────────────────────────────────────────────

#[test]
fn mongodb_read_find_and_aggregate() {
    let node = bootstrap();
    seed(&node);
    let rows = read(
        &node,
        StatementLanguage::Mongo,
        r#"{"collection":"FACTION","filter":{"name":{"$ne":"Sith Order"}},"sort":{"name":1}}"#,
    );
    assert_eq!(rows.len(), 2);
    let agg = read(
        &node,
        StatementLanguage::Mongo,
        r#"{"aggregate":[{"$group":{"_id":"$name","n":{"$sum":1}}}],"collection":"FACTION"}"#,
    );
    assert_eq!(agg.len(), 3);
}

#[test]
fn mongodb_write_upsert_and_delete() {
    let node = bootstrap();
    seed(&node);
    assert_eq!(
        write(
            &node,
            StatementLanguage::Mongo,
            r#"{"insertOne":{"collection":"FACTION","document":{"name":"Rebel Alliance"}}}"#,
        ),
        1
    );
    assert_eq!(label_count(&node, "FACTION"), 4);
    assert_eq!(
        write(
            &node,
            StatementLanguage::Mongo,
            r#"{"updateOne":{"collection":"FACTION","filter":{"name":"Rebel Alliance"},"update":{"$set":{"name":"Alliance"}}}}"#,
        ),
        1
    );
    assert_eq!(
        write(
            &node,
            StatementLanguage::Mongo,
            r#"{"deleteOne":{"collection":"FACTION","filter":{"name":"Alliance"}}}"#,
        ),
        1
    );
    assert_eq!(label_count(&node, "FACTION"), 3);
}

#[test]
fn mongodb_index_ddl() {
    let node = bootstrap();
    seed(&node);
    write(
        &node,
        StatementLanguage::Mongo,
        r#"{"createIndex":{"collection":"FACTION","property":"name","similarity":"cosine","dimensions":4,"name":"faction-name-4"}}"#,
    );
    let indexes = node.scope().list_vector_indexes().unwrap();
    assert_eq!(
        indexes.get("data").and_then(Value::as_array).map(Vec::len),
        Some(1)
    );
}

// ── capability advertisement ────────────────────────────────────────────────

#[test]
fn capabilities_advertise_only_enabled_languages() {
    let node = bootstrap();
    let languages = describe_languages(&node);
    assert_eq!(
        languages.len(),
        6,
        "all six statement languages enabled in the suite"
    );
    assert!(languages.iter().all(|c| {
        c.access
            == match c.language {
                StatementLanguage::SqlPgq => rushdb_protocol::StatementAccess::ReadOnly,
                _ => rushdb_protocol::StatementAccess::ReadWrite,
            }
    }));
    // Honest delivery statuses: SQL/graph-traversal/Mongo are already done;
    // GQL and openCypher are release candidates until their conformance gates
    // go green. No language advertises ComingSoon.
    for c in &languages {
        let expected = match c.language {
            StatementLanguage::Gql | StatementLanguage::Cypher => {
                rushdb_protocol::LanguageStatus::ReleaseCandidate
            }
            _ => rushdb_protocol::LanguageStatus::Stable,
        };
        assert_eq!(
            c.status,
            expected,
            "{} must advertise {:?}, got {:?}",
            c.language.as_str(),
            expected,
            c.status
        );
    }
    let gql = languages
        .iter()
        .find(|c| c.language == StatementLanguage::Gql)
        .unwrap();
    assert!(gql
        .limitations
        .iter()
        .any(|limit| limit == "noFullIsoGqlConformance"));
    for limit in [
        "singlePrimaryLabelPerRecord",
        "statementLabelMutationUnavailable",
    ] {
        assert!(
            gql.limitations.iter().any(|actual| actual == limit),
            "GQL profile must advertise {limit}"
        );
    }
    let cypher = languages
        .iter()
        .find(|c| c.language == StatementLanguage::Cypher)
        .unwrap();
    // Every exclusion of the openCypher TCK profile is advertised.
    let profile: serde_json::Value = serde_json::from_str(include_str!(
        "../conformance/opencypher-tck-v2024.2-profile.json"
    ))
    .unwrap();
    let mut exclusions = 0;
    for category in profile["categories"].as_object().unwrap().values() {
        if category["kind"] != "exclusion" {
            continue;
        }
        for limit in category["limitations"].as_array().unwrap() {
            exclusions += 1;
            let limit = limit.as_str().unwrap();
            assert!(
                cypher.limitations.iter().any(|actual| actual == limit),
                "Cypher profile must advertise {limit}"
            );
        }
    }
    assert!(exclusions > 0);
    assert!(cypher
        .limitations
        .iter()
        .any(|limit| limit == "noFullOpenCypherTckConformance"));
    assert!(
        !cypher
            .limitations
            .iter()
            .any(|limit| limit == "noCypherIndexDdl"),
        "Cypher index DDL works (cypher_conformance index-ddl), so it is no limitation"
    );
}

// ── disabled-language refusal (default config) ──────────────────────────────

#[test]
fn disabled_language_is_refused() {
    let mut cfg = ServerConfig {
        languages: rushdb_protocol::LanguageSet::NONE, // searchquery-only
        ..ServerConfig::default()
    };
    let node = Node::bootstrap(&mut cfg).unwrap();
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: None,
        context: RequestContext::default(),
        operation: Operation::Query {
            query: Query::Statement {
                language: StatementLanguage::Mongo,
                statement: r#"{"collection":"X","filter":{}}"#.into(),
                parameters: None,
            },
        },
    };
    let err = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(envelope)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::LanguageDisabled);
    // searchquery still works.
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: None,
        context: RequestContext::default(),
        operation: Operation::Query {
            query: Query::Records {
                query: QueryDocument::default(),
            },
        },
    };
    let _ = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(envelope)
        .unwrap();
}
#[test]
fn build_info_reports_identity_bindings_and_limits() {
    use rushdb_protocol::{Describe, Operation, RequestContext, RequestEnvelope, Response};
    let node = bootstrap();
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("build-info".into()),
        context: RequestContext::default(),
        operation: Operation::Describe {
            describe: Describe::BuildInfo,
        },
    };
    let response = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(envelope)
        .unwrap();
    let Response::BuildInfo { build } = response.response else {
        panic!("expected BuildInfo response");
    };
    assert_eq!(build.engine, "rushdb-lmpg");
    assert_eq!(build.protocol_version, rushdb_protocol::PROTOCOL_VERSION);
    assert!(!build.version.is_empty());
    // Every transport has a binding; the port is the configured value (0 when
    // ephemeral). `tls` reflects whether the listener serves HTTPS.
    let http = build
        .transports
        .iter()
        .find(|t| t.transport == "http")
        .expect("http transport binding");
    assert!(!http.tls);
    // Limits are populated.
    assert!(build.limits.contains_key("maxRequestBytes"));
    assert!(build.limits.contains_key("storageBudgetBytes"));
}

#[test]
fn mongo_unsupported_forms_fail_with_typed_error() {
    use rushdb_protocol::{ErrorCode, Operation, Query, RequestContext, RequestEnvelope};
    let node = bootstrap();
    let statements = [
        // aggregate stages that are recognized-but-unsupported
        r#"{"collection":"X","aggregate":[{"$match":{"a":1}},{"$lookup":{"from":"Y","localField":"a","foreignField":"b","as":"c"}}]}"#,
        r#"{"collection":"X","aggregate":[{"$match":{"a":1}},{"$geoNear":{"near":{"type":"Point","coordinates":[0,0]},"distanceField":"d"}}]}"#,
        r#"{"collection":"X","aggregate":[{"$match":{}},{"$unwind":"$arr"}]}"#,
    ];
    for stmt in statements {
        let envelope = RequestEnvelope {
            protocol_version: 1,
            request_id: Some("mongo-unsupported".into()),
            context: RequestContext::default(),
            operation: Operation::Query {
                query: Query::Statement {
                    language: StatementLanguage::Mongo,
                    statement: stmt.into(),
                    parameters: None,
                },
            },
        };
        let err = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
            .execute(envelope)
            .unwrap_err();
        assert_eq!(
            err.code,
            ErrorCode::Unsupported,
            "statement {stmt} must fail with Unsupported, got {err:?}"
        );
        assert!(
            err.message.contains("unsupported"),
            "message must explain: {err:?}"
        );
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one maintenance-mode contract test covers reads, writes, indexes, purge, and rollback"
)]
fn maintenance_mode_rejects_writes_but_serves_reads() {
    use rushdb_protocol::{
        ErrorCode, IndexOperation, Mutation, Operation, Query, RequestContext, RequestEnvelope,
        TransactionOperation, VectorIndexDefinition,
    };
    let mut cfg = config();
    cfg.maintenance = true;
    let node = Node::bootstrap(&mut cfg).unwrap();

    // Reads still work.
    let read = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("read-in-maintenance".into()),
        context: RequestContext::default(),
        operation: Operation::Query {
            query: Query::Records {
                query: QueryDocument::default(),
            },
        },
    };
    let _ = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(read)
        .unwrap();

    // Writes are rejected with Unavailable.
    let write = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("write-in-maintenance".into()),
        context: RequestContext::default(),
        operation: Operation::Mutate {
            mutation: Mutation::PutRecord {
                mode: rushdb_protocol::PutMode::Create,
                id: None,
                label: Some("NOTE".into()),
                properties: serde_json::json!({ "text": "x" }),
                vectors: vec![],
                options: serde_json::json!({}),
            },
        },
    };
    let err = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(write)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Unavailable, "{err:?}");
    assert!(err.message.contains("maintenance"));

    let index = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("index-in-maintenance".into()),
        context: RequestContext::default(),
        operation: Operation::Index {
            index: IndexOperation::CreateVector {
                definition: VectorIndexDefinition {
                    name: Some("maintenance-index".into()),
                    label: "NOTE".into(),
                    property: "embedding".into(),
                    dimensions: 2,
                    similarity: "cosine".into(),
                    model: None,
                    owner: None,
                },
            },
        },
    };
    assert_eq!(
        rushdb_server::protocol::ProtocolExecutor::new(&node, None)
            .execute(index)
            .unwrap_err()
            .code,
        ErrorCode::Unavailable
    );

    let purge = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("purge-in-maintenance".into()),
        context: RequestContext::default(),
        operation: Operation::PurgeTenant,
    };
    assert_eq!(
        rushdb_server::protocol::ProtocolExecutor::new(&node, None)
            .execute(purge)
            .unwrap_err()
            .code,
        ErrorCode::Unavailable
    );

    // Rollback remains available so clients can release transaction state
    // while maintenance is active.
    let begun = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(RequestEnvelope {
            protocol_version: 1,
            request_id: Some("begin-in-maintenance".into()),
            context: RequestContext::default(),
            operation: Operation::Transaction {
                transaction: TransactionOperation::Begin {
                    ttl_ms: None,
                    autocommit: false,
                },
            },
        })
        .unwrap();
    let rushdb_protocol::Response::TransactionBegun { transaction_id } = begun.response else {
        panic!("expected transaction begin response");
    };
    let rolled_back = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(RequestEnvelope {
            protocol_version: 1,
            request_id: Some("rollback-in-maintenance".into()),
            context: RequestContext::default(),
            operation: Operation::Transaction {
                transaction: TransactionOperation::Rollback { transaction_id },
            },
        })
        .unwrap();
    assert!(matches!(
        rolled_back.response,
        rushdb_protocol::Response::TransactionOutcome {
            status: rushdb_protocol::TransactionStatus::RolledBack,
            ..
        }
    ));
}

#[test]
fn explain_reports_access_path_and_scan_estimate() {
    use rushdb_protocol::{
        Describe, Operation, QueryDocument, RequestContext, RequestEnvelope, Response,
    };
    let node = bootstrap();
    // Seed three records under one label.
    for i in 0..3 {
        let envelope = RequestEnvelope {
            protocol_version: 1,
            request_id: Some("seed".into()),
            context: RequestContext::default(),
            operation: Operation::Mutate {
                mutation: Mutation::PutRecord {
                    mode: rushdb_protocol::PutMode::Create,
                    id: None,
                    label: Some("BOOK".into()),
                    properties: serde_json::json!({ "t": i }),
                    vectors: vec![],
                    options: serde_json::json!({}),
                },
            },
        };
        rushdb_server::protocol::ProtocolExecutor::new(&node, None)
            .execute(envelope)
            .unwrap();
    }
    let doc = QueryDocument {
        labels: vec!["BOOK".to_string()],
        predicate: serde_json::json!({ "t": 1 }),
        limit: Some(10),
        ..Default::default()
    };
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("explain".into()),
        context: RequestContext::default(),
        operation: Operation::Describe {
            describe: Describe::Explain { query: doc },
        },
    };
    let res = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(envelope)
        .unwrap();
    let Response::Explain { plan } = res.response else {
        panic!("expected explain response");
    };
    // An unindexed equality leaf narrows by walking the property's owners.
    assert_eq!(plan.access_path, "property_walk:t");
    assert_eq!(plan.labels, vec!["BOOK"]);
    assert_eq!(
        plan.scan_estimate, 3,
        "scan estimate must count label members"
    );
    assert_eq!(
        plan.estimated_cardinality,
        Some(1),
        "EXPLAIN must surface the exact predicate-leaf cardinality (t=1)"
    );
    assert_eq!(plan.predicate, serde_json::json!({ "t": 1 }));
    assert_eq!(plan.limit, Some(10));
}

/// `range_selectivity_estimate` is a genuinely
/// separate field from `estimated_cardinality` (which stays exact, never an
/// approximation), and the maintained histogram behind it is never consulted
/// by the exact-result paths (`$count`, `EXPLAIN.estimatedCardinality`).
/// Closed structurally (by leaf shape, not by a `u64::MAX` coincidence), so
/// this test locks the guarantee in as a regression rather than a convention.
#[test]
fn range_selectivity_estimate_never_leaks_into_exact_result_paths() {
    use rushdb_protocol::{Describe, Operation, QueryDocument, RequestContext, RequestEnvelope};
    let node = bootstrap();
    for score in [10i64, 40, 25, 5, 100] {
        rushdb_server::protocol::ProtocolExecutor::new(&node, None)
            .execute(RequestEnvelope {
                protocol_version: 1,
                request_id: None,
                context: RequestContext::default(),
                operation: Operation::Mutate {
                    mutation: Mutation::PutRecord {
                        mode: rushdb_protocol::PutMode::Create,
                        id: None,
                        label: Some("SCORED".into()),
                        properties: serde_json::json!({ "score": score }),
                        vectors: vec![],
                        options: serde_json::json!({}),
                    },
                },
            })
            .unwrap();
    }
    // Three of the five scores (40, 25, 100) are strictly greater than 20.
    let predicate = serde_json::json!({ "score": { "$gt": 20 } });

    let explain_doc = QueryDocument {
        labels: vec!["SCORED".to_string()],
        predicate: predicate.clone(),
        ..Default::default()
    };
    let explained = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(RequestEnvelope {
            protocol_version: 1,
            request_id: None,
            context: RequestContext::default(),
            operation: Operation::Describe {
                describe: Describe::Explain { query: explain_doc },
            },
        })
        .unwrap();
    let Response::Explain { plan } = explained.response else {
        panic!("expected explain response");
    };
    assert_eq!(
        plan.estimated_cardinality, None,
        "a predicate whose only leaf is a range comparison has no exact \
         index-eligible cardinality; the estimate must not be substituted in"
    );
    let estimate = plan
        .range_selectivity_estimate
        .expect("a usable histogram exists for `score`");
    assert!(
        (0.0..=1.0).contains(&estimate),
        "selectivity estimate must be a fraction, got {estimate}"
    );

    let count_doc = QueryDocument {
        labels: vec!["SCORED".to_string()],
        predicate,
        select: Some(serde_json::json!({ "n": { "$count": "*" } })),
        ..Default::default()
    };
    let counted = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(RequestEnvelope {
            protocol_version: 1,
            request_id: None,
            context: RequestContext::default(),
            operation: Operation::Query {
                query: Query::Records { query: count_doc },
            },
        })
        .unwrap();
    let Response::Rows { rows, total } = counted.response else {
        panic!("expected aggregate rows, got {:?}", counted.response)
    };
    assert_eq!(total, 3);
    assert_eq!(
        rows[0]["n"], 3,
        "`$count` must return the exact match count (40, 25, 100), never the \
         approximate selectivity estimate"
    );
}

#[test]
fn cancelled_query_returns_structured_cancelled_error() {
    use rushdb_protocol::{ErrorCode, Query, QueryDocument, RequestContext, RequestEnvelope};
    let node = bootstrap();
    seed(&node);
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("cancel".into()),
        context: RequestContext::default(),
        operation: Operation::Query {
            query: Query::Records {
                query: QueryDocument {
                    labels: vec!["FACTION".to_string()],
                    ..Default::default()
                },
            },
        },
    };
    // Already-cancelled: the executor must abort before running the query.
    let err = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute_cancellable(envelope.clone(), &|| true)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Cancelled, "{err:?}");

    // Cancellation flips after the first poll: still cancelled, never completed.
    let polls = std::cell::Cell::new(0);
    let err = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute_cancellable(envelope, &|| {
            polls.set(polls.get() + 1);
            polls.get() >= 2
        })
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Cancelled, "{err:?}");
}

#[test]
fn aggregate_refuses_unbounded_distinct_groups() {
    use rushdb_protocol::{ErrorCode, Query, QueryDocument, RequestContext, RequestEnvelope};
    let node = bootstrap();
    // Create more distinct group keys than the bounded-aggregation cap.
    let cap = rushdb_query::MAX_AGGREGATE_BUCKETS;
    for i in 0..cap + 2 {
        node.scope()
            .create_record(
                "T",
                &serde_json::json!({ "n": i }),
                &[],
                &serde_json::json!({}),
            )
            .unwrap();
    }
    let doc = QueryDocument {
        labels: vec!["T".to_string()],
        group_by: vec!["$record.n".to_string()],
        select: Some(serde_json::json!({ "count": { "$count": "*" } })),
        ..Default::default()
    };
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("bounded".into()),
        context: RequestContext::default(),
        operation: Operation::Query {
            query: Query::Records { query: doc },
        },
    };
    let err = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(envelope)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourceExhausted, "{err:?}");
    assert!(err.message.contains("aggregate"), "{err:?}");
    assert!(!err.retryable, "the same query fails the same way: {err:?}");

    // A plain projection is one row per record: paging bounds it, not the
    // group cap.
    let doc = QueryDocument {
        labels: vec!["T".to_string()],
        select: Some(serde_json::json!({ "n": "$record.n" })),
        limit: Some(5),
        ..Default::default()
    };
    let response = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(RequestEnvelope {
            protocol_version: 1,
            request_id: None,
            context: RequestContext::default(),
            operation: Operation::Query {
                query: Query::Records { query: doc },
            },
        })
        .unwrap();
    let rushdb_protocol::Response::Rows { total, rows } = response.response else {
        panic!("expected rows: {:?}", response.response)
    };
    assert_eq!(rows.len(), 5);
    assert_eq!(total, (cap + 2) as u64);
}
