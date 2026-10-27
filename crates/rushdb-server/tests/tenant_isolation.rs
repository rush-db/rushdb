//! Cross-tenant isolation (5B): fail-closed, scope-keyed multitenancy must not
//! leak data, backups, or transactions across the opaque scope boundary. These
//! tests drive the canonical protocol executor under explicit request scopes.

use rushdb_authz::{AuthorizationMode, DataAccessPolicy};
use rushdb_protocol::{
    Describe, ErrorCode, Mutation, Operation, PutMode, Query, QueryDocument, RequestContext,
    RequestEnvelope, Response, TransactionMutation, TransactionOperation, TransactionStatus,
};
use rushdb_server::{Node, RequestScope, ServerConfig};

fn put_mode(
    node: &Node,
    scope: &str,
    mode: PutMode,
    id: Option<&str>,
    label: Option<&str>,
    properties: serde_json::Value,
) -> Result<serde_json::Value, rushdb_protocol::ProtocolError> {
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("isolation-put-mode".into()),
        context: RequestContext {
            scope: Some(scope.to_string()),
            deadline_ms: None,
            ..Default::default()
        },
        operation: Operation::Mutate {
            mutation: Mutation::PutRecord {
                mode,
                id: id.map(str::to_string),
                label: label.map(str::to_string),
                properties,
                vectors: vec![],
                options: serde_json::json!({}),
            },
        },
    };
    let response = rushdb_server::protocol::ProtocolExecutor::new(node, Some(scope.to_string()))
        .execute(envelope)?;
    let Response::Record { record } = response.response else {
        panic!("expected record response, got {:?}", response.response);
    };
    Ok(record)
}

fn multitenant() -> Node {
    let mut cfg = ServerConfig {
        multitenant: true,
        ..Default::default()
    };
    Node::bootstrap(&mut cfg).unwrap()
}

fn put(node: &Node, scope: &str, label: &str, text: &str) {
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("isolation-put".into()),
        context: RequestContext {
            scope: Some(scope.to_string()),
            deadline_ms: None,
            ..Default::default()
        },
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
    rushdb_server::protocol::ProtocolExecutor::new(node, Some(scope.to_string()))
        .execute(envelope)
        .unwrap();
}

fn find_text(node: &Node, scope: &str) -> Vec<serde_json::Value> {
    let doc = QueryDocument {
        labels: vec!["NOTE".to_string()],
        limit: Some(100),
        ..Default::default()
    };
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("isolation-find".into()),
        context: RequestContext {
            scope: Some(scope.to_string()),
            deadline_ms: None,
            ..Default::default()
        },
        operation: Operation::Query {
            query: Query::Records { query: doc },
        },
    };
    let res = rushdb_server::protocol::ProtocolExecutor::new(node, Some(scope.to_string()))
        .execute(envelope)
        .unwrap();
    let Response::Records { records, .. } = res.response else {
        panic!("expected records, got {:?}", res.response);
    };
    records
        .into_iter()
        .filter_map(|r| {
            r.as_object()
                .and_then(|o| o.get("properties"))
                .and_then(|p| p.as_object())
                .and_then(|p| p.get("text").cloned())
        })
        .collect()
}

#[test]
fn missing_scope_is_rejected_in_multitenant_mode() {
    let node = multitenant();
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("no-scope".into()),
        context: RequestContext::default(),
        operation: Operation::Query {
            query: Query::Records {
                query: QueryDocument::default(),
            },
        },
    };
    let err = rushdb_server::protocol::ProtocolExecutor::new(&node, None)
        .execute(envelope)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::MissingScope);

    // single-tenant mode: a scope is rejected instead (scope must be omitted).
    let mut cfg = ServerConfig::default();
    let single = Node::bootstrap(&mut cfg).unwrap();
    let scoped_env = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("scoped".into()),
        context: RequestContext {
            scope: Some("tenant-a".to_string()),
            deadline_ms: None,
            ..Default::default()
        },
        operation: Operation::Query {
            query: Query::Records {
                query: QueryDocument::default(),
            },
        },
    };
    // Single-tenant mode never authenticates a transport scope either.
    let err = rushdb_server::protocol::ProtocolExecutor::new(&single, None)
        .execute(scoped_env)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::ScopeMismatch);
}

