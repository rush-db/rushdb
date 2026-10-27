//! Pinned logical graph export. This is not a canonical storage checkpoint:
//! policy/index metadata, outcomes, history, and journal state are not included.

use std::io::Write;

use rushdb_lmpg::{reader::try_access, reconstruct};
use rushdb_storage::{ReadView, ScanBounds, ScanProjection};
use rushdb_types::{
    ids::TenantId,
    keys::{key, P_REL},
    occurrence::{RecordHeader, RelationshipHeader},
};

/// Write records first, then relationships, from exactly one retained view.
/// Header scans are bounded to 128 rows / 1 MiB per batch. Reconstruction is
/// still eager per owner and subject to the view's local read limits; there
/// is not yet a shared graph/operator memory budget.
///
/// Errors stop export and are never represented as missing rows. A writer can
/// contain an incomplete prefix after failure; callers must discard it rather
/// than publish it as a valid backup. Flush/fsync/atomic file publication are
/// the caller's responsibility, not implied by a successful return here.
///
/// # Errors
///
/// Returns an error string if a header row is malformed, a key/header or
/// ordinal mapping is inconsistent, a record/relationship cannot be
/// reconstructed, the view is cancelled, the writer rejects bytes, or the
/// exported count overflows.
pub fn export_tenant_jsonl_to<W: Write + ?Sized>(
    view: &ReadView,
    tenant: TenantId,
    writer: &mut W,
) -> Result<(u64, u64), String> {
    let mut counts = [0u64; 2];
    // P20: one reconstruction session per export shares the family-definition
    // cache across all owners (families are frozen within this pinned view).
    let mut session = reconstruct::ReconstructSession::new(view, tenant);
    for (kind, count) in counts.iter_mut().enumerate() {
        // Records: every header row (`RECORD_HDR`). Relationships: the id-keyed
        // header rows (a trailing separator excludes relationship-ordinal).
        let prefix = if kind == 0 {
            rushdb_types::keys::record_hdr_prefix(tenant)
        } else {
            key(tenant, &[P_REL, b""])
        };
        let mut cursor = view
            .scan(ScanBounds::prefix(&prefix), ScanProjection::KeyValue)
            .map_err(|e| e.to_string())?;
        loop {
            let batch = cursor
                .next_batch(128, 1024 * 1024)
                .map_err(|e| e.to_string())?;
            for row in batch.rows {
                let raw = row.value.ok_or("export: missing header value")?;
                let line = if kind == 0 {
                    record_line(&mut session, view, tenant, &row.key, &raw)?
                } else {
                    relationship_line(&mut session, view, tenant, &row.key, &raw)?
                };
                view.check_cancelled().map_err(|e| e.to_string())?;
                serde_json::to_writer(&mut *writer, &line).map_err(|e| e.to_string())?;
                writer.write_all(b"\n").map_err(|e| e.to_string())?;
                view.check_cancelled().map_err(|e| e.to_string())?;
                *count = count.checked_add(1).ok_or("export count overflow")?;
            }
            if batch.done {
                break;
            }
        }
    }
    // Every header was resolved through its identity row above; a surplus
    // identity row (an id pointing at another record's ordinal) would
    // otherwise be silently dropped from the export.
    let identities = count_keys(view, &rushdb_types::keys::record_id_prefix(tenant))?;
    if identities != counts[0] {
        return Err("export: record identity index does not match headers".into());
    }
    Ok((counts[0], counts[1]))
}

/// Number of keys under `prefix`, scanned in bounded key-only batches.
fn count_keys(view: &ReadView, prefix: &[u8]) -> Result<u64, String> {
    let mut cursor = view
        .scan(ScanBounds::prefix(prefix), ScanProjection::Keys)
        .map_err(|e| e.to_string())?;
    let mut total = 0u64;
    loop {
        let batch = cursor
            .next_batch(1024, 1024 * 1024)
            .map_err(|e| e.to_string())?;
        total = total
            .checked_add(batch.rows.len() as u64)
            .ok_or("export count overflow")?;
        if batch.done {
            return Ok(total);
        }
    }
}

