//! 2A — openCypher conformance subset. A curated, versioned corpus runs in CI;
//! every case is classified as supported or unsupported with a reason. There
//! are no silent skips: if a query's actual outcome diverges from its
//! classification, this test fails and forces a deliberate reclassification.

use rushdb_protocol::{
    Mutation, Operation, Query, RequestContext, RequestEnvelope, StatementLanguage,
};
use rushdb_server::{Node, ServerConfig};

struct Case {
    name: &'static str,
    statement: &'static str,
    /// `Some(reason)` = expected to FAIL (unsupported / upstream defect);
    /// `None` = expected to succeed.
    unsupported: Option<&'static str>,
}

fn bootstrap() -> Node {
    let mut cfg = ServerConfig {
        languages: rushdb_protocol::LanguageSet {
            cypher: true,
            ..Default::default()
        },
        ..Default::default()
    };
    Node::bootstrap(&mut cfg).unwrap()
}

fn run_cypher(node: &Node, statement: &str) -> Result<(), String> {
    // Graph-language writes require the write-authorized Mutation operation;
    // reads go through Query. Route by whether the statement contains a write
    // clause anywhere (a MATCH...CREATE pattern still writes).
    let is_write = ["CREATE", "MERGE", "DELETE", "DROP"]
        .iter()
        .any(|kw| statement.split_whitespace().any(|tok| tok.starts_with(kw)));
    let operation = if is_write {
        Operation::Mutate {
            mutation: Mutation::Statement {
                language: StatementLanguage::Cypher,
                statement: statement.to_string(),
                parameters: None,
            },
        }
    } else {
        Operation::Query {
            query: Query::Statement {
                language: StatementLanguage::Cypher,
                statement: statement.to_string(),
                parameters: None,
            },
        }
    };
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("cypher-conf".into()),
        context: RequestContext::default(),
        operation,
    };
    rushdb_server::protocol::ProtocolExecutor::new(node, None)
        .execute(envelope)
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}

#[expect(
    clippy::too_many_lines,
    reason = "one flat, auditable classification table is clearer than splitting it"
)]
fn corpus() -> Vec<Case> {
    vec![
        // ── supported core (must keep passing) ─────────────────────────────
        Case {
            name: "literal-expr",
            statement: "RETURN 1 + 1 AS n",
            unsupported: None,
        },
        Case {
            name: "create-node",
            statement: "CREATE (n:FACTION {name: 'Rebel'})",
            unsupported: None,
        },
        Case {
            name: "match-return",
            statement: "MATCH (n:FACTION) RETURN n",
            unsupported: None,
        },
        Case {
            name: "match-where-props",
            statement: "MATCH (n:FACTION {name: 'Rebel'}) RETURN n.name",
            unsupported: None,
        },
        Case {
            name: "match-rel-return",
            statement: "MATCH (a:FACTION)-[:OWNS]->(b:PLANET) RETURN b",
            unsupported: None,
        },
        Case {
            name: "order-limit",
            statement: "MATCH (n:FACTION) RETURN n ORDER BY n.name LIMIT 5",
            unsupported: None,
        },
        Case {
            name: "with-projection",
            statement: "MATCH (n:FACTION) WITH n.name AS name RETURN name",
            unsupported: None,
        },
        Case {
            name: "unwind-list",
            statement: "UNWIND [1,2,3] AS x RETURN x",
            unsupported: None,
        },
        Case {
            name: "aggregate-count",
            statement: "MATCH (n:FACTION) RETURN count(n) AS c",
            unsupported: None,
        },
        Case {
            name: "distinct-projection",
            statement: "MATCH (n:FACTION) RETURN DISTINCT n.name",
            unsupported: None,
        },
        Case {
            name: "optional-match",
            statement: "MATCH (n:FACTION) OPTIONAL MATCH (n)-[:OWNS]->(p) RETURN n",
            unsupported: None,
        },
        Case {
            name: "union",
            statement: "MATCH (n:FACTION) RETURN n.name UNION MATCH (p:PLANET) RETURN p.name",
            unsupported: None,
        },
        Case {
            name: "merge-node",
            statement: "MERGE (n:FACTION {name: 'Hutt'})",
            unsupported: None,
        },
        Case {
            name: "order-skip-limit",
            statement: "MATCH (n:FACTION) RETURN n.name AS name ORDER BY name DESC SKIP 1 LIMIT 2",
            unsupported: None,
        },
        Case {
            name: "list-literal",
            statement: "RETURN [1,2,3] AS xs",
            unsupported: None,
        },
        Case {
            name: "map-literal",
            statement: "RETURN {a: 1, b: 'x'} AS m",
            unsupported: None,
        },
        Case {
            name: "string-starts-with",
            statement: "MATCH (n:FACTION) WHERE n.name STARTS WITH 'R' RETURN n",
            unsupported: None,
        },
        Case {
            name: "collect-size",
            statement: "MATCH (n:FACTION) RETURN n.name AS name, size(collect(n)) AS c",
            unsupported: None,
        },
        // Declared ordered indexes: Neo4j DDL before the translator.
        Case {
            name: "index-ddl",
            statement: "CREATE INDEX FOR (n:FACTION) ON (n.name)",
            unsupported: None,
        },
        Case {
            name: "drop-index-ddl",
            statement: "DROP INDEX FACTION_name",
            unsupported: None,
        },
        // ── known unsupported (classified, not silent) ────────────────────
        Case {
            name: "apoc-fn",
            statement: "RETURN apoc.text.join(['a','b'], '-')",
            unsupported: Some("APOC functions are Neo4j-only, out of scope"),
        },
        Case {
            name: "constraint-ddl",
            statement: "CREATE CONSTRAINT FOR (n:FACTION) REQUIRE n.name IS UNIQUE",
            unsupported: Some("Cypher constraint DDL is not implemented"),
        },
        Case {
            name: "standalone-collect",
            statement: "RETURN collect([1,2]) AS c",
            unsupported: Some(
                "Grafeo adapter: collect is an aggregation over matched rows, not a standalone list constructor (GRAFEO-X001 Empty plan)",
            ),
        },
    ]
}

#[test]
fn opencypher_corpus_is_fully_classified_with_no_silent_skips() {
    let node = bootstrap();
    // Seed a graph for the pattern cases.
    for stmt in [
        "CREATE (a:FACTION {name: 'Rebel'})",
        "CREATE (a:FACTION {name: 'Empire'})",
        "CREATE (p:PLANET {name: 'Yavin'})",
        "MATCH (a:FACTION {name: 'Rebel'}), (p:PLANET {name: 'Yavin'}) CREATE (a)-[:OWNS]->(p)",
    ] {
        let r = run_cypher(&node, stmt);
        assert!(r.is_ok(), "seed failed: {stmt}: {r:?}");
    }
    for case in corpus() {
        let actual = run_cypher(&node, case.statement);
        match (case.unsupported, actual) {
            (None, Ok(())) => {} // supported and passed
            (None, Err(e)) => panic!(
                "cypher `{}` was classified supported but failed: {e} — fix or reclassify (no silent skips)",
                case.name
            ),
            (Some(reason), Err(e)) => {
                // Expected failure; the error must be a structured protocol error,
                // not a crash.
                assert!(e.contains("code:"), "unsupported `{}` must be a typed error, got: {e}", case.name);
                let _ = reason;
            }
            (Some(_reason), Ok(())) => panic!(
                "cypher `{}` was classified unsupported but succeeded — reclassify (no silent skips)",
                case.name
            ),
        }
    }
}
