//! Replicated authorization (plan document 23): policies live under the
//! tenant `security/` keyspace, travel through the deterministic state
//! machine and redb, carry a monotonic generation, and are
//! re-evaluated at commit time against live policy state — an opaque
//! transaction id is never authority.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use rushdb_authz::{
    load_active_policy, store_policy, ActiveAuthorizer, AuthorizationMode, DataAccessPolicy,
    SecurityScope,
};
use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{BatchOp, Lmpg};
use rushdb_storage::Store;
use rushdb_tx::{TxCoordinator, TxOutcome};
use rushdb_types::ids::TenantId;
use serde_json::json;

fn policy_allow_episode() -> DataAccessPolicy {
    DataAccessPolicy {
        allowed_labels: vec!["Episode".into()],
        denied_rel_types: vec!["SECRET_LINK".into()],
        visible_properties: BTreeMap::default(),
        max_results: 10,
    }
}

fn memory() -> (Arc<Store>, TenantId) {
    (Arc::new(Store::new_in_memory()), TenantId::from_ordinal(71))
}

fn count_label(lmpg: &Lmpg, label: &str) -> usize {
    rushdb_lmpg::reader::try_access::label_members(
        &lmpg.store.read_view().unwrap(),
        lmpg.tenant,
        label,
    )
    .unwrap()
    .len()
}

#[test]
fn store_policy_then_default_deny_enforced_on_mutation() {
    let (store, tenant) = memory();
    let lmpg = Lmpg::new(store.clone(), tenant);
    lmpg.set_authorizer(Arc::new(ActiveAuthorizer::new(store, tenant)));

    let ap = store_policy(
        &lmpg,
        &policy_allow_episode(),
        AuthorizationMode::PolicyEnforced,
    )
    .unwrap();
    assert_eq!(ap.generation, 1);

    // Granted label passes.
    let rid = lmpg
        .create_record(
            "Episode",
            None,
            vec![("title".into(), FieldChange::Set(json!("s1e1")))],
        )
        .unwrap();

    // Denied label is rejected by the engine itself (default-deny, not just
    // boundary filtering).
    let err = lmpg
        .create_record(
            "Note",
            None,
            vec![("body".into(), FieldChange::Set(json!("x")))],
        )
        .unwrap_err();
    assert!(err.to_reason().contains("not authorized"), "{err:?}");

    // The granted record is still queryable through the compiled scope.
    let scope = SecurityScope::compile(&ap.policy, ap.mode);
    let page = rushdb_authz::find_records_authorized(
        &lmpg,
        &rushdb_query::SearchRequest::default(),
        &scope,
    )
    .unwrap();
    assert_eq!(page.total_matched, 1);
    assert_eq!(page.records[0].get("title"), Some(&json!("s1e1")));
    let _ = rid;
}

#[test]
fn denied_rel_type_rejected_on_mutation() {
    let (store, tenant) = memory();
    let lmpg = Lmpg::new(store.clone(), tenant);
    lmpg.set_authorizer(Arc::new(ActiveAuthorizer::new(store, tenant)));
    store_policy(
        &lmpg,
        &policy_allow_episode(),
        AuthorizationMode::PolicyEnforced,
    )
    .unwrap();

    let a = lmpg.create_record("Episode", None, vec![]).unwrap();
    let b = lmpg.create_record("Episode", None, vec![]).unwrap();

    let err = lmpg
        .create_relationship("SECRET_LINK", a, b, vec![])
        .unwrap_err();
    assert!(err.to_reason().contains("denied"), "{err:?}");

    // A non-denied relationship type passes.
    lmpg.create_relationship("SEQUEL", a, b, vec![]).unwrap();
}

#[test]
fn policy_rotation_aborts_inflight_cross_request_transaction() {
    let (store, tenant) = memory();
    let lmpg = Lmpg::new(store.clone(), tenant);
    lmpg.set_authorizer(Arc::new(ActiveAuthorizer::new(store.clone(), tenant)));
    let coord = TxCoordinator::new_with_authorizer(
        store.clone(),
        tenant,
        Duration::from_secs(60),
        Some(Arc::new(ActiveAuthorizer::new(store, tenant))),
    )
    .unwrap();

    let tx = coord.begin().unwrap();
    coord
        .stage(
            &tx,
            BatchOp::CreateRecord {
                label: "Episode".into(),
                explicit_id: None,
                fields: vec![("title".into(), FieldChange::Set(json!("s1e1")))],
            },
        )
        .unwrap();

    // Rotate the policy mid-transaction: generation 1 -> 2.
    store_policy(
        &lmpg,
        &policy_allow_episode(),
        AuthorizationMode::PolicyEnforced,
    )
    .unwrap();

    // Commit revalidates the generation captured at begin and must abort.
    let out = coord.commit(&tx);
    match out {
        TxOutcome::Aborted(reason) => assert!(reason.contains("authorization_changed"), "{reason}"),
        other => panic!("expected authorization_changed abort, got {other:?}"),
    }
}

