use std::collections::{BTreeSet, HashMap};

use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::model::{LogicalType, TypeState};
use rushdb_types::value::{infer_scalar, normalize_name, scalar_equal, Scalar};
use serde_json::{json, Value as Json};

use crate::plan::FieldChange;
use crate::reader::keys_ext as kx;
use crate::reader::TryRead;
use crate::{Lmpg, LmpgError, RecordRef};

/// Bounded Rust ingestion frontend (document 21 slice): decodes one JSON
/// document into format-neutral drafts and lowers them through the same
/// mutation path as every other writer. Nested objects and arrays of
/// objects promote to child records joined by relationships; scalar arrays
/// decompose into ordered element occurrences; `null` becomes a live
/// explicit-Null occurrence under the `NativeV1` profile.
///
/// # Errors
/// Returns [`LmpgError::Conflict`] if `doc` is not a JSON object or any
/// underlying record/relationship write fails.
pub fn import_document(lmpg: &Lmpg, label: &str, doc: &Json) -> Result<RecordRef, LmpgError> {
    import_document_with_cancel(lmpg, label, doc, &|| false)
}

/// Imports one document while polling a caller-owned cancellation probe before
/// each recursive record and relationship mutation.
///
/// # Errors
/// Returns [`LmpgError::Conflict`] when cancellation is observed or the
/// document/mutation path fails.
pub fn import_document_with_cancel(
    lmpg: &Lmpg,
    label: &str,
    doc: &Json,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<RecordRef, LmpgError> {
    if is_cancelled() {
        return Err(LmpgError::Conflict("import cancelled".into()));
    }
    let Json::Object(fields) = doc else {
        return Err(LmpgError::Conflict("document must be an object".into()));
    };

    let mut scalars: Vec<(String, FieldChange)> = Vec::new();
    let mut children: Vec<(String, Vec<Json>)> = Vec::new();
    for (k, v) in fields {
        match v {
            Json::Object(_) => children.push((k.clone(), vec![v.clone()])),
            Json::Array(items) if items.iter().any(serde_json::Value::is_object) => {
                children.push((k.clone(), items.clone()));
            }
            other => scalars.push((
                k.clone(),
                if other.is_null() {
                    FieldChange::ExplicitNull
                } else {
                    FieldChange::Set(other.clone())
                },
            )),
        }
    }

    if is_cancelled() {
        return Err(LmpgError::Conflict("import cancelled".into()));
    }
    let record_ref = lmpg.create_record(label, None, std::mem::take(&mut scalars))?;

    for (key, docs) in &children {
        let rel_type = child_label(label, key);
        for d in docs {
            if is_cancelled() {
                return Err(LmpgError::Conflict("import cancelled".into()));
            }
            let child = import_document_with_cancel(lmpg, &rel_type, d, is_cancelled)?;
            if is_cancelled() {
                return Err(LmpgError::Conflict("import cancelled".into()));
            }
            lmpg.create_relationship(&child_label(label, key), record_ref, child, vec![])?;
        }
    }
    Ok(record_ref)
}

/// Transactional form of [`import_document`]. All records, child records and
/// relationships are staged in the supplied accumulator and become visible
/// only when the caller commits it.
///
/// # Errors
/// Returns [`LmpgError`] when planning or authorization fails.
pub fn import_document_tx(
    lmpg: &Lmpg,
    label: &str,
    doc: &Json,
    tx: &mut rushdb_storage::TxAccumulator,
) -> Result<RecordRef, LmpgError> {
    import_document_tx_with_cancel(lmpg, label, doc, tx, &|| false)
}

/// Transactional document import with cooperative cancellation checks.
///
/// All mutations remain in `tx`; cancellation therefore discards the whole
/// staged graph instead of leaving a partially imported root or child.
///
/// # Errors
/// Returns [`LmpgError::Conflict`] when cancellation is observed or staging
/// fails.
pub fn import_document_tx_with_cancel(
    lmpg: &Lmpg,
    label: &str,
    doc: &Json,
    tx: &mut rushdb_storage::TxAccumulator,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<RecordRef, LmpgError> {
    if is_cancelled() {
        return Err(LmpgError::Conflict("import cancelled".into()));
    }
    let Json::Object(fields) = doc else {
        return Err(LmpgError::Conflict("document must be an object".into()));
    };
    let mut scalars = Vec::new();
    let mut children = Vec::new();
    for (k, v) in fields {
        match v {
            Json::Object(_) => children.push((k.clone(), vec![v.clone()])),
            Json::Array(items) if items.iter().any(serde_json::Value::is_object) => {
                children.push((k.clone(), items.clone()));
            }
            other => scalars.push((
                k.clone(),
                if other.is_null() {
                    FieldChange::ExplicitNull
                } else {
                    FieldChange::Set(other.clone())
                },
            )),
        }
    }
    if is_cancelled() {
        return Err(LmpgError::Conflict("import cancelled".into()));
    }
    let record_ref = lmpg.create_record_tx(label, None, scalars, Some(tx))?;
    for (key, docs) in children {
        let rel_type = child_label(label, &key);
        for child_doc in docs {
            if is_cancelled() {
                return Err(LmpgError::Conflict("import cancelled".into()));
            }
            let child =
                import_document_tx_with_cancel(lmpg, &rel_type, &child_doc, tx, is_cancelled)?;
            if is_cancelled() {
                return Err(LmpgError::Conflict("import cancelled".into()));
            }
            lmpg.create_relationship_tx(&rel_type, record_ref, child, vec![], Some(tx))?;
        }
    }
    Ok(record_ref)
}

/// Derive the child label for a nested key: `{parent}/{KEY}`, where non
/// alphanumeric characters in `key` become `_` and letters are uppercased.
///
/// # Examples
///
/// ```
/// use rushdb_lmpg::import::child_label;
///
/// assert_eq!(child_label("Person", "first name"), "Person/FIRST_NAME");
/// ```
#[must_use]
pub fn child_label(parent: &str, key: &str) -> String {
    let mut out = String::new();
    for c in key.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    format!("{parent}/{out}")
}

/// Streaming line-delimited importers (document 21 slice): the Rust engine
/// decodes bytes, normalizes and lowers documents through the same
/// mutation path; source framing never changes graph shape or IDs.
///
/// # Errors
/// Returns [`LmpgError::Conflict`] if a line cannot be read or parsed, or any
/// document write fails.
pub fn import_jsonl(
    lmpg: &Lmpg,
    label: &str,
    input: impl std::io::Read,
) -> Result<Vec<RecordRef>, LmpgError> {
    use std::io::BufRead;
    let reader = std::io::BufReader::new(input);
    let mut out = Vec::new();
    for (line_no, line) in reader.lines().enumerate() {
        let line = match line {
            Ok(l) => l.trim().to_string(),
            Err(e) => return Err(LmpgError::Conflict(format!("read error at {line_no}: {e}"))),
        };
        if line.is_empty() {
            continue;
        }
        let doc: Json = serde_json::from_str(&line)
            .map_err(|e| LmpgError::Conflict(format!("bad json on line {line_no}: {e}")))?;
        out.push(import_document(lmpg, label, &doc)?);
    }
    Ok(out)
}

/// Alias of [`import_jsonl`] for newline-delimited JSON input.
///
/// # Errors
/// Returns [`LmpgError::Conflict`] if a line cannot be read or parsed, or any
/// document write fails.
pub fn import_ndjson(
    lmpg: &Lmpg,
    label: &str,
    input: impl std::io::Read,
) -> Result<Vec<RecordRef>, LmpgError> {
    import_jsonl(lmpg, label, input)
}

/// Merge/upsert resolution (document 21): when a document carries `__id`
/// the ingestion path resolves it against existing identity instead of
/// minting a new one; fields overwrite via the normal delete+put flow.
///
/// # Errors
/// Returns [`LmpgError::Conflict`] if the document is not an object, `__id`
/// is malformed, or the read/write fails.
pub fn upsert_document(lmpg: &Lmpg, label: &str, doc: &Json) -> Result<RecordRef, LmpgError> {
    let Json::Object(obj) = doc else {
        return Err(LmpgError::Conflict("document must be an object".into()));
    };

    let Some(id_val) = obj.get("__id").and_then(|v| v.as_str()) else {
        return import_document(lmpg, label, doc);
    };
    let bytes = decode_simple_hex(id_val)
        .ok_or_else(|| LmpgError::Conflict("__id is not a 32-char hex string".into()))?;

    let snap = lmpg
        .store
        .read_view()
        .map_err(|e| LmpgError::Conflict(e.to_string()))?;
    let existing = crate::reader::try_access::record_by_id(
        &snap,
        lmpg.tenant,
        &rushdb_types::ids::RecordId(bytes),
    )
    .map_err(|e| LmpgError::Conflict(e.to_string()))?;

    let explicit = Some(bytes);
    if let Some(hdr) = existing {
        let mut scalars: Vec<(String, FieldChange)> = Vec::new();
        for (k, v) in obj {
            if k == "__id" || k == "__label" {
                continue;
            }
            scalars.push((
                k.clone(),
                if v.is_null() {
                    FieldChange::ExplicitNull
                } else {
                    FieldChange::Set(v.clone())
                },
            ));
        }
        // updates target record `hdr.ordinal` owned by public id bytes
        lmpg.update_fields(
            RecordRef {
                record_id: bytes,
                ordinal: hdr.ordinal,
            },
            std::mem::take(&mut scalars),
        )?;
        Ok(RecordRef {
            record_id: bytes,
            ordinal: hdr.ordinal,
        })
    } else {
        let mut scalars: Vec<(String, FieldChange)> = Vec::new();
        for (k, v) in obj {
            if k == "__id" || k == "__label" {
                continue;
            }
            scalars.push((
                k.clone(),
                if v.is_null() {
                    FieldChange::ExplicitNull
                } else {
                    FieldChange::Set(v.clone())
                },
            ));
        }
        lmpg.create_record(label, explicit, scalars)
    }
}

fn hex_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Decodes a 32-character (dashes optional) hex string into 16 bytes.
#[must_use]
pub fn decode_simple_hex(hex: &str) -> Option<[u8; 16]> {
    let hex: String = hex.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        let hi = hex_nibble(hex.as_bytes()[i * 2])?;
        let lo = hex_nibble(hex.as_bytes()[i * 2 + 1])?;
        *byte = (hi << 4) | lo;
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Product-parity import (reference: platform/core entity import-export).
// BFS serializer mirrors the product's serializeBFS: nested objects and
// arrays of objects promote to child records joined by relationships; scalar
// arrays stay as ordered-many property values; `$vectors` are extracted as
// data drafts (the DB never embeds — NestJS owns model calls). mergeBy upsert
// matches existing records by property equality within a label (empty
// mergeBy = match on all keys), reusing children so parents can relink.
// ---------------------------------------------------------------------------

/// Default relationship type used when no explicit type is configured.
pub const RUSHDB_RELATION_DEFAULT: &str = "__RUSHDB__RELATION__DEFAULT__";

/// Options controlling product-parity import (type suggestion, label casing,
/// merge behavior, relationship typing).
#[expect(
    clippy::struct_excessive_bools,
    reason = "these flags mirror the reference importer's independent boolean options and are part of the public option surface"
)]
#[derive(Debug, Clone)]
pub struct ImportOptions {
    /// Infer types (numbers, booleans, datetimes) from string values.
    pub suggest_types: bool,
    /// When set, numeric-looking strings become numbers under suggestTypes.
    pub convert_numeric_values_to_numbers: bool,
    /// Uppercase inferred labels.
    pub capitalize_labels: bool,
    /// Skip empty strings and arrays with only empty strings.
    pub skip_empty_values: bool,
    /// Default relationship type for nested joins.
    pub relationship_type: Option<String>,
    /// Per-child-key relationship types (`relationshipTypes`): nested ingest
    /// joins the parent to a child under JSON key `K` with
    /// `relationship_types[K]`, falling back to `relationship_type`. This is
    /// the only way to keep heterogeneous edge types (e.g. `HAS_NEST_*`)
    /// through one nested document; the type survives verbatim.
    pub relationship_types: std::collections::BTreeMap<String, String>,
    /// `None` = mergeBy absent (plain create); `Some(vec)` = mergeBy
    /// provided — an empty vec matches on all keys (reference uses
    /// `Array.isArray(mergeBy)`, which is true for `[]`).
    pub merge_by: Option<Vec<String>>,
    /// Merge strategy applied when upserting (`append`, `replace`, ...).
    pub merge_strategy: Option<String>,
    /// When true, importers return the created records in their result.
    pub return_result: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        ImportOptions {
            suggest_types: true,
            convert_numeric_values_to_numbers: false,
            capitalize_labels: false,
            skip_empty_values: false,
            relationship_type: None,
            relationship_types: std::collections::BTreeMap::new(),
            merge_by: None,
            merge_strategy: None,
            return_result: false,
        }
    }
}

