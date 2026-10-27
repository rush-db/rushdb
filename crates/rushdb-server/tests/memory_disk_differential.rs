//! 3A — memory-vs-disk differential suite. The same operation script must
//! produce the identical committed state on a memory node and a disk-backed
//! node, survive reopen on disk, and replay historical states identically.
//! This is the direct proof that redb mode shares memory-mode read semantics.

use rushdb_protocol::{Mutation, Operation, RequestContext, RequestEnvelope, Response};
use rushdb_server::{Node, ServerConfig};

fn disk_node(dir: &std::path::Path) -> Node {
    let mut cfg = ServerConfig {
        data_dir: Some(dir.to_path_buf()),
        ..Default::default()
    };
    Node::bootstrap(&mut cfg).unwrap()
}

fn memory_node() -> Node {
    let mut cfg = ServerConfig::default();
    Node::bootstrap(&mut cfg).unwrap()
}

fn put(node: &Node, _id: &str, label: &str, text: &str) {
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("diff-put".into()),
        context: RequestContext::default(),
        operation: Operation::Mutate {
            mutation: Mutation::PutRecord {
                mode: rushdb_protocol::PutMode::Create,
                id: None,
                label: Some(label.to_string()),
                properties: serde_json::json!({ "text": text }),
                vectors: vec![],
                options: serde_json::json!({}),
            },
        },
    };
    rushdb_server::protocol::ProtocolExecutor::new(node, None)
        .execute(envelope)
        .unwrap();
}

fn delete(node: &Node, id: &str) {
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("diff-del".into()),
        context: RequestContext::default(),
        operation: Operation::Mutate {
            mutation: Mutation::DeleteRecord { id: id.to_string() },
        },
    };
    rushdb_server::protocol::ProtocolExecutor::new(node, None)
        .execute(envelope)
        .unwrap();
}

/// Normalize exported JSONL for comparison: drop the auto-generated record id
/// so memory and disk modes compare on logical content.
fn normalize(jsonl: Vec<u8>) -> Vec<String> {
    let mut out: Vec<String> = String::from_utf8(jsonl)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut v: serde_json::Value = serde_json::from_str(l).unwrap();
            if let Some(o) = v.as_object_mut() {
                o.remove("id");
            }
            serde_json::to_string(&v).unwrap()
        })
        .collect();
    out.sort();
    out
}

/// Normalize all tenants' JSONL into one comparable set.
fn normalize_all(jsonl: Vec<Vec<u8>>) -> Vec<String> {
    let mut out = Vec::new();
    for tenant in jsonl {
        out.extend(normalize(tenant));
    }
    out.sort();
    out
}

fn last_id(node: &Node, label: &str) -> String {
    use rushdb_protocol::{Query, QueryDocument};
    let doc = QueryDocument {
        labels: vec![label.to_string()],
        limit: Some(1),
        ..Default::default()
    };
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("diff-last".into()),
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
    records[0]
        .as_object()
        .and_then(|o| o.get("id"))
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string()
}

fn assert_same(mem: &Node, disk: &Node, context: &str) {
    let (_, m) = snapshot_jsonl(mem);
    let (_, d) = snapshot_jsonl(disk);
    assert_eq!(normalize_all(m), normalize_all(d), "{context}");
}

/// Head-state snapshot as the per-tenant JSONL payloads (logical content,
/// independent of commit indices). Returns the snapshot's commit index too.
fn snapshot_jsonl(node: &Node) -> (u64, Vec<Vec<u8>>) {
    let mut buf = Vec::new();
    let ci = node.export_snapshot(&mut buf, &[]).unwrap();
    let (_, chunks) = rushdb_storage::wal::read_snapshot(&mut buf.as_slice()).unwrap();
    (ci, chunks.into_iter().map(|c| c.jsonl).collect())
}

/// Point-in-time snapshot at a commit index.
fn snapshot_jsonl_at(node: &Node, ci: u64) -> Vec<Vec<u8>> {
    let mut buf = Vec::new();
    node.export_snapshot_at(&mut buf, ci, &[]).unwrap();
    let (_, chunks) = rushdb_storage::wal::read_snapshot(&mut buf.as_slice()).unwrap();
    chunks.into_iter().map(|c| c.jsonl).collect()
}

