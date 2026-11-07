//! By-id updates of relationship properties (`mutate.updateRelationshipsById`),
//! and the typed property writes the by-id relationship mutations share.

use super::relationships::{directed, MatchedEdges};
use super::{bytes_from_hex, RecordId, RequestScope};

impl RequestScope<'_> {
    /// Update the properties of the existing `relation_type` relationships
    /// between every source and every target record, in the given
    /// direction. A pair without such a relationship is skipped; a dangling
    /// id fails the call before anything is written, and every update
    /// commits in one batch. Returns how many relationships were updated.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn update_relationships_by_id(
        &self,
        command: &rushdb_protocol::UpdateRelationshipsMutation,
    ) -> Result<serde_json::Value, String> {
        let snap = self.store.read_view().map_err(|e| e.to_string())?;
        self.update_relationships_by_id_in_view(command, &snap)
    }

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
/// `properties`, and a typed non-null value must match its type: `string`,
/// `number` and `boolean` take that scalar or an array of them; `datetime`
/// takes a scalar ISO 8601 date or datetime string, written as a datetime.
/// A `null` is an explicit null whatever its type; an untyped value is
/// written as given.
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
                (Some(PropertyType::Datetime), serde_json::Value::Array(_)) => {
                    return Err(format!(
                        "invalid {operation}: property `{name}` is typed datetime but holds an \
                         array; datetime arrays are not supported"
                    ));
                }
                (Some(PropertyType::Datetime), serde_json::Value::String(text)) => {
                    FieldChange::SetDatetime(
                        rushdb_types::datetime::parse_datetime(text).ok_or_else(|| {
                            format!(
                                "invalid {operation}: property `{name}` is typed datetime but \
                                 its string is not an ISO 8601 date or datetime"
                            )
                        })?,
                    )
                }
                (Some(ty), value) => {
                    let matches = |item: &serde_json::Value| match ty {
                        PropertyType::String => item.is_string(),
                        PropertyType::Number => item.is_number(),
                        PropertyType::Boolean => item.is_boolean(),
                        PropertyType::Datetime => false,
                    };
                    let mismatch = match value {
                        serde_json::Value::Array(items) => {
                            items.iter().find(|item| !matches(item)).map(|item| {
                                format!(
                                    "an array with a {} element",
                                    rushdb_types::value::json_type_name(item)
                                )
                            })
                        }
                        scalar => (!matches(scalar)).then(|| {
                            format!("a {} value", rushdb_types::value::json_type_name(scalar))
                        }),
                    };
                    if let Some(held) = mismatch {
                        return Err(format!(
                            "invalid {operation}: property `{name}` is typed {} but holds {held}",
                            ty.as_str()
                        ));
                    }
                    FieldChange::Set(value.clone())
                }
            };
            Ok((name.clone(), change))
        })
        .collect()
}
