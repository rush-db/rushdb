//! Protocol introspection handlers on an admitted execution context.

use super::{
    engine_capabilities, execution_failed, query_document_to_parser_input,
    query_language_capabilities, response_array, Describe, ErrorCode, LabelDescription,
    ProtocolError, ProtocolExecutor, Response, SchemaDescription,
};

impl ProtocolExecutor<'_> {
    /// The facet match set of a query document: its labels and predicate,
    /// without projection, grouping, ordering or pagination.
    pub(super) fn facet_search(
        query: &rushdb_protocol::QueryDocument,
    ) -> Result<rushdb_query::SearchRequest, ProtocolError> {
        let document = serde_json::json!({"labels": query.labels, "where": query.predicate});
        rushdb_query::document::parse_document(&document)
            .map(|parsed| parsed.search)
            .map_err(|error| ProtocolError::new(ErrorCode::InvalidRequest, error.to_string()))
    }

    /// Match-set ordinals of a filtered query, or `None` when it has no
    /// predicate (the label catalogs answer it).
    fn facet_matches(
        &self,
        query: &rushdb_protocol::QueryDocument,
        is_cancelled: &dyn Fn() -> bool,
        execution_context: &rushdb_engine::ExecutionContext,
    ) -> Result<Option<Vec<u64>>, ProtocolError> {
        if query
            .predicate
            .as_object()
            .is_some_and(serde_json::Map::is_empty)
        {
            return Ok(None);
        }
        self.node
            .facet_ordinals(
                execution_context.read_view(),
                &Self::facet_search(query)?,
                is_cancelled,
            )
            .map(Some)
            .map_err(execution_failed)
    }

    /// Record count per label over a query's match set: from the label
    /// counters when the query names labels only, else from `matched`.
    fn facet_label_counts(
        &self,
        query: &rushdb_protocol::QueryDocument,
        matched: Option<&[u64]>,
        is_cancelled: &dyn Fn() -> bool,
        execution_context: &rushdb_engine::ExecutionContext,
    ) -> Result<std::collections::BTreeMap<String, u64>, ProtocolError> {
        let view = execution_context.read_view();
        let Some(ords) = matched else {
            return Ok(self
                .node
                .label_counts_in_view(view)
                .map_err(execution_failed)?
                .into_iter()
                .filter(|(label, count)| *count > 0 && query.labels.contains(label))
                .collect());
        };
        self.node
            .facet_labels(view.as_ref(), ords, is_cancelled)
            .map_err(execution_failed)
    }

    /// [`Self::describe_with_access`] under the tenant's policy, read from
    /// the request's pinned view (the dispatcher passes the access it
    /// already built).
    #[cfg(test)]
    pub(super) fn describe(
        &self,
        describe: Describe,
        is_cancelled: &dyn Fn() -> bool,
        execution_context: &mut rushdb_engine::ExecutionContext,
    ) -> Result<Response, ProtocolError> {
        let active = rushdb_authz::load_active_policy_in_view(
            execution_context.read_view().as_ref(),
            execution_context.tenant(),
        )
        .map_err(|error| execution_failed(error.to_string()))?;
        let access = rushdb_lmpg::CatalogAccess::new(active.map(std::sync::Arc::new));
        self.describe_with_access(describe, &access, is_cancelled, execution_context)
    }

    /// A catalog read under `access`: over an enforced policy's allowed
    /// labels, visible properties and allowed relationship types only
    ///, so a hidden one is never named or counted.
    pub(super) fn describe_with_access(
        &self,
        describe: Describe,
        access: &rushdb_lmpg::CatalogAccess,
        is_cancelled: &dyn Fn() -> bool,
        execution_context: &mut rushdb_engine::ExecutionContext,
    ) -> Result<Response, ProtocolError> {
        execution_context
            .check_deadline()
            .map_err(|error| execution_failed(error.to_string()))?;
        // Narrow label-scoped reads to the allowed labels; when none is
        // left, the answer is empty without reading anything.
        let describe = match describe {
            Describe::Schema {
                query,
                detail,
                skip,
                limit,
            } => match self.visible_query(query, access, execution_context)? {
                Some(query) => Describe::Schema {
                    query,
                    detail,
                    skip,
                    limit,
                },
                None => {
                    return Ok(Response::Schema {
                        schema: SchemaDescription {
                            labels: Vec::new(),
                            properties: Vec::new(),
                            relationships: Vec::new(),
                        },
                    })
                }
            },
            Describe::Labels { query } => {
                match self.visible_query(query, access, execution_context)? {
                    Some(query) => Describe::Labels { query },
                    None => return Ok(Response::Labels { labels: Vec::new() }),
                }
            }
            Describe::Properties {
                query,
                detail,
                skip,
                limit,
            } => match self.visible_query(query, access, execution_context)? {
                Some(query) => Describe::Properties {
                    query,
                    detail,
                    skip,
                    limit,
                },
                None => {
                    return Ok(Response::Properties {
                        properties: Vec::new(),
                    })
                }
            },
            other => other,
        };
        let response = clip_introspection(
            self.describe_visible(describe, access, is_cancelled, execution_context)?,
            self.node.introspection_value_max_chars(),
        );
        if !access.enforced() {
            return Ok(response);
        }
        let label_counts: std::collections::BTreeMap<String, u64> = self
            .node
            .label_counts_in_view(execution_context.read_view())
            .map_err(execution_failed)?
            .into_iter()
            .collect();
        Ok(Self::mask_catalog(response, access, &label_counts))
    }

    /// A catalog query narrowed to `access`'s allowed labels: the requested
    /// ones that are allowed, or every allowed one. `None` when no label is
    /// left to read.
    fn visible_query(
        &self,
        mut query: rushdb_protocol::QueryDocument,
        access: &rushdb_lmpg::CatalogAccess,
        execution_context: &rushdb_engine::ExecutionContext,
    ) -> Result<Option<rushdb_protocol::QueryDocument>, ProtocolError> {
        if !access.enforced() {
            return Ok(Some(query));
        }
        if query.labels.is_empty() {
            query.labels = self
                .node
                .labels_in_view(execution_context.read_view())
                .map_err(execution_failed)?;
        }
        query.labels.retain(|label| access.label(label));
        Ok((!query.labels.is_empty()).then_some(query))
    }

    /// Drops what `access` hides from a catalog response: labels outside
    /// its allow-list, masked properties, denied relationship types,
    /// relationship shapes touching a hidden label, and, under a small-count
    /// threshold, groups over fewer records and the samples and extremes
    /// that are often one record's value.
    fn mask_catalog(
        response: Response,
        access: &rushdb_lmpg::CatalogAccess,
        label_counts: &std::collections::BTreeMap<String, u64>,
    ) -> Response {
        let label_visible =
            |label: &str| access.label_listed(label, label_counts.get(label).copied().unwrap_or(0));
        let property_visible = |property: &rushdb_protocol::PropertyDescription| {
            property.label.as_deref().is_some_and(|label| {
                access.column_listed(label, &property.name, property.records_count)
            })
        };
        let shape_visible = |shape: &rushdb_protocol::RelationshipDescription| {
            access.shape_listed(
                &shape.relationship_type,
                &shape.source_label,
                &shape.target_label,
                shape.count,
            )
        };
        let withhold_extremes = |properties: &mut Vec<rushdb_protocol::PropertyDescription>| {
            if !access.shows_extremes() {
                for property in properties {
                    property.sample_values.clear();
                    property.min = None;
                    property.max = None;
                }
            }
        };
        match response {
            Response::Schema { mut schema } => {
                schema.labels.retain(|label| label_visible(&label.name));
                schema.properties.retain(property_visible);
                withhold_extremes(&mut schema.properties);
                schema.relationships.retain(shape_visible);
                Response::Schema { schema }
            }
            Response::Labels { mut labels } => {
                labels.retain(|label| label_visible(label));
                Response::Labels { labels }
            }
            Response::Properties { mut properties } => {
                properties.retain(property_visible);
                withhold_extremes(&mut properties);
                Response::Properties { properties }
            }
            other => other,
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "single linear operation/reader; splitting it would scatter shared borrows and hurt auditability"
    )]
    fn describe_visible(
        &self,
        describe: Describe,
        access: &rushdb_lmpg::CatalogAccess,
        is_cancelled: &dyn Fn() -> bool,
        execution_context: &mut rushdb_engine::ExecutionContext,
    ) -> Result<Response, ProtocolError> {
        match describe {
            Describe::Schema {
                query,
                detail,
                skip,
                limit,
            } => {
                let mut property_document = query_document_to_parser_input(&query);
                if detail.is_some_and(|d| d == rushdb_protocol::PropertyDetail::Catalog) {
                    // As `describe.properties` answers: a request error, not
                    // an internal one from the catalog read below.
                    if query
                        .predicate
                        .as_object()
                        .is_some_and(|predicate| !predicate.is_empty())
                    {
                        return Err(ProtocolError::new(
                            ErrorCode::InvalidRequest,
                            "catalog detail does not support a predicate; use full detail or a label filter",
                        ));
                    }
                    property_document["detail"] = serde_json::Value::String("catalog".into());
                }
                if let Some(skip) = skip {
                    property_document["entry_skip"] = serde_json::Value::from(skip);
                }
                if let Some(limit) = limit {
                    property_document["entry_limit"] = serde_json::Value::from(limit);
                }
                let unscoped = query.labels.is_empty()
                    && query
                        .predicate
                        .as_object()
                        .is_some_and(serde_json::Map::is_empty);
                // One match set feeds both the label and property facets.
                let matched = self.facet_matches(&query, is_cancelled, execution_context)?;
                let labels: Vec<LabelDescription> = if unscoped {
                    self.node
                        .label_counts_in_view(execution_context.read_view())
                        .map_err(execution_failed)?
                        .into_iter()
                        .map(|(name, records_count)| LabelDescription {
                            name,
                            records_count,
                        })
                        .collect()
                } else {
                    self.facet_label_counts(
                        &query,
                        matched.as_deref(),
                        is_cancelled,
                        execution_context,
                    )?
                    .into_iter()
                    .map(|(name, records_count)| LabelDescription {
                        name,
                        records_count,
                    })
                    .collect()
                };
                let properties = self
                    .node
                    .describe_properties_in_view(
                        execution_context.read_view(),
                        &property_document,
                        matched.as_deref(),
                        is_cancelled,
                    )
                    .map_err(execution_failed)?;
                // Relationships narrow like every other facet: over the match
                // set when filtered; from the catalog otherwise, keeping the
                // shapes that touch a requested label (each edge is in
                // exactly one shape, so the counts stay exact).
                let relationships = match matched.as_deref() {
                    Some(ords) => self.node.facet_relationships(
                        execution_context.read_view().as_ref(),
                        ords,
                        is_cancelled,
                    ),
                    None => self
                        .node
                        .describe_relationships_in_view(execution_context.read_view())
                        .map(|mut shapes| {
                            if !query.labels.is_empty() {
                                shapes.retain(|shape| {
                                    query.labels.contains(&shape.source_label)
                                        || query.labels.contains(&shape.target_label)
                                });
                            }
                            shapes
                        }),
                }
                .map_err(execution_failed)?;
                Ok(Response::Schema {
                    schema: SchemaDescription {
                        labels,
                        properties,
                        relationships,
                    },
                })
            }
            Describe::Labels { query } => {
                if query.labels.is_empty()
                    && query
                        .predicate
                        .as_object()
                        .is_some_and(serde_json::Map::is_empty)
                {
                    return Ok(Response::Labels {
                        labels: self
                            .node
                            .labels_in_view(execution_context.read_view())
                            .map_err(execution_failed)?,
                    });
                }
                let matched = self.facet_matches(&query, is_cancelled, execution_context)?;
                let labels = self
                    .facet_label_counts(
                        &query,
                        matched.as_deref(),
                        is_cancelled,
                        execution_context,
                    )?
                    .into_keys()
                    .collect();
                Ok(Response::Labels { labels })
            }
            Describe::Properties {
                query,
                detail,
                skip,
                limit,
            } => {
                let mut document = query_document_to_parser_input(&query);
                if detail.is_some_and(|d| d == rushdb_protocol::PropertyDetail::Catalog) {
                    if query
                        .predicate
                        .as_object()
                        .is_some_and(|predicate| !predicate.is_empty())
                    {
                        return Err(ProtocolError::new(
                            ErrorCode::InvalidRequest,
                            "catalog detail does not support a predicate; use full detail or a label filter",
                        ));
                    }
                    document["detail"] = serde_json::Value::String("catalog".into());
                }
                if let Some(skip) = skip {
                    document["entry_skip"] = serde_json::Value::from(skip);
                }
                if let Some(limit) = limit {
                    document["entry_limit"] = serde_json::Value::from(limit);
                }
                let properties = self
                    .node
                    .describe_properties_in_view(
                        execution_context.read_view(),
                        &document,
                        None,
                        is_cancelled,
                    )
                    .map_err(execution_failed)?;
                Ok(Response::Properties { properties })
            }
            Describe::QueryLanguages => Ok(Response::QueryLanguages {
                languages: query_language_capabilities(self.node.languages()),
            }),
            Describe::Capabilities => Ok(Response::Capabilities {
                capabilities: engine_capabilities(self.node.languages()),
            }),
            Describe::BuildInfo => Ok(Response::BuildInfo {
                build: self.node.build_info(),
            }),
            Describe::Explain { query } => Ok(Response::Explain {
                plan: self
                    .node
                    .explain_in_view(&query, execution_context.read_view())
                    .map_err(execution_failed)?,
            }),
            Describe::ExplainVector { query } => Ok(Response::ExplainVector {
                plan: self
                    .node
                    .explain_vector_in_view(
                        &query,
                        std::sync::Arc::clone(execution_context.read_view()),
                    )
                    .map_err(execution_failed)?,
            }),
            Describe::Indexes => Ok(Response::Indexes {
                indexes: self
                    .node
                    .list_indexes_in_view(execution_context.read_view())
                    .map_err(execution_failed)?,
            }),
            Describe::VectorIndexes => {
                let result = self
                    .node
                    .list_vector_indexes_in_view(execution_context.read_view())
                    .map_err(execution_failed)?;
                Ok(Response::VectorIndexes {
                    indexes: response_array(&result, "data")?,
                })
            }
            Describe::VectorIndex { name } => Ok(Response::VectorIndexStats {
                stats: self
                    .node
                    .vector_index_stats_in_view(&name, execution_context.read_view())
                    .map_err(execution_failed)?,
            }),
            Describe::Names { text, limit } => Ok(Response::Names {
                names: self
                    .node
                    .suggest_names_in_view(
                        execution_context.read_view(),
                        &text,
                        usize::try_from(limit.unwrap_or(5).min(100)).unwrap_or(100),
                        access,
                    )
                    .map_err(execution_failed)?,
            }),
            Describe::SchemaSummary { labels, few_values } => {
                let mut meter = self
                    .node
                    .working_set(is_cancelled, execution_context.deadline());
                Ok(Response::SchemaSummary {
                    labels: self
                        .node
                        .schema_summary_in_view(
                            execution_context.read_view(),
                            &labels,
                            few_values.unwrap_or(10).min(100),
                            access,
                            &mut meter,
                        )
                        .map_err(execution_failed)?,
                })
            }
            Describe::ValueOverlap { pairs } => {
                let mut meter = self
                    .node
                    .working_set(is_cancelled, execution_context.deadline());
                Ok(Response::ValueOverlap {
                    pairs: self
                        .node
                        .value_overlap_in_view(
                            execution_context.read_view(),
                            &pairs,
                            access,
                            &mut meter,
                        )
                        .map_err(execution_failed)?,
                })
            }
        }
    }
}

