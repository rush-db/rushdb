//! Gremlin bytecode (`GraphSON` 3) runs as the equivalent Gremlin traversal:
//! writes through the mutation path, reads through the query path.

use super::*;
use rushdb_protocol::{RequestContext, RequestEnvelope};

fn execute(node: &Node, operation: Operation) -> Result<Response, ProtocolError> {
    ProtocolExecutor::new(node, None)
        .execute(RequestEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: None,
            context: RequestContext::default(),
            operation,
        })
        .map(|envelope| envelope.response)
}

#[test]
fn bytecode_reads_and_writes_like_gremlin_text() {
    let mut cfg = super::tests::config(false);
    cfg.languages.gremlin = true;
    let node = Node::bootstrap(&mut cfg).unwrap();
    for (name, age) in [("marko", 29), ("vadas", 27), ("josh", 32)] {
        // gremlin-python: g.addV('person').property('name', n).property('age', a)
        let written = execute(
            &node,
            Operation::Mutate {
                mutation: Mutation::GremlinBytecode {
                    bytecode: serde_json::json!({"@type": "g:Bytecode", "@value": {"step": [
                        ["addV", "person"],
                        ["property", "name", name],
                        ["property", "age", {"@type": "g:Int32", "@value": age}]
                    ]}}),
                    parameters: None,
                },
            },
        )
        .unwrap();
        assert!(matches!(
            written,
            Response::StatementMutation { affected: 1, .. }
        ));
    }
    // g.V().hasLabel('person').has('age', P.gt(28)).values('name')
    let Response::Rows { rows, .. } = execute(
        &node,
        Operation::Query {
            query: Query::GremlinBytecode {
                bytecode: serde_json::json!({"step": [
                    ["V"],
                    ["hasLabel", "person"],
                    ["has", "age", {"@type": "g:P", "@value": {"predicate": "gt", "value": 28}}],
                    ["values", "name"]
                ]}),
                parameters: None,
            },
        },
    )
    .unwrap() else {
        panic!("rows")
    };
    let mut names: Vec<String> = rows
        .iter()
        .flat_map(|row| row.as_object().unwrap().values())
        .map(|value| value.as_str().unwrap().to_owned())
        .collect();
    names.sort();
    assert_eq!(names, vec!["josh", "marko"]);

    let refused = execute(
        &node,
        Operation::Query {
            query: Query::GremlinBytecode {
                bytecode: serde_json::json!({"step": [["V"], ["map", {"@type": "g:Lambda", "@value": {}}]]}),
                parameters: None,
            },
        },
    )
    .unwrap_err();
    assert_eq!(refused.code, ErrorCode::Unsupported);
}