impl ImportOptions {
    /// Validate the shape of caller-provided import options before applying
    /// defaults. Unknown fields remain forward-compatible, but a known field
    /// with the wrong JSON type must not silently become an unrelated default.
    ///
    /// # Errors
    /// Returns a message when the options value or a known field has the wrong
    /// JSON type.
    pub fn validate_json(v: &Json) -> Result<(), String> {
        let object = v
            .as_object()
            .ok_or_else(|| "import options must be an object".to_string())?;
        for key in [
            "suggestTypes",
            "convertNumericValuesToNumbers",
            "capitalizeLabels",
            "skipEmptyValues",
            "returnResult",
            "merge",
        ] {
            if object.get(key).is_some_and(|value| !value.is_boolean()) {
                return Err(format!("import option `{key}` must be boolean"));
            }
        }
        for key in ["relationshipType", "mergeStrategy"] {
            if object.get(key).is_some_and(|value| !value.is_string()) {
                return Err(format!("import option `{key}` must be string"));
            }
        }
        if let Some(value) = object.get("relationshipTypes") {
            let map = value
                .as_object()
                .ok_or_else(|| "import option `relationshipTypes` must be object".to_string())?;
            if map.values().any(|value| !value.is_string()) {
                return Err("import option `relationshipTypes` values must be strings".into());
            }
        }
        if let Some(value) = object.get("mergeBy") {
            let values = value
                .as_array()
                .ok_or_else(|| "import option `mergeBy` must be array".to_string())?;
            if values.iter().any(|value| !value.is_string()) {
                return Err("import option `mergeBy` values must be strings".into());
            }
        }
        Ok(())
    }

    /// Parses import options from a JSON object, applying reference defaults.
    #[must_use]
    pub fn from_json(v: &Json) -> Self {
        let get = |k: &str| {
            v.get(k)
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        };
        let mut opts = ImportOptions {
            suggest_types: v
                .get("suggestTypes")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
            convert_numeric_values_to_numbers: get("convertNumericValuesToNumbers"),
            capitalize_labels: get("capitalizeLabels"),
            skip_empty_values: get("skipEmptyValues"),
            relationship_type: v
                .get("relationshipType")
                .and_then(|x| x.as_str())
                .map(std::string::ToString::to_string),
            relationship_types: v
                .get("relationshipTypes")
                .and_then(|x| x.as_object())
                .map(|map| {
                    map.iter()
                        .filter_map(|(k, val)| val.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default(),
            merge_by: v.get("mergeBy").map(|x| {
                x.as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|e| e.as_str().map(std::string::ToString::to_string))
                            .collect()
                    })
                    .unwrap_or_default()
            }),
            merge_strategy: v
                .get("mergeStrategy")
                .and_then(|x| x.as_str())
                .map(std::string::ToString::to_string),
            return_result: get("returnResult"),
        };
        // `merge: true` is a shorthand for "match on all incoming keys" — the
        // same as `mergeBy: []` with the append strategy. Without this a bare
        // `merge` option was silently ignored and re-imports created duplicates.
        if let Some(merge) = v.get("merge").and_then(serde_json::Value::as_bool) {
            if merge {
                if opts.merge_by.is_none() {
                    opts.merge_by = Some(vec![]);
                }
                if opts.merge_strategy.is_none() {
                    opts.merge_strategy = Some("append".to_string());
                }
            }
        }
        opts
    }

    /// Default relationship type, or [`RUSHDB_RELATION_DEFAULT`] when unset.
    #[must_use]
    pub fn rel_type(&self) -> &str {
        self.relationship_type
            .as_deref()
            .unwrap_or(RUSHDB_RELATION_DEFAULT)
    }

    /// Relationship type for a nested child under JSON key `child_key`:
    /// `relationshipTypes[key]` when configured, else the single
    /// `relationshipType` / default.
    pub fn rel_type_for(&self, child_key: &str) -> String {
        self.relationship_types
            .get(child_key)
            .map_or_else(|| self.rel_type(), String::as_str)
            .to_string()
    }

    /// Whether the options request an upsert (a merge strategy or mergeBy).
    #[must_use]
    pub fn upsert_requested(&self) -> bool {
        self.merge_strategy.is_some() || self.merge_by.is_some()
    }
}

/// One inline vector attached to a draft record's property.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineVectorEntry {
    /// Property the vector is bound to.
    pub property_name: String,
    /// Vector components.
    pub vector: Vec<f32>,
    /// Optional similarity function name.
    pub similarity_function: Option<String>,
    /// Index/space name; see `rushdb_protocol::VectorQuery::index`. With
    /// `resources.require-vector-index-name` off, an absent name matches the
    /// scope's indexes by `(label, property, dimensions, similarity)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,
}

/// A record draft produced by the BFS serializer.
#[derive(Debug, Clone)]
pub struct BfsRecord {
    /// Draft-local id, replaced by the persisted id after upsert.
    pub draft_id: [u8; 16],
    /// Record label.
    pub label: String,
    /// Planned field writes.
    pub properties: Vec<(String, FieldChange)>,
    /// Caller-chosen record id; `None` allocates one. Creating a record
    /// whose id already exists fails.
    pub explicit_id: Option<[u8; 16]>,
}

/// A relationship draft joining two record drafts.
#[derive(Debug, Clone)]
pub struct BfsRelation {
    /// Source record draft id.
    pub source: [u8; 16],
    /// Target record draft id.
    pub target: [u8; 16],
    /// Relationship type.
    pub rel_type: String,
}

/// A draft record carrying inline vectors extracted from `$vectors`.
#[derive(Debug, Clone)]
pub struct VectorDraft {
    /// Draft-local id.
    pub draft_id: [u8; 16],
    /// Record label.
    pub label: String,
    /// Inline vector entries.
    pub entries: Vec<InlineVectorEntry>,
}