/// Cuts every text sample, listed value and bound of a catalog read to `max`
/// characters (`resources.introspection-value-max-chars`; 0 = no cap): a
/// column of long texts must not flood a schema read. Value queries and
/// records keep their values whole.
fn clip_introspection(response: Response, max: usize) -> Response {
    if max == 0 {
        return response;
    }
    let clip = |value: &mut serde_json::Value| {
        if let serde_json::Value::String(text) = value {
            if let Some((cut, _)) = text.char_indices().nth(max) {
                text.truncate(cut);
                text.push('…');
            }
        }
    };
    let clip_described = |properties: &mut Vec<rushdb_protocol::PropertyDescription>| {
        for property in properties {
            property.sample_values.iter_mut().for_each(clip);
            property.min.iter_mut().for_each(clip);
            property.max.iter_mut().for_each(clip);
        }
    };
    match response {
        Response::Schema { mut schema } => {
            clip_described(&mut schema.properties);
            Response::Schema { schema }
        }
        Response::Properties { mut properties } => {
            clip_described(&mut properties);
            Response::Properties { properties }
        }
        Response::SchemaSummary { mut labels } => {
            for property in labels
                .iter_mut()
                .flat_map(|label| label.properties.iter_mut())
            {
                property.values.iter_mut().for_each(clip);
                property.min.iter_mut().for_each(clip);
                property.max.iter_mut().for_each(clip);
            }
            Response::SchemaSummary { labels }
        }
        other => other,
    }
}
