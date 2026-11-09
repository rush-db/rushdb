//! Data migrations, run once at engine start before the node serves.
//!
//! A migration commits its rewrites through the normal write path (WAL,
//! replication command, derived state), in bounded batches, and is
//! idempotent. A store with nothing in the old form is detected with one
//! seek per tenant and is not written to at all, so a start of an up-to-date
//! store changes nothing. While a tenant is being rewritten it carries a
//! progress row (`migration/<name>`), written with the first batch and
//! deleted with the last, so a start that stopped part way (when the rows
//! already rewritten come first in key order and would look migrated)
//! resumes over the whole tenant.
//!
//! Migrations, in order:
//!
//! * `0001-binary-vector-bindings` (3.0.7): embedding binding rows were
//!   serialized as JSON up to 3.0.6; 3.0.7 stores the 17-byte value
//!   [`rushdb_types::occurrence::EmbeddingBinding::encode_value`] and its
//!   readers refuse the JSON form. Every JSON binding row is rewritten in
//!   place (same key, same binding).

use std::ops::Bound;

use rushdb_storage::{CommitOutcome, ScanBounds, ScanProjection, Store};
use rushdb_types::command::{CommitBatch, IsolationMode, Mutation, COMMAND_VERSION};
use rushdb_types::ids::TenantId;
use rushdb_types::keys::{self, TENANT_NS};
use rushdb_types::occurrence::{BindingValue, EmbeddingBinding};

/// Rows rewritten per commit.
const BATCH_ROWS: usize = 4_096;

const BINARY_VECTOR_BINDINGS: &str = "0001-binary-vector-bindings";

/// What a start's migrations did.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct MigrationReport {
    /// Tenants a migration ran for (their marker was absent).
    pub tenants: u64,
    /// Rows rewritten.
    pub rows: u64,
}

/// Run every pending migration over every tenant of `store`.
///
/// # Errors
/// Returns a message when a read, a decode of an old-form row, or a commit
/// fails; the node then refuses to start rather than serve rows its readers
/// cannot decode.
pub(crate) fn run(store: &Store) -> Result<MigrationReport, String> {
    let mut report = MigrationReport::default();
    let mut next = Some(0u32);
    while let Some(from) = next {
        let Some(tenant) = next_tenant(store, from)? else {
            break;
        };
        next = tenant.0.checked_add(1);
        if binary_vector_bindings(store, tenant, &mut report)? {
            report.tenants += 1;
        }
    }
    if report.tenants > 0 {
        tracing::info!(
            migration = BINARY_VECTOR_BINDINGS,
            tenants = report.tenants,
            rows = report.rows,
            "data migration applied"
        );
    }
    Ok(report)
}

/// The first tenant with any row at or after ordinal `from`.
fn next_tenant(store: &Store, from: u32) -> Result<Option<TenantId>, String> {
    let snap = store.read_view().map_err(|e| e.to_string())?;
    let bounds = ScanBounds {
        start: Bound::Included(keys::tenant_prefix(TenantId(from)).to_vec()),
        end: Bound::Excluded(vec![TENANT_NS + 1]),
    };
    let mut cursor = snap
        .scan(bounds, ScanProjection::Keys)
        .map_err(|e| e.to_string())?;
    let batch = cursor.next_batch(1, 1 << 16).map_err(|e| e.to_string())?;
    Ok(batch
        .rows
        .first()
        .and_then(|row| keys::parse_tenant(&row.key)))
}

fn marker_key(tenant: TenantId, name: &str) -> Vec<u8> {
    keys::key(tenant, &[b"migration", name.as_bytes()])
}