/// The complete plan produced by the BFS serializer.
#[derive(Debug, Clone, Default)]
pub struct BfsPlan {
    /// Record drafts.
    pub records: Vec<BfsRecord>,
    /// Relationship drafts.
    pub relations: Vec<BfsRelation>,
    /// Vector drafts.
    pub vector_drafts: Vec<VectorDraft>,
    /// Validation failures encountered while extracting reserved fields.
    /// Import rejects a plan containing any error instead of treating a
    /// malformed reserved value as if it were absent.
    #[doc(hidden)]
    pub validation_errors: Vec<String>,
}

/// Mapping from a draft id to the persisted record id after an upsert.
pub struct UpsertIdMap {
    /// Draft-local id.
    pub draft_id: [u8; 16],
    /// Id of the matching persisted record.
    pub persisted_id: [u8; 16],
}

fn is_primitive(v: &Json) -> bool {
    !(v.is_object() || v.is_array() && v.as_array().is_some_and(|a| a.iter().any(Json::is_object)))
}

fn is_numeric_str(s: &str) -> bool {
    !s.is_empty() && s.parse::<f64>().is_ok()
}

/// JS `typeof`-like classification (null => "object") used to detect mixed
/// arrays, mirroring the reference `arrayIsConsistent`.
fn json_js_type(v: &Json) -> &'static str {
    match v {
        Json::Null | Json::Array(_) | Json::Object(_) => "object",
        Json::Bool(_) => "boolean",
        Json::Number(_) => "number",
        Json::String(_) => "string",
    }
}

/// `String(item)` equivalent used when a mixed/non-scalar array is coerced to
/// a string array (reference `value.map(String)`): arrays join elements with
/// ",", plain objects become "[object Object]", scalars stringify normally.
fn json_to_js_string(v: &Json) -> String {
    match v {
        Json::String(s) => s.clone(),
        Json::Null => "null".to_string(),
        Json::Bool(b) => b.to_string(),
        Json::Number(n) => n.to_string(),
        Json::Array(items) => items
            .iter()
            .map(json_to_js_string)
            .collect::<Vec<_>>()
            .join(","),
        Json::Object(_) => "[object Object]".to_string(),
    }
}

/// Scalar coercion mirroring the product's parsePrimitive under
/// `TImportOptions`. ISO-8601 / date-only strings become typed datetime writes
/// (epoch ms).
#[must_use]
pub fn coerce_change(value: &Json, options: &ImportOptions) -> Option<FieldChange> {
    // Product parity: an array whose elements are not all the same JS type is
    // coerced to a string array instead of failing (reference processArrayValue:
    // `isInconsistentArray || !suggestTypes ? value.map(String) : value`).
    if value.is_null() {
        // explicit Null (document 22): a live null occurrence, never an
        // untypeable `Set(null)` that the planner would reject
        return Some(FieldChange::ExplicitNull);
    }
    if let Json::Array(items) = value {
        if items.is_empty() {
            // Product parity: a genuinely empty array is preserved as `[]`
            // (the reference stores `property.value = []`), never collapsed to
            // null. The write path persists a payload-free EmptyArray marker
            // occurrence so reconstruction emits a real `[]`.
            return Some(FieldChange::Set(Json::Array(vec![])));
        }
        let is_scalar = |i: &Json| matches!(i, Json::Bool(_) | Json::Number(_) | Json::String(_));
        // pass-through: all-null arrays (kept as explicit-null elements) and
        // uniform scalar arrays (same JS type, no nulls/objects/nested arrays)
        let all_null = items.iter().all(serde_json::Value::is_null);
        let uniform_scalars = items.iter().all(is_scalar)
            && items
                .iter()
                .map(|i| json_js_type(i))
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == 1;
        if !(all_null || uniform_scalars) {
            // mixed scalar types, nulls mixed with scalars, or non-scalar
            // elements (objects / nested arrays): coerce to strings — the
            // occurrence model is scalar-only and never rejects (product parity)
            let coerced: Vec<Json> = items
                .iter()
                .map(|i| Json::String(json_to_js_string(i)))
                .collect();
            return Some(FieldChange::Set(Json::Array(coerced)));
        }
    }
    if options.skip_empty_values {
        match value {
            Json::String(s) if s.is_empty() => return None,
            Json::Array(items) => {
                let stripped: Vec<Json> = items
                    .iter()
                    .filter(|i| !(i.is_string() && i.as_str().is_some_and(str::is_empty)))
                    .cloned()
                    .collect();
                if stripped.is_empty() {
                    return None;
                }
                return Some(FieldChange::Set(Json::Array(stripped)));
            }
            _ => {}
        }
    }
    if options.suggest_types {
        if let Json::String(s) = value {
            if let Some(ms) = rushdb_types::datetime::parse_datetime(s) {
                return Some(FieldChange::SetDatetime(ms));
            }
            if options.convert_numeric_values_to_numbers && is_numeric_str(s) {
                if let Ok(i) = s.parse::<i64>() {
                    return Some(FieldChange::Set(json!(i)));
                }
                if !s.contains('.')
                    && !s.contains('e')
                    && !s.contains('E')
                    && s.parse::<u64>().is_ok()
                {
                    return Some(FieldChange::Set(value.clone()));
                }
                if let Ok(f) = s.parse::<f64>() {
                    return Some(FieldChange::Set(
                        serde_json::Number::from_f64(f).map_or(Json::Null, Json::Number),
                    ));
                }
            }
            return Some(FieldChange::Set(value.clone()));
        }
        return Some(FieldChange::Set(value.clone()));
    }
    // Inference off: every value is stored as text, element by element for
    // an array (reference parity: `value.map(String)`).
    match value {
        Json::String(_) => Some(FieldChange::Set(value.clone())),
        Json::Array(items) => Some(FieldChange::Set(Json::Array(
            items
                .iter()
                .map(|item| match item {
                    Json::Null => Json::Null,
                    other => Json::String(json_to_js_string(other)),
                })
                .collect(),
        ))),
        other => Some(FieldChange::Set(Json::String(json_to_js_string(other)))),
    }
}

/// The JSON value of a `FieldChange` (for key matching against stored records).
#[must_use]
pub fn change_value(c: &FieldChange) -> Json {
    match c {
        FieldChange::Set(v) => v.clone(),
        FieldChange::SetDatetime(ms) => json!(ms),
        FieldChange::ExplicitNull | FieldChange::Delete => Json::Null,
    }
}

fn capitalize(s: &str) -> String {
    s.to_uppercase()
}

struct QueueItem {
    key: String,
    value: Json,
    parent_id: Option<[u8; 16]>,
    skip: bool,
    /// Relationship type joining this item to its parent (per-child-key);
    /// `None` for roots.
    rel_type: Option<String>,
}

/// Product-parity BFS serializer (reference serializeBFS).
#[expect(
    clippy::too_many_lines,
    reason = "the BFS walk must stay in one function so queue ordering and child-parent linkage remain visibly deterministic"
)]
#[must_use]
pub fn serialize_bfs(data: &Json, label: &str, options: &ImportOptions) -> BfsPlan {
    let mut plan = BfsPlan::default();
    let mut queue: Vec<QueueItem> = Vec::new();

    if let Some(arr) = data.as_array() {
        if !arr.is_empty() {
            for value in arr {
                queue.push(QueueItem {
                    key: if options.capitalize_labels {
                        capitalize(label)
                    } else {
                        label.to_string()
                    },
                    value: value.clone(),
                    parent_id: None,
                    skip: false,
                    rel_type: None,
                });
            }
        }
    } else {
        let has_explicit_label = !label.trim().is_empty();
        let obj = data.as_object().cloned().unwrap_or_default();
        let has_primitive_at_root = obj.iter().any(|(_, v)| is_primitive(v));
        let complex_children_count = obj
            .iter()
            .filter(|(_, v)| {
                v.is_object()
                    || (v.is_array() && v.as_array().is_some_and(|a| a.iter().any(Json::is_object)))
            })
            .count();
        let skip = !has_explicit_label || !(has_primitive_at_root || complex_children_count > 1);
        queue.push(QueueItem {
            key: if options.capitalize_labels {
                capitalize(label)
            } else {
                label.to_string()
            },
            value: data.clone(),
            parent_id: None,
            skip,
            rel_type: None,
        });
    }

    while !queue.is_empty() {
        let current = queue.remove(0);
        let mut record_draft = BfsRecord {
            draft_id: *uuid::Uuid::now_v7().as_bytes(),
            label: current.key.clone(),
            properties: Vec::new(),
            explicit_id: None,
        };

        let mut value_obj = match current.value {
            Json::Object(o) => o,
            _ => serde_json::Map::new(),
        };

        // `__id` names the record itself (the identity every read returns),
        // never a stored property: a create uses it as the new record's id
        // and an upsert by `__id` finds the record by id.
        if let Some(id) = value_obj.remove(ID_KEY) {
            match id
                .as_str()
                .and_then(|text| uuid::Uuid::parse_str(text).ok())
            {
                Some(id) => record_draft.explicit_id = Some(*id.as_bytes()),
                None => plan.validation_errors.push(format!(
                    "invalid __id {id}: a record id must be a UUID (32 hex digits, dashes optional)"
                )),
            }
        }

        let raw_vectors: Vec<InlineVectorEntry> = match value_obj.remove("$vectors") {
            Some(value) => match serde_json::from_value::<Vec<InlineVectorEntry>>(value) {
                Ok(entries) => entries,
                Err(error) => {
                    plan.validation_errors
                        .push(format!("invalid $vectors field: {error}"));
                    Vec::new()
                }
            },
            None => Vec::new(),
        };

        if !current.skip {
            if let Some(parent) = current.parent_id {
                plan.relations.push(BfsRelation {
                    source: parent,
                    target: record_draft.draft_id,
                    rel_type: current
                        .rel_type
                        .clone()
                        .unwrap_or_else(|| options.rel_type().to_string()),
                });
            }
            if !raw_vectors.is_empty() {
                plan.vector_drafts.push(VectorDraft {
                    draft_id: record_draft.draft_id,
                    label: record_draft.label.clone(),
                    entries: raw_vectors,
                });
            }
        }

        let child_parent_id = if current.skip {
            current.parent_id
        } else {
            Some(record_draft.draft_id)
        };

        for (key, value) in value_obj {
            if value.is_object() {
                let rel_type = Some(options.rel_type_for(&key));
                queue.push(QueueItem {
                    key: if options.capitalize_labels {
                        capitalize(&key)
                    } else {
                        key.clone()
                    },
                    value,
                    parent_id: child_parent_id,
                    skip: false,
                    rel_type,
                });
            } else if value.is_array()
                && value
                    .as_array()
                    .is_some_and(|a| a.iter().any(Json::is_object))
            {
                let rel_type = Some(options.rel_type_for(&key));
                for val in value.as_array().cloned().unwrap_or_default() {
                    queue.push(QueueItem {
                        key: if options.capitalize_labels {
                            capitalize(&key)
                        } else {
                            key.clone()
                        },
                        value: val,
                        parent_id: child_parent_id,
                        skip: false,
                        rel_type: rel_type.clone(),
                    });
                }
            } else if let Some(change) = coerce_change(&value, options) {
                record_draft.properties.push((key, change));
            }
        }

        if !current.skip {
            plan.records.push(record_draft);
        }
    }

    plan
}

