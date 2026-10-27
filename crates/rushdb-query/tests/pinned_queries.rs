//! Pinned-snapshot query coverage: executor reads must resolve against the
//! caller-supplied committed view across in-memory and persistent backends.

use rushdb_lmpg::{import, plan::FieldChange, Lmpg};
use rushdb_query::{
    aggregate, aggregate_with_cancel, find_records, find_records_with_cancel, AggregateRequest,
    CollectSpec, Direction, Expr, FieldPred, QueryError, RelatedExpr, SearchRequest,
};
use rushdb_storage::{StorageError, Store};
use rushdb_types::{
    command::{CommitBatch, IsolationMode, Mutation, COMMAND_VERSION},
    ids::TenantId,
};
use serde_json::json;
use std::{cell::Cell, sync::Arc};

fn on_backends(test: impl Fn(&Lmpg)) {
    let path = rushdb_storage::TempPath::store("pinned-query");
    for persistent in [false, true] {
        let store = Arc::new(if persistent {
            Store::open(&path).unwrap()
        } else {
            Store::new_in_memory()
        });
        let lmpg = Lmpg::new(store, TenantId::from_ordinal(67));
        test(&lmpg);
    }
    rushdb_storage::remove_store(&path).unwrap();
}

#[test]
fn traversal_and_collects_share_root_snapshot_across_writes_and_gc() {
    on_backends(|lmpg| {
        let root = import::import_document(lmpg, "Root", &json!({"name": "before"})).unwrap();
        let child = import::import_document(lmpg, "Child", &json!({"name": "old child"})).unwrap();
        let edge = lmpg
            .create_relationship("HAS", root, child, vec![])
            .unwrap();
        let req = SearchRequest {
            labels: vec!["Root".into()],
            where_: Some(Expr::Related(RelatedExpr {
                target_labels: vec!["Child".into()],
                alias: None,
                rel_type: Some("HAS".into()),
                direction: Direction::Outgoing,
                hops: None,
                cycle: false,
                filter: Some(Box::new(Expr::Field(FieldPred::Eq(
                    "name".into(),
                    json!("old child"),
                )))),
            })),
            collects: vec![CollectSpec {
                steps: Vec::new(),
                alias: "children".into(),
                rel_type: Some("HAS".into()),
                direction: Direction::Outgoing,
                target_labels: vec!["Child".into()],
                fields: vec![("name".into(), "name".into())],
                nested: vec![],
                filter: None,
                order_by: vec![],
                skip: 0,
                unique: true,
                limit: None,
            }],
            ..Default::default()
        };
        let before = lmpg.store.committed_version();
        let mutated = Cell::new(false);
        let page = find_records_with_cancel(lmpg, &req, &|| {
            if !mutated.replace(true) {
                lmpg.update_fields(
                    root,
                    vec![("name".into(), FieldChange::Set(json!("after")))],
                )
                .unwrap();
                lmpg.update_fields(
                    child,
                    vec![("name".into(), FieldChange::Set(json!("new child")))],
                )
                .unwrap();
                lmpg.delete_relationship(edge).unwrap();
                import::import_document(lmpg, "Root", &json!({"name": "new root"})).unwrap();
                lmpg.store.gc(u64::MAX).unwrap();
                assert_eq!(lmpg.store.gc_watermark(), before);
            }
            false
        })
        .unwrap();
        assert!(mutated.get());
        assert_eq!(page.total_matched, 1);
        assert_eq!(page.records[0]["name"], json!("before"));
        assert_eq!(page.records[0]["children"], json!([{"name": "old child"}]));
        assert_eq!(find_records(lmpg, &req).unwrap().total_matched, 0);
        lmpg.store.gc(u64::MAX).unwrap();
        assert_eq!(lmpg.store.gc_watermark(), lmpg.store.committed_version());
    });
}

#[test]
fn aggregate_keeps_binding_and_reconstruction_at_one_version() {
    on_backends(|lmpg| {
        let root = import::import_document(lmpg, "Root", &json!({"amount": 10})).unwrap();
        let req = AggregateRequest {
            labels: vec!["Root".into()],
            sums: vec![("total".into(), "amount".into())],
            ..Default::default()
        };
        let mutated = Cell::new(false);
        let result = aggregate_with_cancel(lmpg, &req, &|| {
            if !mutated.replace(true) {
                lmpg.update_fields(root, vec![("amount".into(), FieldChange::Set(json!(99)))])
                    .unwrap();
                lmpg.store.gc(u64::MAX).unwrap();
            }
            false
        })
        .unwrap();
        assert!(mutated.get());
        assert_eq!(result.0[0].sums["total"], json!(10.0));
        assert_eq!(aggregate(lmpg, &req).unwrap()[0].sums["total"], json!(99.0));
    });
}