#[test]
fn tenant_a_writes_are_invisible_to_tenant_b() {
    let node = multitenant();
    put(&node, "tenant-a", "NOTE", "safe");
    put(&node, "tenant-b", "NOTE", "other");
    let a = find_text(&node, "tenant-a");
    assert_eq!(a, vec![serde_json::json!("safe")]);
    let b = find_text(&node, "tenant-b");
    assert_eq!(b, vec![serde_json::json!("other")]);
}

#[test]
fn direct_record_lookup_cannot_cross_tenant_scope() {
    let node = multitenant();
    let id = RequestScope::new(&node, Some("tenant-a".to_string()))
        .create_record(
            "NOTE",
            &serde_json::json!({ "text": "private-a" }),
            &[],
            &serde_json::json!({}),
        )
        .unwrap()
        .get("id")
        .and_then(|value| value.as_str())
        .expect("created record id")
        .to_owned();
    let response =
        rushdb_server::protocol::ProtocolExecutor::new(&node, Some("tenant-b".to_string()))
            .execute(RequestEnvelope {
                protocol_version: 1,
                request_id: Some("cross-tenant-record-id".into()),
                context: RequestContext {
                    scope: Some("tenant-b".into()),
                    deadline_ms: None,
                    ..Default::default()
                },
                operation: Operation::Query {
                    query: Query::Record { id },
                },
            })
            .expect_err("foreign record id must not be visible");
    assert_eq!(response.code, ErrorCode::NotFound);
    assert!(response.message.contains("record not found"));
}

#[test]
fn backup_is_scoped_to_requested_tenants() {
    let node = multitenant();
    put(&node, "tenant-a", "NOTE", "a-secret");
    put(&node, "tenant-b", "NOTE", "b-secret");
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("backup".into()),
        context: RequestContext {
            scope: Some("tenant-a".to_string()),
            deadline_ms: None,
            ..Default::default()
        },
        operation: Operation::Backup {
            request: rushdb_protocol::BackupRequest {
                scopes: vec!["tenant-a".to_string()],
            },
        },
    };
    let res = rushdb_server::protocol::ProtocolExecutor::new(&node, Some("tenant-a".to_string()))
        .execute(envelope)
        .unwrap();
    let Response::Backup { tenants, .. } = res.response else {
        panic!("expected backup response");
    };
    assert_eq!(tenants.len(), 1, "backup must contain only tenant-a");
    let jsonl = tenants[0].jsonl.as_str();
    assert!(jsonl.contains("a-secret"));
    assert!(!jsonl.contains("b-secret"), "tenant-b must not leak");
}

#[test]
fn tenant_scoped_backup_cannot_select_or_enumerate_other_tenants() {
    let node = multitenant();
    put(&node, "tenant-a", "NOTE", "a-secret");
    put(&node, "tenant-b", "NOTE", "b-secret");

    // An empty list is tenant A's own backup, not an all-tenant export.
    let own = rushdb_server::protocol::ProtocolExecutor::new(&node, Some("tenant-a".to_string()))
        .execute(RequestEnvelope {
            protocol_version: 1,
            request_id: Some("backup-default-scope".into()),
            context: RequestContext {
                scope: Some("tenant-a".into()),
                deadline_ms: None,
                ..Default::default()
            },
            operation: Operation::Backup {
                request: rushdb_protocol::BackupRequest { scopes: vec![] },
            },
        })
        .expect("tenant A's own backup must be allowed");
    let Response::Backup { tenants, .. } = own.response else {
        panic!("expected backup response");
    };
    assert_eq!(tenants.len(), 1);
    assert!(tenants[0].jsonl.contains("a-secret"));
    assert!(!tenants[0].jsonl.contains("b-secret"));

    let rejected =
        rushdb_server::protocol::ProtocolExecutor::new(&node, Some("tenant-a".to_string()))
            .execute(RequestEnvelope {
                protocol_version: 1,
                request_id: Some("backup-cross-tenant".into()),
                context: RequestContext {
                    scope: Some("tenant-a".into()),
                    deadline_ms: None,
                    ..Default::default()
                },
                operation: Operation::Backup {
                    request: rushdb_protocol::BackupRequest {
                        scopes: vec!["tenant-b".into()],
                    },
                },
            });
    assert_eq!(rejected.unwrap_err().code, ErrorCode::ScopeMismatch);
}

