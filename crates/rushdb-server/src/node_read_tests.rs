use super::*;
use rushdb_types::command::{CommitBatch, IsolationMode, COMMAND_VERSION};
use serde_json::json;

fn on_backends(test: impl Fn(&Node)) {
    let dir = rushdb_storage::TempPath::dir("node-native");
    for persistent in [false, true] {
        let mut cfg = crate::ServerConfig {
            data_dir: persistent.then(|| dir.to_path_buf()),
            max_request_bytes: 64 * 1024 * 1024,
            listen_addr: "127.0.0.1".into(),
            tls_cert: None,
            tls_key: None,
            ..Default::default()
        };
        let node = Node::bootstrap(&mut cfg).unwrap();
        test(&node);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

fn corrupt(node: &Node, key: Vec<u8>) {
    node.store.set_expander(Arc::new(Box::new(move |_, _, txn| {
        txn.put(key.clone(), vec![1]);
        Ok(())
    })));
    let commit_key = uuid::Uuid::now_v7();
    node.store
        .commit(commit_key, |_, _| {
            Ok((
                CommitBatch {
                    command_version: COMMAND_VERSION,
                    tenant: node.scope().tenant(),
                    transaction_id: String::new(),
                    commit_key,
                    start_index: 0,
                    isolation: IsolationMode::ReadCommitted,
                    preconditions: vec![],
                    mutations: vec![],
                },
                (),
            ))
        })
        .unwrap();
    // Restore ordinary deterministic expansion after the fixture-only write.
    drop(node.scope().lmpg());
}

#[test]
fn record_get_and_schema_report_corruption_instead_of_missing_data() {
    on_backends(|node| {
        let record = node
            .scope()
            .import_json_document("Doc", &json!({"name": "before"}))
            .unwrap();
        assert_eq!(
            node.scope()
                .get_record(&hex_simple(&record.record_id))
                .unwrap()["properties"]["name"],
            json!("before")
        );
        let view = node.store.read_view().unwrap();
        let (family_key, _) = view
            .range_prefix(&key(
                node.scope().tenant(),
                &[rushdb_types::keys::P_FAMILY, b""],
            ))
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        corrupt(node, family_key);
        assert!(node
            .scope()
            .describe_properties(&json!({}))
            .unwrap_err()
            .contains("corrupt property family"));
        assert!(node
            .scope()
            .resolve_property_name("p0000000000000001")
            .unwrap_err()
            .contains("corrupt property family"));
        let error = node
            .scope()
            .get_record(&hex_simple(&record.record_id))
            .unwrap_err();
        assert!(!error.contains("not found"), "{error}");
        assert_eq!(
            node.scope()
                .record_value_in_view(&view, record.ordinal)
                .unwrap()["properties"]["name"],
            json!("before")
        );
    });
}

#[test]
fn node_retention_consumers_protect_and_release_history_floor() {
    on_backends(|node| {
        assert_eq!(node.protected_retention_floor(), None);
        node.acknowledge_retention_consumer("backup", 12).unwrap();
        node.acknowledge_retention_consumer("replica", 20).unwrap();
        assert_eq!(node.protected_retention_floor(), Some(12));
        node.acknowledge_retention_consumer("backup", 15).unwrap();
        assert_eq!(node.protected_retention_floor(), Some(15));
        assert!(node.release_retention_consumer("replica"));
        assert_eq!(node.protected_retention_floor(), Some(15));
    });
}

#[test]
fn active_read_views_protect_command_metadata_retention_floor() {
    on_backends(|node| {
        let view = node.store.read_view().unwrap();
        assert_eq!(
            node.protected_retention_floor(),
            Some(view.commit_index()),
            "an active snapshot must protect metadata needed for its retry/read state"
        );
        drop(view);
        assert_eq!(node.protected_retention_floor(), None);
    });
}

#[test]
fn retention_consumer_acknowledgements_survive_restart() {
    let dir = rushdb_storage::TempPath::dir("retention");
    let mut cfg = crate::ServerConfig {
        data_dir: Some(dir.to_path_buf()),
        ..Default::default()
    };
    {
        let node = Node::bootstrap(&mut cfg).unwrap();
        node.acknowledge_retention_consumer("backup", 42).unwrap();
        assert_eq!(node.protected_retention_floor(), Some(42));
    }
    let mut reopened_cfg = crate::ServerConfig {
        data_dir: Some(dir.to_path_buf()),
        ..Default::default()
    };
    let reopened = Node::bootstrap(&mut reopened_cfg).unwrap();
    assert_eq!(reopened.protected_retention_floor(), Some(42));
    assert!(reopened.release_retention_consumer("backup"));
    drop(reopened);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn query_response_metadata_uses_the_query_view_after_concurrent_corruption() {
    on_backends(|node| {
        node.scope()
            .import_json_document("Doc", &json!({"name": "before"}))
            .unwrap();
        let (family_key, _) = node
            .store
            .read_view()
            .unwrap()
            .range_prefix(&key(
                node.scope().tenant(),
                &[rushdb_types::keys::P_FAMILY, b""],
            ))
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let changed = std::cell::Cell::new(false);
        let (total, records) = node
            .scope()
            .search_with_cancel(&SearchRequest::default(), &|| {
                if !changed.replace(true) {
                    corrupt(node, family_key.clone());
                    node.store.gc(u64::MAX).unwrap();
                }
                false
            })
            .unwrap();
        assert_eq!(total, 1);
        assert_eq!(records[0]["properties"]["name"], json!("before"));
        assert_eq!(records[0]["propertyTypes"]["name"], json!("string"));
        assert!(node
            .scope()
            .search_with_cancel(&SearchRequest::default(), &|| false)
            .is_err());
    });
}

#[test]
fn schema_description_propagates_invalid_query_documents() {
    on_backends(|node| {
        node.scope()
            .import_json_document("Doc", &json!({"name": "before"}))
            .unwrap();
        // Unknown nested keys can be interpreted as relationship blocks by
        // the parser. Use an invalid shape of a recognized logical operator.
        let document = json!({"where": {"$and": 1}});
        assert!(rushdb_query::document::parse_document(&document).is_err());
        assert!(node.scope().describe_properties(&document).is_err());
    });
}

#[test]
fn native_record_enumeration_and_bulk_delete_preserve_label_selection() {
    on_backends(|node| {
        let a = node
            .scope()
            .import_json_document("A", &json!({"name": "one"}))
            .unwrap();
        let b = node
            .scope()
            .import_json_document("AB", &json!({"name": "two"}))
            .unwrap();
        let records = node.scope().enumerate_records("A", None).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].1, a.ordinal);
        node.scope()
            .delete_records(&json!({"labels": ["A"]}))
            .unwrap();
        assert!(node.scope().get_record(&hex_simple(&a.record_id)).is_err());
        assert_eq!(
            node.scope().get_record(&hex_simple(&b.record_id)).unwrap()["properties"]["name"],
            json!("two")
        );
        assert_eq!(node.scope().labels().unwrap(), vec!["AB"]);
    });
}

#[test]
fn bulk_delete_streams_in_bounded_pages() {
    // Regression: delete_records materialized every match and committed once
    // per record, so large deletes were unbounded in memory and slower than
    // any client timeout (left partially applied). Deletes now stream in
    // bounded pages with one commit per page: 1100 records take 3 pages
    // (500 + 500 + 100) plus a terminating empty page.
    on_backends(|node| {
        for i in 0..1100 {
            node.scope()
                .import_json_document("Bulk", &json!({"n": i}))
                .unwrap();
        }
        let affected = node
            .scope()
            .delete_records(&json!({"labels": ["Bulk"]}))
            .unwrap();
        assert_eq!(affected, 1100);
        assert!(
            node.scope()
                .enumerate_records("Bulk", None)
                .unwrap()
                .is_empty(),
            "all Bulk records must be deleted"
        );
    });
}

#[test]
fn delete_records_with_cancel_aborts_before_deleting() {
    on_backends(|node| {
        for i in 0..10 {
            node.scope()
                .import_json_document("Cancel", &json!({"n": i}))
                .unwrap();
        }
        // Cancel before the first page: nothing is deleted and the call reports
        // cancellation (E4: a long delete must be abortable, not silent).
        let err = node
            .scope()
            .delete_records_with_cancel(&json!({"labels": ["Cancel"]}), &|| true)
            .unwrap_err();
        assert!(err.contains("operation cancelled"), "got: {err}");
        assert_eq!(
            node.scope()
                .enumerate_records("Cancel", None)
                .unwrap()
                .len(),
            10,
            "a cancelled delete must not remove records"
        );
        // Uncanceled, the same call completes.
        assert_eq!(
            node.scope()
                .delete_records(&json!({"labels": ["Cancel"]}))
                .unwrap(),
            10
        );
    });
}

#[test]
fn relationship_scans_exclude_ordinal_rows_and_propagate_header_corruption() {
    on_backends(|node| {
        let a = node
            .scope()
            .import_json_document("A", &json!({"name": "one"}))
            .unwrap();
        let b = node
            .scope()
            .import_json_document("B", &json!({"name": "two"}))
            .unwrap();
        let edge = node
            .scope()
            .lmpg()
            .create_relationship("LINK", a, b, vec![])
            .unwrap();
        let query = serde_json::from_value(json!({})).unwrap();
        assert_eq!(
            node.scope().scan_relationships(&query).unwrap()["data"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(node
            .scope()
            .relationship_exists(a.ordinal, "LINK", b.ordinal)
            .unwrap());
        corrupt(
            node,
            rushdb_lmpg::reader::keys_ext::rel_row_key(
                node.scope().tenant(),
                &edge.relationship_id,
            ),
        );
        assert!(node
            .scope()
            .scan_relationships(&query)
            .unwrap_err()
            .contains("corrupt relationship header"));
        let command = serde_json::from_value(json!({"sourceLabel": "A", "targetLabel": "B", "sourceType": "LINK", "targetType": "NEW"})).unwrap();
        assert!(node.scope().retype_relations(&command).is_err());
    });
}

#[test]
fn vector_registry_stats_candidates_and_filtered_search_use_native_reads() {
    on_backends(|node| {
        let a = node
            .scope()
            .import_json_document("Doc", &json!({"name": "one"}))
            .unwrap();
        let b = node
            .scope()
            .import_json_document("Doc", &json!({"name": "two"}))
            .unwrap();
        let index = node
            .scope()
            .create_vector_index(&rushdb_protocol::VectorIndexDefinition {
                label: "Doc".into(),
                property: "name".into(),
                dimensions: 2,
                similarity: "cosine".into(),
                name: Some("doc-name-2".into()),
                model: None,
                owner: None,
            })
            .unwrap();
        let alias = index["name"].as_str().unwrap();
        node.scope()
            .upsert_vector_index(
                alias,
                &[
                    rushdb_protocol::VectorItem {
                        record_id: hex_simple(&a.record_id),
                        vector: vec![1., 0.],
                        source_hash: None,
                        index_generation: None,
                    },
                    rushdb_protocol::VectorItem {
                        record_id: hex_simple(&b.record_id),
                        vector: vec![0., 1.],
                        source_hash: None,
                        index_generation: None,
                    },
                ],
            )
            .unwrap();
        let stats = node.scope().vector_index_stats(alias).unwrap();
        assert_eq!(stats["totalRecords"], 2);
        assert_eq!(stats["indexedRecords"], 2);
        let query: rushdb_protocol::VectorQuery = serde_json::from_value(json!({
            "labels": ["Doc"], "property": "name", "vector": [1.0, 0.0], "index": alias,
            "predicate": {"name": "two"}, "limit": 1
        }))
        .unwrap();
        let result = node.scope().search_vectors(&query).unwrap();
        assert_eq!(result["data"][0]["properties"]["name"], "two");
        assert_eq!(
            node.scope().vector_search(alias, &[1., 0.], 1).unwrap()[0].0,
            hex_simple(&a.record_id)
        );
        let view = Arc::new(node.store.read_view().unwrap());
        node.scope()
            .bind_field_vector_external(alias, &hex_simple(&b.record_id), "name", &[1., 0.])
            .unwrap();
        node.scope()
            .lmpg()
            .update_fields(
                b,
                vec![("name".into(), rushdb_lmpg::FieldChange::Set(json!("three")))],
            )
            .unwrap();
        node.store.gc(u64::MAX).unwrap();
        let pinned = node.scope().search_vectors_in_view(&query, &view).unwrap();
        assert_eq!(pinned["data"][0]["properties"]["name"], "two");
        assert_eq!(pinned["data"][0]["score"], 0.0);
        assert!(node.scope().search_vectors(&query).unwrap()["data"]
            .as_array()
            .unwrap()
            .is_empty());
        corrupt(node, node.scope().vector_index_key(alias).into_bytes());
        assert!(node.scope().list_vector_indexes().is_err());
        assert!(node.scope().vector_index_stats(alias).is_err());
        assert!(node
            .scope()
            .create_vector_index(&rushdb_protocol::VectorIndexDefinition {
                label: "Doc".into(),
                property: "name".into(),
                dimensions: 2,
                similarity: "cosine".into(),
                name: Some(alias.into()),
                model: None,
                owner: None,
            })
            .is_err());
    });
}

#[test]
fn short_vector_family_signature_is_an_error_not_a_panic_or_missing_candidate() {
    on_backends(|node| {
        let a = node
            .scope()
            .import_json_document("Doc", &json!({"name": "one"}))
            .unwrap();
        let index = node
            .scope()
            .create_vector_index(&rushdb_protocol::VectorIndexDefinition {
                label: "Doc".into(),
                property: "name".into(),
                dimensions: 2,
                similarity: "cosine".into(),
                name: Some("unused".into()),
                model: None,
                owner: None,
            })
            .unwrap();
        let index_name = index["name"].as_str().unwrap().to_owned();
        corrupt(
            node,
            key(
                node.scope().tenant(),
                &[
                    P_FAMILY_SIG,
                    b"default",
                    &[OwnerKind::Record.as_byte()],
                    b"name",
                ],
            ),
        );
        assert!(node
            .scope()
            .bind_field_vector_external(&index_name, &hex_simple(&a.record_id), "name", &[1., 0.])
            .unwrap_err()
            .contains("corrupt property family signature"));
        let query = serde_json::from_value(
            json!({"label": "Doc", "property": "name", "index": index_name, "skip": 0, "limit": 10}),
        )
        .unwrap();
        assert!(node
            .scope()
            .vector_index_candidates(&query)
            .unwrap_err()
            .contains("corrupt property family signature"));
    });
}

#[test]
fn corrupt_vector_index_metadata_is_not_treated_as_record_index() {
    on_backends(|node| {
        let index = node
            .scope()
            .create_vector_index(&rushdb_protocol::VectorIndexDefinition {
                label: "Doc".into(),
                property: "embedding".into(),
                dimensions: 2,
                similarity: "cosine".into(),
                name: Some("broken".into()),
                model: None,
                owner: None,
            })
            .unwrap();
        let alias = index["name"].as_str().unwrap();
        corrupt(node, node.scope().vector_index_key(alias).into_bytes());
        let query = serde_json::from_value(json!({
            "label": "Doc",
            "property": "embedding",
            "index": alias,
            "skip": 0,
            "limit": 10
        }))
        .unwrap();
        assert!(node
            .scope()
            .vector_index_candidates(&query)
            .unwrap_err()
            .contains("corrupt vector index metadata"));
        assert!(node
            .scope()
            .upsert_vector_index(alias, &[])
            .unwrap_err()
            .contains("corrupt vector index metadata"));

        let missing = serde_json::from_value(json!({
            "label": "Doc",
            "property": "embedding",
            "index": "does-not-exist",
            "skip": 0,
            "limit": 10
        }))
        .unwrap();
        assert_eq!(
            node.scope().vector_index_candidates(&missing).unwrap_err(),
            "vector index not found"
        );
    });
}

#[test]
fn cursor_walk_visits_every_record_once_on_a_pinned_snapshot() {
    // E6: a multi-page cursor walk over one pinned view visits every match
    // exactly once with a stable total, and writes committed mid-walk do
    // not leak into the pages (snapshot isolation). Continuations reuse
    // the creation total instead of rescanning for a count.
    on_backends(|node| {
        for i in 0..1200 {
            node.scope()
                .import_json_document("Walk", &json!({"n": i}))
                .unwrap();
        }
        let search = || rushdb_query::SearchRequest {
            labels: vec!["Walk".to_string()],
            pagination: rushdb_query::Pagination {
                skip: 0,
                limit: Some(500),
            },
            ..Default::default()
        };
        let no_cancel = || false;
        let (total, first, cursor) = node
            .scope()
            .start_cursor(&search(), 60_000, &no_cancel)
            .unwrap();
        assert_eq!(total, 1200);
        assert_eq!(first.len(), 500);
        let cursor = cursor.expect("three pages expected");
        // Mid-walk write: must not affect the pinned walk.
        node.scope()
            .import_json_document("Walk", &json!({"n": 9999}))
            .unwrap();
        let mut ids: std::collections::HashSet<String> = first.iter().map(record_id_of).collect();
        let (total, second, cursor) = node
            .scope()
            .continue_cursor(&cursor, 500, &no_cancel)
            .unwrap();
        assert_eq!(total, 1200, "pinned total survives concurrent writes");
        ids.extend(second.iter().map(record_id_of));
        let cursor = cursor.expect("third page expected");
        let (total, third, cursor) = node
            .scope()
            .continue_cursor(&cursor, 500, &no_cancel)
            .unwrap();
        assert_eq!(total, 1200);
        assert_eq!(third.len(), 200);
        assert!(cursor.is_none(), "walk ends after the last page");
        ids.extend(third.iter().map(record_id_of));
        assert_eq!(ids.len(), 1200, "every record visited exactly once");
        assert!(
            !ids.iter().any(std::string::String::is_empty),
            "all page records carry ids"
        );
    });
}

#[test]
fn cursor_walk_with_filter_keeps_stable_total_across_pages() {
    // E6 filtered path: the known-total continuation skips the exact-count
    // rescan (the reconstruction-heavy half of a filtered page) while
    // reporting the same total on every page.
    on_backends(|node| {
        for i in 0..300 {
            node.scope()
                .import_json_document("Filt", &json!({"n": i}))
                .unwrap();
        }
        let mut search = rushdb_query::SearchRequest {
            labels: vec!["Filt".to_string()],
            pagination: rushdb_query::Pagination {
                skip: 0,
                limit: Some(100),
            },
            ..Default::default()
        };
        search.where_ =
            Some(rushdb_query::document::parse_where(&json!({"n": {"$gte": 100}})).unwrap());
        let no_cancel = || false;
        let (total, first, cursor) = node
            .scope()
            .start_cursor(&search, 60_000, &no_cancel)
            .unwrap();
        assert_eq!(total, 200);
        assert_eq!(first.len(), 100);
        let (total, second, cursor) = node
            .scope()
            .continue_cursor(&cursor.unwrap(), 100, &no_cancel)
            .unwrap();
        assert_eq!((total, second.len()), (200, 100));
        assert!(cursor.is_none());
    });
}

fn record_id_of(record: &serde_json::Value) -> String {
    record
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

#[test]
fn delete_records_respects_caller_limit_and_skip() {
    // Regression: a *limited* delete must delete only that many records. The
    // batched rewrite ignored the caller's window and drained every match,
    // which turned a label-scoped limited probe into a full-label wipe.
    on_backends(|node| {
        for i in 0..10 {
            node.scope()
                .import_json_document("Del", &json!({"n": i}))
                .unwrap();
        }
        let affected = node
            .scope()
            .delete_records(&json!({"labels": ["Del"], "limit": 3}))
            .unwrap();
        assert_eq!(affected, 3, "limit must cap the delete");
        assert_eq!(
            node.scope().enumerate_records("Del", None).unwrap().len(),
            7
        );

        // skip leaves the first two matches in place; limit caps this call.
        let affected = node
            .scope()
            .delete_records(&json!({"labels": ["Del"], "skip": 2, "limit": 2}))
            .unwrap();
        assert_eq!(affected, 2);
        assert_eq!(
            node.scope().enumerate_records("Del", None).unwrap().len(),
            5
        );

        // No limit drains the remainder (legacy behavior).
        let affected = node
            .scope()
            .delete_records(&json!({"labels": ["Del"]}))
            .unwrap();
        assert_eq!(affected, 5);
        assert!(node
            .scope()
            .enumerate_records("Del", None)
            .unwrap()
            .is_empty());

        // limit 0 deletes nothing.
        node.scope()
            .import_json_document("Del", &json!({"n": 1}))
            .unwrap();
        assert_eq!(
            node.scope()
                .delete_records(&json!({"labels": ["Del"], "limit": 0}))
                .unwrap(),
            0
        );
        assert_eq!(
            node.scope().enumerate_records("Del", None).unwrap().len(),
            1
        );
    });
}

#[test]
fn equality_and_in_lookups_resolve_through_the_index_path() {
    // Exercises the equality-posting candidate path end to end: a selective
    // equality on a property must return exactly its matches (and be counted
    // exactly) without depending on the scan order.
    on_backends(|node| {
        for i in 0..200 {
            node.scope()
                .import_json_document(
                    "L",
                    &json!({
                        "extId": format!("id-{i:03}"),
                        "kind": if i == 7 { "needle" } else { "hay" },
                    }),
                )
                .unwrap();
        }
        let search = |predicate| rushdb_query::SearchRequest {
            labels: vec!["L".to_string()],
            where_: Some(rushdb_query::document::parse_where(predicate).unwrap()),
            pagination: rushdb_query::Pagination::default(),
            ..Default::default()
        };
        let by_id = json!({"extId": "id-007"});
        let (total, records) = node.scope().search(&search(&by_id)).unwrap();
        assert_eq!((total, records.len()), (1, 1));
        assert_eq!(records[0]["properties"]["extId"], json!("id-007"));

        let missing_id = json!({"extId": "id-999"});
        let (total, _) = node.scope().search(&search(&missing_id)).unwrap();
        assert_eq!(total, 0, "absent value must match nothing");

        let in_pred = json!({"kind": {"$in": ["needle"]}});
        let (total, _) = node.scope().search(&search(&in_pred)).unwrap();
        assert_eq!(total, 1, "$in index path");
    });
}

#[test]
fn metadata_cache_is_exact_and_invalidated_by_commit() {
    // Commit-index-keyed derived metadata: entries are exact for the current
    // commit and a later commit misses, so discovery loops stay O(1) without
    // ever serving stale labels/counts.
    on_backends(|node| {
        assert!(node.scope().labels().unwrap().is_empty());
        node.scope().import_json_document("A", &json!({})).unwrap();
        assert_eq!(node.scope().labels().unwrap(), vec!["A"]);
        assert_eq!(
            node.scope().label_counts().unwrap(),
            vec![("A".to_string(), 1)]
        );
        node.scope().import_json_document("A", &json!({})).unwrap();
        assert_eq!(
            node.scope().label_counts().unwrap(),
            vec![("A".to_string(), 2)],
            "a commit must invalidate the cached counts"
        );
        node.scope().import_json_document("B", &json!({})).unwrap();
        assert_eq!(node.scope().labels().unwrap(), vec!["A", "B"]);
    });
}

#[test]
fn metadata_and_property_caches_share_one_logical_budget() {
    let mut cfg = crate::ServerConfig::default();
    cfg.resources.logical_cache_bytes = crate::resource_config::ByteSize(64 * 1024);
    let node = Node::bootstrap(&mut cfg).unwrap();
    for index in 0..12 {
        let mut properties = serde_json::Map::new();
        properties.insert(format!("property{index}"), json!(index));
        node.scope()
            .import_json_document(&format!("Label{index}"), &properties.into())
            .unwrap();
    }
    let _ = node.scope().labels().unwrap();
    let view = node.store.read_view().unwrap();
    let _ = node.scope().proptypes_cached(&view).unwrap();
    let metadata_entries = node
        .metadata_cache
        .lock()
        .unwrap()
        .as_ref()
        .map_or(0, |cache| cache.stats().entries);
    let property_entries = node
        .property_cache
        .lock()
        .unwrap()
        .as_ref()
        .map_or(0, |cache| cache.stats().entries);
    let snapshot = node.store.resource_budget().snapshot();
    assert!(snapshot.used <= snapshot.limit);
    assert!(
        metadata_entries + property_entries > 0,
        "at least one derived cache should be resident"
    );
}

#[test]
fn relationship_catalog_matches_scan_for_groups_and_property_shapes() {
    on_backends(|node| {
        let lmpg = node.scope().lmpg();
        let first = node.scope().import_json_document("A", &json!({})).unwrap();
        let second = node.scope().import_json_document("B", &json!({})).unwrap();
        let third = node.scope().import_json_document("C", &json!({})).unwrap();
        lmpg.create_relationship(
            "R",
            first,
            second,
            vec![("w".into(), rushdb_lmpg::plan::FieldChange::Set(json!(1)))],
        )
        .unwrap();
        lmpg.create_relationship(
            "R",
            third,
            second,
            vec![("w".into(), rushdb_lmpg::plan::FieldChange::Set(json!(2)))],
        )
        .unwrap();
        lmpg.create_relationship("S", first, third, vec![]).unwrap();

        let catalog = node.scope().describe_relationships().unwrap();
        let scan = node.scope().describe_relationships_scan().unwrap();

        let groups = |list: &[rushdb_protocol::RelationshipDescription]| {
            list.iter()
                .map(|d| {
                    (
                        d.source_label.clone(),
                        d.relationship_type.clone(),
                        d.target_label.clone(),
                        d.count,
                    )
                })
                .collect::<std::collections::BTreeSet<_>>()
        };
        assert_eq!(groups(&catalog), groups(&scan), "group counts must match");

        // Property names/types per relationship type agree with the scan.
        let shapes = |list: &[rushdb_protocol::RelationshipDescription]| {
            let mut out = std::collections::BTreeMap::<
                String,
                std::collections::BTreeSet<(String, String)>,
            >::new();
            for d in list {
                for p in &d.properties {
                    out.entry(d.relationship_type.clone())
                        .or_default()
                        .insert((p.name.clone(), p.value_type.clone()));
                }
            }
            out
        };
        assert_eq!(shapes(&catalog), shapes(&scan));
        // Property counts are per shape: each R shape has one edge carrying w.
        for r in catalog.iter().filter(|d| d.relationship_type == "R") {
            let w = r.properties.iter().find(|p| p.name == "w").expect("w");
            assert_eq!(w.relationships_count, 1, "{r:?}");
        }
    });
}

/// The counter-based catalog describe must return exactly what the
/// member-scan oracle returns, across scalars, arrays, empty arrays,
/// explicit nulls, type splits, a retype, and a delete.
#[test]
fn catalog_detail_counts_match_the_member_scan() {
    let mut cfg = crate::ServerConfig {
        data_dir: None,
        ..Default::default()
    };
    let node = Node::bootstrap(&mut cfg).unwrap();

    for i in 0..24 {
        let mut obj = json!({"name": format!("a{i}")});
        if i % 2 == 0 {
            obj["n"] = json!(i);
        }
        if i % 3 == 0 {
            obj["tags"] = json!(["x", "y"]);
        }
        if i == 1 {
            obj["empty"] = json!([]);
            obj["nv"] = json!(null);
        }
        node.scope().import_json_document("A", &obj).unwrap();
    }
    // Type split: `n` as int on A and float on B are distinct pids.
    for i in 0..9 {
        let n = if i % 3 == 0 {
            json!(i)
        } else {
            json!(f64::from(i) + 0.25)
        };
        node.scope()
            .import_json_document("B", &json!({"n": n, "name": format!("b{i}")}))
            .unwrap();
    }
    // Label-scoped counters must move on retype and delete.
    let moved = node
        .scope()
        .import_json_document("A", &json!({"name": "a-move", "n": 99}))
        .unwrap();
    node.scope()
        .retype_record(&hex_simple(&moved.record_id), "A", "C")
        .unwrap();
    node.scope()
        .delete_records(&json!({"labels": ["A"], "where": {"name": "a0"}}))
        .unwrap();

    let view = Arc::new(node.store.read_view().unwrap());
    let meta = node.scope().property_meta_in_view(&view).unwrap();
    let no_siblings = |_: &str, _: &rushdb_types::model::LogicalType| Vec::new();
    for body in [
        json!({"detail": "catalog"}),
        json!({"detail": "catalog", "labels": ["A"]}),
        json!({"detail": "catalog", "labels": ["B", "C"]}),
    ] {
        let mut counter = node
            .scope()
            .describe_properties_catalog(&view, &meta, &[], &no_siblings, &body)
            .unwrap();
        let mut scan = node
            .scope()
            .describe_properties_catalog_scan(&view, &meta, &no_siblings, &body)
            .unwrap();
        let sort = |entries: &mut Vec<rushdb_protocol::PropertyDescription>| {
            entries.sort_by(|a, b| {
                (a.label.clone(), a.id.clone()).cmp(&(b.label.clone(), b.id.clone()))
            });
        };
        sort(&mut counter);
        sort(&mut scan);
        assert_eq!(counter.len(), scan.len(), "entry count for {body}");
        for (c, s) in counter.iter().zip(scan.iter()) {
            assert_eq!((&c.label, &c.id), (&s.label, &s.id), "identity for {body}");
            assert_eq!(
                c.records_count, s.records_count,
                "records_count for {} label {:?}",
                c.name, c.label
            );
            assert_eq!(
                c.is_array, s.is_array,
                "is_array for {} label {:?}",
                c.name, c.label
            );
            assert_eq!(c.name, s.name);
            assert_eq!(c.value_type, s.value_type);
        }
    }
}

#[test]
fn metrics_expose_storage_byte_accounting() {
    let node = Node::bootstrap(&mut crate::ServerConfig::default()).unwrap();
    for i in 0..50 {
        node.scope()
            .import_json_document("S", &json!({"u": i}))
            .unwrap();
    }
    node.refresh_storage_stats(std::time::Duration::ZERO);
    let stats = node.storage_stats().expect("storage stats computed");
    assert!(stats.data_keys > 0, "canonical data rows counted");
    assert_eq!(stats.log_keys, 50, "one command-log row per commit");
    let rendered = node.metrics_render();
    for metric in [
        "rushdb_storage_data_bytes",
        "rushdb_storage_history_bytes",
        "rushdb_storage_log_bytes",
        "rushdb_storage_receipt_bytes",
    ] {
        assert!(rendered.contains(metric), "missing {metric} in metrics");
    }
    drop(node);
}

#[test]
fn metrics_expose_attributable_work_counters_and_boot_phases() {
    let node = Node::bootstrap(&mut crate::ServerConfig::default()).unwrap();
    node.observe_work("sql_scan", "candidate_rows", 7);
    node.observe_work("sql_scan", "reconstructed_rows", 3);
    node.observe_boot_phase("storage", 0.001);
    let rendered = node.metrics_render();
    assert!(rendered.contains("rushdb_operation_work_total"));
    assert!(rendered.contains("operation=\"sql_scan\""));
    assert!(rendered.contains("counter=\"candidate_rows\""));
    assert!(rendered.contains("rushdb_boot_phase_duration_seconds"));
    assert!(rendered.contains("phase=\"config\""));
    assert!(rendered.contains("phase=\"storage\""));
    assert!(rendered.contains("phase=\"total\""));
}

#[test]
fn sql_scan_records_runtime_work_counters() {
    let node = Node::bootstrap(&mut crate::ServerConfig::default()).unwrap();
    node.scope()
        .import_json_document("Person", &json!({"name": "Ada", "salary": 150_000}))
        .unwrap();
    let rows = node
        .scope()
        .sql("SELECT name FROM person WHERE salary >= 100000")
        .unwrap();
    assert_eq!(rows.len(), 1);
    let rendered = node.metrics_render();
    assert!(rendered.contains("operation=\"sql_scan\""));
    assert!(rendered.contains("counter=\"candidate_rows\""));
    assert!(rendered.contains("counter=\"reconstructed_rows\""));
    assert!(rendered.contains("counter=\"posting_bytes\""));
}

#[test]
fn raft_node_can_reopen_repeatedly_after_owned_runtime_shutdown() {
    let dir = rushdb_storage::TempPath::dir("raft-reopen");
    for round in 0..3 {
        let mut cfg = crate::ServerConfig {
            data_dir: Some(dir.to_path_buf()),
            pgwire_port: 0,
            raft: Some(crate::RaftConfig { size: 1 }),
            ..crate::ServerConfig::default()
        };
        let node = Node::bootstrap(&mut cfg).expect("raft bootstrap");
        node.scope()
            .import_json_document("Round", &json!({"round": round}))
            .expect("raft commit");
        assert!(
            node.committed_version() >= u64::try_from(round + 1).expect("small round fits u64")
        );
        drop(node);
    }
    let mut cfg = crate::ServerConfig {
        data_dir: Some(dir.to_path_buf()),
        pgwire_port: 0,
        raft: Some(crate::RaftConfig { size: 1 }),
        ..crate::ServerConfig::default()
    };
    let reopened = Node::bootstrap(&mut cfg).expect("reopen raft store");
    assert_eq!(reopened.committed_version(), 3);
    drop(reopened);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn label_delete_of_fat_records_removes_the_label() {
    on_backends(|node| {
        let blob = "x".repeat(8 * 1024);
        for i in 0..20 {
            node.scope()
                .import_json_document("Fat", &json!({"n": i, "blob": blob}))
                .unwrap();
        }
        node.scope()
            .import_json_document("Keep", &json!({"n": 1}))
            .unwrap();
        let affected = node
            .scope()
            .delete_records(&json!({"labels": ["Fat"]}))
            .unwrap();
        assert_eq!(affected, 20);
        assert!(node
            .scope()
            .enumerate_records("Fat", None)
            .unwrap()
            .is_empty());
        assert_eq!(
            node.scope().enumerate_records("Keep", None).unwrap().len(),
            1
        );
    });
}

#[test]
fn link_pairs_skip_existing_without_scanning_unrelated_edges() {
    on_backends(|node| {
        let source = node
            .scope()
            .import_json_document("Src", &json!({"n": 0}))
            .unwrap();
        let mut targets = Vec::new();
        for i in 0..40 {
            targets.push(
                node.scope()
                    .import_json_document("Tgt", &json!({"n": i}))
                    .unwrap(),
            );
        }
        let extra = node
            .scope()
            .import_json_document("Tgt", &json!({"n": 40}))
            .unwrap();
        let pairs: Vec<rushdb_protocol::RelationshipPair> = targets
            .iter()
            .map(|tgt| rushdb_protocol::RelationshipPair {
                source_id: hex_simple(&source.record_id),
                target_id: hex_simple(&tgt.record_id),
                relation_type: None,
            })
            .collect();
        let first = node
            .scope()
            .link_records_pairs(&rushdb_protocol::RelationshipPairsMutation {
                relation_type: "LINK".into(),
                direction: rushdb_protocol::Direction::Out,
                pairs,
                properties: json!({}),
            })
            .unwrap();
        assert_eq!(first["created"], 40);
        let second = node
            .scope()
            .link_records_pairs(&rushdb_protocol::RelationshipPairsMutation {
                relation_type: "LINK".into(),
                direction: rushdb_protocol::Direction::Out,
                pairs: vec![
                    rushdb_protocol::RelationshipPair {
                        source_id: hex_simple(&source.record_id),
                        target_id: hex_simple(&extra.record_id),
                        relation_type: None,
                    },
                    rushdb_protocol::RelationshipPair {
                        source_id: hex_simple(&source.record_id),
                        target_id: hex_simple(&targets[0].record_id),
                        relation_type: None,
                    },
                ],
                properties: json!({}),
            })
            .unwrap();
        assert_eq!(second["created"], 1);
        assert_eq!(second["skipped_existing"], 1);
    });
}

/// Release profile for the native load composition. Not part of `make gate`.
/// Run: `cargo test -p rushdb-server --release -- --ignored load_phase_profile --nocapture`
#[test]
#[ignore = "prints phase timings; run in release when attributing load cost"]
fn load_phase_profile() {
    let dir = rushdb_storage::TempPath::dir("load-phase");
    let mut cfg = crate::ServerConfig {
        data_dir: Some(dir.to_path_buf()),
        max_request_bytes: 64 * 1024 * 1024,
        listen_addr: "127.0.0.1".into(),
        tls_cert: None,
        tls_key: None,
        ..Default::default()
    };
    let node = Node::bootstrap(&mut cfg).unwrap();
    let n = 1_000usize;
    let docs = serde_json::Value::Array(
        (0..n)
            .map(|i| json!({"i": i, "name": format!("r{i}")}))
            .collect(),
    );
    let t0 = std::time::Instant::now();
    let ingested = node
        .scope()
        .ingest_documents(Some("Load"), &docs, &json!({"returnResult": true}))
        .unwrap();
    let ingest_ms = t0.elapsed().as_millis();
    let data = ingested["data"].as_array().expect("ingest returns records");
    assert_eq!(data.len(), n);
    let ids: Vec<String> = data
        .iter()
        .map(|rec| rec["id"].as_str().unwrap().to_string())
        .collect();
    let pairs: Vec<rushdb_protocol::RelationshipPair> = (0..750)
        .map(|i| rushdb_protocol::RelationshipPair {
            source_id: ids[i].clone(),
            target_id: ids[i + 1].clone(),
            relation_type: None,
        })
        .collect();
    let t1 = std::time::Instant::now();
    let linked = node
        .scope()
        .link_records_pairs(&rushdb_protocol::RelationshipPairsMutation {
            relation_type: "REL".into(),
            direction: rushdb_protocol::Direction::Out,
            pairs,
            properties: json!({}),
        })
        .unwrap();
    let link_ms = t1.elapsed().as_millis();
    assert_eq!(linked["created"], 750);
    let no_cancel = || false;
    let search = rushdb_query::SearchRequest {
        labels: vec!["Load".into()],
        pagination: rushdb_query::Pagination {
            skip: 0,
            limit: Some(200),
        },
        ..Default::default()
    };
    let t2 = std::time::Instant::now();
    let (total, page, cursor) = node
        .scope()
        .start_cursor(&search, 60_000, &no_cancel)
        .unwrap();
    let cursor_ms = t2.elapsed().as_millis();
    assert_eq!(total, n as u64);
    assert_eq!(page.len(), 200);
    assert!(cursor.is_some());
    let mut offset = search;
    offset.pagination.skip = 800;
    let t3 = std::time::Instant::now();
    let (_off_total, off_page) = node
        .scope()
        .search_with_cancel(&offset, &no_cancel)
        .unwrap();
    let offset_ms = t3.elapsed().as_millis();
    assert_eq!(off_page.len(), 200);
    let t4 = std::time::Instant::now();
    let described = node.scope().describe_relationships().unwrap();
    let describe_ms = t4.elapsed().as_millis();
    assert_eq!(described.len(), 1);
    assert_eq!(described[0].count, 750);
    let t5 = std::time::Instant::now();
    let walked = node
        .scope()
        .scan_relationships(&rushdb_protocol::RelationshipQuery {
            limit: Some(u64::MAX),
            ..Default::default()
        })
        .unwrap();
    let walk_ms = t5.elapsed().as_millis();
    assert_eq!(walked["total"], 750);
    eprintln!(
        "load_phase n={n} ingest_ms={ingest_ms} link_ms={link_ms} cursor_page_ms={cursor_ms} offset_skip800_ms={offset_ms} describe_relationships_ms={describe_ms} scan_relationships_ms={walk_ms}"
    );
    drop(node);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn require_vector_index_name_gate_refuses_derived_alias() {
    let mut cfg = crate::ServerConfig::default();
    cfg.resources.require_vector_index_name = true;
    let node = Node::bootstrap(&mut cfg).unwrap();
    assert!(node.scope().require_vector_index_name());

    // createVector without a name is refused.
    let err = node
        .scope()
        .create_vector_index(&rushdb_protocol::VectorIndexDefinition {
            name: None,
            label: "Doc".into(),
            property: "emb".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap_err();
    assert!(err.contains("requires an explicit `name`"), "got: {err}");

    // An explicit name works.
    node.scope()
        .create_vector_index(&rushdb_protocol::VectorIndexDefinition {
            name: Some("doc-emb-2".into()),
            label: "Doc".into(),
            property: "emb".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();

    // search.vector without an index is refused.
    let err = node
        .scope()
        .search_vectors(&rushdb_protocol::VectorQuery {
            labels: vec!["Doc".into()],
            property: "emb".into(),
            vector: vec![1.0, 0.0],
            similarity: "cosine".into(),
            index: None,
            predicate: json!({}),
            offset: 0,
            limit: 1,
            ef: None,
            mode: None,
            score_threshold: None,
        })
        .unwrap_err();
    assert!(err.contains("requires an explicit `index`"), "got: {err}");

    // Inline `$vectors` without an index is refused before any record exists.
    let err = node
        .scope()
        .create_record(
            "Doc",
            &json!({ "n": 1 }),
            &[json!({ "propertyName": "emb", "vector": [1.0, 0.0] })],
            &json!({}),
        )
        .unwrap_err();
    assert!(err.contains("require an explicit `index`"), "got: {err}");
    assert!(
        node.scope()
            .enumerate_records("Doc", None)
            .unwrap()
            .is_empty(),
        "a refused inline vector must not create the record"
    );

    let err = node
        .scope()
        .create_record(
            "Doc",
            &json!({ "n": 2 }),
            &[json!({ "propertyName": "emb", "vector": "not-a-vector" })],
            &json!({}),
        )
        .unwrap_err();
    assert!(err.contains("invalid inline vector"), "got: {err}");
}

#[test]
fn derived_vector_alias_still_resolves_without_the_gate() {
    let mut cfg = crate::ServerConfig::default();
    cfg.resources.require_vector_index_name = false;
    let node = Node::bootstrap(&mut cfg).unwrap();
    assert!(!node.scope().require_vector_index_name());
    let row = node
        .scope()
        .create_vector_index(&rushdb_protocol::VectorIndexDefinition {
            name: None,
            label: "Doc".into(),
            property: "emb".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    assert_eq!(row["nameSource"], "derived");
}

#[test]
fn reclaim_vector_slots_erases_superseded_slots_after_gc() {
    let node = Node::bootstrap(&mut crate::ServerConfig::default()).unwrap();
    let rec = node
        .scope()
        .import_json_document("Doc", &json!({ "emb": [1.0, 0.0] }))
        .unwrap();
    node.scope()
        .create_vector_index(&rushdb_protocol::VectorIndexDefinition {
            name: Some("idx".into()),
            label: "Doc".into(),
            property: "emb".into(),
            dimensions: 2,
            similarity: "cosine".into(),
            model: None,
            owner: None,
        })
        .unwrap();
    let id = hex_simple(&rec.record_id);
    node.scope()
        .upsert_vector_index(
            "idx",
            &[rushdb_protocol::VectorItem {
                record_id: id.clone(),
                vector: vec![1.0, 0.0],
                source_hash: None,
                index_generation: None,
            }],
        )
        .unwrap();
    // Re-bind: allocates a fresh slot and stages the superseded one.
    node.scope()
        .upsert_vector_index(
            "idx",
            &[rushdb_protocol::VectorItem {
                record_id: id,
                vector: vec![0.0, 1.0],
                source_hash: None,
                index_generation: None,
            }],
        )
        .unwrap();
    node.store.gc(u64::MAX).unwrap();
    let reclaimed = node.reclaim_vector_slots(u64::MAX).unwrap();
    assert!(
        reclaimed >= 1,
        "expected a superseded slot reclaimed, got {reclaimed}"
    );
    // The current binding still searches correctly.
    let result = node
        .scope()
        .search_vectors(&rushdb_protocol::VectorQuery {
            labels: vec!["Doc".into()],
            property: "emb".into(),
            vector: vec![0.0, 1.0],
            similarity: "cosine".into(),
            index: Some("idx".into()),
            predicate: json!({}),
            offset: 0,
            limit: 10,
            ef: None,
            mode: None,
            score_threshold: None,
        })
        .unwrap();
    assert_eq!(result["data"].as_array().map_or(0, Vec::len), 1);
}

#[test]
fn reclaim_vector_slots_discovers_cold_tenants_from_the_directory() {
    let node = Node::bootstrap(&mut crate::ServerConfig::default()).unwrap();
    // A registered tenant with no resident engine (cold) is still found:
    // enumeration reads the tenant directory, not resident caches.
    let cold_tenant =
        rushdb_lmpg::tenancy::resolve_or_register_scope(&node.store, "cold-tenant").unwrap();
    let lmpg = rushdb_lmpg::Lmpg::new(Arc::clone(&node.store), cold_tenant);
    let engine = rushdb_vector::VectorEngine::new(Arc::new(lmpg));
    engine.ensure_space("cold", 2).unwrap();
    let source = rushdb_vector::SourceRef {
        kind: rushdb_types::ids::OwnerKind::Record,
        owner_ord: 1,
        pid: 1,
        elem: 0,
    };
    engine.bind_external("cold", source, &[1.0, 0.0]).unwrap();
    engine.bind_external("cold", source, &[0.0, 1.0]).unwrap();
    node.store.gc(u64::MAX).unwrap();
    drop(engine);
    assert!(node.reclaim_vector_slots(u64::MAX).unwrap() >= 1);
}

#[test]
fn metrics_render_serves_cached_stats_without_scanning() {
    // A fresh node has no cached byte accounting; rendering must not scan the
    // keyspace (the scan is O(keyspace) and runs off the request path).
    let node = Node::bootstrap(&mut crate::ServerConfig::default()).unwrap();
    let rendered = node.metrics_render();
    assert!(rendered.contains("rushdb_storage_file_bytes"));
    assert!(rendered.contains("rushdb_retention_consumers"));
    assert!(rendered.contains("rushdb_retention_protected_floor"));
    // Store-level gauges render without a keyspace scan.
    assert!(rendered.contains("rushdb_sequencer_inflight_keys"));
    assert!(rendered.contains("rushdb_storage_group_commits_total"));
    assert!(rendered.contains("rushdb_retention_oldest_reader_commit"));
    // RM-7 backend-mutex measurement gauges render.
    assert!(rendered.contains("rushdb_storage_backend_mutex_wait_ms_total"));
    assert!(rendered.contains("rushdb_storage_backend_mutex_hold_ms_total"));
    assert!(rendered.contains("rushdb_storage_backend_mutex_hold_max_ms"));
    // An explicit refresh populates the cache the scrape then serves.
    node.refresh_storage_stats(std::time::Duration::ZERO);
    let rendered = node.metrics_render();
    assert!(rendered.contains("rushdb_storage_data_bytes"));
}

#[test]
fn ingest_compensation_removes_create_only_records_after_binding_failure() {
    let node = Node::bootstrap(&mut crate::ServerConfig::default()).unwrap();
    let record = node
        .scope()
        .import_json_document("Doc", &serde_json::json!({"name": "temporary"}))
        .unwrap();
    node.scope()
        .rollback_ingest_records(&[rushdb_lmpg::import::UpsertIdMap {
            draft_id: record.record_id,
            persisted_id: record.record_id,
        }])
        .unwrap();
    assert!(node
        .scope()
        .get_record(&crate::node::hex_simple(&record.record_id))
        .is_err());
}

// RM-3: Node's own tenant-slot guard was deleted — `BoundedScheduler` is now
// the single admission owner. The equivalent coverage (capacity released
// after a job completes, including a panicking one) lives in
// `rushdb-engine/src/scheduler.rs`'s own tests
// (`per_tenant_cap_rejects_work_before_global_queue_is_full`,
// `panic_in_tenant_job_is_typed_and_releases_the_tenant_slot`), which this
// test duplicated at a layer that no longer owns the policy.

/// C2 (cloud-capacity F2): the skip-scan tenant enumeration must return exactly
/// the tenants a naive walk of every `T/` key finds — including tenant ids at
/// the byte extremes and one whose bytes contain `/` (0x2F) and `0` (0x30),
/// the separator and its successor the skip-scan seeks past.
/// C0 (cloud-capacity F12): durable commits feed the commit-latency histogram
/// (it was never observed), and the WAL / conflict-window / in-flight-body /
/// GC-tick families are exported.
#[test]
fn commit_latency_and_capacity_gauges_are_exported() {
    on_backends(|node| {
        node.scope()
            .create_record("Doc", &json!({ "n": 1 }), &[], &json!({}))
            .unwrap();
        let rendered = node.metrics_render();
        let count = rendered
            .lines()
            .find_map(|line| line.strip_prefix("rushdb_commit_latency_seconds_count "))
            .and_then(|value| value.trim().parse::<f64>().ok())
            .expect("commit latency histogram exported");
        assert!(count >= 1.0, "a durable commit must be observed: {count}");
        for family in [
            "rushdb_wal_retained_bytes",
            "rushdb_conflict_window_bytes",
            "rushdb_http_inflight_body_bytes",
            "rushdb_gc_tick_seconds",
            "rushdb_storage_backend_mutex_wait_ms",
        ] {
            assert!(rendered.contains(family), "missing {family}");
        }
    });
}

/// Every commit-pipeline phase is timed and exported, so the commit
/// line's cost can be attributed before choosing how to scale it.
#[test]
fn commit_phase_timings_are_exported() {
    on_backends(|node| {
        node.scope()
            .create_record("Doc", &json!({ "n": 1 }), &[], &json!({}))
            .unwrap();
        let rendered = node.metrics_render();
        for phase in rushdb_storage::CommitPhase::ALL {
            let prefix = format!(
                "rushdb_commit_phase_seconds_count{{phase=\"{}\"}} ",
                phase.label()
            );
            let count = rendered
                .lines()
                .find_map(|line| line.strip_prefix(prefix.as_str()))
                .and_then(|value| value.trim().parse::<f64>().ok())
                .unwrap_or_else(|| panic!("phase {} not exported", phase.label()));
            assert!(count >= 1.0, "phase {} never observed", phase.label());
        }
    });
}
