//! Pinned-policy and generation-isolation gates for replicated authorization:
//! policy state is scoped per tenant, historical views keep the policy pinned
//! at their commit, and malformed policy state fails closed rather than
//! degrading to boundary-only or no-policy behavior.

use rushdb_authz::{
    load_active_policy, load_active_policy_in_view, store_policy, ActiveAuthorizer,
    AuthorizationMode, AuthzError, DataAccessPolicy,
};
use rushdb_lmpg::{Authorizer, BatchOp, Lmpg};
use rushdb_query::SearchRequest;
use rushdb_storage::Store;
use rushdb_tx::{TxCoordinator, TxOutcome};
use rushdb_types::{
    command::{CommitBatch, IsolationMode, Mutation, COMMAND_VERSION},
    ids::TenantId,
    keys::*,
};
use std::{sync::Arc, time::Duration};

fn policy(label: &str) -> DataAccessPolicy {
    DataAccessPolicy {
        allowed_labels: vec![label.into()],
        denied_rel_types: vec!["SECRET".into()],
        ..Default::default()
    }
}

fn on_backends(test: impl Fn(Arc<Store>, TenantId)) {
    let path = rushdb_storage::TempPath::store("policy");
    for disk in [false, true] {
        test(
            Arc::new(if disk {
                Store::open(&path).unwrap()
            } else {
                Store::new_in_memory()
            }),
            TenantId(u32::MAX - 1),
        );
    }
    rushdb_storage::remove_store(&path).unwrap();
}

fn apply(
    store: &Store,
    tenant: TenantId,
    version: u32,
    mutations: Vec<Mutation>,
) -> Result<u64, String> {
    store
        .apply_replicated_batch(&CommitBatch {
            command_version: version,
            tenant,
            transaction_id: "policy-test".into(),
            commit_key: uuid::Uuid::now_v7(),
            start_index: 0,
            isolation: IsolationMode::ReadCommitted,
            preconditions: vec![],
            mutations,
        })
        .map_err(|e| e.to_string())
}

fn coordinator(store: Arc<Store>, tenant: TenantId) -> TxCoordinator {
    TxCoordinator::new_with_authorizer(
        store.clone(),
        tenant,
        Duration::from_secs(30),
        Some(Arc::new(ActiveAuthorizer::new(store, tenant))),
    )
    .unwrap()
}

#[test]
fn tenant_policies_and_transaction_generations_are_isolated_and_pinned() {
    on_backends(|store, tenant| {
        let a = Lmpg::new(store.clone(), tenant);
        let b = Lmpg::new(store.clone(), TenantId(u32::MAX - 2));
        let initial = store_policy(&a, &policy("A"), AuthorizationMode::PolicyEnforced).unwrap();
        let view = store.read_view().unwrap();
        let coord = coordinator(store.clone(), tenant);
        let tx = coord.begin().unwrap();
        coord
            .stage(
                &tx,
                BatchOp::CreateRecord {
                    label: "A".into(),
                    explicit_id: None,
                    fields: vec![],
                },
            )
            .unwrap();
        store_policy(&b, &policy("B"), AuthorizationMode::PolicyEnforced).unwrap();
        assert!(matches!(coord.commit(&tx), TxOutcome::Committed(_)));
        let auth_a = ActiveAuthorizer::new(store.clone(), tenant);
        let auth_b = ActiveAuthorizer::new(store.clone(), b.tenant);
        assert!(auth_a.authorize_label("A").is_ok());
        assert!(auth_a.authorize_label("B").is_err());
        assert!(auth_b.authorize_label("A").is_err());
        let tx = coord.begin().unwrap();
        let changed =
            store_policy(&a, &policy("Other"), AuthorizationMode::PolicyEnforced).unwrap();
        assert!(changed.generation > initial.generation);
        assert!(
            matches!(coord.commit(&tx), TxOutcome::Aborted(reason) if reason.contains("authorization_changed"))
        );
        store.gc(u64::MAX).unwrap();
        assert_eq!(
            load_active_policy_in_view(&view, tenant).unwrap().unwrap(),
            initial
        );
        assert_eq!(load_active_policy(&a).unwrap().unwrap(), changed);
    });
}