/// The records a label-less upsert could update: per label, the record whose
/// merge keys (the `mergeBy` list, or every given property) equal
/// `properties`. The caller updates a single match, refuses several (it
/// cannot tell which one was meant) and never creates on none, because the
/// engine does not invent a label.
///
/// # Errors
/// Returns [`LmpgError::Conflict`] when the lookup cannot read the store.
pub fn find_by_keys_any_label(
    lmpg: &Lmpg,
    properties: &[(String, FieldChange)],
    options: &ImportOptions,
) -> Result<Vec<(String, RecordRef)>, LmpgError> {
    check_merge_keys(options)?;
    // The update that follows writes these properties; a reserved name is
    // refused here, before the lookup, so it is never reported as no match.
    if let Some((name, _)) = properties
        .iter()
        .find(|(name, _)| rushdb_types::value::is_reserved_name(&normalize_name(name)))
    {
        return Err(LmpgError::Plan(crate::plan::PlanError::ReservedName(
            name.clone(),
        )));
    }
    let keys = match &options.merge_by {
        Some(keys) if !keys.is_empty() => keys.clone(),
        _ => properties.iter().map(|(k, _)| k.clone()).collect(),
    };
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let snap = merge_snapshot(lmpg)?;
    let labels = crate::reader::try_access::labels(&snap, lmpg.tenant)
        .map_err(|error| LmpgError::Conflict(format!("merge lookup labels failed: {error}")))?;
    let mut index = MergeIndex::new(lmpg);
    let mut found = Vec::new();
    for label in labels {
        if let Some(record) =
            find_record_by_keys(lmpg, &snap, &mut index, &label, (&keys, None), properties)?
        {
            found.push((label, record));
        }
    }
    Ok(found)
}

fn merge_snapshot(lmpg: &Lmpg) -> Result<rushdb_storage::ReadView, LmpgError> {
    lmpg.store
        .read_view()
        .map_err(|error| LmpgError::Conflict(format!("merge lookup read failed: {error}")))
}

fn fields_from_props(props: &[(String, FieldChange)]) -> Vec<(String, FieldChange)> {
    props.to_vec()
}

fn find_record_by_keys(
    lmpg: &Lmpg,
    snap: &rushdb_storage::ReadView,
    index: &mut MergeIndex,
    label: &str,
    (keys, explicit_id): (&[String], Option<[u8; 16]>),
    props: &[(String, FieldChange)],
) -> Result<Option<RecordRef>, LmpgError> {
    if keys.iter().any(|key| normalize_name(key) == ID_KEY) {
        return find_record_by_id(lmpg, snap, label, (keys, explicit_id), props);
    }
    let Some(candidates) = index_candidates(snap, index, lmpg.tenant, label, keys, props)? else {
        return scan_record_by_keys(snap, lmpg.tenant, label, keys, props);
    };
    for ord in candidates {
        let Some(hdr) = crate::reader::try_access::record_by_ord(snap, lmpg.tenant, ord)
            .map_err(|error| LmpgError::Conflict(format!("merge lookup record failed: {error}")))?
        else {
            continue;
        };
        // Candidates come from the label's presence rows; re-check the label
        // against the header read at the same snapshot.
        if hdr.label != label {
            continue;
        }
        let Some(rec) =
            crate::reconstruct::reconstruct_record(snap, lmpg.tenant, ord).map_err(|error| {
                LmpgError::Conflict(format!("merge lookup reconstruction failed: {error}"))
            })?
        else {
            continue;
        };
        if record_matches_keys(&rec, keys, props) {
            return Ok(Some(RecordRef {
                record_id: hdr.record_id,
                ordinal: ord,
            }));
        }
    }
    Ok(None)
}

/// An upsert keyed on `__id`: one id lookup instead of a label walk. The
/// record matches when it exists under `label` and its other keys are equal;
/// a draft without an `__id` cannot match.
fn find_record_by_id(
    lmpg: &Lmpg,
    snap: &rushdb_storage::ReadView,
    label: &str,
    (keys, explicit_id): (&[String], Option<[u8; 16]>),
    props: &[(String, FieldChange)],
) -> Result<Option<RecordRef>, LmpgError> {
    let Some(id) = explicit_id else {
        return Ok(None);
    };
    let Some(hdr) = crate::reader::try_access::record_by_id(
        snap,
        lmpg.tenant,
        &rushdb_types::ids::RecordId(id),
    )
    .map_err(|error| LmpgError::Conflict(format!("merge id lookup failed: {error}")))?
    else {
        return Ok(None);
    };
    if hdr.label != label {
        return Ok(None);
    }
    let others: Vec<String> = keys
        .iter()
        .filter(|key| normalize_name(key) != ID_KEY)
        .cloned()
        .collect();
    if !others.is_empty() {
        let rec = crate::reconstruct::reconstruct_record(snap, lmpg.tenant, hdr.ordinal).map_err(
            |error| LmpgError::Conflict(format!("merge lookup reconstruction failed: {error}")),
        )?;
        if !rec.is_some_and(|rec| record_matches_keys(&rec, &others, props)) {
            return Ok(None);
        }
    }
    Ok(Some(RecordRef {
        record_id: id,
        ordinal: hdr.ordinal,
    }))
}

/// Candidate ordinals for every equality-matchable merge key: the
/// intersection of the per-key owner sets from the [`MergeIndex`]. `None`
/// means no key was matchable, so the caller must fall back to the label
/// scan; `Some(empty)` is a definitive non-match.
fn index_candidates(
    snap: &impl TryRead,
    index: &mut MergeIndex,
    tenant: TenantId,
    label: &str,
    keys: &[String],
    props: &[(String, FieldChange)],
) -> Result<Option<BTreeSet<u64>>, LmpgError> {
    let mut acc: Option<BTreeSet<u64>> = None;
    let mut indexed = false;
    for key in keys {
        let norm = normalize_name(key);
        if norm.is_empty() || rushdb_types::value::is_reserved_name(&norm) {
            continue;
        }
        let Some((_, change)) = props.iter().find(|(pk, _)| normalize_name(pk) == norm) else {
            // The incoming record cannot carry this key, so nothing can match:
            // the comparison requires every key to be present and equal.
            return Ok(Some(BTreeSet::new()));
        };
        let value = change_value(change);
        if infer_scalar(&value).is_none() {
            // Arrays, objects and nulls are not equality-indexed; exact-match
            // verification below still checks them.
            continue;
        }
        let Some(owners) = owners_for_key_value(snap, index, tenant, label, &norm, &value)? else {
            continue;
        };
        indexed = true;
        acc = Some(match acc {
            None => owners,
            Some(prev) => prev.intersection(&owners).copied().collect(),
        });
        if acc.as_ref().is_some_and(BTreeSet::is_empty) {
            return Ok(acc);
        }
    }
    if indexed {
        Ok(Some(acc.unwrap_or_default()))
    } else {
        Ok(None)
    }
}