#[test]
fn committed_under_stable_policy_survives_rotation() {
    let (store, tenant) = memory();
    let lmpg = Lmpg::new(store.clone(), tenant);
    lmpg.set_authorizer(Arc::new(ActiveAuthorizer::new(store.clone(), tenant)));
    let coord = TxCoordinator::new_with_authorizer(
        store.clone(),
        tenant,
        Duration::from_secs(60),
        Some(Arc::new(ActiveAuthorizer::new(store, tenant))),
    )
    .unwrap();

    let tx = coord.begin().unwrap();
    coord
        .stage(
            &tx,
            BatchOp::CreateRecord {
                label: "Episode".into(),
                explicit_id: None,
                fields: vec![],
            },
        )
        .unwrap();

    // No rotation before commit: the transaction commits cleanly.
    match coord.commit(&tx) {
        TxOutcome::Committed(_) => {}
        other => panic!("expected commit, got {other:?}"),
    }

    // A later rotation does not retroactively abort the committed data.
    store_policy(
        &lmpg,
        &policy_allow_episode(),
        AuthorizationMode::PolicyEnforced,
    )
    .unwrap();
    assert_eq!(count_label(&lmpg, "Episode"), 1);
}

#[test]
fn load_active_policy_roundtrip_matches_fingerprint() {
    let (store, tenant) = memory();
    let lmpg = Lmpg::new(store, tenant);

    assert!(
        load_active_policy(&lmpg).unwrap().is_none(),
        "no policy before first store"
    );

    let ap = store_policy(
        &lmpg,
        &policy_allow_episode(),
        AuthorizationMode::PolicyEnforced,
    )
    .unwrap();
    let loaded = load_active_policy(&lmpg)
        .unwrap()
        .expect("policy persisted");
    assert_eq!(loaded.generation, ap.generation);
    assert_eq!(loaded.mode, AuthorizationMode::PolicyEnforced);
    assert_eq!(loaded.fingerprint, ap.fingerprint);
    assert_eq!(loaded.policy.allowed_labels, vec!["Episode".to_string()]);
}

#[test]
fn policy_and_enforcement_survive_redb_restart() {
    let dir = rushdb_storage::TempPath::dir("authz-restart");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let tenant = TenantId::from_ordinal(71);

    // First process: store a policy and an allowed record in redb.
    let db = dir.join("rushdb.redb");
    let store1 = Arc::new(Store::open(&db).unwrap());
    let lmpg1 = Lmpg::new(store1.clone(), tenant);
    lmpg1.set_authorizer(Arc::new(ActiveAuthorizer::new(store1.clone(), tenant)));
    let ap = store_policy(
        &lmpg1,
        &policy_allow_episode(),
        AuthorizationMode::PolicyEnforced,
    )
    .unwrap();
    lmpg1.create_record("Episode", None, vec![]).unwrap();
    let ci = store1.committed_version();
    drop(lmpg1);
    drop(store1);

    // Second process: reopen from redb. Policy, generation, and enforcement
    // must all come back.
    let store2 = Arc::new(Store::open(&db).unwrap());
    assert_eq!(store2.committed_version(), ci);

    let lmpg2 = Lmpg::new(store2.clone(), tenant);
    lmpg2.set_authorizer(Arc::new(ActiveAuthorizer::new(store2, tenant)));

    let loaded = load_active_policy(&lmpg2)
        .unwrap()
        .expect("policy survives restart");
    assert_eq!(loaded.generation, 1);
    assert_eq!(loaded.fingerprint, ap.fingerprint);

    // Record created before restart is still there.
    assert_eq!(count_label(&lmpg2, "Episode"), 1);

    // Denied label still rejected after restart.
    let err = lmpg2.create_record("Note", None, vec![]).unwrap_err();
    assert!(err.to_reason().contains("not authorized"), "{err:?}");

    let _ = std::fs::remove_dir_all(&dir);
}
