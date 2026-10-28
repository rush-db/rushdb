//! Integration tests for the default-deny authorization policy slice.

use std::sync::Arc;

use rushdb_authz::{authorize_mutation, AuthorizationMode, DataAccessPolicy, SecurityScope};
use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_query::SearchRequest;
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::json;

fn policy_default_deny() -> DataAccessPolicy {
    DataAccessPolicy {
        allowed_labels: vec!["Public".into()],
        denied_rel_types: vec!["SECRET_LINK".into()],
        visible_properties: {
            let mut m: std::collections::BTreeMap<String, Vec<String>> =
                std::collections::BTreeMap::new();
            m.insert("public".to_string(), vec!["name".to_string()]);
            m
        },
        max_results: 1,
    }
}

#[test]
fn default_deny_narrows_and_masks() {
    let store = Arc::new(Store::new_in_memory());
    let lmpg = Lmpg::new(store, TenantId::from_ordinal(51));

    lmpg.create_record(
        "Public",
        None,
        vec![
            ("name".into(), FieldChange::Set(json!("ok"))),
            ("salary".into(), FieldChange::Set(json!(100))),
        ],
    )
    .unwrap();
    lmpg.create_record(
        "Secret",
        None,
        vec![("name".into(), FieldChange::Set(json!("hidden")))],
    )
    .unwrap();

    let policy = policy_default_deny();
    let scope = SecurityScope::compile(&policy, AuthorizationMode::PolicyEnforced);

    let page = rushdb_authz::find_records_authorized(&lmpg, &SearchRequest::default(), &scope)
        .expect("boundary-only default request must pass");

    // Secret label row masked to empty and dropped; salary field masked out.
    assert_eq!(page.total_matched, 1);
    let rec = &page.records[0];
    assert_eq!(rec.get("name"), Some(&json!("ok")));
    assert!(rec.get("salary").is_none(), "masked properties vanish");
}

#[test]
fn mutations_rejected_outside_domain() {
    let policy = policy_default_deny();
    let scope = SecurityScope::compile(&policy, AuthorizationMode::PolicyEnforced);

    assert!(
        authorize_mutation(&scope, "Public").is_ok(),
        "granted labels pass"
    );
    match authorize_mutation(&scope, "Secret") {
        Err(rushdb_authz::AuthzError::LabelDenied(_)) => {}
        other => panic!("expected denial, got {other:?}"),
    }

    // BoundaryOnly mode keeps compatibility (label checks skipped).
    let boundary = SecurityScope::compile(&policy, AuthorizationMode::BoundaryOnly);
    assert_eq!(
        boundary.fingerprint(),
        SecurityScope::compile(&policy, AuthorizationMode::BoundaryOnly).fingerprint()
    );
    assert_ne!(
        boundary.fingerprint(),
        scope.fingerprint(),
        "mode changes fingerprint"
    );
}