#[test]
fn staged_transaction_writes_never_leak_tenant_boundary() {
    let node = multitenant();
    put(&node, "tenant-a", "NOTE", "committed-a");
    let begin = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("tx-begin".into()),
        context: RequestContext {
            scope: Some("tenant-a".to_string()),
            deadline_ms: None,
            ..Default::default()
        },
        operation: Operation::Transaction {
            transaction: TransactionOperation::Begin {
                ttl_ms: None,
                autocommit: false,
            },
        },
    };
    let res = rushdb_server::protocol::ProtocolExecutor::new(&node, Some("tenant-a".to_string()))
        .execute(begin)
        .unwrap();
    let Response::TransactionBegun { transaction_id } = res.response else {
        panic!("expected begun transaction");
    };

    // Stage a write inside tenant-a's transaction.
    let stage = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("tx-stage".into()),
        context: RequestContext {
            scope: Some("tenant-a".to_string()),
            deadline_ms: None,
            ..Default::default()
        },
        operation: Operation::Transaction {
            transaction: TransactionOperation::Stage {
                transaction_id,
                mutation: TransactionMutation::CreateRecord {
                    label: "NOTE".to_string(),
                    id: None,
                    properties: serde_json::json!({ "text": "staged-a" }),
                    options: serde_json::json!({}),
                },
            },
        },
    };
    rushdb_server::protocol::ProtocolExecutor::new(&node, Some("tenant-a".to_string()))
        .execute(stage)
        .unwrap();

    // Pre-commit: staged writes are invisible to tenant-a, and tenant-b sees
    // nothing of tenant-a at all.
    assert!(!find_text(&node, "tenant-a").contains(&serde_json::json!("staged-a")));
    assert!(find_text(&node, "tenant-b").is_empty());
}

#[test]
fn build_info_is_global_metadata_with_no_tenant_data() {
    let node = multitenant();
    put(&node, "tenant-a", "NOTE", "a-secret");
    let envelope = RequestEnvelope {
        protocol_version: 1,
        request_id: Some("describe".into()),
        context: RequestContext {
            scope: Some("tenant-a".to_string()),
            deadline_ms: None,
            ..Default::default()
        },
        operation: Operation::Describe {
            describe: Describe::BuildInfo,
        },
    };
    let res = rushdb_server::protocol::ProtocolExecutor::new(&node, Some("tenant-a".to_string()))
        .execute(envelope)
        .unwrap();
    let Response::BuildInfo { build } = res.response else {
        panic!("expected build info");
    };
    let serialized = serde_json::to_string(&build).unwrap();
    assert!(!serialized.contains("a-secret"));
    assert!(!serialized.contains("tenant-a"));
}

/// Tenancy Phase 1 (`AuthorizedView`): a tenant's `DataAccessPolicy` label
/// grant is enforced on direct-fetch-by-id — through `record_value_in_view`,
/// the same consolidated check `search` already applied — and a second
/// tenant with no policy of its own is unaffected by tenant A's. Every write
/// that touches an owner (create, retype, and plain field updates alike)
/// already rejects a disallowed label at commit time
/// (`Lmpg::authorize_label_in_txn`), so a revoked label cannot be written to
/// again either; this test is the read-side half of that same guarantee.
#[test]
fn data_access_policy_label_denial_is_scoped_per_tenant() {
    let node = multitenant();

    // Created while tenant-a has no policy yet (boundary-only).
    let a_created = put_mode(
        &node,
        "tenant-a",
        PutMode::Create,
        None,
        Some("SECRET"),
        serde_json::json!({ "value": "classified" }),
    )
    .unwrap();
    let a_id = a_created["id"].as_str().unwrap().to_string();

    // tenant-a now narrows its policy to a different label, revoking SECRET.
    RequestScope::new(&node, Some("tenant-a".to_string()))
        .security_put_policy(
            &DataAccessPolicy {
                allowed_labels: vec!["NOTE".into()],
                ..Default::default()
            },
            AuthorizationMode::PolicyEnforced,
        )
        .unwrap();

    // A direct fetch by id is denied once the label is revoked.
    let fetch_denied =
        rushdb_server::protocol::ProtocolExecutor::new(&node, Some("tenant-a".to_string()))
            .execute(RequestEnvelope {
                protocol_version: 1,
                request_id: Some("fetch-revoked".into()),
                context: RequestContext {
                    scope: Some("tenant-a".into()),
                    deadline_ms: None,
                    ..Default::default()
                },
                operation: Operation::Query {
                    query: Query::Record { id: a_id.clone() },
                },
            })
            .expect_err("a revoked label must not be fetchable by id");
    assert_eq!(fetch_denied.code, ErrorCode::Unauthorized);
    assert!(fetch_denied.message.contains("denied by policy"));

    // Writes to the now-denied record are rejected too (defense in depth,
    // not this bucket's own mechanism) — the write never reaches the
    // read-side check at all.
    put_mode(
        &node,
        "tenant-a",
        PutMode::Replace,
        Some(&a_id),
        None,
        serde_json::json!({ "value": "still-classified" }),
    )
    .expect_err("a revoked label must not accept further writes either");

    // tenant-b has no policy at all (boundary-only). The identical label
    // remains fully fetchable on its own — tenant-a's policy must not leak
    // into, or restrict, a different tenant.
    let b_created = put_mode(
        &node,
        "tenant-b",
        PutMode::Create,
        None,
        Some("SECRET"),
        serde_json::json!({ "value": "classified" }),
    )
    .unwrap();
    let b_id = b_created["id"].as_str().unwrap().to_string();
    let b_fetched =
        rushdb_server::protocol::ProtocolExecutor::new(&node, Some("tenant-b".to_string()))
            .execute(RequestEnvelope {
                protocol_version: 1,
                request_id: Some("fetch-b".into()),
                context: RequestContext {
                    scope: Some("tenant-b".into()),
                    deadline_ms: None,
                    ..Default::default()
                },
                operation: Operation::Query {
                    query: Query::Record { id: b_id },
                },
            })
            .unwrap();
    let Response::Record { record } = b_fetched.response else {
        panic!("expected record response");
    };
    assert_eq!(record["properties"]["value"], "classified");
}