/// Rewrite `tenant`'s JSON binding rows in the binary form. `false` when it
/// had none (nothing is written then).
fn binary_vector_bindings(
    store: &Store,
    tenant: TenantId,
    report: &mut MigrationReport,
) -> Result<bool, String> {
    let progress = marker_key(tenant, BINARY_VECTOR_BINDINGS);
    let snap = store.read_view().map_err(|e| e.to_string())?;
    let resuming = snap.get(&progress).map_err(|e| e.to_string())?.is_some();
    let prefix = keys::key(tenant, &[keys::P_EMB_BINDING, b""]);
    let mut cursor = snap
        .scan(ScanBounds::prefix(&prefix), ScanProjection::KeyValue)
        .map_err(|e| e.to_string())?;
    // An earlier release wrote every binding row as JSON, and a migration
    // rewrites rows in key order: unless one was interrupted, the first row
    // tells the form of them all.
    let first = cursor.next_batch(1, 8 << 20).map_err(|e| e.to_string())?;
    let first_is_old = first
        .rows
        .first()
        .and_then(|row| row.value.as_deref())
        .is_some_and(|value| BindingValue::decode(value).is_none());
    if !resuming && !first_is_old {
        return Ok(false);
    }
    let mut pending: Vec<Mutation> = Vec::new();
    let mut rewritten = 0u64;
    let mut batch = first;
    loop {
        for row in batch.rows {
            let Some(value) = row.value else {
                continue;
            };
            if BindingValue::decode(&value).is_some() {
                continue;
            }
            let binding: EmbeddingBinding = serde_json::from_slice(&value).map_err(|e| {
                format!(
                    "migration {BINARY_VECTOR_BINDINGS}: binding row {} is neither binary nor JSON: {e}",
                    keys::describe_key(&row.key)
                )
            })?;
            pending.push(Mutation::PutRawKey {
                key: row.key,
                value: binding.encode_value().to_vec(),
            });
            if pending.len() >= BATCH_ROWS {
                pending.push(Mutation::PutRawKey {
                    key: progress.clone(),
                    value: b"rewriting".to_vec(),
                });
                rewritten += commit(store, tenant, std::mem::take(&mut pending))? - 1;
            }
        }
        if batch.done {
            break;
        }
        batch = cursor
            .next_batch(BATCH_ROWS, 8 << 20)
            .map_err(|e| e.to_string())?;
    }
    rewritten += pending.len() as u64;
    pending.push(Mutation::DeleteRawKey { key: progress });
    commit(store, tenant, pending)?;
    report.rows += rewritten;
    Ok(true)
}

