//! L03: superseded values leave no residue. Values live inline in one
//! occurrence row per value, and the per-property value dictionary
//! is derived at apply time, so an update or delete must not leave an
//! old occurrence row or a stale dictionary entry behind.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_storage::Store;
use rushdb_types::ids::{OwnerKind, TenantId};
use serde_json::json;

fn occurrence_rows(eng: &Lmpg) -> usize {
    let prefix = rushdb_types::keys::occ_kind_prefix(eng.tenant, OwnerKind::Record);
    eng.store.try_range_prefix(&prefix).unwrap().len()
}

fn dictionary_entries(eng: &Lmpg) -> usize {
    let mut prefix = rushdb_types::keys::tenant_prefix(eng.tenant).to_vec();
    prefix.push(rushdb_types::keys::F_VALUE_DICT);
    eng.store.try_range_prefix(&prefix).unwrap().len()
}

fn updated_twenty_times(eng: &Lmpg) -> rushdb_lmpg::RecordRef {
    let rec = eng
        .create_record("Doc", None, vec![("n".into(), FieldChange::Set(json!(0)))])
        .unwrap();
    for i in 1..20 {
        eng.update_fields(rec, vec![("n".into(), FieldChange::Set(json!(i)))])
            .unwrap();
    }
    rec
}

#[test]
fn updating_a_scalar_leaves_one_occurrence_and_one_dictionary_entry() {
    let eng = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(1));
    updated_twenty_times(&eng);
    assert_eq!(
        occurrence_rows(&eng),
        1,
        "one live occurrence after 20 updates"
    );
    assert_eq!(
        dictionary_entries(&eng),
        1,
        "only the live value is counted"
    );
}

#[test]
fn deleting_a_record_reclaims_its_occurrences_and_dictionary_entries() {
    let eng = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(1));
    let rec = updated_twenty_times(&eng);
    eng.delete_record(rec).unwrap();
    assert_eq!(occurrence_rows(&eng), 0);
    assert_eq!(dictionary_entries(&eng), 0);
}
