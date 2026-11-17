//! Tenant directory: scope → never-reused 4-byte ordinal.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::reader::try_access;
use rushdb_lmpg::{tenancy, Lmpg};
use rushdb_storage::Store;
use rushdb_types::ids::{TenantId, TenantScope};
use serde_json::json;

fn members(store: &Arc<Store>, tenant: TenantId, label: &str) -> usize {
    let view = store.read_view().unwrap();
    try_access::label_members(&view, tenant, label)
        .unwrap()
        .len()
}

#[test]
fn registration_assigns_dense_ordinals_and_is_idempotent() {
    let store = Arc::new(Store::new_in_memory());
    let a = tenancy::resolve_or_register_scope(&store, "project-a").unwrap();
    let b = tenancy::resolve_or_register_scope(&store, "project-b").unwrap();
    assert_eq!(a, TenantId(TenantId::FIRST_ASSIGNED));
    assert_eq!(b, TenantId(TenantId::FIRST_ASSIGNED + 1));
    assert_eq!(
        tenancy::resolve_or_register_scope(&store, "project-a").unwrap(),
        a
    );
    let version = store.committed_version();
    assert_eq!(
        tenancy::resolve_or_register_scope(&store, "project-b").unwrap(),
        b
    );
    assert_eq!(
        store.committed_version(),
        version,
        "resolving a registered scope never commits"
    );
}

#[test]
fn concurrent_registration_of_one_scope_yields_one_ordinal() {
    let store = Arc::new(Store::new_in_memory());
    let mut handles = Vec::new();
    for _ in 0..16 {
        let store = Arc::clone(&store);
        handles.push(std::thread::spawn(move || {
            tenancy::resolve_or_register_scope(&store, "shared").unwrap()
        }));
    }
    let mut ids: Vec<TenantId> = Vec::new();
    for handle in handles {
        ids.push(handle.join().unwrap());
    }
    assert!(ids.iter().all(|id| *id == ids[0]), "{ids:?}");
    let active = store.active_tenants().unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0], (ids[0], TenantScope::of("shared")));
}

#[test]
fn unregistered_scope_resolves_to_nothing_and_never_allocates() {
    let store = Arc::new(Store::new_in_memory());
    assert_eq!(
        store.resolve_tenant(&TenantScope::of("ghost")).unwrap(),
        None
    );
    assert_eq!(store.committed_version(), 0);
    assert_eq!(members(&store, TenantId::UNREGISTERED, "User"), 0);
}

#[test]
fn tenants_never_see_each_others_keys() {
    let store = Arc::new(Store::new_in_memory());
    let a = tenancy::resolve_or_register_scope(&store, "a").unwrap();
    let b = tenancy::resolve_or_register_scope(&store, "b").unwrap();
    for (tenant, count) in [(a, 3usize), (b, 5)] {
        let lmpg = Lmpg::new(Arc::clone(&store), tenant);
        for i in 0..count {
            lmpg.create_record("User", None, vec![("n".into(), FieldChange::Set(json!(i)))])
                .unwrap();
        }
    }
    assert_eq!(members(&store, a, "User"), 3);
    assert_eq!(members(&store, b, "User"), 5);
}

#[test]
fn purged_scope_reregisters_with_a_fresh_ordinal_and_sees_nothing_old() {
    let store = Arc::new(Store::new_in_memory());
    let first = tenancy::resolve_or_register_scope(&store, "churn").unwrap();
    let lmpg = Lmpg::new(Arc::clone(&store), first);
    lmpg.create_record("User", None, vec![("n".into(), FieldChange::Set(json!(1)))])
        .unwrap();
    lmpg.purge_tenant().unwrap();
    assert_eq!(
        store.resolve_tenant(&TenantScope::of("churn")).unwrap(),
        None
    );
    let second = tenancy::resolve_or_register_scope(&store, "churn").unwrap();
    assert_ne!(first, second, "a retired ordinal is never reassigned");
    assert_eq!(members(&store, second, "User"), 0);
    assert!(store
        .active_tenants()
        .unwrap()
        .iter()
        .all(|(tenant, _)| *tenant != first));
}

#[test]
fn directory_survives_command_log_replay() {
    let store = Arc::new(Store::new_in_memory());
    let a = tenancy::resolve_or_register_scope(&store, "a").unwrap();
    let b = tenancy::resolve_or_register_scope(&store, "b").unwrap();
    let log = store.log_bytes();
    let replayed = Arc::new(Store::new_in_memory());
    drop(Lmpg::new(Arc::clone(&replayed), TenantId::SYSTEM));
    replayed.try_replay(&log).unwrap();
    assert_eq!(
        replayed.resolve_tenant(&TenantScope::of("a")).unwrap(),
        Some(a)
    );
    assert_eq!(
        replayed.resolve_tenant(&TenantScope::of("b")).unwrap(),
        Some(b)
    );
}

fn replicated(
    tenant: TenantId,
    mutation: rushdb_types::command::Mutation,
) -> rushdb_types::command::CommitBatch {
    rushdb_types::command::CommitBatch {
        command_version: rushdb_types::command::COMMAND_VERSION,
        tenant,
        transaction_id: "replicated".into(),
        commit_key: uuid::Uuid::now_v7(),
        start_index: 0,
        isolation: rushdb_types::command::IsolationMode::ReadCommitted,
        preconditions: vec![],
        mutations: vec![mutation],
    }
}

/// A follower applies the leader's register and retire commits through
/// replication; its resolution of the scope follows both at once. (Before
/// the resident framework only the node that drove a purge forgot the
/// scope, so a follower kept resolving the retired ordinal.)
#[test]
fn a_follower_stops_resolving_a_tenant_retired_by_replication() {
    let follower = Arc::new(Store::new_in_memory());
    drop(Lmpg::new(Arc::clone(&follower), TenantId::SYSTEM));
    let scope = TenantScope::of("replicated");
    assert_eq!(follower.resolve_tenant(&scope).unwrap(), None);
    follower
        .apply_replicated_batch(&replicated(
            TenantId::SYSTEM,
            rushdb_types::command::Mutation::RegisterTenant { scope },
        ))
        .unwrap();
    let tenant = follower
        .resolve_tenant(&scope)
        .unwrap()
        .expect("registered");
    // Resolved twice: the second answer may come from the resident layer.
    assert_eq!(follower.resolve_tenant(&scope).unwrap(), Some(tenant));
    follower
        .apply_replicated_batch(&replicated(
            tenant,
            rushdb_types::command::Mutation::RetireTenant,
        ))
        .unwrap();
    assert_eq!(
        follower.resolve_tenant(&scope).unwrap(),
        None,
        "a retired tenant still resolves on the follower"
    );
}