/// Record ordinals of `label` whose property `norm` equals `value`. An
/// absent property family yields the empty set (no stored record can match);
/// the value is looked up under every type that could legally have stored it.
fn owners_for_key_value(
    snap: &impl TryRead,
    index: &mut MergeIndex,
    tenant: TenantId,
    label: &str,
    norm: &str,
    value: &Json,
) -> Result<Option<BTreeSet<u64>>, LmpgError> {
    let Some((ty, scalar)) = infer_scalar(value) else {
        return Ok(None);
    };
    let Some(fid) = family_id(snap, tenant, norm)? else {
        return Ok(Some(BTreeSet::new()));
    };
    // `(definition type, encoding type, value)` probes. A stored value pairs
    // a definition id with the *inferred* value encoding, and the two can
    // disagree: `pick_typed_definition` widens an Int64 write to an existing
    // Float64 definition while `stage_put_values` still encodes the value as
    // Int64 (`int_encode`). So probe the value's own encoding under every
    // definition that could hold it.
    let mut probes = vec![(ty, ty, scalar.clone())];
    match &scalar {
        Scalar::Int64(v) => {
            // Widened Int64 stored under the Float64 definition.
            probes.push((LogicalType::Float64, LogicalType::Int64, scalar.clone()));
            // DateTime shares the `int_encode` bytes.
            probes.push((
                LogicalType::DateTime,
                LogicalType::DateTime,
                Scalar::DateTime(*v),
            ));
        }
        Scalar::Utf8(s) => {
            // A date/datetime string is coerced to `SetDatetime` on import, so
            // probe the DateTime definition too.
            if let Some(ms) = rushdb_types::datetime::parse_datetime(s) {
                probes.push((
                    LogicalType::DateTime,
                    LogicalType::DateTime,
                    Scalar::DateTime(ms),
                ));
            }
        }
        Scalar::Float64(f) => {
            // An integral float (`9.0`) equals a stored integer `9`, kept in
            // its Int64 encoding under the Int64 or the widened Float64
            // definition.
            if f.fract() == 0.0 && f.abs() < 9.007_199_254_740_992e15 {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "integral and within the exactly representable range"
                )]
                let int = Scalar::Int64(*f as i64);
                probes.push((LogicalType::Int64, LogicalType::Int64, int.clone()));
                probes.push((LogicalType::Float64, LogicalType::Int64, int));
            }
        }
        Scalar::Boolean(_) | Scalar::DateTime(_) => {}
    }
    let mut owners = BTreeSet::new();
    // A Ready ordered index on the key holds `(entry, owner)` for every
    // typed definition of the property under this label: one range read per
    // probe instead of walking the label's presence rows.
    if let Some(id) = index.ordered(snap, tenant, label, norm)? {
        for (_, enc_ty, scalar) in &probes {
            let Some(entry) = value_entry(*enc_ty, scalar) else {
                continue;
            };
            let end = rushdb_storage::prefix_range(&entry).1;
            for (found, owner) in crate::index::entries(snap, tenant, id, &entry, &end)
                .map_err(|error| LmpgError::Conflict(format!("merge index read failed: {error}")))?
            {
                if found == entry {
                    owners.insert(owner);
                }
            }
        }
        return Ok(Some(owners));
    }
    for (pid_ty, enc_ty, scalar) in &probes {
        add_typed_owners(
            snap,
            index,
            (tenant, label, norm, fid),
            *pid_ty,
            *enc_ty,
            scalar,
            &mut owners,
        )?;
    }
    Ok(Some(owners))
}

/// `tag ‖ order-preserving payload` of a scalar: the occurrence entry the
/// value dictionary and ordered indexes key on.
fn value_entry(enc_ty: LogicalType, scalar: &Scalar) -> Option<Vec<u8>> {
    let enc = rushdb_types::encoding::encode_order_preserving(enc_ty, scalar).ok()?;
    let mut entry = Vec::with_capacity(enc.len() + 1);
    entry.push(rushdb_types::encoding::logical_tag(enc_ty));
    entry.extend_from_slice(&enc);
    Some(entry)
}

/// Extend `owners` with the owners of one `(definition, encoding)` pair:
/// `pid_ty` selects the property definition, `enc_ty` the value encoding.
fn add_typed_owners(
    snap: &impl TryRead,
    index: &mut MergeIndex,
    (tenant, label, norm, fid): (TenantId, &str, &str, u64),
    pid_ty: LogicalType,
    enc_ty: LogicalType,
    scalar: &Scalar,
    owners: &mut BTreeSet<u64>,
) -> Result<(), LmpgError> {
    let Some(pid) = property_id(snap, tenant, fid, pid_ty)? else {
        return Ok(());
    };
    let Some(entry) = value_entry(enc_ty, scalar) else {
        return Ok(());
    };
    if let Some(found) = index.values(snap, tenant, (label, norm, pid))?.get(&entry) {
        owners.extend(found);
    }
    Ok(())
}

/// Value → owners of `(label, pid)` at one snapshot, for merge matching.
/// Without a declared index a lookup must walk the label's presence
/// rows; building the map once per `(label, pid)` makes a chunk of `M`
/// lookups cost `O(label + M)` instead of `O(label × M)`.
/// Bytes one walked upsert-key value holds beside its payload: the owner
/// ordinal, its set node and the map entry.
const MERGE_ENTRY_BYTES: u64 = 64;

struct MergeIndex {
    values: HashMap<(String, u64), HashMap<Vec<u8>, BTreeSet<u64>>>,
    /// Ready ordered index id per `(label, normalized property)`, if any.
    ordered: HashMap<(String, String), Option<u32>>,
    /// The unindexed-walk limit: the largest key population walked without
    /// an index (none by default, QRY-3).
    walk_limit: Option<u64>,
    /// The working-set cap the walked values are charged against
    /// (`query-memory-max`, tightened by the request).
    memory_limit: Option<u64>,
}

impl MergeIndex {
    fn new(lmpg: &Lmpg) -> Self {
        Self {
            values: HashMap::new(),
            ordered: HashMap::new(),
            walk_limit: lmpg.limits().unindexed_walk_max_owners(),
            memory_limit: lmpg.limits().query_memory_max_bytes(),
        }
    }

    /// The Ready ordered index on `(label, norm)` at this snapshot, if any.
    fn ordered(
        &mut self,
        snap: &impl TryRead,
        tenant: TenantId,
        label: &str,
        norm: &str,
    ) -> Result<Option<u32>, LmpgError> {
        if let Some(id) = self.ordered.get(&(label.to_owned(), norm.to_owned())) {
            return Ok(*id);
        }
        let id = crate::index::ready_ordered(snap, tenant, label, norm)
            .map_err(|error| LmpgError::Conflict(format!("merge index lookup failed: {error}")))?
            .map(|def| def.id);
        self.ordered.insert((label.to_owned(), norm.to_owned()), id);
        Ok(id)
    }

    fn values(
        &mut self,
        snap: &impl TryRead,
        tenant: TenantId,
        (label, norm, pid): (&str, &str, u64),
    ) -> Result<&HashMap<Vec<u8>, BTreeSet<u64>>, LmpgError> {
        let lookup = |error: rushdb_storage::StorageError| {
            LmpgError::Conflict(format!("merge value lookup failed: {error}"))
        };
        match self.values.entry((label.to_owned(), pid)) {
            std::collections::hash_map::Entry::Occupied(slot) => Ok(slot.into_mut()),
            std::collections::hash_map::Entry::Vacant(slot) => {
                // The population is exact (a counter) and known before the
                // walk: refuse a large one with the index to declare instead
                // of failing deep in the scan.
                let population =
                    crate::derived::prop_support_label(snap, tenant, OwnerKind::Record, pid, label)
                        .map_err(lookup)?;
                if let Some(limit) = self.walk_limit.filter(|limit| population > *limit) {
                    return Err(LmpgError::Conflict(format!(
                        "unindexed walk refused: upsert key `{norm}` would check {population} \
                         records of `{label}`, over the unindexed-walk limit ({limit}); declare \
                         an ordered index on `{norm}`"
                    )));
                }
                let mut map: HashMap<Vec<u8>, BTreeSet<u64>> = HashMap::new();
                let mut held = 0u64;
                for owner in crate::derived::presence_owners(
                    snap,
                    tenant,
                    OwnerKind::Record,
                    label,
                    pid,
                    rushdb_types::keys::NULLNESS_VALUE,
                )
                .map_err(lookup)?
                {
                    for occ in crate::reader::try_access::occurrences_of_pid(
                        snap,
                        tenant,
                        OwnerKind::Record,
                        owner,
                        pid,
                    )
                    .map_err(lookup)?
                    {
                        if let rushdb_types::occurrence::OccurrenceValue::Stored { ty } = occ.value
                        {
                            let mut entry = Vec::with_capacity(occ.payload.len() + 1);
                            entry.push(rushdb_types::encoding::logical_tag(ty));
                            entry.extend_from_slice(&occ.payload);
                            // The value, the owner and the map's own
                            // bookkeeping, charged like a query's working set.
                            held = held.saturating_add(entry.len() as u64 + MERGE_ENTRY_BYTES);
                            if let Some(limit) = self.memory_limit.filter(|limit| held > *limit) {
                                return Err(LmpgError::Conflict(format!(
                                    "query working set exceeded: upsert key `{norm}` walks the \
                                     values of {population} records of `{label}`, over \
                                     `query-memory-max` ({limit} bytes); declare an ordered \
                                     index on `{norm}`"
                                )));
                            }
                            map.entry(entry).or_default().insert(owner);
                        }
                    }
                }
                Ok(slot.insert(map))
            }
        }
    }
}

