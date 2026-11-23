//! By-id updates of relationship properties (`mutate.updateRelationshipsById`),
//! and the typed property writes the by-id relationship mutations share.

use super::relationships::{directed, MatchedEdges};
use super::{bytes_from_hex, RecordId, RequestScope};

impl RequestScope<'_> {
    pub(super) fn update_relationships_by_id_in_view(
        &self,
        command: &rushdb_protocol::UpdateRelationshipsMutation,
        snap: &rushdb_storage::ReadView,
    ) -> Result<serde_json::Value, String> {
        use rushdb_lmpg::plan::FieldChange;
        if command.relation_type.trim().is_empty() {
            return Err("invalid updateRelationshipsById: relationType must be non-empty".into());
        }
        let set = command
            .set
            .as_object()
            .ok_or("invalid updateRelationshipsById: set must be an object")?;
        if let Some(name) = command.unset.iter().find(|name| set.contains_key(*name)) {
            return Err(format!(
                "invalid updateRelationshipsById: property `{name}` is both set and unset"
            ));
        }
        let fields: Vec<(String, FieldChange)> =
            typed_property_changes("updateRelationshipsById", "set", set, &command.types)?
                .into_iter()
                .chain(
                    command
                        .unset
                        .iter()
                        .map(|name| (name.clone(), FieldChange::Delete)),
                )
                .collect();

        let ordinal_of = |id_hex: &String| -> Result<u64, String> {
            let id = RecordId(bytes_from_hex(id_hex)?);
            rushdb_lmpg::reader::try_access::record_by_id(&snap, self.tenant(), &id)
                .map_err(|e| e.to_string())?
                .map(|hdr| hdr.ordinal)
                .ok_or_else(|| format!("record not found: {id_hex}"))
        };
        let sources = command
            .source_ids
            .iter()
            .map(ordinal_of)
            .collect::<Result<Vec<_>, _>>()?;
        let targets = command
            .target_ids
            .iter()
            .map(ordinal_of)
            .collect::<Result<Vec<_>, _>>()?;

        let mut matched = MatchedEdges::default();
        for source in &sources {
            for target in &targets {
                let (from, to) = directed(command.direction, *source, *target);
                matched.edges_between(
                    snap,
                    self.tenant(),
                    from,
                    to,
                    Some(&command.relation_type),
                )?;
            }
        }
        let updated = matched.update(&self.lmpg(), &fields)?;
        Ok(serde_json::json!({ "message": "Relations updated", "updated": updated }))
    }

    /// Update relationships between explicit record IDs using the
    /// caller-owned execution view.
    pub(crate) fn update_relationships_by_id_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        command: &rushdb_protocol::UpdateRelationshipsMutation,
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        self.update_relationships_by_id_in_view(command, context.read_view())
    }
}

/// The writes of a relationship mutation's properties (`field`, `set` or
/// `properties`) under its declared `types`. Every typed name must be in
/// `properties`, and a typed non-null value (or each element of an array)
/// must be a value of its type ([`typed_value`]). A `null` is an explicit
/// null whatever its type; an untyped value is written as given.
///
/// # Errors
///
/// Returns an `invalid {operation}: ...` message for a typed name missing
/// from `properties` or a value that does not match its type.
pub(super) fn typed_property_changes(
    operation: &str,
    field: &str,
    properties: &serde_json::Map<String, serde_json::Value>,
    types: &std::collections::BTreeMap<String, rushdb_protocol::PropertyType>,
) -> Result<Vec<(String, rushdb_lmpg::plan::FieldChange)>, String> {
    use rushdb_lmpg::plan::FieldChange;
    use rushdb_protocol::PropertyType;
    if let Some(name) = types.keys().find(|name| !properties.contains_key(*name)) {
        return Err(format!(
            "invalid {operation}: types names `{name}`, which is not in {field}"
        ));
    }
    properties
        .iter()
        .map(|(name, value)| {
            let declared = types.get(name).copied();
            let change = match (declared, value) {
                (_, serde_json::Value::Null) => FieldChange::ExplicitNull,
                (None, _) => FieldChange::Set(value.clone()),
                (Some(ty), value) => {
                    let as_typed = |item: &serde_json::Value| typed_value(ty, item);
                    let changed = match value {
                        serde_json::Value::Array(items) => items
                            .iter()
                            .map(|item| as_typed(item).ok_or(item))
                            .collect::<Result<Vec<_>, _>>()
                            .map(serde_json::Value::Array)
                            .map_err(|item| {
                                format!(
                                    "holds an array with a {} element",
                                    rushdb_types::value::json_type_name(item)
                                )
                            }),
                        serde_json::Value::String(_)
                            if !matches!(
                                ty,
                                PropertyType::String | PropertyType::Number | PropertyType::Boolean
                            ) =>
                        {
                            as_typed(value)
                                .ok_or_else(|| format!("its string is not {}", literal_form(ty)))
                        }
                        scalar => as_typed(scalar).ok_or_else(|| {
                            format!(
                                "holds a {} value",
                                rushdb_types::value::json_type_name(scalar)
                            )
                        }),
                    };
                    match changed {
                        Ok(changed) => FieldChange::Set(changed),
                        Err(held) => {
                            return Err(format!(
                                "invalid {operation}: property `{name}` is typed {} but {held}",
                                ty.as_str()
                            ))
                        }
                    }
                }
            };
            Ok((name.clone(), change))
        })
        .collect()
}

/// A value written under a declared type, in the form the write path
/// stores (JSON's own value, or the tagged form of the other types);
/// `None` when it is not a value of that type.
fn typed_value(
    ty: rushdb_protocol::PropertyType,
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    use rushdb_protocol::PropertyType;
    use rushdb_types::model::LogicalType;
    let logical = match ty {
        PropertyType::String => return value.is_string().then(|| value.clone()),
        PropertyType::Number => return value.is_number().then(|| value.clone()),
        PropertyType::Boolean => return value.is_boolean().then(|| value.clone()),
        PropertyType::Datetime => LogicalType::DateTime,
        PropertyType::Date => LogicalType::Date,
        PropertyType::Localtime => LogicalType::LocalTime,
        PropertyType::Localdatetime => LogicalType::LocalDateTime,
        PropertyType::Duration => LogicalType::Duration,
        PropertyType::Point => LogicalType::Point,
    };
    // A datetime reads a date or a zone-less date-time in UTC, as the hint
    // always has; the other types take their own form only.
    let scalar = if logical == LogicalType::DateTime && value.is_string() {
        rushdb_types::types::coerce_literal(logical, value)
    } else {
        rushdb_types::types::parse_literal(logical, value)
    }?;
    Some(rushdb_types::types::to_doc_json(&scalar))
}

/// The literal form a declared type takes from a string, for refusals.
fn literal_form(ty: rushdb_protocol::PropertyType) -> &'static str {
    use rushdb_protocol::PropertyType;
    match ty {
        PropertyType::Datetime => "an ISO 8601 date or datetime",
        PropertyType::Date => "an ISO 8601 date",
        PropertyType::Localtime => "an ISO 8601 time of day",
        PropertyType::Localdatetime => "an ISO 8601 date and time",
        PropertyType::Duration => "an ISO 8601 duration",
        PropertyType::Point => "a GeoJSON point",
        PropertyType::Number => "a number",
        PropertyType::Boolean => "a boolean",
        PropertyType::String => "a string",
    }
}
