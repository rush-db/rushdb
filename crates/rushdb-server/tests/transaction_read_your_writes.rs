//! T-5b: cross-statement read-your-writes. A statement staged later in a
//! transaction must observe the writes staged by earlier statements, because
//! statement planning reads through the transaction overlay.

use rushdb_protocol::{
    Operation, Query, QueryDocument, RequestContext, RequestEnvelope, Response, StatementLanguage,
    TransactionMutation, TransactionOperation,
};
use rushdb_server::{Node, ServerConfig};

fn cypher_node() -> Node {
    let mut config = ServerConfig::default();
    config.languages.cypher = true;
    Node::bootstrap(&mut config).unwrap()
}

fn execute(node: &Node, operation: Operation) -> Response {
    rushdb_server::protocol::ProtocolExecutor::new(node, None)
        .execute(RequestEnvelope {
            protocol_version: 1,
            request_id: None,
            context: RequestContext::default(),
            operation,
        })
        .unwrap()
        .response
}

fn begin(node: &Node) -> String {
    let Response::TransactionBegun { transaction_id } = execute(
        node,
        Operation::Transaction {
            transaction: TransactionOperation::Begin {
                ttl_ms: None,
                autocommit: false,
            },
        },
    ) else {
        panic!("expected transaction begin");
    };
    transaction_id
}

fn stage_statement(node: &Node, tx: &str, statement: &str) {
    execute(
        node,
        Operation::Transaction {
            transaction: TransactionOperation::Stage {
                transaction_id: tx.to_string(),
                mutation: TransactionMutation::Statement {
                    language: StatementLanguage::Cypher,
                    statement: statement.to_string(),
                    parameters: None,
                },
            },
        },
    );
}

fn commit(node: &Node, tx: &str) {
    execute(
        node,
        Operation::Transaction {
            transaction: TransactionOperation::Commit {
                transaction_id: tx.to_string(),
            },
        },
    );
}

fn t5_records(node: &Node) -> Vec<serde_json::Value> {
    let Response::Records { records, .. } = execute(
        node,
        Operation::Query {
            query: Query::Records {
                query: QueryDocument {
                    labels: vec!["T5".into()],
                    ..Default::default()
                },
            },
        },
    ) else {
        panic!("expected records");
    };
    records
}

#[test]
fn later_statement_sees_earlier_statement_writes() {
    let node = cypher_node();
    let tx = begin(&node);

    // Statement 1 creates a node in the transaction overlay.
    stage_statement(&node, &tx, "CREATE (:T5 {tag: 'seed', v: 1})");
    // Statement 2 matches that *staged* node and updates it. Without the
    // overlay it would match nothing, produce no changes, and be rejected.
    stage_statement(&node, &tx, "MATCH (n:T5 {tag: 'seed'}) SET n.v = 2");

    commit(&node, &tx);

    let records = t5_records(&node);
    assert_eq!(records.len(), 1, "exactly the staged node committed");
    assert_eq!(records[0]["properties"]["v"], serde_json::json!(2));
    assert_eq!(records[0]["properties"]["tag"], serde_json::json!("seed"));
}

#[test]
fn later_statement_sees_earlier_statement_for_gated_create() {
    let node = cypher_node();
    let tx = begin(&node);

    stage_statement(&node, &tx, "CREATE (:T5 {tag: 'a'})");
    // A create gated on matching an earlier staged node can only succeed if the
    // overlay exposes it.
    stage_statement(
        &node,
        &tx,
        "MATCH (a:T5 {tag: 'a'}) CREATE (:T5 {tag: 'b'})",
    );

    commit(&node, &tx);

    let records = t5_records(&node);
    assert_eq!(records.len(), 2, "both staged nodes committed");
}

fn cypher_rows(node: &Node, statement: &str) -> Vec<serde_json::Value> {
    let Response::Rows { rows, .. } = execute(
        node,
        Operation::Query {
            query: Query::Statement {
                language: StatementLanguage::Cypher,
                statement: statement.to_string(),
                parameters: None,
            },
        },
    ) else {
        panic!("expected rows");
    };
    rows
}

#[test]
fn statement_relationships_stage_in_a_transaction() {
    let node = cypher_node();
    let tx = begin(&node);

    // A relationship between nodes the same statement creates, and one from
    // a node an earlier statement staged to a node this statement creates.
    stage_statement(
        &node,
        &tx,
        "CREATE (:T5 {tag: 'a'})-[:T5_LINK {w: 1}]->(:T5 {tag: 'b'})",
    );
    stage_statement(
        &node,
        &tx,
        "MATCH (a:T5 {tag: 'a'}) CREATE (a)-[:T5_LINK {w: 2}]->(:T5 {tag: 'c'})",
    );

    commit(&node, &tx);

    let rows = cypher_rows(
        &node,
        "MATCH (a:T5)-[r:T5_LINK]->(b:T5) RETURN a.tag AS a, r.w AS w, b.tag AS b ORDER BY w",
    );
    assert_eq!(
        rows,
        vec![
            serde_json::json!({"a": "a", "w": 1, "b": "b"}),
            serde_json::json!({"a": "a", "w": 2, "b": "c"}),
        ]
    );
}
