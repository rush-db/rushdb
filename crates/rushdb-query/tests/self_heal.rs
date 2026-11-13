//! SELF-HEAL from a reader's side: a read that finds a label's derived rows
//! disagreeing with its base rows reports it, is answered from base rows
//! (exactly) while the label is suspect, and after the label's re-derive is
//! answered from its derived rows again.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{BatchOp, BatchTx, Lmpg};
use rushdb_query::document::parse_document;
use rushdb_query::find_records;
use rushdb_storage::{DeriveRead, Deriver, Op, StorageError, Store};
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::keys as k;
use serde_json::json;

/// The real deriver with a bug: `Bad`'s presence rows are lost, its support
/// counters kept.
struct LosesPresence;

impl Deriver for LosesPresence {
    fn derive(
        &self,
        ci: u64,
        ops: &[Op],
        before: &dyn DeriveRead,
    ) -> Result<Vec<Op>, StorageError> {
        Ok(rushdb_lmpg::derived::LmpgDeriver
            .derive(ci, ops, before)?
            .into_iter()
            .filter(|op| match op {
                Op::Put(key, _) => k::parse_tenant(key).is_none_or(|tenant| {
                    !key.starts_with(&k::presence_label_prefix(tenant, OwnerKind::Record, "Bad"))
                }),
                _ => true,
            })
            .collect())
    }

    fn is_derived_key(&self, key: &[u8]) -> bool {
        rushdb_lmpg::derived::LmpgDeriver.is_derived_key(key)
    }

    fn prefix_reaches_derived(&self, prefix: &[u8]) -> bool {
        rushdb_lmpg::derived::LmpgDeriver.prefix_reaches_derived(prefix)
    }
}

fn total(lmpg: &Lmpg) -> u64 {
    let request = parse_document(&json!({"labels": ["Bad"], "where": {"tag": {"$exists": true}}}))
        .unwrap()
        .search;
    find_records(lmpg, &request).unwrap().total_matched
}

#[test]
fn a_disagreement_is_reported_answered_from_base_rows_and_repaired() {
    let store = Arc::new(Store::new_in_memory());
    let tenant = TenantId::from_ordinal(43);
    let lmpg = Lmpg::new(Arc::clone(&store), tenant);
    store.set_deriver(Arc::new(LosesPresence));
    lmpg.run_batch(BatchTx {
        ops: (0..12)
            .map(|i| BatchOp::CreateRecord {
                label: "Bad".into(),
                explicit_id: None,
                fields: vec![("tag".into(), FieldChange::Set(json!(format!("t{i}"))))],
            })
            .collect(),
    })
    .unwrap();
    store.set_deriver(Arc::new(rushdb_lmpg::derived::LmpgDeriver));
    let health = store.derived_health();
    assert!(!health.is_suspect(tenant, "Bad"));

    // The read that finds it is already exact: it plans from base rows.
    assert_eq!(total(&lmpg), 12);
    assert!(health.is_suspect(tenant, "Bad"));
    let findings = health.take_findings();
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].kind, "presence_count");
    assert_eq!(total(&lmpg), 12);

    lmpg.rederive_label("Bad").unwrap();
    assert!(!health.is_suspect(tenant, "Bad"));
    assert_eq!(total(&lmpg), 12);
    assert!(
        health.take_findings().is_empty(),
        "the repair left nothing to find"
    );
}
