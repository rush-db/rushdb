//! Catalog reads over names and the exact value dictionaries: names spelled
//! like a requested one, a compact schema in one read, and
//! value overlap between property pairs.
//!
//! Value overlap is evidence only: how much of one property's distinct
//! values another holds, counted over the whole dictionaries rather than
//! samples. Which pairs are references is decided by the caller (the Core
//! platform's relationship suggestions), not here.

use super::working_set::{value_bytes, WorkingSet};
use super::{Arc, RequestScope};
use rushdb_lmpg::CatalogAccess;
use rushdb_protocol::{
    JoinEnd, JoinPair, LabelSummary, NameSuggestion, PropertyDescription, PropertySummary,
    RelationshipSummary, ValueOverlap,
};
use rushdb_types::names::name_similarity;
use std::collections::{BTreeMap, HashMap};

/// Most distinct values compared per property; a larger dictionary is
/// reported as not compared.
const MAX_COMPARED_DISTINCT: u64 = 200_000;
/// Most pairs one `describe.valueOverlap` compares.
pub(crate) const MAX_OVERLAP_PAIRS: usize = 10_000;

/// A value's join key: strings as stored, numbers and booleans as text, so a
/// numeric id matches the same id stored as a string.
fn join_key(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

/// Facts about one (label, property) value overlap reads.
struct Column {
    label: String,
    property: String,
    records_count: u64,
    is_array: bool,
}

/// A property's distinct values with owner counts, or why they were not read.
enum Dictionary {
    Values(Values),
    NotCompared(&'static str),
}

/// Distinct join keys with owner counts, and the numeric span when (nearly)
/// every key is a number, as digits stored in strings often are.
struct Values {
    counts: HashMap<String, u64>,
    span: Option<(f64, f64)>,
}

impl Values {
    fn new(counts: HashMap<String, u64>) -> Self {
        let numbers: Vec<f64> = counts.keys().filter_map(|key| key.parse().ok()).collect();
        #[expect(
            clippy::cast_precision_loss,
            reason = "a share of counts; precision is irrelevant"
        )]
        let numeric = !numbers.is_empty() && numbers.len() as f64 >= 0.9 * counts.len() as f64;
        let span = numeric.then(|| {
            numbers
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &n| {
                    (lo.min(n), hi.max(n))
                })
        });
        Self { counts, span }
    }

    fn len(&self) -> usize {
        self.counts.len()
    }
}