#[test]
fn concurrent_policy_rotations_return_distinct_committed_generations() {
    on_backends(|store, tenant| {
        let lmpg = Arc::new(Lmpg::new(store, tenant));
        let generations = std::thread::scope(|scope| {
            (0..12)
                .map(|i| {
                    let lmpg = lmpg.clone();
                    scope.spawn(move || {
                        store_policy(
                            &lmpg,
                            &policy(&format!("L{i}")),
                            AuthorizationMode::PolicyEnforced,
                        )
                        .unwrap()
                        .generation
                    })
                })
                .map(|worker| worker.join().unwrap())
                .collect::<Vec<_>>()
        });
        let mut sorted = generations;
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 12);
        assert_eq!(
            load_active_policy(&lmpg).unwrap().unwrap().generation,
            *sorted.last().unwrap()
        );
    });
}

fn corrupt(store: &Store, tenant: TenantId, key: Vec<u8>, value: Option<Vec<u8>>) {
    store.set_expander(Arc::new(Box::new(move |_, _, txn| {
        match &value {
            Some(value) => txn.put(key.clone(), value.clone()),
            None => txn.del(key.clone()),
        }
        Ok(())
    })));
    apply(store, tenant, COMMAND_VERSION, vec![]).unwrap();
    // Restore the real expander after fixture corruption.
    store.set_expander(Arc::new(Box::new(|ci, batch, txn| {
        rushdb_lmpg::expand::expand_commit(ci, batch, txn).map_err(|e| e.to_string())
    })));
}

#[test]
fn malformed_policy_state_never_becomes_no_policy_or_boundary_only() {
    for mode in 0..8 {
        on_backends(|store, tenant| {
            let lmpg = Lmpg::new(store.clone(), tenant);
            store_policy(&lmpg, &policy("A"), AuthorizationMode::PolicyEnforced).unwrap();
            let record = lmpg.create_record("A", None, vec![]).unwrap();
            let auth = Arc::new(ActiveAuthorizer::new(store.clone(), tenant));
            lmpg.set_authorizer(auth.clone());
            let coord = coordinator(store.clone(), tenant);
            let pending = coord.begin().unwrap();
            coord
                .stage(
                    &pending,
                    BatchOp::CreateRecord {
                        label: "A".into(),
                        explicit_id: None,
                        fields: vec![],
                    },
                )
                .unwrap();
            match mode {
                0 => corrupt(
                    &store,
                    tenant,
                    security_generation_key(tenant),
                    Some(vec![1]),
                ),
                1 => corrupt(&store, tenant, security_generation_key(tenant), None),
                2 => corrupt(&store, tenant, security_policy_key(tenant), None),
                3 => corrupt(
                    &store,
                    tenant,
                    security_policy_key(tenant),
                    Some(b"not json".to_vec()),
                ),
                _ => {
                    let mut value: serde_json::Value = serde_json::from_slice(
                        &store
                            .try_get(&security_policy_key(tenant))
                            .unwrap()
                            .unwrap(),
                    )
                    .unwrap();
                    match mode {
                        4 => value["mode"] = serde_json::json!("UnknownMode"),
                        5 => value["fingerprint"] = serde_json::json!(0),
                        6 => value["generation"] = serde_json::json!(999),
                        _ => {
                            value.as_object_mut().unwrap().remove("policy");
                        }
                    }
                    corrupt(
                        &store,
                        tenant,
                        security_policy_key(tenant),
                        Some(serde_json::to_vec(&value).unwrap()),
                    );
                }
            }
            assert!(matches!(
                load_active_policy(&lmpg),
                Err(AuthzError::InvalidPolicy(_))
            ));
            assert!(auth.authorize_label("A").is_err());
            assert!(auth.authorize_rel_type("PUBLIC").is_err());
            assert!(auth.current_generation().is_err());
            assert!(coordinator(store, tenant).begin().is_err());
            assert!(
                matches!(coord.commit(&pending), TxOutcome::Aborted(reason) if reason.contains("invalid authorization policy"))
            );
            assert!(lmpg.create_record("A", None, vec![]).is_err());
            assert!(lmpg.update_fields(record, vec![]).is_err());
            assert!(lmpg.delete_record(record).is_err());
        });
    }
}