/// Family id for a normalized property name, or `None` when no record in the
/// tenant has ever carried the property.
fn family_id(snap: &impl TryRead, tenant: TenantId, norm: &str) -> Result<Option<u64>, LmpgError> {
    let raw = snap
        .get(&kx::family_sig_key(
            tenant,
            crate::plan::DEFAULT_NAMESPACE,
            OwnerKind::Record,
            norm,
        ))
        .map_err(|error| LmpgError::Conflict(format!("merge family lookup failed: {error}")))?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    let bytes: [u8; 8] = raw
        .as_slice()
        .try_into()
        .map_err(|_| LmpgError::Conflict("merge family row has invalid width".into()))?;
    Ok(Some(u64::from_be_bytes(bytes)))
}

/// Property id for one logical type of a family, when a definition exists.
fn property_id(
    snap: &impl TryRead,
    tenant: TenantId,
    fid: u64,
    ty: LogicalType,
) -> Result<Option<u64>, LmpgError> {
    let raw = snap
        .get(&kx::definition_key(tenant, fid, TypeState::Known(ty)))
        .map_err(|error| LmpgError::Conflict(format!("merge definition lookup failed: {error}")))?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    let bytes: [u8; 8] = raw
        .as_slice()
        .try_into()
        .map_err(|_| LmpgError::Conflict("merge definition row has invalid width".into()))?;
    Ok(Some(u64::from_be_bytes(bytes)))
}

/// Exact key comparison against a reconstructed record.
fn record_matches_keys(rec: &Json, keys: &[String], props: &[(String, FieldChange)]) -> bool {
    let Some(obj) = rec.as_object() else {
        return false;
    };
    keys.iter().all(|k| {
        let nk = normalize_name(k);
        let want = props
            .iter()
            .find(|(pk, _)| normalize_name(pk) == nk)
            .map(|(_, c)| change_value(c));
        let have = obj.get(&nk).or_else(|| {
            obj.iter()
                .find(|(ok, _)| normalize_name(ok) == nk)
                .map(|(_, v)| v)
        });
        match (want, have) {
            (Some(w), Some(h)) => key_values_equal(&w, h),
            _ => false,
        }
    })
}

/// Whether an incoming key value equals a stored one. Numbers compare by
/// value, not JSON representation: the planner widens an integer to `Float64`
/// when the property already has a float definition, so a re-import of `43`
/// must still match a stored `43.0` instead of creating a duplicate.
fn key_values_equal(want: &Json, have: &Json) -> bool {
    if want == have {
        return true;
    }
    // `Scalar` has no UInt64 variant. Do not reinterpret a large JSON u64 as
    // a rounded Float64, which could make distinct merge keys equal.
    if [want, have].iter().any(|value| {
        value
            .as_number()
            .is_some_and(|number| number.is_u64() && number.as_i64().is_none())
    }) {
        return false;
    }
    match (infer_scalar(want), infer_scalar(have)) {
        (Some((_, left)), Some((_, right))) => scalar_equal(&left, &right),
        _ => false,
    }
}

/// Legacy fallback: a full label scan with per-candidate reconstruction. Only
/// reached when no merge key is equality-indexable (arrays/objects/system
/// fields alone), so plain scalar upserts never pay the O(label) cost.
fn scan_record_by_keys(
    snap: &impl TryRead,
    tenant: TenantId,
    label: &str,
    keys: &[String],
    props: &[(String, FieldChange)],
) -> Result<Option<RecordRef>, LmpgError> {
    let members = crate::reader::try_access::label_members(snap, tenant, label)
        .map_err(|error| LmpgError::Conflict(format!("merge label scan failed: {error}")))?;
    for ord in members {
        let Some(hdr) = crate::reader::try_access::record_by_ord(snap, tenant, ord)
            .map_err(|error| LmpgError::Conflict(format!("merge scan record failed: {error}")))?
        else {
            continue;
        };
        let Some(rec) =
            crate::reconstruct::reconstruct_record(snap, tenant, ord).map_err(|error| {
                LmpgError::Conflict(format!("merge scan reconstruction failed: {error}"))
            })?
        else {
            continue;
        };
        if record_matches_keys(&rec, keys, props) {
            return Ok(Some(RecordRef {
                record_id: hdr.record_id,
                ordinal: ord,
            }));
        }
    }
    Ok(None)
}

/// Apply mergeBy upsert (or plain create) to a BFS record set and return the
/// draft->persisted id map so relations and inline vectors can be remapped
/// (reference importUpsertRecords / importRecords).
///
/// # Errors
/// Returns [`LmpgError`] if any record or relationship write fails.
pub fn import_bfs(
    lmpg: &Lmpg,
    plan: &BfsPlan,
    options: &ImportOptions,
) -> Result<Vec<UpsertIdMap>, LmpgError> {
    import_bfs_with_cancel(lmpg, plan, options, &|| false)
}

