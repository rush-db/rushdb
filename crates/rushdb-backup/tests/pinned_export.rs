//! Behavioral tests for the pinned streaming tenant exporter: view pinning
//! across concurrent writes, bounded-writer streaming, corruption rejection,
//! writer/cancellation failure handling, and cross-reopen stability.

use std::{
    io::{self, Write},
    sync::Arc,
};

use rushdb_backup::{export_tenant_jsonl, export_tenant_jsonl_to};
use rushdb_lmpg::{FieldChange, Lmpg};
use rushdb_storage::{CommitOutcome, Store};
use rushdb_types::{
    command::{CommitBatch, IsolationMode, COMMAND_VERSION},
    ids::TenantId,
};
use serde_json::json;

fn on_backends(test: impl Fn(&Lmpg)) {
    let path = rushdb_storage::TempPath::store("export");
    for disk in [false, true] {
        let store = Arc::new(if disk {
            Store::open(&path).unwrap()
        } else {
            Store::new_in_memory()
        });
        test(&Lmpg::new(store, TenantId(u32::MAX - 1)));
    }
    rushdb_storage::remove_store(&path).unwrap();
}

struct HookWriter<F> {
    bytes: Vec<u8>,
    hook: Option<F>,
}
impl<F: FnOnce()> Write for HookWriter<F> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(hook) = self.hook.take() {
            hook();
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn streaming_export_retains_one_view_across_writes_delete_and_gc() {
    on_backends(|lmpg| {
        let a = lmpg
            .create_record(
                "Doc",
                None,
                vec![("name".into(), FieldChange::Set(json!("before")))],
            )
            .unwrap();
        let b = lmpg.create_record("Doc", None, vec![]).unwrap();
        lmpg.create_relationship(
            "LINK",
            a,
            b,
            vec![("weight".into(), FieldChange::Set(json!(2)))],
        )
        .unwrap();
        let other = Lmpg::new(lmpg.store.clone(), TenantId(u32::MAX - 2));
        other.create_record("OtherTenant", None, vec![]).unwrap();
        let view = lmpg.store.read_view().unwrap();
        let baseline = export_tenant_jsonl(&view, lmpg.tenant).unwrap();
        let mut writer = HookWriter {
            bytes: Vec::new(),
            hook: Some(|| {
                lmpg.update_fields(a, vec![("name".into(), FieldChange::Set(json!("after")))])
                    .unwrap();
                lmpg.delete_record(b).unwrap(); // Removes the relationship too.
                lmpg.create_record("New", None, vec![]).unwrap();
                lmpg.store.gc(u64::MAX).unwrap();
            }),
        };
        assert_eq!(
            export_tenant_jsonl_to(&view, lmpg.tenant, &mut writer).unwrap(),
            (2, 1)
        );
        assert_eq!(String::from_utf8(writer.bytes).unwrap(), baseline.0);
        let latest = export_tenant_jsonl(&lmpg.store.read_view().unwrap(), lmpg.tenant).unwrap();
        assert_eq!((latest.1, latest.2), (2, 0));
        assert!(latest.0.contains("after"));
        assert!(!latest.0.contains("OtherTenant"));
        assert_eq!(lmpg.store.gc_watermark(), view.commit_index());
    });
}

#[test]
fn streaming_export_crosses_header_batches_without_collecting_output() {
    struct CountWriter {
        lines: usize,
        largest_write: usize,
    }
    impl Write for CountWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.lines += bytes.split(|&b| b == b'\n').count() - 1;
            self.largest_write = self.largest_write.max(bytes.len());
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    on_backends(|lmpg| {
        for _ in 0..260 {
            lmpg.create_record("Batch", None, vec![]).unwrap();
        }
        let mut writer = CountWriter {
            lines: 0,
            largest_write: 0,
        };
        assert_eq!(
            export_tenant_jsonl_to(&lmpg.store.read_view().unwrap(), lmpg.tenant, &mut writer)
                .unwrap(),
            (260, 0)
        );
        assert_eq!(writer.lines, 260);
        assert!(writer.largest_write < 1024);
    });
}

fn corrupt(lmpg: &Lmpg, key: Vec<u8>, value: Option<Vec<u8>>) {
    lmpg.store.set_expander(Arc::new(Box::new(move |_, _, txn| {
        match &value {
            Some(value) => txn.put(key.clone(), value.clone()),
            None => txn.del(key.clone()),
        }
        Ok(())
    })));
    let commit_key = uuid::Uuid::now_v7();
    // Malformed base rows are refused by apply-time derivation; plant them
    // with derivation suspended.
    let result = lmpg.store.inject_without_derivation(|| {
        lmpg.store
            .commit(commit_key, |_, _| {
                Ok((
                    CommitBatch {
                        command_version: COMMAND_VERSION,
                        tenant: lmpg.tenant,
                        transaction_id: "corrupt-export-fixture".into(),
                        commit_key,
                        start_index: 0,
                        isolation: IsolationMode::ReadCommitted,
                        preconditions: vec![],
                        mutations: vec![],
                    },
                    (),
                ))
            })
            .unwrap()
    });
    assert!(matches!(result, CommitOutcome::Committed { .. }));
}

#[test]
fn corrupt_headers_ordinals_and_endpoints_fail_instead_of_silently_omitting_rows() {
    for mode in 0..4 {
        on_backends(|lmpg| {
            let a = lmpg.create_record("Doc", None, vec![]).unwrap();
            let b = lmpg.create_record("Doc", None, vec![]).unwrap();
            lmpg.create_relationship("LINK", a, b, vec![]).unwrap();
            match mode {
                0 => corrupt(
                    lmpg,
                    rushdb_types::keys::record_id_key(lmpg.tenant, &a.record_id),
                    Some(b"invalid json".to_vec()),
                ),
                1 => corrupt(
                    lmpg,
                    rushdb_types::keys::record_hdr_key(lmpg.tenant, a.ordinal),
                    None,
                ),
                2 => corrupt(
                    lmpg,
                    rushdb_types::keys::record_id_key(lmpg.tenant, &b.record_id),
                    None,
                ),
                _ => {
                    let raw = lmpg
                        .store
                        .try_get(&rushdb_types::keys::record_id_key(
                            lmpg.tenant,
                            &a.record_id,
                        ))
                        .unwrap()
                        .unwrap();
                    corrupt(
                        lmpg,
                        rushdb_types::keys::record_id_key(lmpg.tenant, &[255; 16]),
                        Some(raw),
                    );
                }
            }
            assert!(
                export_tenant_jsonl(&lmpg.store.read_view().unwrap(), lmpg.tenant).is_err(),
                "mode {mode}"
            );
        });
    }
}

#[test]
fn writer_failures_and_cancellation_never_return_success() {
    struct FailedWriter;
    impl Write for FailedWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("sink failed"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    on_backends(|lmpg| {
        lmpg.create_record("Doc", None, vec![]).unwrap();
        lmpg.create_record("Doc", None, vec![]).unwrap();
        let view = lmpg.store.read_view().unwrap();
        assert!(
            export_tenant_jsonl_to(&view, lmpg.tenant, &mut FailedWriter)
                .unwrap_err()
                .contains("sink failed")
        );
        let cancellation = view.cancellation();
        let mut writer = HookWriter {
            bytes: Vec::new(),
            hook: Some(|| cancellation.cancel()),
        };
        assert!(export_tenant_jsonl_to(&view, lmpg.tenant, &mut writer).is_err());
        assert!(
            !writer.bytes.is_empty(),
            "failed stream may have a partial prefix"
        );
        assert!(export_tenant_jsonl(&view, lmpg.tenant).is_err());
    });
}

#[test]
fn binary_tenant_export_matches_after_native_reopen() {
    let path = rushdb_storage::TempPath::store("export-reopen");
    let tenant = TenantId(u32::MAX - 1);
    let before = {
        let lmpg = Lmpg::new(Arc::new(Store::open(&path).unwrap()), tenant);
        let a = lmpg
            .create_record(
                "Doc",
                None,
                vec![("tags".into(), FieldChange::Set(json!(["a", "b"])))],
            )
            .unwrap();
        let b = lmpg.create_record("Doc", None, vec![]).unwrap();
        lmpg.create_relationship("LINK", a, b, vec![]).unwrap();
        export_tenant_jsonl(&lmpg.store.read_view().unwrap(), tenant).unwrap()
    };
    let store = Store::open(&path).unwrap();
    assert_eq!(
        export_tenant_jsonl(&store.read_view().unwrap(), tenant).unwrap(),
        before
    );
    drop(store);
    rushdb_storage::remove_store(&path).unwrap();
}

#[test]
fn cancellation_during_the_final_relationship_write_is_not_success() {
    struct CancelLast {
        lines: usize,
        cancellation: rushdb_storage::ReadCancellation,
    }
    impl Write for CancelLast {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.lines += bytes.split(|&b| b == b'\n').count() - 1;
            if self.lines == 3 {
                self.cancellation.cancel();
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    on_backends(|lmpg| {
        let a = lmpg.create_record("Doc", None, vec![]).unwrap();
        let b = lmpg.create_record("Doc", None, vec![]).unwrap();
        lmpg.create_relationship("LINK", a, b, vec![]).unwrap();
        let view = lmpg.store.read_view().unwrap();
        let mut writer = CancelLast {
            lines: 0,
            cancellation: view.cancellation(),
        };
        assert!(export_tenant_jsonl_to(&view, lmpg.tenant, &mut writer).is_err());
        assert_eq!(writer.lines, 3);
    });
}