#[test]
fn unsupported_global_policy_metadata_fails_closed_for_every_tenant() {
    on_backends(|store, tenant| {
        let lmpg = Lmpg::new(store.clone(), tenant);
        apply(
            &store,
            tenant,
            COMMAND_VERSION,
            vec![Mutation::SetMetaBlob {
                key: SECURITY_POLICY_ACTIVE_KEY.into(),
                value: b"unsupported global policy metadata".to_vec(),
            }],
        )
        .unwrap();
        assert!(matches!(
            load_active_policy(&lmpg),
            Err(AuthzError::InvalidPolicy(_))
        ));
        let other = Lmpg::new(store.clone(), TenantId(u32::MAX - 2));
        assert!(matches!(
            load_active_policy(&other),
            Err(AuthzError::InvalidPolicy(_))
        ));
        let auth = ActiveAuthorizer::new(store, tenant);
        assert!(auth.authorize_label("A").is_err());
        assert!(auth.authorize_rel_type("PUBLIC").is_err());
    });
}

#[test]
fn new_policy_intent_rejects_old_versions_without_publishing() {
    on_backends(|store, tenant| {
        let lmpg = Lmpg::new(store.clone(), tenant);
        for version in (0..COMMAND_VERSION).chain([COMMAND_VERSION + 1]) {
            assert!(apply(
                &store,
                tenant,
                version,
                vec![Mutation::SetSecurityPolicy {
                    policy: policy("A"),
                    mode: AuthorizationMode::PolicyEnforced
                }]
            )
            .is_err());
            assert_eq!(store.committed_version(), 0);
            assert!(load_active_policy(&lmpg).unwrap().is_none());
        }
    });
}

#[test]
fn scoped_policy_and_admitted_enforcement_survive_reopen() {
    let path = rushdb_storage::TempPath::store("policy-reopen");
    let tenant = TenantId(u32::MAX - 1);
    let active = {
        let lmpg = Lmpg::new(Arc::new(Store::open(&path).unwrap()), tenant);
        store_policy(&lmpg, &policy("A"), AuthorizationMode::PolicyEnforced).unwrap()
    };
    let store = Arc::new(Store::open(&path).unwrap());
    let lmpg = Lmpg::new(store.clone(), tenant);
    lmpg.set_authorizer(Arc::new(ActiveAuthorizer::new(store.clone(), tenant)));
    assert_eq!(load_active_policy(&lmpg).unwrap().unwrap(), active);
    assert!(lmpg.create_record("Denied", None, vec![]).is_err());
    lmpg.create_record("A", None, vec![]).unwrap();
    drop(lmpg);
    drop(store);
    rushdb_storage::remove_store(&path).unwrap();
}

#[test]
fn admitted_authorization_and_gc_complete_without_nested_view_locks() {
    on_backends(|store, tenant| {
        let lmpg = Arc::new(Lmpg::new(store.clone(), tenant));
        store_policy(&lmpg, &policy("A"), AuthorizationMode::PolicyEnforced).unwrap();
        lmpg.set_authorizer(Arc::new(ActiveAuthorizer::new(store, tenant)));
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let mut workers = Vec::new();
        for mode in 0..3 {
            let lmpg = lmpg.clone();
            let done = done_tx.clone();
            workers.push(std::thread::spawn(move || {
                for _ in 0..20 {
                    match mode {
                        0 => {
                            lmpg.create_record("A", None, vec![]).unwrap();
                        }
                        1 => {
                            lmpg.store.gc(u64::MAX).unwrap();
                        }
                        _ => {
                            // Coordinator commits legitimately conflict with
                            // concurrent record writes (the ordinal counter is
                            // shared): a serializability abort is a valid
                            // outcome. This worker exists to prove the lock
                            // order never stalls — not that every commit wins.
                            for _ in 0..20 {
                                let coord = coordinator(lmpg.store.clone(), tenant);
                                let tx = coord.begin().unwrap();
                                coord
                                    .stage(
                                        &tx,
                                        BatchOp::CreateRecord {
                                            label: "A".into(),
                                            explicit_id: None,
                                            fields: vec![],
                                        },
                                    )
                                    .unwrap();
                                drop(coord.commit(&tx));
                            }
                        }
                    }
                }
                done.send(()).unwrap();
            }));
        }
        for _ in 0..3 {
            done_rx
                .recv_timeout(Duration::from_secs(10))
                .expect("authorization/GC lock order stalled");
        }
        for worker in workers {
            worker.join().unwrap();
        }
    });
}

