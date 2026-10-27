//! MongoDB `$text`: `{"$text": {"$search": "…"}}` at the top of a `find`
//! filter or of a pipeline's first `$match`, searched in the collection's
//! one text index, plus `{"$meta": "textScore"}` in a find's projection and
//! sort.
//!
//! The search runs once, at the statement's snapshot. Its matches restrict
//! the rest of the query (`$id $in`), so every other filter, sort, page and
//! group applies to them unchanged.

use std::collections::HashMap;
use std::sync::Arc;

use rushdb_lmpg::reader::try_access as access;
use rushdb_lmpg::Lmpg;
use rushdb_storage::ReadView;
use rushdb_textsearch::{LexicalOptions, TextIndexes, TextQuery, ALL_HITS};
use rushdb_types::index::{IndexKind, IndexState};
use serde_json::{json, Map, Value};

/// The matches of a `$text` filter: record id → BM25 score.
pub(crate) type Scores = HashMap<String, f64>;

/// Take `$text` out of `filter`, search it and restrict `filter` to its
/// matches. `None` when the filter has no `$text`.
pub(crate) fn take_text(
    lmpg: &Lmpg,
    snap: &ReadView,
    collection: &str,
    filter: &mut Map<String, Value>,
    text: Option<&Arc<TextIndexes>>,
) -> Result<Option<Scores>, String> {
    let Some(spec) = filter.remove("$text") else {
        return Ok(None);
    };
    let search = parse_spec(&spec)?;
    let text = text.ok_or("mongo: `$text` is unavailable here")?;
    let index = collection_index(snap, lmpg, collection)?;
    let found = text.search(
        lmpg.tenant,
        snap,
        TextQuery {
            index: &index,
            query: &search,
            opts: LexicalOptions::default(),
            top_k: ALL_HITS,
        },
        None,
    )?;
    let mut scores = Scores::with_capacity(found.hits.len());
    for hit in found.hits {
        if let Some(header) =
            access::record_by_ord(snap, lmpg.tenant, hit.owner_ord).map_err(|e| e.to_string())?
        {
            scores.insert(
                uuid::Uuid::from_bytes(header.record_id)
                    .hyphenated()
                    .to_string(),
                hit.score,
            );
        }
    }
    let mut ids: Vec<&String> = scores.keys().collect();
    ids.sort();
    let matches = json!({ "$id": { "$in": ids } });
    if filter.is_empty() {
        *filter = matches.as_object().cloned().unwrap_or_default();
    } else {
        let rest = std::mem::take(filter);
        filter.insert("$and".into(), json!([Value::Object(rest), matches]));
    }
    Ok(Some(scores))
}

/// The `$search` string of a `$text` operator.
fn parse_spec(spec: &Value) -> Result<String, String> {
    let fields = spec.as_object().ok_or("mongo: `$text` must be an object")?;
    let mut search = None;
    for (name, value) in fields {
        match name.as_str() {
            "$search" => {
                search = Some(
                    value
                        .as_str()
                        .ok_or("mongo: `$text.$search` must be a string")?
                        .to_owned(),
                );
            }
            // Text indexes have one analyzer, which lowercases and keeps
            // diacritics; the language option names a stemmer they lack.
            "$language" => {}
            "$caseSensitive" | "$diacriticSensitive" => {
                if value.as_bool() != Some(false) {
                    return Err(format!("mongo: `$text.{name}` is unsupported"));
                }
            }
            _ => return Err(format!("mongo: unknown `$text` option `{name}`")),
        }
    }
    search.ok_or_else(|| "mongo: `$text` requires `$search`".to_string())
}

/// The one ready text index of `collection` (MongoDB allows one per
/// collection).
fn collection_index(snap: &ReadView, lmpg: &Lmpg, collection: &str) -> Result<String, String> {
    let mut names: Vec<String> = rushdb_lmpg::index::definitions(snap, lmpg.tenant)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|def| {
            matches!(def.kind, IndexKind::Text { .. })
                && def.state == IndexState::Ready
                && def.label == collection
        })
        .map(|def| def.name)
        .collect();
    match names.len() {
        0 => Err(format!(
            "mongo: `$text` requires a ready text index on `{collection}`"
        )),
        1 => Ok(names.remove(0)),
        _ => Err(format!(
            "mongo: `$text` requires exactly one text index on `{collection}`, found {}",
            names.join(", ")
        )),
    }
}

/// Whether `value` is `{"$meta": "textScore"}`.
pub(crate) fn is_text_score(value: &Value) -> bool {
    value
        .as_object()
        .and_then(|fields| fields.get("$meta"))
        .and_then(Value::as_str)
        == Some("textScore")
}

/// Remove every `{"$meta": "textScore"}` entry of `fields` (a projection or
/// a sort); the names they had, in order.
pub(crate) fn take_text_score(fields: &mut Map<String, Value>) -> Vec<String> {
    let names: Vec<String> = fields
        .iter()
        .filter(|(_, value)| is_text_score(value))
        .map(|(name, _)| name.clone())
        .collect();
    for name in &names {
        fields.remove(name);
    }
    names
}