#[test]
fn label_prefixes_do_not_leak_into_exact_label_queries() {
    on_backends(|lmpg| {
        for label in ["A", "AB", "A/B"] {
            import::import_document(lmpg, label, &json!({"name": label})).unwrap();
        }
        let req = SearchRequest {
            labels: vec!["A".into()],
            ..Default::default()
        };
        let page = find_records(lmpg, &req).unwrap();
        assert_eq!(page.total_matched, 1);
        assert_eq!(page.records[0]["__label"], json!("A"));
    });
}

fn corrupt(lmpg: &Lmpg, key: Vec<u8>) {
    // Malformed base rows are refused by apply-time derivation, so plant them
    // with derivation suspended to prove readers fail on them.
    let commit_key = uuid::Uuid::now_v7();
    lmpg.store.inject_without_derivation(|| {
        lmpg.store
            .commit(commit_key, |_, _| {
                Ok((
                    CommitBatch {
                        command_version: COMMAND_VERSION,
                        tenant: lmpg.tenant,
                        transaction_id: String::new(),
                        commit_key,
                        start_index: 0,
                        isolation: IsolationMode::ReadCommitted,
                        preconditions: vec![],
                        mutations: vec![Mutation::PutRawKey {
                            key,
                            value: vec![1],
                        }],
                    },
                    (),
                ))
            })
            .unwrap()
    });
}

#[test]
fn corrupt_occurrence_is_an_error_not_null_or_missing_data() {
    on_backends(|lmpg| {
        let root = import::import_document(lmpg, "Root", &json!({"amount": 10})).unwrap();
        // Resolve the exact occurrence row through the owner prefix before
        // damaging it through the durable command path.
        let entries = lmpg
            .store
            .read_view()
            .unwrap()
            .range_prefix(&rushdb_types::keys::occ_owner_prefix(
                lmpg.tenant,
                rushdb_types::ids::OwnerKind::Record,
                root.ordinal,
            ))
            .unwrap();
        assert_eq!(entries.len(), 1);
        let key = entries[0].0.clone();
        assert!(lmpg.store.try_get(&key).unwrap().is_some());
        corrupt(lmpg, key);
        assert!(matches!(
            find_records(lmpg, &SearchRequest::default()),
            Err(QueryError::Storage(StorageError::CorruptLog))
        ));
    });
}

#[test]
fn malformed_record_ordinal_key_returns_error_without_panicking() {
    on_backends(|lmpg| {
        corrupt(lmpg, {
            // A header key truncated before its 8-byte ordinal.
            let mut key = rushdb_types::keys::record_hdr_prefix(lmpg.tenant);
            key.push(1);
            key
        });
        assert!(matches!(
            find_records(lmpg, &SearchRequest::default()),
            Err(QueryError::Storage(StorageError::CorruptLog))
        ));
    });
}

#[test]
fn caller_owned_view_stays_pinned_and_rejects_a_different_store() {
    on_backends(|lmpg| {
        let root = import::import_document(lmpg, "Root", &json!({"amount": 10})).unwrap();
        let view = Arc::new(lmpg.store.read_view().unwrap());
        lmpg.update_fields(root, vec![("amount".into(), FieldChange::Set(json!(20)))])
            .unwrap();
        lmpg.store.gc(u64::MAX).unwrap();
        let page = rushdb_query::find_records_in_view(
            lmpg,
            &SearchRequest::default(),
            &|| false,
            view.clone(),
        )
        .unwrap();
        assert_eq!(page.records[0]["amount"], json!(10));
        let foreign = Lmpg::new(Arc::new(Store::new_in_memory()), lmpg.tenant);
        assert!(matches!(
            rushdb_query::find_records_in_view(
                &foreign,
                &SearchRequest::default(),
                &|| false,
                view
            ),
            Err(QueryError::Storage(StorageError::InvalidRead(_)))
        ));
    });
}

#[test]
fn already_cancelled_empty_queries_do_not_report_success() {
    on_backends(|lmpg| {
        assert!(matches!(
            find_records_with_cancel(lmpg, &SearchRequest::default(), &|| true),
            Err(QueryError::Cancelled)
        ));
    });
}
