//! Protocol queries handlers on an admitted execution context.

use super::{
    execution_failed, query_document_to_parser_input, query_failed, response_array, response_u64,
    values_range, ErrorCode, ProtocolError, ProtocolExecutor, QueryDocument, Response,
};
use rushdb_lmpg::CatalogAccess;
use rushdb_types::value_match::{TextMatch, ValueMatcher};

/// Rank matching values for a value search: descending owner count, ties by
/// the value's JSON text, so results are stable across reads.
fn rank_values(values: &mut [(serde_json::Value, u64)]) {
    values.sort_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| {
            let left = serde_json::to_string(&a.0).unwrap_or_default();
            let right = serde_json::to_string(&b.0).unwrap_or_default();
            left.cmp(&right)
        })
    });
}

/// The engine's matcher for a wire value search.
fn value_matcher(search: &rushdb_protocol::ValueSearch) -> ValueMatcher {
    use rushdb_protocol::ValueMatch;
    let matching = match search.matching {
        ValueMatch::Contains => TextMatch::Contains,
        ValueMatch::Prefix => TextMatch::Prefix,
        ValueMatch::Exact => TextMatch::Exact,
        ValueMatch::Similar => TextMatch::Similar,
    };
    ValueMatcher::new(
        &search.text,
        matching,
        search.case_sensitive,
        search.min_similarity,
    )
}

/// A `similar` search ranks by similarity, then by the count order already
/// given; other searches keep it.
fn rank_by_similarity(
    search: &ValueMatcher,
    ranked: Vec<(serde_json::Value, u64)>,
) -> Vec<(serde_json::Value, u64)> {
    if search.matching() != TextMatch::Similar {
        return ranked;
    }
    let mut scored: Vec<(f64, (serde_json::Value, u64))> = ranked
        .into_iter()
        .map(|entry| (search.similarity(&entry.0), entry))
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored.into_iter().map(|(_, entry)| entry).collect()
}

/// A property-value response over ranked matches (CAT-2 `search`).
#[expect(
    clippy::cast_possible_truncation,
    reason = "limits are bounded request values"
)]
fn property_values_from_ranked(
    property: String,
    mode: rushdb_protocol::PropertyValuesMode,
    limit: Option<u64>,
    ranked: Vec<(serde_json::Value, u64)>,
) -> Response {
    use rushdb_protocol::PropertyValuesMode;
    let distinct_count = Some(ranked.len() as u64);
    let empty = |min, max| Response::PropertyValues {
        property: property.clone(),
        values: Vec::new(),
        distinct_count,
        min,
        max,
        truncated: false,
        counts: Vec::new(),
        search_missed: false,
    };
    match mode {
        PropertyValuesMode::Count => empty(None, None),
        PropertyValuesMode::Range => {
            let values: Vec<serde_json::Value> = ranked.into_iter().map(|(v, _)| v).collect();
            let (min, max) = values_range(&values);
            empty(min, max)
        }
        PropertyValuesMode::TopK => {
            let k = limit.unwrap_or(10).min(1000) as usize;
            let (values, counts) = ranked.into_iter().take(k).unzip();
            Response::PropertyValues {
                property,
                values,
                distinct_count,
                min: None,
                max: None,
                truncated: false,
                counts,
                search_missed: false,
            }
        }
        PropertyValuesMode::Values => {
            let n = limit.map_or(ranked.len(), |n| (n as usize).min(ranked.len()));
            let truncated = n < ranked.len();
            let values = ranked.into_iter().take(n).map(|(v, _)| v).collect();
            Response::PropertyValues {
                property,
                values,
                distinct_count,
                min: None,
                max: None,
                truncated,
                counts: Vec::new(),
                search_missed: false,
            }
        }
    }
}