/// Tenancy Phase 1 (`AuthorizedView`): a tenant's property mask is applied
/// on the mutation-response path (previously unmasked for merge/replace/
/// retype), and is scoped to the tenant that configured it.
#[test]
fn data_access_policy_property_mask_is_scoped_per_tenant() {
    let node = multitenant();
    RequestScope::new(&node, Some("tenant-a".to_string()))
        .security_put_policy(
            &DataAccessPolicy {
                allowed_labels: vec!["NOTE".into()],
                visible_properties: std::collections::BTreeMap::from([(
                    "NOTE".into(),
                    vec!["title".into()],
                )]),
                ..Default::default()
            },
            AuthorizationMode::PolicyEnforced,
        )
        .unwrap();

    let a_created = put_mode(
        &node,
        "tenant-a",
        PutMode::Create,
        None,
        Some("NOTE"),
        serde_json::json!({ "title": "visible", "secretField": "hidden" }),
    )
    .unwrap();
    let a_id = a_created["id"].as_str().unwrap().to_string();
    let a_merged = put_mode(
        &node,
        "tenant-a",
        PutMode::Merge,
        Some(&a_id),
        None,
        serde_json::json!({ "title": "still-visible" }),
    )
    .unwrap();
    let a_props = a_merged["properties"].as_object().unwrap();
    assert_eq!(a_props.get("title").unwrap(), "still-visible");
    assert!(
        !a_props.contains_key("secretField"),
        "masked property must not round-trip on the merge response: {a_props:?}"
    );

    // tenant-b has no policy; the same shape of record keeps every property
    // on its own merge response.
    let b_created = put_mode(
        &node,
        "tenant-b",
        PutMode::Create,
        None,
        Some("NOTE"),
        serde_json::json!({ "title": "visible", "secretField": "hidden" }),
    )
    .unwrap();
    let b_id = b_created["id"].as_str().unwrap().to_string();
    let b_merged = put_mode(
        &node,
        "tenant-b",
        PutMode::Merge,
        Some(&b_id),
        None,
        serde_json::json!({ "title": "still-visible" }),
    )
    .unwrap();
    let b_props = b_merged["properties"].as_object().unwrap();
    assert_eq!(b_props.get("title").unwrap(), "still-visible");
    assert_eq!(b_props.get("secretField").unwrap(), "hidden");
}