#[test]
fn policy_rotation_denies_existing_record_and_relationship_mutations() {
    on_backends(|store, tenant| {
        let lmpg = Lmpg::new(store.clone(), tenant);
        let a = lmpg.create_record("Denied", None, vec![]).unwrap();
        let b = lmpg.create_record("Denied", None, vec![]).unwrap();
        let rel = lmpg.create_relationship("SECRET", a, b, vec![]).unwrap();
        store_policy(&lmpg, &policy("Allowed"), AuthorizationMode::PolicyEnforced).unwrap();
        lmpg.set_authorizer(Arc::new(ActiveAuthorizer::new(store.clone(), tenant)));
        let ci = store.committed_version();
        assert!(lmpg.update_fields(a, vec![]).is_err());
        assert!(lmpg.delete_record(a).is_err());
        assert!(lmpg.update_relationship_fields(rel, vec![]).is_err());
        assert!(lmpg.delete_relationship(rel).is_err());
        assert!(lmpg.create_relationship("SECRET", a, b, vec![]).is_err());
        assert_eq!(store.committed_version(), ci);
    });
}

#[test]
fn policy_read_limits_and_cancellation_propagate_as_storage_errors() {
    on_backends(|store, tenant| {
        let lmpg = Lmpg::new(store.clone(), tenant);
        store_policy(&lmpg, &policy("A"), AuthorizationMode::PolicyEnforced).unwrap();
        let view = store
            .read_with_limits(
                None,
                rushdb_storage::ReadLimits {
                    max_item_bytes: 1,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(matches!(
            load_active_policy_in_view(&view, tenant),
            Err(AuthzError::Storage(_))
        ));
        let view = store.read_view().unwrap();
        view.cancellation().cancel();
        assert!(matches!(
            load_active_policy_in_view(&view, tenant),
            Err(AuthzError::Storage(rushdb_storage::StorageError::Cancelled))
        ));
    });
}

#[test]
fn tenant_purge_retains_policy_state_and_replays_cleanly() {
    on_backends(|store, tenant| {
        let lmpg = Lmpg::new(store.clone(), tenant);
        let active = store_policy(&lmpg, &policy("A"), AuthorizationMode::PolicyEnforced).unwrap();
        lmpg.purge_tenant_chunked(1024).unwrap();
        // A data purge wipes graph state but must never erase enforcement, so
        // the active policy survives in-process and through WAL replay.
        assert_eq!(load_active_policy(&lmpg).unwrap().unwrap(), active);
        let recovered = Arc::new(Store::new_in_memory());
        let recovered_lmpg = Lmpg::new(recovered.clone(), tenant);
        recovered.try_replay(&store.take_log_for_crash()).unwrap();
        assert_eq!(
            load_active_policy(&recovered_lmpg).unwrap().unwrap(),
            active
        );
    });
}

#[test]
fn authorized_query_storage_errors_are_not_no_candidates() {
    on_backends(|store, tenant| {
        let lmpg = Lmpg::new(store.clone(), tenant);
        let record = lmpg.create_record("A", None, vec![]).unwrap();
        // P20 co-locates the header at the ordinal key, which is the row the
        // label/ordinal query path reads.
        corrupt(
            &store,
            tenant,
            rushdb_types::keys::record_hdr_key(tenant, record.ordinal),
            Some(b"bad header".to_vec()),
        );
        let policy = policy("A");
        let scope =
            rushdb_authz::SecurityScope::compile(&policy, AuthorizationMode::PolicyEnforced);
        assert!(matches!(
            rushdb_authz::find_records_authorized(&lmpg, &SearchRequest::default(), &scope),
            Err(AuthzError::Query(_))
        ));
    });
}
