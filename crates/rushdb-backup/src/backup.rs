//! Logical JSONL backup and restore.
//!
//! This is the retained half of the former `rushdb-migrate` crate: export a
//! tenant's records and relationships as newline-delimited JSON, and restore
//! them into a live engine preserving public identity. It is **not** a
//! canonical storage checkpoint (policy/index metadata, outcomes, history, and
//! journal state are not included), and it is **not** a migration tool —
//! migrations are performed outside the engine.

use std::io::BufRead;

use rushdb_lmpg::Lmpg;

/// Export one tenant's records and relationships as a JSONL document, returning
/// the text plus `(records, relationships)` counts.
///
/// # Errors
///
/// Returns an error string if the underlying export fails or the produced bytes
/// are not valid UTF-8.
pub fn export_tenant_jsonl(
    snap: &rushdb_storage::ReadView,
    tenant: rushdb_types::ids::TenantId,
) -> Result<(String, u64, u64), String> {
    let mut bytes = Vec::new();
    let (records, relationships) = crate::export::export_tenant_jsonl_to(snap, tenant, &mut bytes)?;
    let jsonl = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    Ok((jsonl, records, relationships))
}

/// Restore one record line: upsert by `__id` and re-apply the user-facing
/// `__id` property that `upsert_document` consumes as physical identity.
fn restore_record(lmpg: &Lmpg, v: &serde_json::Value) -> Result<(), String> {
    let label = v
        .get("label")
        .and_then(|x| x.as_str())
        .ok_or("backup record missing label")?
        .to_string();
    let id = v
        .get("id")
        .and_then(|x| x.as_str())
        .ok_or("backup record missing id")?;
    let properties = v
        .get("properties")
        .cloned()
        .unwrap_or(serde_json::json!({}));
    let mut doc = properties.as_object().cloned().unwrap_or_default();
    doc.insert("__id".to_string(), serde_json::json!(id));
    doc.insert("__label".to_string(), serde_json::json!(label));
    rushdb_lmpg::import::upsert_document(lmpg, &label, &serde_json::Value::Object(doc))
        .map_err(|e| e.to_reason())?;
    Ok(())
}

/// Restore one relationship line: link endpoints by decoded hex ids, writing
/// the supplied properties onto the new edge. Idempotent on re-run: an edge
/// with the same type between the same endpoints is left untouched.
fn restore_relationship(lmpg: &Lmpg, v: &serde_json::Value) -> Result<(), String> {
    let relationship_id = v
        .get("id")
        .and_then(|x| x.as_str())
        .and_then(rushdb_lmpg::import::decode_simple_hex)
        .ok_or("backup relationship missing stable id")?;
    let rel_type = v
        .get("type")
        .and_then(|x| x.as_str())
        .ok_or("backup relationship missing type")?
        .to_string();
    let src = v
        .get("source")
        .and_then(|x| x.as_str())
        .and_then(rushdb_lmpg::import::decode_simple_hex)
        .ok_or("backup relationship bad source id")?;
    let tgt = v
        .get("target")
        .and_then(|x| x.as_str())
        .and_then(rushdb_lmpg::import::decode_simple_hex)
        .ok_or("backup relationship bad target id")?;
    let fields: Vec<(String, rushdb_lmpg::plan::FieldChange)> = v
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .map(|o| {
            o.iter()
                .map(|(k, val)| {
                    let change = if val.is_null() {
                        rushdb_lmpg::plan::FieldChange::ExplicitNull
                    } else {
                        rushdb_lmpg::plan::FieldChange::Set(val.clone())
                    };
                    (k.clone(), change)
                })
                .collect()
        })
        .unwrap_or_default();

    let snap = lmpg.store.read_view().map_err(|e| e.to_string())?;
    let s_hdr = rushdb_lmpg::reader::try_access::record_by_id(
        &snap,
        lmpg.tenant,
        &rushdb_types::ids::RecordId(src),
    )
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "restore: source record not found".to_string())?;
    let t_hdr = rushdb_lmpg::reader::try_access::record_by_id(
        &snap,
        lmpg.tenant,
        &rushdb_types::ids::RecordId(tgt),
    )
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "restore: target record not found".to_string())?;
    // P16 resume idempotency: preserve the source relationship identity so
    // parallel edges are distinct and retries skip exactly the same edge.
    if rushdb_lmpg::reader::try_access::relationship_by_id(
        &snap,
        lmpg.tenant,
        &rushdb_types::ids::RelationshipId(relationship_id),
    )
    .map_err(|e| e.to_string())?
    .is_none()
    {
        lmpg.create_relationship_with_id(
            &rel_type,
            rushdb_lmpg::RecordRef {
                record_id: src,
                ordinal: s_hdr.ordinal,
            },
            rushdb_lmpg::RecordRef {
                record_id: tgt,
                ordinal: t_hdr.ordinal,
            },
            relationship_id,
            fields,
        )
        .map_err(|e| e.to_reason())?;
    }
    Ok(())
}

/// Restore a JSONL backup into a live engine. Records upsert by `__id`
/// (preserving public identity), relationships are linked by endpoint ids,
/// relationship properties are written onto the new edges. Returns the number
/// of records and relationships materialized.
///
/// # Errors
///
/// Returns an error string if a line is not valid JSON or has an unknown
/// `kind`, if a record/relationship line is malformed, or if the engine
/// operation for a line fails.
pub fn restore_backup(lmpg: &Lmpg, jsonl: &str) -> Result<(u64, u64), String> {
    restore_backup_reader(lmpg, std::io::Cursor::new(jsonl.as_bytes()))
}

/// Restore a JSONL backup incrementally from a buffered reader. The reader is
/// consumed line-by-line, so callers can restore large logical exports without
/// first materializing the complete artifact as one `String`.
///
/// # Errors
/// Returns an error when a line cannot be read/decoded or an engine operation
/// fails. A caller must discard its target if restoration stops after a prefix.
pub fn restore_backup_reader<R: BufRead>(lmpg: &Lmpg, reader: R) -> Result<(u64, u64), String> {
    let mut records = 0u64;
    let mut relationships = 0u64;
    for line in reader.lines() {
        let line = line.map_err(|e| format!("read backup line: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        match v.get("kind").and_then(|x| x.as_str()).unwrap_or("") {
            "record" => {
                restore_record(lmpg, &v)?;
                records += 1;
            }
            "relationship" => {
                restore_relationship(lmpg, &v)?;
                relationships += 1;
            }
            other => return Err(format!("unknown backup op kind `{other}`")),
        }
    }
    Ok((records, relationships))
}