/// Tenancy Phase 2: a policy rotation mid-transaction causes the commit to
/// abort with a typed `authorization_changed` outcome instead of a false
/// success — `rushdb_tx::TxCoordinator` captures the policy generation at
/// `begin` and revalidates it against live state inside the same atomic
/// commit attempt (`crates/rushdb-tx/src/coordinator.rs`), so a transaction
/// straddling a revocation can never durably commit under the old grant.
/// `rushdb-authz-integration/tests/persistence.rs`'s
/// `policy_rotation_aborts_inflight_cross_request_transaction` proves the
/// mechanism directly against `TxCoordinator`; this is the same guarantee
/// exercised through the full protocol stack, with a second tenant proving
/// the revalidation is scoped to the tenant whose policy rotated.
#[test]
fn policy_rotation_mid_transaction_aborts_commit_and_is_scoped_per_tenant() {
    let node = multitenant();

    let begin = |scope: &str| {
        let res = rushdb_server::protocol::ProtocolExecutor::new(&node, Some(scope.to_string()))
            .execute(RequestEnvelope {
                protocol_version: 1,
                request_id: Some("tx-begin".into()),
                context: RequestContext {
                    scope: Some(scope.to_string()),
                    deadline_ms: None,
                    ..Default::default()
                },
                operation: Operation::Transaction {
                    transaction: TransactionOperation::Begin {
                        ttl_ms: None,
                        autocommit: false,
                    },
                },
            })
            .unwrap();
        let Response::TransactionBegun { transaction_id } = res.response else {
            panic!("expected begun transaction");
        };
        transaction_id
    };
    let stage_create = |scope: &str, transaction_id: String| {
        rushdb_server::protocol::ProtocolExecutor::new(&node, Some(scope.to_string()))
            .execute(RequestEnvelope {
                protocol_version: 1,
                request_id: Some("tx-stage".into()),
                context: RequestContext {
                    scope: Some(scope.to_string()),
                    deadline_ms: None,
                    ..Default::default()
                },
                operation: Operation::Transaction {
                    transaction: TransactionOperation::Stage {
                        transaction_id,
                        mutation: TransactionMutation::CreateRecord {
                            label: "NOTE".to_string(),
                            id: None,
                            properties: serde_json::json!({ "text": "staged" }),
                            options: serde_json::json!({}),
                        },
                    },
                },
            })
            .unwrap();
    };
    let commit = |scope: &str, transaction_id: String| {
        rushdb_server::protocol::ProtocolExecutor::new(&node, Some(scope.to_string()))
            .execute(RequestEnvelope {
                protocol_version: 1,
                request_id: Some("tx-commit".into()),
                context: RequestContext {
                    scope: Some(scope.to_string()),
                    deadline_ms: None,
                    ..Default::default()
                },
                operation: Operation::Transaction {
                    transaction: TransactionOperation::Commit { transaction_id },
                },
            })
            .unwrap()
            .response
    };

    // tenant-a begins and stages a write, then a policy is published for the
    // first time mid-transaction (generation 0 -> 1): a rotation even though
    // no policy previously existed.
    let a_tx = begin("tenant-a");
    stage_create("tenant-a", a_tx.clone());
    RequestScope::new(&node, Some("tenant-a".to_string()))
        .security_put_policy(
            &DataAccessPolicy {
                allowed_labels: vec!["NOTE".into()],
                ..Default::default()
            },
            AuthorizationMode::PolicyEnforced,
        )
        .unwrap();
    let Response::TransactionOutcome { status, reason, .. } = commit("tenant-a", a_tx) else {
        panic!("expected transaction outcome");
    };
    assert_eq!(status, TransactionStatus::Aborted);
    assert!(
        reason.is_some_and(|r| r.contains("authorization_changed")),
        "commit must abort with a typed authorization_changed reason"
    );

    // tenant-b runs the identical begin/stage/commit sequence with no policy
    // rotation of its own: tenant-a's rotation must not affect it.
    let b_tx = begin("tenant-b");
    stage_create("tenant-b", b_tx.clone());
    let Response::TransactionOutcome { status, .. } = commit("tenant-b", b_tx) else {
        panic!("expected transaction outcome");
    };
    assert_eq!(status, TransactionStatus::Committed);
}

// RM-3: Node's own `try_acquire_tenant_slot`/`release_tenant_slot` gate was
// deleted — `BoundedScheduler::execute_for_tenant` is the single admission
// owner now, and it is not `pub` outside the crate (by design: scheduling is
// an internal transport concern, not public API). The per-tenant isolation
// property this test asserted (a noisy tenant can't exceed its quota; a
// different tenant is unaffected) is covered where the policy now actually
// lives: `rushdb-engine/src/scheduler.rs`'s
// `per_tenant_cap_rejects_work_before_global_queue_is_full`.