/// Commit `mutations` for `tenant`; the number committed.
fn commit(store: &Store, tenant: TenantId, mutations: Vec<Mutation>) -> Result<u64, String> {
    let count = mutations.len() as u64;
    let outcome = store
        .commit_for_tenant(&tenant, uuid::Uuid::now_v7(), move |_ci, _txn| {
            let batch = CommitBatch {
                command_version: COMMAND_VERSION,
                tenant,
                transaction_id: String::new(),
                commit_key: uuid::Uuid::nil(),
                start_index: 0,
                isolation: IsolationMode::Snapshot,
                preconditions: vec![],
                mutations,
            };
            Ok((batch, ()))
        })
        .map_err(|e| format!("migration {BINARY_VECTOR_BINDINGS}: commit failed: {e}"))?;
    match outcome {
        CommitOutcome::Committed { .. } => Ok(count),
        CommitOutcome::Conflicted(reason) => Err(format!(
            "migration {BINARY_VECTOR_BINDINGS}: commit not applied: {reason}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Node;
    use crate::protocol::ProtocolExecutor;
    use rushdb_protocol::{RequestContext, RequestEnvelope, PROTOCOL_VERSION};
    use serde_json::{json, Value};

    /// The bytes an earlier release stored for `binding`: its JSON form.
    fn json_row(binding: &EmbeddingBinding) -> Vec<u8> {
        serde_json::to_vec(binding).unwrap()
    }

    /// Rewrite every binding row of `tenant` in its JSON (≤ 3.0.6) form, as
    /// a store written by an earlier release holds it.
    fn downgrade(store: &Store, tenant: TenantId) -> usize {
        let snap = store.read_view().unwrap();
        let rows = snap
            .range_prefix(&keys::key(tenant, &[keys::P_EMB_BINDING, b""]))
            .unwrap();
        let mutations: Vec<Mutation> = rows
            .iter()
            .map(|(key, value)| Mutation::PutRawKey {
                key: key.clone(),
                value: json_row(&EmbeddingBinding::decode(key, value).unwrap()),
            })
            .collect();
        let count = rows.len();
        commit(store, tenant, mutations).unwrap();
        count
    }

    fn binding_rows(store: &Store, tenant: TenantId) -> Vec<(Vec<u8>, Vec<u8>)> {
        store
            .read_view()
            .unwrap()
            .range_prefix(&keys::key(tenant, &[keys::P_EMB_BINDING, b""]))
            .unwrap()
    }

    fn execute(node: &Node, operation: Value) -> Result<Value, String> {
        ProtocolExecutor::new(node, None)
            .execute(RequestEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: None,
                context: RequestContext::default(),
                operation: serde_json::from_value(operation).unwrap(),
            })
            .map(|done| serde_json::to_value(done.response).unwrap())
            .map_err(|error| format!("{error:?}"))
    }

    fn nearest(node: &Node) -> Result<Vec<u64>, String> {
        let response = execute(
            node,
            json!({"kind": "query", "query": {"kind": "vector", "query": {
                "labels": ["Doc"], "property": "embedding", "index": "doc-embedding",
                "vector": [1.0, 0.0, 0.0], "limit": 5, "mode": "exact"}}}),
        )?;
        Ok(response["records"]
            .as_array()
            .ok_or_else(|| format!("no records: {response}"))?
            .iter()
            .map(|record| record["properties"]["n"].as_u64().unwrap())
            .collect())
    }

    #[test]
    fn a_store_with_json_bindings_is_migrated_once_at_start() {
        let dir = rushdb_storage::TempPath::dir("binding-migration");
        let config = || crate::ServerConfig {
            data_dir: Some(dir.to_path_buf()),
            ..Default::default()
        };
        let tenant = TenantId::single();
        let expected = {
            let node = Node::bootstrap(&mut config()).unwrap();
            execute(
                &node,
                json!({"kind": "index", "index": {"kind": "createVector", "definition": {
                    "name": "doc-embedding", "label": "Doc", "property": "embedding",
                    "dimensions": 3, "similarity": "cosine"}}}),
            )
            .unwrap();
            let docs: Vec<Value> = (0..40u32)
                .map(|n| {
                    let angle = f64::from(n) / 10.0;
                    json!({"n": n, "$vectors": [{"propertyName": "embedding",
                        "vector": [angle.cos(), angle.sin(), 0.5], "index": "doc-embedding"}]})
                })
                .collect();
            execute(
                &node,
                json!({"kind": "ingest", "ingest": {"kind": "documents", "label": "Doc",
                    "documents": docs, "options": {}}}),
            )
            .unwrap();
            let expected = nearest(&node).unwrap();
            assert_eq!(downgrade(&node.store(), tenant), 40);
            // What a 3.0.7 reader makes of an unmigrated store.
            assert!(nearest(&node).is_err(), "JSON binding rows are refused");
            expected
        };
        // The next start rewrites them before serving.
        let node = Node::bootstrap(&mut config()).unwrap();
        assert_eq!(nearest(&node).unwrap(), expected);
        let rows = binding_rows(&node.store(), tenant);
        assert_eq!(rows.len(), 40);
        assert!(rows
            .iter()
            .all(|(_, value)| BindingValue::decode(value).is_some()));
        // Once: a migrated store is recognized by its first row and not
        // written again; a later start commits nothing.
        let version = node.store().committed_version();
        assert_eq!(run(&node.store()).unwrap(), MigrationReport::default());
        assert_eq!(node.store().committed_version(), version);
        drop(node);
        let node = Node::bootstrap(&mut config()).unwrap();
        assert_eq!(nearest(&node).unwrap(), expected);
        drop(node);
        let _ = std::fs::remove_dir_all(dir);
    }

    fn binding(space: u64, owner: u64) -> EmbeddingBinding {
        EmbeddingBinding {
            space_id: space,
            owner_kind: rushdb_types::ids::OwnerKind::Record,
            owner_ord: owner,
            pid: 4,
            elem: 0,
            vector_slot: owner * 3,
            source_hash: owner ^ 0xfeed,
            source_hash_tracked: owner.is_multiple_of(2),
            status: rushdb_types::occurrence::BindingStatus::Ready,
        }
    }

    fn put(tenant: TenantId, b: &EmbeddingBinding, value: Vec<u8>) -> Mutation {
        Mutation::PutRawKey {
            key: keys::emb_binding_key(
                tenant,
                b.space_id,
                b.owner_kind,
                b.owner_ord,
                b.pid,
                b.elem,
            ),
            value,
        }
    }

    /// An in-memory store wired as a node wires it: LMPG's expander installed.
    fn lmpg_store() -> std::sync::Arc<Store> {
        let store = std::sync::Arc::new(Store::new_in_memory());
        let _wire = rushdb_lmpg::Lmpg::new(std::sync::Arc::clone(&store), TenantId(1));
        store
    }

    #[test]
    fn every_tenant_is_migrated_across_batches() {
        let store = lmpg_store();
        let (a, b) = (TenantId(1), TenantId(7));
        // Tenant a: more JSON rows than one batch, plus rows already binary.
        let mut rows: Vec<Mutation> = (0..(BATCH_ROWS as u64 + 900))
            .map(|owner| put(a, &binding(2, owner), json_row(&binding(2, owner))))
            .collect();
        rows.extend((90_000..90_010).map(|owner| {
            put(
                a,
                &binding(2, owner),
                binding(2, owner).encode_value().to_vec(),
            )
        }));
        commit(&store, a, rows).unwrap();
        commit(
            &store,
            b,
            (0..30)
                .map(|owner| put(b, &binding(5, owner), json_row(&binding(5, owner))))
                .collect(),
        )
        .unwrap();
        assert_eq!(
            run(&store).unwrap(),
            MigrationReport {
                tenants: 2,
                rows: BATCH_ROWS as u64 + 900 + 30,
            }
        );
        for (tenant, space, owners) in [(a, 2, BATCH_ROWS as u64 + 900), (b, 5, 30)] {
            for owner in 0..owners {
                let key = keys::emb_binding_key(
                    tenant,
                    space,
                    rushdb_types::ids::OwnerKind::Record,
                    owner,
                    4,
                    0,
                );
                let stored = store.read_view().unwrap().get(&key).unwrap().unwrap();
                assert_eq!(
                    EmbeddingBinding::decode(&key, &stored),
                    Some(binding(space, owner))
                );
            }
        }
        // Migrated: recognized by the first row, nothing written again.
        let version = store.committed_version();
        assert_eq!(run(&store).unwrap(), MigrationReport::default());
        assert_eq!(store.committed_version(), version);
    }

    #[test]
    fn an_interrupted_migration_finishes_at_the_next_start() {
        // Its first rows are rewritten, the rest still JSON, and the progress
        // row says so.
        let store = lmpg_store();
        let d = TenantId(8);
        let mut rows: Vec<Mutation> = (0..40)
            .map(|owner| {
                let value = if owner < 20 {
                    binding(3, owner).encode_value().to_vec()
                } else {
                    json_row(&binding(3, owner))
                };
                put(d, &binding(3, owner), value)
            })
            .collect();
        rows.push(Mutation::PutRawKey {
            key: marker_key(d, BINARY_VECTOR_BINDINGS),
            value: b"rewriting".to_vec(),
        });
        commit(&store, d, rows).unwrap();
        assert_eq!(
            run(&store).unwrap(),
            MigrationReport {
                tenants: 1,
                rows: 20
            }
        );
        let snap = store.read_view().unwrap();
        assert!(snap
            .get(&marker_key(d, BINARY_VECTOR_BINDINGS))
            .unwrap()
            .is_none());
        assert!(binding_rows(&store, d)
            .iter()
            .all(|(_, value)| BindingValue::decode(value).is_some()));
    }

    #[test]
    fn a_row_in_neither_form_stops_the_start() {
        let store = lmpg_store();
        let c = TenantId(9);
        commit(&store, c, vec![put(c, &binding(1, 1), b"garbage".to_vec())]).unwrap();
        let error = run(&store).unwrap_err();
        assert!(error.contains("neither binary nor JSON"), "{error}");
    }
}