#[test]
fn disk_replay_matches_memory_and_survives_reopen_and_history() {
    let dir = rushdb_storage::TempPath::dir("diff");
    let memory = memory_node();
    let disk = disk_node(&dir);

    // Operation script: create, create, delete, create, delete+recreate.
    let mut steps: Vec<(u64, Vec<Vec<u8>>)> = Vec::new();
    for (id, label, text) in [
        ("00000000-0000-0000-0000-0000000000a1", "A", "one"),
        ("00000000-0000-0000-0000-0000000000b1", "B", "two"),
        ("00000000-0000-0000-0000-0000000000c1", "C", "three"),
        ("00000000-0000-0000-0000-0000000000d1", "D", "four"),
    ] {
        put(&memory, id, label, text);
        put(&disk, id, label, text);
        assert_same(
            &memory,
            &disk,
            &format!("state diverged after put {label}={text}"),
        );
        let (ci, mem_snap) = snapshot_jsonl(&memory);
        steps.push((ci, mem_snap));
    }
    // Delete the most recent record on both.
    let doomed_mem = last_id(&memory, "D");
    let doomed_disk = last_id(&disk, "D");
    delete(&memory, &doomed_mem);
    delete(&disk, &doomed_disk);
    assert_same(&memory, &disk, "state diverged after delete");
    let (ci2, mem2) = snapshot_jsonl(&memory);
    steps.push((ci2, mem2));
    // Recreate under the same label.
    put(
        &memory,
        "00000000-0000-0000-0000-0000000000d2",
        "D",
        "four-again",
    );
    put(
        &disk,
        "00000000-0000-0000-0000-0000000000d2",
        "D",
        "four-again",
    );
    assert_same(&memory, &disk, "state diverged after recreate");
    let (ci3, mem3) = snapshot_jsonl(&memory);
    steps.push((ci3, mem3));

    drop(disk);

    // Reopen the same directory and compare head + every captured historical CI.
    let reopened = disk_node(&dir);
    let (_, reopened_snap) = snapshot_jsonl(&reopened);
    let (_, memory_snap) = snapshot_jsonl(&memory);
    assert_eq!(
        normalize_all(reopened_snap),
        normalize_all(memory_snap),
        "reopened disk head diverged from memory"
    );
    for (ci, mem_at_step) in &steps {
        let disk_at = snapshot_jsonl_at(&reopened, *ci);
        assert_eq!(
            normalize_all(disk_at),
            normalize_all(mem_at_step.clone()),
            "historical read at CI {ci} diverged from memory at the same step"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn transactions_behave_identically_on_memory_and_disk() {
    use rushdb_protocol::{TransactionMutation, TransactionOperation};
    let dir = rushdb_storage::TempPath::dir("diff-tx");
    let memory = memory_node();
    let disk = disk_node(&dir);

    for node in [&memory, &disk] {
        let begin = RequestEnvelope {
            protocol_version: 1,
            request_id: Some("diff-tx-begin".into()),
            context: RequestContext::default(),
            operation: Operation::Transaction {
                transaction: TransactionOperation::Begin {
                    ttl_ms: None,
                    autocommit: false,
                },
            },
        };
        let res = rushdb_server::protocol::ProtocolExecutor::new(node, None)
            .execute(begin)
            .unwrap();
        let Response::TransactionBegun { transaction_id } = res.response else {
            panic!("expected begun tx");
        };
        let stage = RequestEnvelope {
            protocol_version: 1,
            request_id: Some("diff-tx-stage".into()),
            context: RequestContext::default(),
            operation: Operation::Transaction {
                transaction: TransactionOperation::Stage {
                    transaction_id: transaction_id.clone(),
                    mutation: TransactionMutation::CreateRecord {
                        label: "T".to_string(),
                        id: Some("00000000-0000-0000-0000-0000000000e1".to_string()),
                        properties: serde_json::json!({ "text": "txd" }),
                        options: serde_json::json!({}),
                    },
                },
            },
        };
        rushdb_server::protocol::ProtocolExecutor::new(node, None)
            .execute(stage)
            .unwrap();
        // Staged writes are invisible before commit on both modes.
        assert_eq!(
            normalize_all(snapshot_jsonl(node).1),
            Vec::<String>::new(),
            "staged write leaked before commit"
        );
        let commit = RequestEnvelope {
            protocol_version: 1,
            request_id: Some("diff-tx-commit".into()),
            context: RequestContext::default(),
            operation: Operation::Transaction {
                transaction: TransactionOperation::Commit { transaction_id },
            },
        };
        rushdb_server::protocol::ProtocolExecutor::new(node, None)
            .execute(commit)
            .unwrap();
    }

    assert_same(&memory, &disk, "transaction commit diverged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "single linear operation/reader; splitting it would scatter shared borrows and hurt auditability"
)]
fn tenant_boundaries_hold_on_both_modes() {
    use rushdb_protocol::{Query, QueryDocument};
    let dir = rushdb_storage::TempPath::dir("diff-tenant");
    let mut mcfg = ServerConfig {
        multitenant: true,
        ..Default::default()
    };
    let mut dcfg = ServerConfig {
        multitenant: true,
        data_dir: Some(dir.to_path_buf()),
        ..Default::default()
    };
    let memory = Node::bootstrap(&mut mcfg).unwrap();
    let disk = Node::bootstrap(&mut dcfg).unwrap();

    let put_scoped = |node: &Node, scope: &str, _id: &str, text: &str| {
        let envelope = RequestEnvelope {
            protocol_version: 1,
            request_id: Some("diff-tenant-put".into()),
            context: RequestContext {
                scope: Some(scope.to_string()),
                deadline_ms: None,
                ..Default::default()
            },
            operation: Operation::Mutate {
                mutation: Mutation::PutRecord {
                    mode: rushdb_protocol::PutMode::Create,
                    id: None,
                    label: Some("T".to_string()),
                    properties: serde_json::json!({ "text": text }),
                    vectors: vec![],
                    options: serde_json::json!({}),
                },
            },
        };
        rushdb_server::protocol::ProtocolExecutor::new(node, Some(scope.to_string()))
            .execute(envelope)
            .unwrap();
    };

    put_scoped(
        &memory,
        "tenant-a",
        "00000000-0000-0000-0000-0000000000f1",
        "a-data",
    );
    put_scoped(
        &memory,
        "tenant-b",
        "00000000-0000-0000-0000-0000000000f2",
        "b-data",
    );
    put_scoped(
        &disk,
        "tenant-a",
        "00000000-0000-0000-0000-0000000000f1",
        "a-data",
    );
    put_scoped(
        &disk,
        "tenant-b",
        "00000000-0000-0000-0000-0000000000f2",
        "b-data",
    );

    // Full per-tenant state must be identical between modes.
    let mut s_memory = Vec::new();
    let mut s_disk = Vec::new();
    for scope in ["tenant-a", "tenant-b"] {
        let envelope = RequestEnvelope {
            protocol_version: 1,
            request_id: Some("diff-tenant-read".into()),
            context: RequestContext {
                scope: Some(scope.to_string()),
                deadline_ms: None,
                ..Default::default()
            },
            operation: Operation::Query {
                query: Query::Records {
                    query: QueryDocument {
                        labels: vec!["T".to_string()],
                        ..Default::default()
                    },
                },
            },
        };
        let m = rushdb_server::protocol::ProtocolExecutor::new(&memory, Some(scope.to_string()))
            .execute(envelope.clone())
            .unwrap();
        let d = rushdb_server::protocol::ProtocolExecutor::new(&disk, Some(scope.to_string()))
            .execute(envelope)
            .unwrap();
        let Response::Records { records: mr, .. } = m.response else {
            panic!()
        };
        let Response::Records { records: dr, .. } = d.response else {
            panic!()
        };
        assert_eq!(mr.len(), dr.len(), "tenant {scope} record count diverged");
        let strip = |records: &[serde_json::Value]| -> Vec<String> {
            let mut out = records
                .iter()
                .map(|r| {
                    let mut v = r.clone();
                    if let Some(o) = v.as_object_mut() {
                        o.remove("id");
                    }
                    serde_json::to_string(&v).unwrap()
                })
                .collect::<Vec<_>>();
            out.sort();
            out
        };
        s_memory.push(serde_json::to_string(&strip(&mr)).unwrap());
        s_disk.push(serde_json::to_string(&strip(&dr)).unwrap());
    }
    assert_eq!(s_memory, s_disk);
    let _ = std::fs::remove_dir_all(&dir);
}