/// One exported record line from its header row (`ordinal` key suffix).
fn record_line(
    session: &mut reconstruct::ReconstructSession<'_, ReadView>,
    view: &ReadView,
    tenant: TenantId,
    header_key: &[u8],
    raw: &[u8],
) -> Result<serde_json::Value, String> {
    let ordinal = rushdb_types::keys::parse_record_hdr_key(header_key)
        .ok_or("export: invalid record header key")?;
    let (record_id, label) =
        rushdb_types::keys::decode_record_hdr(raw).ok_or("export: invalid record header")?;
    let hdr = RecordHeader {
        record_id,
        ordinal,
        label: label.to_owned(),
    };
    let indexed = try_access::record_by_id(view, tenant, &rushdb_types::ids::RecordId(record_id))
        .map_err(|e| e.to_string())?
        .ok_or("export: missing record id mapping")?;
    if indexed.ordinal != hdr.ordinal {
        return Err("export: record id/ordinal identity mismatch".into());
    }
    let rec = session
        .reconstruct_record(hdr.ordinal)
        .map_err(|e| e.to_string())?
        .ok_or("export: missing record during reconstruction")?;
    let serde_json::Value::Object(mut properties) = rec else {
        return Err("export: invalid record object".into());
    };
    properties.retain(|name, _| !rushdb_types::value::is_reserved_name(name));
    Ok(serde_json::json!({
        "op": "put", "kind": "record",
        "id": hex(&hdr.record_id), "label": hdr.label,
        "properties": properties,
    }))
}

/// One exported relationship line from its id-keyed header row.
fn relationship_line(
    session: &mut reconstruct::ReconstructSession<'_, ReadView>,
    view: &ReadView,
    tenant: TenantId,
    row_key: &[u8],
    raw: &[u8],
) -> Result<serde_json::Value, String> {
    let hdr: RelationshipHeader = serde_json::from_slice(raw)
        .map_err(|e| format!("export: invalid relationship header: {e}"))?;
    if row_key != key(tenant, &[P_REL, &hdr.relationship_id]) {
        return Err("export: relationship key/header identity mismatch".into());
    }
    let indexed = try_access::relationship_by_ord(view, tenant, hdr.ordinal)
        .map_err(|e| e.to_string())?
        .ok_or("export: missing relationship ordinal mapping")?;
    if indexed.relationship_id != hdr.relationship_id {
        return Err("export: relationship ordinal identity mismatch".into());
    }
    for (id, ordinal) in [
        (hdr.source_id, hdr.source_ord),
        (hdr.target_id, hdr.target_ord),
    ] {
        // Resolve by public id so a record whose id row was lost is caught
        // instead of emitting a dangling relationship.
        let endpoint = try_access::record_by_id(view, tenant, &rushdb_types::ids::RecordId(id))
            .map_err(|e| e.to_string())?
            .ok_or("export: missing relationship endpoint")?;
        if endpoint.ordinal != ordinal {
            return Err("export: relationship endpoint identity mismatch".into());
        }
    }
    let rel = session
        .reconstruct_relationship(hdr.ordinal)
        .map_err(|e| e.to_string())?
        .ok_or("export: missing relationship during reconstruction")?;
    let serde_json::Value::Object(mut properties) = rel else {
        return Err("export: invalid relationship object".into());
    };
    properties.retain(|name, _| !rushdb_types::value::is_reserved_name(name));
    Ok(serde_json::json!({
        "op": "put", "kind": "relationship",
        "id": hex(&hdr.relationship_id), "type": hdr.rel_type,
        "source": hex(&hdr.source_id), "target": hex(&hdr.target_id),
        "properties": properties,
    }))
}

fn hex(id: &[u8; 16]) -> String {
    uuid::Uuid::from_bytes(*id).simple().to_string()
}