impl ProtocolExecutor<'_> {
    pub(super) fn query_records_with_context(
        &self,
        query: &QueryDocument,
        is_cancelled: &dyn Fn() -> bool,
        mut execution_context: Option<&mut rushdb_engine::ExecutionContext>,
    ) -> Result<Response, ProtocolError> {
        if query.cursor_ttl_ms == Some(0) {
            return Err(ProtocolError::new(
                ErrorCode::InvalidRequest,
                "cursorTtlMs must be greater than zero",
            ));
        }
        if query.cursor_ttl_ms.is_some() && query.limit.is_none() {
            return Err(ProtocolError::new(
                ErrorCode::InvalidRequest,
                "cursor pagination requires a finite limit",
            ));
        }
        let document = query_document_to_parser_input(query);
        let mut parsed = rushdb_query::document::parse_document(&document)
            .map_err(|error| ProtocolError::new(ErrorCode::InvalidRequest, error.to_string()))?;
        if let Some(context) = execution_context.as_deref_mut() {
            parsed.search = self
                .node
                .authorize_search_in_view(context.read_view().as_ref(), &parsed.search)
                .map_err(|error| ProtocolError::new(ErrorCode::Unauthorized, error))?;
            if let Some(aggregate) = parsed.aggregate.as_mut() {
                *aggregate = self
                    .node
                    .authorize_aggregate_in_view(context.read_view().as_ref(), aggregate)
                    .map_err(|error| ProtocolError::new(ErrorCode::Unauthorized, error))?;
            }
        }
        if let Some(aggregate) = parsed.aggregate {
            if query.cursor_ttl_ms.is_some() {
                return Err(ProtocolError::new(
                    ErrorCode::InvalidRequest,
                    "cursor pagination is available only for records queries",
                ));
            }
            let (buckets, matched) = if let Some(context) = execution_context.as_mut() {
                self.node
                    .aggregate_with_execution_context(context, &aggregate, is_cancelled)
                    .map_err(query_failed)?
            } else {
                rushdb_query::aggregate_with_cancel(&self.node.lmpg(), &aggregate, is_cancelled)
                    .map_err(query_failed)?
            };
            let records = if let Some(context) = execution_context.as_ref() {
                rushdb_query::aggregate_to_rows_in_view_with_budget(
                    &self.node.lmpg(),
                    context.read_view(),
                    &buckets,
                    &aggregate,
                    context.remaining_budget(),
                )
            } else {
                rushdb_query::aggregate_to_rows(&self.node.lmpg(), &buckets, &aggregate)
            }
            .map_err(query_failed)?;
            return Ok(Response::Rows {
                total: matched,
                rows: records,
            });
        }
        let (total, records, cursor_id) = match query.cursor_ttl_ms {
            Some(ttl) => {
                if let Some(context) = execution_context.as_mut() {
                    self.node.start_cursor_with_execution_context(
                        context,
                        &parsed.search,
                        ttl,
                        is_cancelled,
                    )
                } else {
                    self.node.start_cursor(&parsed.search, ttl, is_cancelled)
                }
            }
            None => {
                if let Some(context) = execution_context.as_mut() {
                    self.node
                        .search_with_execution_context(context, &parsed.search, is_cancelled)
                        .map(|(total, records)| (total, records, None))
                } else {
                    self.node
                        .search_with_cancel(&parsed.search, is_cancelled)
                        .map(|(total, records)| (total, records, None))
                }
            }
        }
        .map_err(execution_failed)?;
        Ok(Response::Records {
            records,
            total,
            cursor_id,
            vector_search: None,
            text_search: None,
            fusion: None,
        })
    }

    pub(super) fn query_cursor_with_context(
        &self,
        cursor_id: &str,
        limit: u64,
        is_cancelled: &dyn Fn() -> bool,
        context: &mut rushdb_engine::ExecutionContext,
    ) -> Result<Response, ProtocolError> {
        if limit == 0 || limit > 10_000 {
            return Err(ProtocolError::new(
                ErrorCode::InvalidRequest,
                "cursor page limit must be between 1 and 10000",
            ));
        }
        let (total, records, cursor_id) = self
            .node
            .continue_cursor_with_execution_context(context, cursor_id, limit, is_cancelled)
            .map_err(|error| ProtocolError::new(ErrorCode::NotFound, error))?;
        Ok(Response::Records {
            records,
            total,
            cursor_id,
            vector_search: None,
            text_search: None,
            fusion: None,
        })
    }

    /// Intercept SQL index DDL: `CREATE VECTOR INDEX` / `DROP INDEX`. Returns
    /// `Ok(Some(response))` when the statement was DDL (handled), `Ok(None)`
    /// to fall through to `DataFusion`.
    #[expect(
        clippy::too_many_lines,
        reason = "DDL parsing and one atomic native index operation stay together at the protocol boundary"
    )]
    pub(super) fn sql_index_ddl(
        &self,
        sql: &str,
        execution_context: &mut rushdb_engine::ExecutionContext,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<Response>, ProtocolError> {
        execution_context
            .check_deadline()
            .map_err(|error| execution_failed(error.to_string()))?;
        if is_cancelled() {
            return Err(ProtocolError::new(
                ErrorCode::Cancelled,
                "operation cancelled",
            ));
        }
        let trimmed = sql.trim().trim_end_matches(';').trim();
        let upper = trimmed.to_ascii_uppercase();
        if upper.starts_with("CREATE VECTOR INDEX") {
            // Keep the caller's spelling for the durable identity while using
            // the uppercase copy only for case-insensitive grammar matching.
            let rest = &trimmed["CREATE VECTOR INDEX".len()..];
            let name = rest
                .split_whitespace()
                .next()
                .ok_or_else(|| {
                    ProtocolError::new(
                        ErrorCode::InvalidRequest,
                        "CREATE VECTOR INDEX requires a name",
                    )
                })?
                .to_string();
            // ... ON <label>(<property>) [SIMILARITY cosine|dot] [DIMENSIONS n]
            let on_idx = upper.find(" ON ").ok_or_else(|| {
                ProtocolError::new(
                    ErrorCode::InvalidRequest,
                    "CREATE VECTOR INDEX requires `ON <label>(<property>)`",
                )
            })?;
            let on_clause = trimmed[on_idx + 4..].trim();
            let paren = on_clause.find('(').ok_or_else(|| {
                ProtocolError::new(
                    ErrorCode::InvalidRequest,
                    "CREATE VECTOR INDEX requires `ON <label>(<property>)`",
                )
            })?;
            let label = on_clause[..paren].trim().to_string();
            let prop_end = on_clause.find(')').ok_or_else(|| {
                ProtocolError::new(ErrorCode::InvalidRequest, "missing `)` in property")
            })?;
            let property = on_clause[paren + 1..prop_end].trim().to_string();
            let similarity = if let Some(s) = upper.rfind(" SIMILARITY ") {
                trimmed[s + " SIMILARITY ".len()..]
                    .split_whitespace()
                    .next()
                    .unwrap_or("cosine")
                    .to_ascii_lowercase()
            } else {
                "cosine".to_string()
            };
            let dims = if let Some(d) = upper.rfind(" DIMENSIONS ") {
                trimmed[d + " DIMENSIONS ".len()..]
                    .split_whitespace()
                    .next()
                    .and_then(|n| n.parse::<u32>().ok())
                    .ok_or_else(|| {
                        ProtocolError::new(
                            ErrorCode::InvalidRequest,
                            "DIMENSIONS must be an integer",
                        )
                    })?
            } else {
                return Err(ProtocolError::new(
                    ErrorCode::InvalidRequest,
                    "CREATE VECTOR INDEX requires `DIMENSIONS n`",
                ));
            };
            let index = self
                .node
                .create_vector_index_with_execution_context(
                    execution_context,
                    &rushdb_protocol::VectorIndexDefinition {
                        label,
                        property,
                        similarity,
                        dimensions: dims,
                        name: Some(name),
                        model: None,
                        owner: None,
                    },
                )
                .map_err(execution_failed)?;
            execution_context
                .check_deadline()
                .map_err(|error| execution_failed(error.to_string()))?;
            if is_cancelled() {
                return Err(ProtocolError::new(
                    ErrorCode::Cancelled,
                    "operation cancelled",
                ));
            }
            return Ok(Some(Response::StatementMutation {
                total: 0,
                rows: vec![],
                affected: 1,
                commit_index: self.node.lmpg().store.committed_version(),
                status: index.get("name").and_then(|v| v.as_str()).map(String::from),
            }));
        }
        if upper.starts_with("CREATE INDEX") {
            // CREATE INDEX <name> ON <label>(<property>): a declared ordered
            // index.
            let malformed = || {
                ProtocolError::new(
                    ErrorCode::InvalidRequest,
                    "expected CREATE INDEX <name> ON <label>(<property>) [USING TEXT [IMMEDIATE]]",
                )
            };
            let name = trimmed
                .split_whitespace()
                .nth(2)
                .filter(|name| !name.eq_ignore_ascii_case("ON"))
                .ok_or_else(malformed)?;
            let on_idx = upper.find(" ON ").ok_or_else(malformed)?;
            let on_clause = trimmed[on_idx + 4..].trim();
            let paren = on_clause.find('(').ok_or_else(malformed)?;
            let close = on_clause.rfind(')').ok_or_else(malformed)?;
            let label = on_clause[..paren].trim().trim_matches('"');
            let property = on_clause[paren + 1..close].trim().trim_matches('"');
            if label.is_empty() || property.is_empty() || property.contains(',') {
                return Err(malformed());
            }
            // `… USING TEXT [IMMEDIATE]` declares a full-text index.
            let tail = on_clause[close + 1..].trim().to_ascii_uppercase();
            let kind = match tail.as_str() {
                "" => rushdb_language::ddl::DdlKind::Ordered,
                "USING TEXT" => rushdb_language::ddl::DdlKind::Fulltext { immediate: false },
                "USING TEXT IMMEDIATE" => {
                    rushdb_language::ddl::DdlKind::Fulltext { immediate: true }
                }
                _ => return Err(malformed()),
            };
            return self
                .create_declared_index(name, label, property, kind)
                .map(Some);
        }
        if upper.starts_with("DROP INDEX") {
            let name = trimmed
                .split_whitespace()
                .nth(2)
                .ok_or_else(|| {
                    ProtocolError::new(ErrorCode::InvalidRequest, "DROP INDEX requires a name")
                })?
                .to_string();
            self.drop_any_index(&name, execution_context)?;
            execution_context
                .check_deadline()
                .map_err(|error| execution_failed(error.to_string()))?;
            if is_cancelled() {
                return Err(ProtocolError::new(
                    ErrorCode::Cancelled,
                    "operation cancelled",
                ));
            }
            return Ok(Some(Response::StatementMutation {
                total: 0,
                rows: vec![],
                affected: 1,
                commit_index: self.node.lmpg().store.committed_version(),
                status: None,
            }));
        }
        Ok(None)
    }

    pub(super) fn query_relationships(
        &self,
        query: &rushdb_protocol::RelationshipQuery,
        context: &mut rushdb_engine::ExecutionContext,
    ) -> Result<Response, ProtocolError> {
        let result = self
            .node
            .scan_relationships_with_execution_context(context, query)
            .map_err(execution_failed)?;
        let relationships = response_array(&result, "data")?;
        // `total` is the filtered relationship count before pagination; the
        // page itself is `data`.
        let total = response_u64(&result, "total")?;
        Ok(Response::Relationships {
            relationships,
            total,
        })
    }

    #[expect(
        clippy::too_many_lines,
        clippy::too_many_arguments,
        clippy::cast_possible_truncation,
        reason = "single linear operation/reader; splitting it would scatter shared borrows and hurt auditability"
    )]
    pub(super) fn query_property_values(
        &self,
        property: &str,
        query: &QueryDocument,
        mode: Option<rushdb_protocol::PropertyValuesMode>,
        limit: Option<u64>,
        search: Option<&rushdb_protocol::ValueSearch>,
        is_cancelled: &dyn Fn() -> bool,
        context: &mut rushdb_engine::ExecutionContext,
        access: &CatalogAccess,
    ) -> Result<Response, ProtocolError> {
        use rushdb_protocol::PropertyValuesMode;
        let mode = mode.unwrap_or(PropertyValuesMode::Values);
        // A search that matches nothing may answer with the property's whole
        // domain instead, when it is small (`domainOnMiss`).
        if let Some((search, most)) = search.and_then(|search| {
            search.domain_on_miss.map(|most| {
                let mut plain = search.clone();
                plain.domain_on_miss = None;
                (plain, most.min(1000))
            })
        }) {
            let searched = self.query_property_values(
                property,
                query,
                Some(mode),
                limit,
                Some(&search),
                is_cancelled,
                context,
                access,
            )?;
            if !matches!(
                searched,
                Response::PropertyValues {
                    distinct_count: Some(0),
                    ..
                }
            ) {
                return Ok(searched);
            }
            let domain = self.query_property_values(
                property,
                query,
                Some(PropertyValuesMode::TopK),
                Some(most),
                None,
                is_cancelled,
                context,
                access,
            )?;
            return Ok(match domain {
                Response::PropertyValues {
                    property,
                    values,
                    distinct_count: Some(total),
                    counts,
                    ..
                } if total > 0 && total <= most => Response::PropertyValues {
                    property,
                    values,
                    distinct_count: Some(total),
                    min: None,
                    max: None,
                    truncated: false,
                    counts,
                    search_missed: true,
                },
                _ => searched,
            });
        }
        let property = self
            .node
            .resolve_property_name_in_view(context.read_view().as_ref(), property)
            .map_err(execution_failed)?;
        let matcher = search.map(value_matcher);
        let mut meter = self.node.working_set(is_cancelled, context.deadline());
        // The maintained-posting fast path below answers over *all* labels and
        // cannot subtract a data-access policy. When a policy is enforced, force
        // the match-set count below, which intersects the policy before computing
        // values. This keeps counts/range/top-k correct under `PolicyEnforced`
        // instead of leaking denied labels.
        // No predicate: the per-label dictionaries are the facet, under the
        // requested labels (all labels when none are given).
        let unfiltered = !access.enforced()
            && query
                .predicate
                .as_object()
                .is_some_and(serde_json::Map::is_empty);
        let unscoped = unfiltered && query.labels.is_empty();
        // A value search over an unfiltered match set is answered from the
        // value dictionaries: matching values with exact owner counts,
        // without reading records.
        if let (Some(search), true) = (matcher.as_ref(), unfiltered) {
            if let Some(ranked) = self
                .node
                .value_dictionary_in_view(
                    context.read_view().as_ref(),
                    &property,
                    &query.labels,
                    Some(search),
                    &mut meter,
                )
                .map_err(execution_failed)?
            {
                return Ok(property_values_from_ranked(
                    property,
                    mode,
                    limit,
                    rank_by_similarity(search, ranked),
                ));
            }
        }
        // Unscoped Count is O(1) from the maintained exact-distinct counter
        //; the posting-scan domain below is the fallback for arrays,
        // missing counters, or Range/TopK (which need values/min/max).
        if unscoped && search.is_none() && mode == PropertyValuesMode::Count {
            if let Some(count) = self
                .node
                .distinct_count_from_index_in_view(context.read_view().as_ref(), &property)
                .map_err(execution_failed)?
            {
                return Ok(Response::PropertyValues {
                    property,
                    values: Vec::new(),
                    distinct_count: Some(count),
                    min: None,
                    max: None,
                    truncated: false,
                    counts: Vec::new(),
                    search_missed: false,
                });
            }
        }
        // Unfiltered Count/Range/TopK reads are answered from the maintained
        // dictionaries (`O(distinct)`), with the match-set count below for
        // filtered queries, array-valued properties, or non-numeric ranges.
        let indexed = if unfiltered && search.is_none() {
            self.node
                .value_domain_from_index_in_view(
                    context.read_view().as_ref(),
                    &property,
                    &query.labels,
                    &mut meter,
                )
                .map_err(execution_failed)?
        } else {
            None
        };
        // Unfiltered Values, and Range over non-numeric values, rank or bound
        // the dictionary's (value, owner count) pairs below, the pairs the
        // match-set scan would collect, instead of reading every record.
        // Explicit nulls are not dictionary entries: those properties scan.
        let from_dictionary =
            matches!(mode, PropertyValuesMode::Values | PropertyValuesMode::Range)
                && indexed.is_some()
                && !self
                    .node
                    .explicit_nulls_in_view(context.read_view().as_ref(), &property, &query.labels)
                    .map_err(execution_failed)?;
        if let Some(domain) = indexed {
            match mode {
                PropertyValuesMode::Count => {
                    return Ok(Response::PropertyValues {
                        property,
                        values: Vec::new(),
                        distinct_count: Some(domain.distinct_total()),
                        min: None,
                        max: None,
                        truncated: false,
                        counts: Vec::new(),
                        search_missed: false,
                    });
                }
                PropertyValuesMode::Range if domain.numeric_only => {
                    return Ok(Response::PropertyValues {
                        property,
                        values: Vec::new(),
                        distinct_count: Some(domain.distinct_total()),
                        min: domain.min,
                        max: domain.max,
                        truncated: false,
                        counts: Vec::new(),
                        search_missed: false,
                    });
                }
                PropertyValuesMode::TopK => {
                    let k = limit.unwrap_or(10).min(1000) as usize;
                    if let Some(tops) = self
                        .node
                        .value_domain_top_k_in_view(
                            context.read_view().as_ref(),
                            &property,
                            k,
                            &query.labels,
                            &mut meter,
                        )
                        .map_err(execution_failed)?
                    {
                        let (values, counts) = tops.into_iter().unzip();
                        return Ok(Response::PropertyValues {
                            property,
                            values,
                            distinct_count: Some(domain.distinct_total()),
                            min: None,
                            max: None,
                            truncated: false,
                            counts,
                            search_missed: false,
                        });
                    }
                }
                // A limited list keeps only its head while the dictionary
                // streams; the count and `truncated` come from the domain.
                PropertyValuesMode::Values if from_dictionary => {
                    if let Some(n) = limit {
                        let n = usize::try_from(n).unwrap_or(usize::MAX);
                        if let Some(head) = self
                            .node
                            .value_domain_top_k_in_view(
                                context.read_view().as_ref(),
                                &property,
                                n,
                                &query.labels,
                                &mut meter,
                            )
                            .map_err(execution_failed)?
                        {
                            let total = domain.distinct_total();
                            return Ok(Response::PropertyValues {
                                property,
                                values: head.into_iter().map(|(value, _)| value).collect(),
                                distinct_count: Some(total),
                                min: None,
                                max: None,
                                truncated: u64::try_from(n).unwrap_or(u64::MAX) < total,
                                counts: Vec::new(),
                                search_missed: false,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        let dictionary = if from_dictionary {
            self.node
                .value_dictionary_in_view(
                    context.read_view().as_ref(),
                    &property,
                    &query.labels,
                    None,
                    &mut meter,
                )
                .map_err(execution_failed)?
        } else {
            None
        };
        let mut seen = if let Some(pairs) = dictionary {
            pairs
        } else {
            // Scoped (or policy-enforced) reads count over the match set from
            // occurrence rows: first-seen order, one count per distinct value.
            let ords = self
                .node
                .facet_ordinals(
                    context.read_view(),
                    &Self::facet_search(query)?,
                    is_cancelled,
                )
                .map_err(execution_failed)?;
            // Counts and top-k need each distinct value's count, not the value:
            // they count by digest and read back only the values returned, so
            // a column of long texts fits the working set.
            if matcher.is_none()
                && matches!(mode, PropertyValuesMode::Count | PropertyValuesMode::TopK)
            {
                let view = context.read_view().as_ref();
                let mut counts = self
                    .node
                    .facet_value_digests(view, &ords, &property, access, &mut meter)
                    .map_err(execution_failed)?;
                counts.retain(|(_, count, _)| access.counted(*count));
                let distinct_count = Some(counts.len() as u64);
                let (values, counts) = if mode == PropertyValuesMode::TopK {
                    let k = limit.unwrap_or(10).min(1000) as usize;
                    // Stable: equal counts keep first-seen order.
                    counts.sort_by_key(|entry| std::cmp::Reverse(entry.1));
                    counts.truncate(k);
                    let owners: Vec<u64> = counts.iter().map(|(_, _, ord)| *ord).collect();
                    let values = self
                        .node
                        .facet_values_of(view, &owners, &property, access, &mut meter)
                        .map_err(execution_failed)?;
                    (
                        values,
                        counts.into_iter().map(|(_, count, _)| count).collect(),
                    )
                } else {
                    (Vec::new(), Vec::new())
                };
                return Ok(Response::PropertyValues {
                    property,
                    values,
                    distinct_count,
                    min: None,
                    max: None,
                    truncated: false,
                    counts,
                    search_missed: false,
                });
            }
            self.node
                .facet_property_values(
                    context.read_view().as_ref(),
                    &ords,
                    &property,
                    matcher.as_ref(),
                    access,
                    &mut meter,
                )
                .map_err(execution_failed)?
        };
        // An enforced policy reads through this match-set path; under its
        // small-count threshold, values held by fewer records are withheld
        //, so counts, ranges and lists cover the reported values only.
        seen.retain(|(_, count)| access.counted(*count));
        // The scan kept only matching values.
        if let Some(search) = matcher.as_ref() {
            let mut ranked: Vec<(serde_json::Value, u64)> = seen;
            rank_values(&mut ranked);
            return Ok(property_values_from_ranked(
                property,
                mode,
                limit,
                rank_by_similarity(search, ranked),
            ));
        }
        let distinct_count = seen.len() as u64;
        match mode {
            PropertyValuesMode::Count => Ok(Response::PropertyValues {
                property,
                values: Vec::new(),
                distinct_count: Some(distinct_count),
                min: None,
                max: None,
                truncated: false,
                counts: Vec::new(),
                search_missed: false,
            }),
            PropertyValuesMode::Range => {
                let values: Vec<serde_json::Value> =
                    seen.into_iter().map(|(value, _)| value).collect();
                let (min, max) = values_range(&values);
                Ok(Response::PropertyValues {
                    property,
                    values: Vec::new(),
                    distinct_count: Some(distinct_count),
                    min,
                    max,
                    truncated: false,
                    counts: Vec::new(),
                    search_missed: false,
                })
            }
            PropertyValuesMode::TopK => {
                let k = limit.unwrap_or(10).min(1000) as usize;
                // Stable: equal counts keep first-seen order.
                let mut pairs = seen;
                pairs.sort_by_key(|entry| std::cmp::Reverse(entry.1));
                let (values, counts) = pairs.into_iter().take(k).unzip();
                Ok(Response::PropertyValues {
                    property,
                    values,
                    distinct_count: Some(distinct_count),
                    min: None,
                    max: None,
                    truncated: false,
                    counts,
                    search_missed: false,
                })
            }
            PropertyValuesMode::Values => {
                // By descending record count, ties by value, as a search lists
                // them: scan order is not an order, and `limit` keeps the head.
                let mut seen = seen;
                rank_values(&mut seen);
                let mut values: Vec<serde_json::Value> =
                    seen.into_iter().map(|(value, _)| value).collect();
                let (values, truncated) = match limit {
                    Some(n) => {
                        let n = (n as usize).min(values.len());
                        let truncated = n < values.len();
                        values.truncate(n);
                        (values, truncated)
                    }
                    None => (values, false),
                };
                Ok(Response::PropertyValues {
                    property,
                    values,
                    distinct_count: Some(distinct_count),
                    min: None,
                    max: None,
                    truncated,
                    counts: Vec::new(),
                    search_missed: false,
                })
            }
        }
    }

    /// Value search across properties (`query.searchValues`): every
    /// (label, property) in scope that `access` lists is searched with one
    /// matcher and one working-set meter. Dictionaries answer what they
    /// can, read straight (exact per label, so also under a policy); a
    /// label's long or array values are scanned once over its records, for
    /// all of its such properties. A property whose read fails (e.g. over
    /// the working-set limit) is reported in `skipped` instead of failing
    /// the request; cancellation and the deadline fail it.
    #[expect(
        clippy::too_many_lines,
        reason = "one pass over the catalog's columns; splitting it would scatter the meter and the ranking it shares"
    )]
    pub(super) fn query_search_values(
        &self,
        search: &rushdb_protocol::ValueSearch,
        labels: &[String],
        limit: Option<u64>,
        is_cancelled: &dyn Fn() -> bool,
        context: &mut rushdb_engine::ExecutionContext,
        access: &CatalogAccess,
    ) -> Result<Response, ProtocolError> {
        let wanted = value_matcher(search);
        let limit = usize::try_from(limit.unwrap_or(50).min(1000)).unwrap_or(1000);
        let view = std::sync::Arc::clone(context.read_view());
        let mut meter = self.node.working_set(is_cancelled, context.deadline());
        let mut by_label: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for column in self
            .node
            .catalog_columns(&view, labels, access)
            .map_err(execution_failed)?
        {
            if column.value_type == "null" {
                continue;
            }
            if let Some(label) = column.label {
                by_label.entry(label).or_default().push(column.name);
            }
        }
        let mut matches: Vec<(f64, rushdb_protocol::ValueMatchRow)> = Vec::new();
        let mut skipped = Vec::new();
        let mut total = 0u64;
        // A read that stops for cancellation or the deadline fails the
        // request; any other failure skips the property.
        let stopped = |error: &str| error.contains("cancelled") || error.contains("deadline");
        // Keeps a property's best `limit` matches (by count, or similarity
        // for `similar`), counted toward `total`, past the policy's minCount.
        let mut keep =
            |label: &str,
             property: &str,
             ranked: Vec<(serde_json::Value, u64)>,
             matches: &mut Vec<(f64, rushdb_protocol::ValueMatchRow)>| {
                let ranked: Vec<_> = ranked
                    .into_iter()
                    .filter(|(_, count)| access.counted(*count))
                    .collect();
                total += ranked.len() as u64;
                for (value, count) in rank_by_similarity(&wanted, ranked).into_iter().take(limit) {
                    matches.push((
                        wanted.similarity(&value),
                        rushdb_protocol::ValueMatchRow {
                            label: label.to_string(),
                            property: property.to_string(),
                            value,
                            count,
                        },
                    ));
                }
            };
        for (label, properties) in by_label {
            let scope = [label.clone()];
            let mut scanned = Vec::new();
            for property in properties {
                meter.check().map_err(execution_failed)?;
                let before = meter.charged();
                let read = self.node.value_dictionary_in_view(
                    view.as_ref(),
                    &property,
                    &scope,
                    Some(&wanted),
                    &mut meter,
                );
                // What is kept is small (at most `limit` per property); the
                // dictionary's working set ends here.
                meter.release(meter.charged() - before);
                match read {
                    Ok(Some(ranked)) => keep(&label, &property, ranked, &mut matches),
                    Ok(None) => scanned.push(property),
                    Err(error) if stopped(&error) => return Err(execution_failed(error)),
                    Err(_) => skipped.push(format!("{label}.{property}")),
                }
            }
            if scanned.is_empty() {
                continue;
            }
            let document = rushdb_protocol::QueryDocument {
                labels: scope.to_vec(),
                ..rushdb_protocol::QueryDocument::default()
            };
            let ords = self
                .node
                .facet_ordinals(&view, &Self::facet_search(&document)?, is_cancelled)
                .map_err(execution_failed)?;
            for property in scanned {
                let before = meter.charged();
                let read = self.node.facet_property_values(
                    view.as_ref(),
                    &ords,
                    &property,
                    Some(&wanted),
                    access,
                    &mut meter,
                );
                meter.release(meter.charged() - before);
                match read {
                    Ok(mut seen) => {
                        rank_values(&mut seen);
                        keep(&label, &property, seen, &mut matches);
                    }
                    Err(error) if stopped(&error) => return Err(execution_failed(error)),
                    Err(_) => skipped.push(format!("{label}.{property}")),
                }
            }
        }
        let similar = wanted.matching() == TextMatch::Similar;
        matches.sort_by(|(a_score, a), (b_score, b)| {
            let by_score = if similar {
                b_score.total_cmp(a_score)
            } else {
                std::cmp::Ordering::Equal
            };
            by_score
                .then_with(|| b.count.cmp(&a.count))
                .then_with(|| a.label.cmp(&b.label))
                .then_with(|| a.property.cmp(&b.property))
                .then_with(|| a.value.to_string().cmp(&b.value.to_string()))
        });
        matches.truncate(limit);
        Ok(Response::ValueMatches {
            matches: matches.into_iter().map(|(_, row)| row).collect(),
            total,
            skipped,
        })
    }

    pub(super) fn query_vector_with_context(
        &self,
        execution_context: &mut rushdb_engine::ExecutionContext,
        query: &rushdb_protocol::VectorQuery,
    ) -> Result<Response, ProtocolError> {
        let result = self
            .node
            .search_vectors_with_execution_context(execution_context, query)
            .map_err(|error| {
                if error == "vector query denied by policy" {
                    ProtocolError::new(ErrorCode::Unauthorized, error)
                } else {
                    execution_failed(error)
                }
            })?;
        let records = response_array(&result, "data")?;
        let total = records.len() as u64;
        Ok(Response::Records {
            records,
            total,
            cursor_id: None,
            vector_search: report(&result, "vectorSearch")?,
            text_search: report(&result, "textSearch")?,
            fusion: report(&result, "fusion")?,
        })
    }

    pub(super) fn query_vector_index_candidates_with_context(
        &self,
        execution_context: &mut rushdb_engine::ExecutionContext,
        query: &rushdb_protocol::VectorIndexCandidatesQuery,
    ) -> Result<Response, ProtocolError> {
        let (records, total) = self
            .node
            .vector_index_candidates_with_execution_context(execution_context, query)
            .map_err(execution_failed)?;
        Ok(Response::VectorIndexCandidates { records, total })
    }

    pub(super) fn query_text_with_context(
        &self,
        index: &str,
        query: &str,
        limit: u64,
        opts: rushdb_textsearch::LexicalOptions,
        is_cancelled: &dyn Fn() -> bool,
        execution_context: &rushdb_engine::ExecutionContext,
    ) -> Result<Response, ProtocolError> {
        if is_cancelled() {
            return Err(ProtocolError::new(
                ErrorCode::Cancelled,
                "operation cancelled",
            ));
        }
        let (rows, pending) = self
            .node
            .text_search_in_view(
                index,
                query,
                opts,
                usize::try_from(limit).unwrap_or(10),
                execution_context.read_view().as_ref(),
            )
            .map_err(execution_failed)?;
        let total = rows.len() as u64;
        Ok(Response::TextHits {
            rows,
            total,
            pending,
        })
    }

    /// Combined graph + vector + lexical query (document 26's hybrid
    /// pattern); see `Node::hybrid_search_in_view` for the fusion contract.
    pub(super) fn query_hybrid_with_context(
        &self,
        execution_context: &mut rushdb_engine::ExecutionContext,
        query: &rushdb_protocol::HybridQuery,
    ) -> Result<Response, ProtocolError> {
        let result = self
            .node
            .hybrid_search_with_execution_context(execution_context, query)
            .map_err(|error| {
                if error == "hybrid query denied by policy" {
                    ProtocolError::new(ErrorCode::Unauthorized, error)
                } else {
                    execution_failed(error)
                }
            })?;
        let records = response_array(&result, "data")?;
        let total = records.len() as u64;
        Ok(Response::Records {
            records,
            total,
            cursor_id: None,
            vector_search: report(&result, "vectorSearch")?,
            text_search: report(&result, "textSearch")?,
            fusion: report(&result, "fusion")?,
        })
    }
}

/// A search report (`vectorSearch`, `textSearch`) returned next to the rows.
fn report<T: serde::de::DeserializeOwned>(
    result: &serde_json::Value,
    name: &str,
) -> Result<Option<T>, ProtocolError> {
    result
        .get(name)
        .map(|report| serde_json::from_value(report.clone()))
        .transpose()
        .map_err(|error| execution_failed(format!("engine {name} report: {error}")))
}