/// Cancellation-aware BFS import. The callback is polled before each
/// bounded commit phase so a cancelled request does not start another batch.
///
/// # Errors
/// Returns [`LmpgError`] when validation, cancellation, planning, or a
/// durable record/relationship commit fails.
pub fn import_bfs_with_cancel(
    lmpg: &Lmpg,
    plan: &BfsPlan,
    options: &ImportOptions,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<UpsertIdMap>, LmpgError> {
    if let Some(error) = plan.validation_errors.first() {
        return Err(LmpgError::Conflict(error.clone()));
    }
    // Plain create (no mergeBy / mergeStrategy): commit every record and
    // relationship in a tiny number of batch commits instead of one commit per
    // record. Per-record commits each run a redb write transaction with fsync +
    // MVCC bookkeeping (~12ms each), which made a 800-row CSV take ~10s.
    if options.merge_by.is_none() && options.merge_strategy.is_none() {
        return import_bfs_batch_create(lmpg, plan, is_cancelled);
    }
    import_bfs_merge(lmpg, plan, options, is_cancelled)
}

/// BFS import without merge semantics: all record creates are staged into one
/// atomic batch commit (durable once), and all relationships into a second.
/// The caller-provided draft ids map to persisted ids in plan order because
/// `run_batch` returns `created_records` in op order.
fn import_bfs_batch_create(
    lmpg: &Lmpg,
    plan: &BfsPlan,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<UpsertIdMap>, LmpgError> {
    if is_cancelled() {
        return Err(LmpgError::Conflict("import cancelled".into()));
    }
    let mut record_batch = lmpg.begin_batch();
    for rec in &plan.records {
        record_batch.ops.push(crate::BatchOp::CreateRecord {
            label: rec.label.clone(),
            explicit_id: rec.explicit_id,
            fields: fields_from_props(&rec.properties),
        });
    }
    let committed = lmpg.run_batch(record_batch)?;
    let idmap: Vec<UpsertIdMap> = plan
        .records
        .iter()
        .zip(committed.created_records.iter())
        .map(|(rec, rr)| UpsertIdMap {
            draft_id: rec.draft_id,
            persisted_id: rr.record_id,
        })
        .collect();

    if !plan.relations.is_empty() {
        if is_cancelled() {
            return Err(LmpgError::Conflict(
                "partial import progress: records committed, relationships not; retrying a plain-create import creates the records again: import cancelled"
                    .into(),
            ));
        }
        let mut rel_batch = lmpg.begin_batch();
        let snap = lmpg
            .store
            .read_view()
            .map_err(|e| LmpgError::Conflict(e.to_string()))?;
        let tenant = lmpg.tenant;
        // dedupe pairs within one plan (the sequential path relies on each
        // relationship being committed before the next adjacency check)
        let mut seen: std::collections::BTreeSet<(u64, String, u64)> =
            std::collections::BTreeSet::new();
        for rel in &plan.relations {
            let source = idmap
                .iter()
                .find(|m| m.draft_id == rel.source)
                .map_or(rel.source, |m| m.persisted_id);
            let target = idmap
                .iter()
                .find(|m| m.draft_id == rel.target)
                .map_or(rel.target, |m| m.persisted_id);
            let Some(src) = crate::reader::try_access::record_by_id(
                &snap,
                tenant,
                &rushdb_types::ids::RecordId(source),
            )
            .map_err(|e| LmpgError::Conflict(e.to_string()))?
            else {
                continue;
            };
            let Some(tgt) = crate::reader::try_access::record_by_id(
                &snap,
                tenant,
                &rushdb_types::ids::RecordId(target),
            )
            .map_err(|e| LmpgError::Conflict(e.to_string()))?
            else {
                continue;
            };
            if !seen.insert((src.ordinal, rel.rel_type.clone(), tgt.ordinal)) {
                continue;
            }
            let already_linked = crate::reader::try_access::edge_exists(
                &snap,
                tenant,
                src.ordinal,
                &rel.rel_type,
                tgt.ordinal,
            )
            .map_err(|e| LmpgError::Conflict(e.to_string()))?;
            if already_linked {
                continue;
            }
            rel_batch.ops.push(crate::BatchOp::CreateRelationship {
                rel_type: rel.rel_type.clone(),
                source: RecordRef {
                    record_id: source,
                    ordinal: src.ordinal,
                },
                target: RecordRef {
                    record_id: target,
                    ordinal: tgt.ordinal,
                },
                fields: vec![],
            });
        }
        if !rel_batch.ops.is_empty() {
            lmpg.run_batch(rel_batch).map_err(|error| {
                LmpgError::Conflict(format!(
                    "partial import progress: records committed, relationships not; retrying a plain-create import creates the records again: {error}"
                ))
            })?;
        }
    }

    Ok(idmap)
}

/// Records staged into one atomic merge commit. Merge keys must be unique
/// within a chunk so each record resolves against committed state alone; a
/// chunk containing duplicate keys falls back to per-record commits.
const MERGE_BATCH: usize = 1024;

/// Merge/upsert path (mergeBy / mergeStrategy). Records are matched through
/// a per-chunk [`MergeIndex`] (`find_record_by_keys`) and written in
/// bounded atomic batches instead of one commit per record: on a large store a
/// per-record commit costs ~25ms, so a 192-row upsert spent ~5s in pure commit
/// overhead. Records sharing a merge key inside one chunk still resolve
/// sequentially, so a later row upserts the row an earlier one created. Plain
/// CSV/JSON imports use the batched create path instead.
fn import_bfs_merge(
    lmpg: &Lmpg,
    plan: &BfsPlan,
    options: &ImportOptions,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Vec<UpsertIdMap>, LmpgError> {
    check_merge_keys(options)?;
    let mut idmap: Vec<UpsertIdMap> = Vec::with_capacity(plan.records.len());
    for chunk in plan.records.chunks(MERGE_BATCH) {
        if is_cancelled() {
            return Err(LmpgError::Conflict("import cancelled".into()));
        }
        if chunk_has_duplicate_keys(chunk, options) {
            import_chunk_sequential(lmpg, chunk, options, &mut idmap)?;
        } else {
            import_chunk_batched(lmpg, chunk, options, &mut idmap)?;
        }
    }
    if is_cancelled() {
        return Err(LmpgError::Conflict("import cancelled".into()));
    }
    import_relations_batched(lmpg, plan, &idmap)?;
    Ok(idmap)
}

/// The merge keys for one draft: the configured `mergeBy` list, or every
/// incoming property when `mergeBy` is empty or only a strategy is set.
fn merge_keys(rec: &BfsRecord, options: &ImportOptions) -> Vec<String> {
    let every_key = || {
        rec.explicit_id
            .map(|_| ID_KEY.to_owned())
            .into_iter()
            .chain(rec.properties.iter().map(|(k, _)| k.clone()))
            .collect()
    };
    match &options.merge_by {
        Some(keys) if !keys.is_empty() => keys.clone(),
        Some(_) => every_key(),
        None if options.merge_strategy.is_some() => every_key(),
        None => Vec::new(),
    }
}

/// The merge key that names the record identity.
const ID_KEY: &str = rushdb_types::value::ID_FIELD;

/// Refuse a `mergeBy` key the engine reserves: no record stores a `__*`
/// property, so such a key could never match (`__id` is the identity and is
/// looked up by id).
fn check_merge_keys(options: &ImportOptions) -> Result<(), LmpgError> {
    for key in options.merge_by.iter().flatten() {
        let norm = normalize_name(key);
        if norm != ID_KEY && rushdb_types::value::is_reserved_name(&norm) {
            return Err(LmpgError::Conflict(format!(
                "invalid merge key `{key}`: names starting with `__` are reserved for the engine"
            )));
        }
    }
    Ok(())
}

/// `(label, [(normalized key, canonical value)])` used to detect rows that
/// share a merge key within one chunk.
type MergeSignature = (String, Vec<(String, Option<String>)>);

/// True when two drafts in `chunk` share a merge-key signature. Such rows
/// depend on each other's committed state, so the chunk must be written one
/// record at a time (the batched path resolves every row against committed
/// state only).
fn chunk_has_duplicate_keys(chunk: &[BfsRecord], options: &ImportOptions) -> bool {
    let mut seen: BTreeSet<MergeSignature> = BTreeSet::new();
    for rec in chunk {
        let keys = merge_keys(rec, options);
        if keys.is_empty() {
            continue;
        }
        let signature = (
            rec.label.clone(),
            keys.iter()
                .map(|k| {
                    let norm = normalize_name(k);
                    if norm == ID_KEY {
                        let id = rec
                            .explicit_id
                            .map(|id| format!("id:{}", uuid::Uuid::from_bytes(id).simple()));
                        return (norm, id);
                    }
                    let value = rec
                        .properties
                        .iter()
                        .find(|(pk, _)| normalize_name(pk) == norm)
                        .map(|(_, c)| canonical_key_repr(&change_value(c)));
                    (norm, value)
                })
                .collect(),
        );
        if !seen.insert(signature) {
            return true;
        }
    }
    false
}

/// Canonical string form of a key value for duplicate detection. Numeric
/// values use the same exact integer/float round-trip rule as merge matching,
/// so large integers cannot collide after an imprecise `f64` widening.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    reason = "exact numeric canonicalization checks the integer round trip before using the integer form"
)]
fn canonical_key_repr(value: &Json) -> String {
    match value {
        Json::Number(_) => match infer_scalar(value) {
            Some((_, Scalar::Float64(_)))
                if value
                    .as_number()
                    .is_some_and(|number| number.is_u64() && number.as_i64().is_none()) =>
            {
                format!("u:{}", value.as_u64().unwrap_or_default())
            }
            Some((_, Scalar::Int64(i))) => format!("i:{i}"),
            Some((_, Scalar::Float64(f))) if f.is_finite() && f.fract() == 0.0 => {
                // Float-to-int casts saturate at the integer bounds. Check the
                // exclusive positive edge first so 2^63 cannot masquerade as
                // i64::MAX after a saturating cast and round-trip.
                let in_i64_domain = f >= i64::MIN as f64 && f < 2_f64.powi(63);
                if in_i64_domain {
                    let i = f as i64;
                    if (i as f64) == f {
                        return format!("i:{i}");
                    }
                }
                format!("f:{:016x}", f.to_bits())
            }
            Some((_, Scalar::Float64(f))) => format!("f:{:016x}", f.to_bits()),
            _ => value.to_string(),
        },
        other => other.to_string(),
    }
}

/// Batched merge for a chunk with pairwise-unique merge keys: matched records
/// become `UpdateFields`, the rest `CreateRecord`, all in one atomic commit.
fn import_chunk_batched(
    lmpg: &Lmpg,
    chunk: &[BfsRecord],
    options: &ImportOptions,
    idmap: &mut Vec<UpsertIdMap>,
) -> Result<(), LmpgError> {
    let rewrite = options.merge_strategy.as_deref() == Some("rewrite");
    let mut batch = lmpg.begin_batch();
    // Per draft: the matched id, or `None` when the batch creates it (created
    // ids come back in op order).
    let mut resolved: Vec<Option<[u8; 16]>> = Vec::with_capacity(chunk.len());
    // Every record resolves against the same committed snapshot (keys are
    // pairwise unique within the chunk), so one merge index serves it.
    let snap = merge_snapshot(lmpg)?;
    let mut index = MergeIndex::new(lmpg);
    for rec in chunk {
        let keys = merge_keys(rec, options);
        let existing = if keys.is_empty() {
            None
        } else {
            find_record_by_keys(
                lmpg,
                &snap,
                &mut index,
                &rec.label,
                (&keys, rec.explicit_id),
                &rec.properties,
            )?
        };
        if let Some(rr) = existing {
            // The batch `UpdateFields` arm does not gate on label policy, so
            // mirror the per-record path's default-deny check here.
            lmpg.authorize_label(&rec.label)
                .map_err(LmpgError::Conflict)?;
            let mut fields = Vec::new();
            if rewrite {
                fields.extend(existing_field_deletes(lmpg, rr)?);
            }
            fields.extend(fields_from_props(&rec.properties));
            batch.ops.push(crate::BatchOp::UpdateFields {
                owner: OwnerKind::Record,
                owner_ord: rr.ordinal,
                owner_public_id: rr.record_id,
                fields,
            });
            resolved.push(Some(rr.record_id));
        } else {
            batch.ops.push(crate::BatchOp::CreateRecord {
                label: rec.label.clone(),
                explicit_id: rec.explicit_id,
                fields: fields_from_props(&rec.properties),
            });
            resolved.push(None);
        }
    }
    let committed = lmpg.run_batch(batch)?;
    let mut created = committed.created_records.into_iter();
    for (rec, slot) in chunk.iter().zip(resolved) {
        let persisted_id = match slot {
            Some(id) => id,
            None => {
                created
                    .next()
                    .ok_or_else(|| {
                        LmpgError::Conflict("merge batch created fewer records than staged".into())
                    })?
                    .record_id
            }
        };
        idmap.push(UpsertIdMap {
            draft_id: rec.draft_id,
            persisted_id,
        });
    }
    Ok(())
}

/// Per-record merge fallback for chunks that contain duplicate merge keys.
fn import_chunk_sequential(
    lmpg: &Lmpg,
    chunk: &[BfsRecord],
    options: &ImportOptions,
    idmap: &mut Vec<UpsertIdMap>,
) -> Result<(), LmpgError> {
    let rewrite = options.merge_strategy.as_deref() == Some("rewrite");
    for rec in chunk {
        let keys = merge_keys(rec, options);
        // Each record sees its predecessors' commits: a fresh snapshot and
        // merge index per record.
        let existing = if keys.is_empty() {
            None
        } else {
            find_record_by_keys(
                lmpg,
                &merge_snapshot(lmpg)?,
                &mut MergeIndex::new(lmpg),
                &rec.label,
                (&keys, rec.explicit_id),
                &rec.properties,
            )?
        };
        let persisted_id = if let Some(rr) = existing {
            if rewrite {
                lmpg.update_fields(rr, existing_field_deletes(lmpg, rr)?)?;
            }
            lmpg.update_fields(rr, fields_from_props(&rec.properties))?;
            rr.record_id
        } else {
            lmpg.create_record(
                &rec.label,
                rec.explicit_id,
                fields_from_props(&rec.properties),
            )?
            .record_id
        };
        idmap.push(UpsertIdMap {
            draft_id: rec.draft_id,
            persisted_id,
        });
    }
    Ok(())
}

/// Deletions for every live property of an existing record (`rewrite` merge).
fn existing_field_deletes(
    lmpg: &Lmpg,
    rr: RecordRef,
) -> Result<Vec<(String, FieldChange)>, LmpgError> {
    let snap = lmpg
        .store
        .read_view()
        .map_err(|e| LmpgError::Conflict(e.to_string()))?;
    let fields: Vec<String> =
        crate::reconstruct::reconstruct_record(&snap, lmpg.tenant, rr.ordinal)
            .map_err(|e| LmpgError::Conflict(e.to_string()))?
            .and_then(|r| r.as_object().cloned())
            .map(|o| {
                o.keys()
                    .filter(|k| !rushdb_types::value::is_reserved_name(k))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
    Ok(fields
        .into_iter()
        .map(|k| (k, FieldChange::Delete))
        .collect())
}

/// All relationship creates for a merge plan in one atomic commit. Runs after
/// every record chunk is committed, so endpoints resolve from persisted state.
fn import_relations_batched(
    lmpg: &Lmpg,
    plan: &BfsPlan,
    idmap: &[UpsertIdMap],
) -> Result<(), LmpgError> {
    if plan.relations.is_empty() {
        return Ok(());
    }
    let by_draft: std::collections::HashMap<[u8; 16], [u8; 16]> =
        idmap.iter().map(|m| (m.draft_id, m.persisted_id)).collect();
    let snap = lmpg
        .store
        .read_view()
        .map_err(|e| LmpgError::Conflict(e.to_string()))?;
    let mut batch = lmpg.begin_batch();
    // A single commit cannot see edges it stages earlier in the same batch, so
    // dedupe pairs here (the per-record path relied on each commit being
    // visible to the next `edge_exists` probe).
    let mut seen: BTreeSet<(u64, String, u64)> = BTreeSet::new();
    for rel in &plan.relations {
        let source = by_draft.get(&rel.source).copied().unwrap_or(rel.source);
        let target = by_draft.get(&rel.target).copied().unwrap_or(rel.target);
        let Some(src) = crate::reader::try_access::record_by_id(
            &snap,
            lmpg.tenant,
            &rushdb_types::ids::RecordId(source),
        )
        .map_err(|e| LmpgError::Conflict(e.to_string()))?
        else {
            continue;
        };
        let Some(tgt) = crate::reader::try_access::record_by_id(
            &snap,
            lmpg.tenant,
            &rushdb_types::ids::RecordId(target),
        )
        .map_err(|e| LmpgError::Conflict(e.to_string()))?
        else {
            continue;
        };
        if !seen.insert((src.ordinal, rel.rel_type.clone(), tgt.ordinal)) {
            continue;
        }
        let already_linked = crate::reader::try_access::edge_exists(
            &snap,
            lmpg.tenant,
            src.ordinal,
            &rel.rel_type,
            tgt.ordinal,
        )
        .map_err(|e| LmpgError::Conflict(e.to_string()))?;
        if already_linked {
            continue;
        }
        batch.ops.push(crate::BatchOp::CreateRelationship {
            rel_type: rel.rel_type.clone(),
            source: RecordRef {
                record_id: source,
                ordinal: src.ordinal,
            },
            target: RecordRef {
                record_id: target,
                ordinal: tgt.ordinal,
            },
            fields: vec![],
        });
    }
    if !batch.ops.is_empty() {
        lmpg.run_batch(batch)?;
    }
    Ok(())
}

/// Canonical space alias for an inline external-vector entry (DB contract:
/// `NestJS` resolves its external index to this alias before binding). Vectors
/// are data; embedding generation stays in the control plane.
#[must_use]
pub fn external_space_alias(label: &str, property: &str, sim: &str, dims: usize) -> String {
    format!("emb__{label}__{property}__{sim}__{dims}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_option_shapes_are_rejected_before_defaults() {
        assert!(ImportOptions::validate_json(&serde_json::json!({
            "mergeBy": "id"
        }))
        .is_err());
        assert!(ImportOptions::validate_json(&serde_json::json!({
            "relationshipTypes": {"children": 1}
        }))
        .is_err());
        assert!(ImportOptions::validate_json(&serde_json::json!({
            "mergeBy": ["id"],
            "returnResult": true
        }))
        .is_ok());
    }

    #[test]
    fn large_unsigned_merge_keys_do_not_round_into_float_keys() {
        let large = serde_json::json!(18_014_398_509_481_985u64);
        let rounded = serde_json::json!(18_014_398_509_481_984.0f64);
        assert!(!key_values_equal(&large, &rounded));
        assert_ne!(canonical_key_repr(&large), canonical_key_repr(&rounded));
    }

    #[test]
    fn float_power_of_two_does_not_alias_i64_max_merge_key() {
        let max = serde_json::json!(i64::MAX);
        let power = serde_json::json!(9_223_372_036_854_776_000.0f64);
        assert_ne!(canonical_key_repr(&max), canonical_key_repr(&power));
    }

    #[test]
    fn malformed_inline_vectors_are_recorded_as_plan_errors() {
        let plan = serialize_bfs(
            &serde_json::json!({"name": "Ada", "$vectors": {"broken": true}}),
            "Person",
            &ImportOptions::default(),
        );
        assert_eq!(plan.vector_drafts.len(), 0);
        assert_eq!(plan.validation_errors.len(), 1);
        assert!(plan.validation_errors[0].contains("invalid $vectors"));
    }

    #[test]
    fn cancelled_transactional_import_discards_staged_graph() {
        let store = std::sync::Arc::new(rushdb_storage::Store::new_in_memory());
        let tenant = rushdb_types::ids::TenantId::from_ordinal(95);
        let lmpg = Lmpg::new(std::sync::Arc::clone(&store), tenant);
        let mut tx = rushdb_storage::TxAccumulator::new(tenant, store.committed_version());
        let checks = std::cell::Cell::new(0usize);
        let error = import_document_tx_with_cancel(
            &lmpg,
            "Parent",
            &serde_json::json!({"name": "root", "child": {"name": "nested"}}),
            &mut tx,
            &|| {
                let current = checks.get();
                checks.set(current + 1);
                current >= 2
            },
        )
        .expect_err("cancellation should abort the staged import");
        assert!(error.to_string().contains("cancelled"));
        assert!(
            !tx.mutations.is_empty(),
            "root should have been staged before cancellation"
        );
        assert_eq!(store.committed_version(), 0);
        assert!(store
            .read_view()
            .unwrap()
            .range_prefix(&rushdb_types::keys::record_hdr_prefix(tenant))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn cancelled_bfs_import_does_not_start_a_commit() {
        let store = std::sync::Arc::new(rushdb_storage::Store::new_in_memory());
        let lmpg = Lmpg::new(
            std::sync::Arc::clone(&store),
            rushdb_types::ids::TenantId::from_ordinal(94),
        );
        let plan = serialize_bfs(
            &serde_json::json!({"name": "cancelled"}),
            "Doc",
            &ImportOptions::default(),
        );
        let error = import_bfs_with_cancel(&lmpg, &plan, &ImportOptions::default(), &|| true)
            .err()
            .expect("cancelled import");
        assert!(error.to_string().contains("cancelled"));
        assert_eq!(store.committed_version(), 0);
    }

    #[test]
    fn cancelled_relationship_phase_reports_resumable_partial_progress() {
        let store = std::sync::Arc::new(rushdb_storage::Store::new_in_memory());
        let tenant = rushdb_types::ids::TenantId::from_ordinal(93);
        let lmpg = Lmpg::new(std::sync::Arc::clone(&store), tenant);
        let plan = serialize_bfs(
            &serde_json::json!({"name": "root", "department": [{"name": "nested"}]}),
            "Parent",
            &ImportOptions::default(),
        );
        assert_eq!(plan.records.len(), 2);
        assert_eq!(plan.relations.len(), 1);

        let checks = std::cell::Cell::new(0usize);
        let Err(error) = import_bfs_with_cancel(&lmpg, &plan, &ImportOptions::default(), &|| {
            let current = checks.get();
            checks.set(current + 1);
            current >= 1
        }) else {
            panic!("cancellation between record and relationship batches")
        };

        assert!(error.to_string().contains("partial import progress"));
        // Plain create has no merge key, so a blind retry would duplicate
        // the committed records; the message must not promise otherwise.
        assert!(!error.to_string().contains("idempotent"));
        assert!(error.to_string().contains("creates the records again"));
        assert_eq!(store.committed_version(), 1);
        let snapshot = store.read_view().unwrap();
        assert_eq!(
            crate::reader::try_access::label_members(&snapshot, tenant, "Parent")
                .unwrap()
                .len(),
            1
        );
    }
}