impl RequestScope<'_> {
    /// Every (label, property) of the catalog `access` lists, from catalog
    /// indexes only.
    pub(crate) fn catalog_columns(
        &self,
        view: &Arc<rushdb_storage::ReadView>,
        labels: &[String],
        access: &CatalogAccess,
    ) -> Result<Vec<PropertyDescription>, String> {
        let body = serde_json::json!({ "labels": labels, "detail": "catalog" });
        let mut columns = self.describe_properties_in_view(view, &body, None, &|| false)?;
        columns.retain(|column| {
            column.label.as_deref().is_some_and(|label| {
                access.column_listed(label, &column.name, column.records_count)
            })
        });
        Ok(columns)
    }

    /// Labels and properties spelled like `text`, best first, with a
    /// score of at least 0.5. `Label.property` text scores both parts.
    pub(crate) fn suggest_names_in_view(
        &self,
        view: &Arc<rushdb_storage::ReadView>,
        text: &str,
        limit: usize,
        access: &CatalogAccess,
    ) -> Result<Vec<NameSuggestion>, String> {
        let (label_part, property_part) = match text.split_once('.') {
            Some((label, property)) => (Some(label), property),
            None => (None, text),
        };
        let mut out: Vec<NameSuggestion> = Vec::new();
        if label_part.is_none() {
            for (label, count) in self.label_counts_in_view(view)? {
                if count == 0 || !access.label_listed(&label, count) {
                    continue;
                }
                let score = name_similarity(text, &label);
                out.push(NameSuggestion {
                    label,
                    property: None,
                    score,
                });
            }
        }
        for column in self.catalog_columns(view, &[], access)? {
            let Some(label) = column.label else { continue };
            let property_score = name_similarity(property_part, &column.name);
            let score = match label_part {
                Some(wanted) => property_score.min(name_similarity(wanted, &label)),
                None => property_score,
            };
            out.push(NameSuggestion {
                label,
                property: Some(column.name),
                score,
            });
        }
        out.retain(|suggestion| suggestion.score >= 0.5);
        // Equal scores (similarity folds case and plurals) rank the name
        // written exactly first, then one equal but for case, then a label
        // before a property of the same spelling.
        let closeness = |suggestion: &NameSuggestion| {
            let (wanted, name) = match (&suggestion.property, label_part) {
                (Some(property), _) => (property_part, property.as_str()),
                (None, _) => (text, suggestion.label.as_str()),
            };
            let label_ok = label_part.is_none_or(|wanted| wanted == suggestion.label);
            let spelled = if wanted == name && label_ok {
                0
            } else if wanted.eq_ignore_ascii_case(name) {
                1
            } else {
                2
            };
            (spelled, u8::from(suggestion.property.is_some()))
        };
        out.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| closeness(a).cmp(&closeness(b)))
                .then_with(|| a.label.cmp(&b.label))
                .then_with(|| a.property.cmp(&b.property))
        });
        out.dedup_by(|a, b| a.label == b.label && a.property == b.property);
        out.truncate(limit);
        Ok(out)
    }

    /// Every label with its properties in one read. Each property's
    /// dictionary is working set (`meter`) only while it is summarized.
    pub(crate) fn schema_summary_in_view(
        &self,
        view: &Arc<rushdb_storage::ReadView>,
        labels: &[String],
        few_values: u64,
        access: &CatalogAccess,
        meter: &mut WorkingSet<'_>,
    ) -> Result<Vec<LabelSummary>, String> {
        let counts: BTreeMap<String, u64> = self
            .label_counts_in_view(view)?
            .into_iter()
            .filter(|(label, count)| {
                *count > 0
                    && (labels.is_empty() || labels.contains(label))
                    && access.label_listed(label, *count)
            })
            .collect();
        let mut properties: BTreeMap<String, Vec<PropertySummary>> = BTreeMap::new();
        for column in self.catalog_columns(view, labels, access)? {
            let Some(label) = column.label.clone() else {
                continue;
            };
            if !counts.contains_key(&label) {
                continue;
            }
            meter.check()?;
            let summary =
                self.property_summary(view, &label, &column, few_values, access, meter)?;
            properties.entry(label).or_default().push(summary);
        }
        let mut shapes = self.describe_relationships_in_view(view)?;
        shapes.retain(|shape| {
            access.shape_listed(
                &shape.relationship_type,
                &shape.source_label,
                &shape.target_label,
                shape.count,
            )
        });
        Ok(counts
            .into_iter()
            .map(|(name, records_count)| {
                let mut relationships: Vec<RelationshipSummary> = Vec::new();
                for shape in &shapes {
                    if shape.source_label == name {
                        relationships.push(RelationshipSummary {
                            relationship_type: shape.relationship_type.clone(),
                            label: shape.target_label.clone(),
                            direction: "out".into(),
                            count: shape.count,
                        });
                    }
                    if shape.target_label == name && shape.source_label != name {
                        relationships.push(RelationshipSummary {
                            relationship_type: shape.relationship_type.clone(),
                            label: shape.source_label.clone(),
                            direction: "in".into(),
                            count: shape.count,
                        });
                    }
                }
                let mut properties = properties.remove(&name).unwrap_or_default();
                properties.sort_by(|a, b| a.name.cmp(&b.name));
                LabelSummary {
                    name,
                    records_count,
                    properties,
                    relationships,
                }
            })
            .collect())
    }

    fn property_summary(
        &self,
        view: &Arc<rushdb_storage::ReadView>,
        label: &str,
        column: &PropertyDescription,
        few_values: u64,
        access: &CatalogAccess,
        meter: &mut WorkingSet<'_>,
    ) -> Result<PropertySummary, String> {
        let scope = [label.to_string()];
        let mut summary = PropertySummary {
            name: column.name.clone(),
            value_type: column.value_type.clone(),
            records_count: column.records_count,
            is_array: column.is_array,
            sibling_types: column.sibling_types.clone(),
            distinct_count: None,
            min: None,
            max: None,
            values: Vec::new(),
        };
        if column.value_type == "null" {
            return Ok(summary);
        }
        // This entry's own type: a type split's text and datetime entries
        // each read their own values, range and distinct count.
        let types = logical_types_of(&column.value_type);
        let Some(domain) =
            self.value_domain_of_types_in_view(view, &column.name, &scope, types, meter)?
        else {
            // The dictionary cannot answer for values too long for it: a
            // text column is counted from the long-value counters too, so
            // every scalar column reports its distinct count (arrays still
            // do not: their values are whole arrays).
            if column.value_type == "string" && !column.is_array {
                if let Some((distinct, ranked)) =
                    self.long_text_values_in_view(view, &column.name, label, few_values, meter)?
                {
                    summary.distinct_count = Some(distinct);
                    summary.values = ranked
                        .into_iter()
                        .filter(|(_, count)| access.counted(*count))
                        .map(|(value, _)| value)
                        .collect();
                }
            }
            return Ok(summary);
        };
        let distinct = domain.distinct_total();
        summary.distinct_count = Some(distinct);
        // Under a small-count threshold extremes are withheld: a minimum or
        // maximum is often one record's value.
        if domain.numeric_only && access.shows_extremes() {
            summary.min = domain.min;
            summary.max = domain.max;
        }
        let wants_values = distinct > 0 && distinct <= few_values;
        let wants_dates = column.value_type == "datetime"
            && distinct <= MAX_COMPARED_DISTINCT
            && access.shows_extremes();
        if wants_values || wants_dates {
            let before = meter.charged();
            let ranked = self.value_dictionary_of_types_in_view(
                view,
                &column.name,
                &scope,
                types,
                None,
                meter,
            )?;
            // Only the few values listed outlive this property.
            meter.release(meter.charged() - before);
            if let Some(ranked) = ranked {
                if wants_dates {
                    // By instant: rendered text does not order signed years.
                    let instants = ranked.iter().filter_map(|(value, _)| {
                        let text = value.as_str()?;
                        Some((rushdb_types::datetime::parse_datetime(text)?, text))
                    });
                    summary.min = instants
                        .clone()
                        .min_by_key(|(ms, _)| *ms)
                        .map(|(_, text)| serde_json::Value::from(text));
                    summary.max = instants
                        .max_by_key(|(ms, _)| *ms)
                        .map(|(_, text)| serde_json::Value::from(text));
                }
                if wants_values {
                    summary.values = ranked
                        .into_iter()
                        .filter(|(_, count)| access.counted(*count))
                        .map(|(value, _)| value)
                        .collect();
                }
            }
        }
        Ok(summary)
    }

    /// A column's dictionary for comparison; the values stay charged to
    /// `meter` while the overlap holds them.
    fn dictionary(
        &self,
        view: &Arc<rushdb_storage::ReadView>,
        column: &Column,
        meter: &mut WorkingSet<'_>,
    ) -> Result<Dictionary, String> {
        if column.is_array {
            return Ok(Dictionary::NotCompared("array values"));
        }
        let scope = [column.label.clone()];
        match self.value_domain_from_index_in_view(view, &column.property, &scope, meter)? {
            Some(domain) if domain.distinct_total() > MAX_COMPARED_DISTINCT => {
                return Ok(Dictionary::NotCompared("too many distinct values"));
            }
            Some(_) => {}
            None => return Ok(Dictionary::NotCompared("long or array values")),
        }
        Ok(
            match self.value_dictionary_in_view(view, &column.property, &scope, None, meter)? {
                Some(ranked) => {
                    // Join keys replace the read values: charge the keys,
                    // release the values.
                    let mut counts = HashMap::with_capacity(ranked.len());
                    for (value, count) in ranked {
                        if let Some(key) = join_key(&value) {
                            meter.charge(value_bytes(key.len()))?;
                            counts.insert(key, count);
                        }
                        let encoded = serde_json::to_vec(&value).map_or(0, |bytes| bytes.len());
                        meter.release(value_bytes(encoded));
                    }
                    Dictionary::Values(Values::new(counts))
                }
                None => Dictionary::NotCompared("long or array values"),
            },
        )
    }

    /// Value overlap for each pair, in request order. A pair naming
    /// a property that does not exist, or that `access` hides, comes back
    /// not compared (the two are indistinguishable); under a policy's
    /// `minCount`, overlap over fewer records is withheld. The dictionaries
    /// compared are working set (`meter`) until the response is built.
    pub(crate) fn value_overlap_in_view(
        &self,
        view: &Arc<rushdb_storage::ReadView>,
        pairs: &[JoinPair],
        access: &CatalogAccess,
        meter: &mut WorkingSet<'_>,
    ) -> Result<Vec<ValueOverlap>, String> {
        if pairs.len() > MAX_OVERLAP_PAIRS {
            return Err(format!(
                "invalid request: at most {MAX_OVERLAP_PAIRS} pairs per valueOverlap"
            ));
        }
        let columns: Vec<Column> = self
            .catalog_columns(view, &[], access)?
            .into_iter()
            .filter_map(|description| {
                Some(Column {
                    label: description.label?,
                    property: description.name,
                    records_count: description.records_count,
                    is_array: description.is_array,
                })
            })
            .collect();
        let find = |end: &JoinEnd| {
            columns
                .iter()
                .position(|column| column.label == end.label && column.property == end.property)
        };
        let mut dictionaries: HashMap<usize, Dictionary> = HashMap::new();
        let mut out = Vec::with_capacity(pairs.len());
        for pair in pairs {
            meter.check()?;
            let mut overlap = ValueOverlap {
                source: pair.source.clone(),
                target: pair.target.clone(),
                source_distinct: None,
                target_distinct: None,
                matched_distinct: None,
                matched_records: None,
                coverage: None,
                source_uniqueness: None,
                target_uniqueness: None,
                numeric_span: None,
                not_compared: None,
            };
            let (Some(s), Some(t)) = (find(&pair.source), find(&pair.target)) else {
                overlap.not_compared = Some("no such property".into());
                out.push(overlap);
                continue;
            };
            for index in [s, t] {
                if let std::collections::hash_map::Entry::Vacant(entry) = dictionaries.entry(index)
                {
                    entry.insert(self.dictionary(view, &columns[index], meter)?);
                }
            }
            match (&dictionaries[&s], &dictionaries[&t]) {
                (Dictionary::Values(from), Dictionary::Values(to)) => {
                    let (matched, records) = from
                        .counts
                        .iter()
                        .filter(|(key, _)| to.counts.contains_key(*key))
                        .fold((0u64, 0u64), |(n, r), (_, count)| (n + 1, r + count));
                    if !access.counted(records) {
                        overlap.not_compared = Some("withheld under the policy's minCount".into());
                        out.push(overlap);
                        continue;
                    }
                    #[expect(
                        clippy::cast_precision_loss,
                        reason = "counts are far below 2^52; the ratios are scores"
                    )]
                    {
                        let ratio = |n: usize, d: u64| (d > 0).then(|| n as f64 / d as f64);
                        overlap.coverage =
                            (from.len() > 0).then(|| matched as f64 / from.len() as f64);
                        overlap.source_uniqueness = ratio(from.len(), columns[s].records_count);
                        overlap.target_uniqueness = ratio(to.len(), columns[t].records_count);
                    }
                    if let (Some((lo, hi)), Some((target_lo, target_hi))) = (from.span, to.span) {
                        overlap.numeric_span =
                            Some((hi - lo + 1.0) / (target_hi - target_lo + 1.0));
                    }
                    overlap.source_distinct = Some(from.len() as u64);
                    overlap.target_distinct = Some(to.len() as u64);
                    overlap.matched_distinct = Some(matched);
                    overlap.matched_records = Some(records);
                }
                (Dictionary::NotCompared(why), _) | (_, Dictionary::NotCompared(why)) => {
                    overlap.not_compared = Some((*why).to_string());
                }
            }
            out.push(overlap);
        }
        Ok(out)
    }
}

/// The stored logical types a catalog entry's type name covers (`number`
/// covers both integers and floats).
fn logical_types_of(value_type: &str) -> &'static [rushdb_types::model::LogicalType] {
    use rushdb_types::model::LogicalType;
    match value_type {
        "number" => &[LogicalType::Int64, LogicalType::Float64],
        "string" => &[LogicalType::Utf8],
        "datetime" => &[LogicalType::DateTime],
        "boolean" => &[LogicalType::Boolean],
        _ => &[],
    }
}
