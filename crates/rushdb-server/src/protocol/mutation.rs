//! Protocol mutation handlers on an admitted execution context.

use super::{
    execution_failed, language_execution_failed, query_document_to_parser_input, response_u64,
    sql_failed, ErrorCode, Mutation, ProtocolError, ProtocolExecutor, PutMode, Response,
    StatementLanguage,
};

impl ProtocolExecutor<'_> {
    #[expect(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        reason = "single linear operation/reader; splitting it would scatter shared borrows and hurt auditability"
    )]
    pub(super) fn mutate(
        &self,
        mutation: Mutation,
        is_cancelled: &dyn Fn() -> bool,
        execution_context: &mut rushdb_engine::ExecutionContext,
    ) -> Result<Response, ProtocolError> {
        match mutation {
            Mutation::GremlinBytecode { .. } => Err(super::bytecode::not_rewritten()),
            Mutation::Statement {
                language,
                statement,
                parameters,
            } => {
                if !self.node.languages().enabled(language) {
                    return Err(ProtocolError::new(
                        ErrorCode::LanguageDisabled,
                        format!("mutation language `{language:?}` is not enabled"),
                    ));
                }
                // Neo4j-style index DDL before the graph translator.
                if matches!(language, StatementLanguage::Cypher | StatementLanguage::Gql) {
                    if let Some(ddl) = rushdb_language::ddl::parse_index_ddl(&statement) {
                        return self.index_ddl(ddl, execution_context);
                    }
                }
                // SQL index DDL is intercepted before DataFusion (which has no
                // DDL): CREATE [VECTOR] INDEX / DROP INDEX -> native index ops.
                if language == StatementLanguage::Sql {
                    if let Some(response) =
                        self.sql_index_ddl(&statement, execution_context, is_cancelled)?
                    {
                        return Ok(response);
                    }
                }
                if language == StatementLanguage::Sql {
                    let result = rushdb_sql::execute_sql_mutation_in_view_with_budget(
                        &self.node.lmpg(),
                        std::sync::Arc::clone(execution_context.read_view()),
                        &statement,
                        parameters.as_ref(),
                        execution_context.remaining_budget(),
                        Some(self.node.text_indexes()),
                    )
                    .map_err(sql_failed)?;
                    let rows = result
                        .rows
                        .into_iter()
                        .map(|row| {
                            serde_json::Value::Object(row.into_iter().collect::<serde_json::Map<
                                String,
                                serde_json::Value,
                            >>(
                            ))
                        })
                        .collect::<Vec<_>>();
                    return Ok(Response::StatementMutation {
                        total: rows.len() as u64,
                        rows,
                        affected: result.affected,
                        commit_index: result.commit_index,
                        status: None,
                    });
                }
                let language = match language {
                    StatementLanguage::Gql => rushdb_language::QueryLanguage::Gql,
                    StatementLanguage::Cypher => rushdb_language::QueryLanguage::Cypher,
                    StatementLanguage::Gremlin => rushdb_language::QueryLanguage::Gremlin,
                    StatementLanguage::SqlPgq => {
                        return Err(ProtocolError::new(
                            ErrorCode::InvalidRequest,
                            "SQL/PGQ is a read-only profile",
                        ))
                    }
                    StatementLanguage::Sql => unreachable!(),
                    StatementLanguage::Mongo => {
                        // Index commands are handled at the engine level.
                        let doc: serde_json::Value =
                            serde_json::from_str(&statement).map_err(execution_failed)?;
                        if let Some(idx) = doc.get("createIndex") {
                            let label = idx.get("collection").and_then(|v| v.as_str()).ok_or_else(
                                || {
                                    ProtocolError::new(
                                        ErrorCode::InvalidRequest,
                                        "createIndex requires `collection`",
                                    )
                                },
                            )?;
                            let property = idx
                                .get("property")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    ProtocolError::new(
                                        ErrorCode::InvalidRequest,
                                        "createIndex requires `property`",
                                    )
                                })?;
                            if idx.get("dimensions").is_none() {
                                let name = idx
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .map_or_else(|| format!("{label}_{property}"), str::to_string);
                                // `"kind": "text"` (optionally with
                                // `"consistency": "immediate"`) declares a
                                // full-text index.
                                let kind = match idx.get("kind").and_then(|v| v.as_str()) {
                                    None | Some("ordered") => {
                                        rushdb_language::ddl::DdlKind::Ordered
                                    }
                                    Some("text") => rushdb_language::ddl::DdlKind::Fulltext {
                                        immediate: idx.get("consistency").and_then(|v| v.as_str())
                                            == Some("immediate"),
                                    },
                                    Some(other) => {
                                        return Err(ProtocolError::new(
                                            ErrorCode::InvalidRequest,
                                            format!("unsupported index kind `{other}`"),
                                        ))
                                    }
                                };
                                return self.create_declared_index(&name, label, property, kind);
                            }
                            let similarity = idx
                                .get("similarity")
                                .and_then(|v| v.as_str())
                                .unwrap_or("cosine")
                                .to_string();
                            let dims = idx
                                .get("dimensions")
                                .and_then(serde_json::Value::as_u64)
                                .ok_or_else(|| {
                                    ProtocolError::new(
                                        ErrorCode::InvalidRequest,
                                        "createIndex requires `dimensions`",
                                    )
                                })?;
                            let name = idx.get("name").and_then(|v| v.as_str()).map(str::to_string);
                            let index = self
                                .node
                                .create_vector_index_with_execution_context(
                                    execution_context,
                                    &rushdb_protocol::VectorIndexDefinition {
                                        label: label.to_string(),
                                        property: property.to_string(),
                                        similarity,
                                        dimensions: dims as u32,
                                        name,
                                        model: None,
                                        owner: None,
                                    },
                                )
                                .map_err(execution_failed)?;
                            return Ok(Response::StatementMutation {
                                total: 0,
                                rows: vec![],
                                affected: 1,
                                commit_index: self.node.lmpg().store.committed_version(),
                                status: Some(
                                    index
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                ),
                            });
                        }
                        if let Some(idx) = doc.get("dropIndex") {
                            let name =
                                idx.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                                    ProtocolError::new(
                                        ErrorCode::InvalidRequest,
                                        "dropIndex requires `name`",
                                    )
                                })?;
                            self.drop_any_index(name, execution_context)?;
                            return Ok(Response::StatementMutation {
                                total: 0,
                                rows: vec![],
                                affected: 1,
                                commit_index: self.node.lmpg().store.committed_version(),
                                status: None,
                            });
                        }
                        if let Some(idx) = doc.get("reIndex") {
                            let name =
                                idx.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                                    ProtocolError::new(
                                        ErrorCode::InvalidRequest,
                                        "reIndex requires `name`",
                                    )
                                })?;
                            self.node
                                .revalidate_vector_index_with_execution_context(
                                    execution_context,
                                    name,
                                )
                                .map_err(execution_failed)?;
                            return Ok(Response::StatementMutation {
                                total: 0,
                                rows: vec![],
                                affected: 1,
                                commit_index: self.node.lmpg().store.committed_version(),
                                status: None,
                            });
                        }
                        let cancelled =
                            || is_cancelled() || execution_context.check_deadline().is_err();
                        let affected = rushdb_mongodb::execute_mutation_in_view_with_cancel(
                            &self.node.lmpg(),
                            execution_context.read_view(),
                            &statement,
                            &cancelled,
                        )
                        .map_err(execution_failed)?;
                        return Ok(Response::StatementMutation {
                            total: 0,
                            rows: vec![],
                            affected,
                            commit_index: self.node.lmpg().store.committed_version(),
                            status: None,
                        });
                    }
                };
                execution_context
                    .check_deadline()
                    .map_err(|error| execution_failed(error.to_string()))?;
                let result = rushdb_language::execute_mutation_in_view_with_limits(
                    &self.node.lmpg(),
                    execution_context.read_view(),
                    language,
                    &statement,
                    parameters.as_ref(),
                    rushdb_language::MUTATION_MAX_NODES,
                    execution_context
                        .remaining_budget()
                        .unwrap_or(rushdb_language::MUTATION_MAX_BYTES)
                        .min(rushdb_language::MUTATION_MAX_BYTES),
                    &|| execution_context.check_active().is_err(),
                    Some(self.node.text_indexes()),
                )
                .map_err(language_execution_failed)?;
                execution_context
                    .check_deadline()
                    .map_err(|error| execution_failed(error.to_string()))?;
                Ok(Response::StatementMutation {
                    total: result.result.rows.len() as u64,
                    rows: result.result.rows,
                    affected: result.affected,
                    commit_index: result.commit_index,
                    status: result.result.status,
                })
            }
            Mutation::PutRecord {
                mode,
                id,
                label,
                properties,
                vectors,
                options,
            } => {
                rushdb_lmpg::import::ImportOptions::validate_json(&options)
                    .map_err(|error| ProtocolError::new(ErrorCode::InvalidRequest, error))?;
                scalar_properties(&properties)?;
                // A supplied id is part of the request contract: a
                // malformed one is the caller's error, not an engine fault.
                if let Some(id) = id.as_deref() {
                    let compact: String = id.chars().filter(|c| *c != '-').collect();
                    if compact.len() != 32 || rushdb_query::hex_decode_16(&compact).is_none() {
                        return Err(ProtocolError::new(
                            ErrorCode::InvalidRequest,
                            format!("invalid record id `{id}`: expected a UUID or 32 hex digits"),
                        ));
                    }
                }
                let record = match mode {
                    // An upsert without a label only updates; the
                    // engine never invents a label.
                    PutMode::Create if label.is_none() && upsert_requested(&options) => {
                        use crate::node::UnlabeledUpsert;
                        match self
                            .node
                            .upsert_unlabeled_with_execution_context(
                                execution_context,
                                &properties,
                                &vectors,
                                &options,
                            )
                            .map_err(execution_failed)?
                        {
                            UnlabeledUpsert::Updated(record) => Ok(record),
                            UnlabeledUpsert::NoMatch => Err(ProtocolError::new(
                                ErrorCode::InvalidRequest,
                                "no record matches the mergeBy keys and no label is given; \
                                 an upsert without a label only updates",
                            )),
                            UnlabeledUpsert::Ambiguous(labels) => Err(ProtocolError::new(
                                ErrorCode::Conflict,
                                format!(
                                    "the mergeBy keys match records labelled {}; give a label",
                                    labels.join(", ")
                                ),
                            )),
                        }
                    }
                    PutMode::Create
                        if id.as_deref().is_some_and(|id| {
                            self.node
                                .record_exists(execution_context, id)
                                .unwrap_or(false)
                        }) =>
                    {
                        Err(ProtocolError::new(
                            ErrorCode::Conflict,
                            format!(
                                "record {} already exists",
                                id.as_deref().unwrap_or_default()
                            ),
                        ))
                    }
                    PutMode::Create => self
                        .node
                        .create_record_with_execution_context(
                            execution_context,
                            id.as_deref(),
                            label.as_deref().ok_or_else(|| {
                                ProtocolError::new(
                                    ErrorCode::InvalidRequest,
                                    "create requires label",
                                )
                            })?,
                            &properties,
                            &vectors,
                            &options,
                        )
                        .map_err(execution_failed),
                    PutMode::Merge => {
                        let id = id.as_deref().ok_or_else(|| {
                            ProtocolError::new(ErrorCode::InvalidRequest, "merge requires id")
                        })?;
                        if label.is_none()
                            && !self
                                .node
                                .record_exists(execution_context, id)
                                .map_err(execution_failed)?
                        {
                            return Err(ProtocolError::new(
                                ErrorCode::InvalidRequest,
                                "no record has this id and no label is given; \
                                 merge creates a missing record only when `label` is given",
                            ));
                        }
                        self.node
                            .merge_record_with_execution_context(
                                execution_context,
                                id,
                                label.as_deref(),
                                &properties,
                                &vectors,
                            )
                            .map_err(execution_failed)
                    }
                    PutMode::Replace => self
                        .node
                        .replace_record_with_execution_context(
                            execution_context,
                            id.as_deref().ok_or_else(|| {
                                ProtocolError::new(ErrorCode::InvalidRequest, "replace requires id")
                            })?,
                            &properties,
                            &vectors,
                        )
                        .map_err(execution_failed),
                }?;
                Ok(Response::Record { record })
            }
            Mutation::DeleteRecord { id } => {
                self.node
                    .delete_record_with_execution_context(execution_context, &id)
                    .map_err(execution_failed)?;
                Ok(Response::Mutation { affected: 1 })
            }
            Mutation::RetypeRecord {
                id,
                from_label,
                to_label,
            } => {
                let record = self
                    .node
                    .retype_record_with_execution_context(
                        execution_context,
                        &id,
                        &from_label,
                        &to_label,
                    )
                    .map_err(execution_failed)?;
                Ok(Response::Record { record })
            }
            Mutation::DeleteRecords { query } => {
                let document = query_document_to_parser_input(&query);
                // The affected count comes from the batched delete itself:
                // pre-counting here would materialize the full match set a
                // second time (the same unbounded read the batching removes).
                let affected = self
                    .node
                    .delete_records_with_execution_context(
                        execution_context,
                        &document,
                        is_cancelled,
                    )
                    .map_err(execution_failed)?;
                Ok(Response::Mutation { affected })
            }
            Mutation::DeleteProperty { name } => {
                let result = self
                    .node
                    .delete_property_with_execution_context(execution_context, &name)
                    .map_err(execution_failed)?
                    .ok_or_else(|| {
                        ProtocolError::new(
                            ErrorCode::NotFound,
                            format!("property `{name}` does not exist"),
                        )
                    })?;
                Ok(Response::Mutation {
                    affected: response_u64(&result, "deleted")?,
                })
            }
            Mutation::LinkRecords { command } => {
                let result = self
                    .node
                    .link_records_with_execution_context(execution_context, &command)
                    .map_err(execution_failed)?;
                Ok(Response::Linked {
                    created: response_u64(&result, "created")?,
                    skipped_existing: response_u64(&result, "skipped_existing")?,
                })
            }
            Mutation::UnlinkRecords { command } => {
                let result = self
                    .node
                    .unlink_records_with_execution_context(execution_context, &command)
                    .map_err(execution_failed)?;
                Ok(Response::Mutation {
                    affected: response_u64(&result, "deleted")?,
                })
            }
            Mutation::LinkRecordsById { command } => {
                let result = self
                    .node
                    .link_records_by_id_with_execution_context(execution_context, &command)
                    .map_err(execution_failed)?;
                Ok(Response::Linked {
                    created: response_u64(&result, "created")?,
                    skipped_existing: response_u64(&result, "skipped_existing")?,
                })
            }
            Mutation::LinkRecordsPairs { command } => {
                let result = self
                    .node
                    .link_records_pairs_with_execution_context(execution_context, &command)
                    .map_err(execution_failed)?;
                Ok(Response::Linked {
                    created: response_u64(&result, "created")?,
                    skipped_existing: response_u64(&result, "skipped_existing")?,
                })
            }
            Mutation::UnlinkRecordsById { command } => {
                let result = self
                    .node
                    .unlink_records_by_id_with_execution_context(execution_context, &command)
                    .map_err(execution_failed)?;
                Ok(Response::Mutation {
                    affected: response_u64(&result, "deleted")?,
                })
            }
            Mutation::RetypeRelations { command } => {
                let result = self
                    .node
                    .retype_relations_with_execution_context(execution_context, &command)
                    .map_err(execution_failed)?;
                Ok(Response::Mutation {
                    affected: response_u64(&result, "retyped")?,
                })
            }
        }
    }
}

/// True when put options ask for an upsert (`mergeBy`, `mergeStrategy` or
/// `merge`).
fn upsert_requested(options: &serde_json::Value) -> bool {
    rushdb_lmpg::import::ImportOptions::from_json(options).upsert_requested()
}

/// Property values are scalars, nulls or arrays of them. An object (or an
/// array holding one) is refused up front: nested objects become related
/// records through ingest, and storing one as a property would either fail
/// deep in planning or lose it as `"[object Object]"`.
pub(super) fn scalar_properties(properties: &serde_json::Value) -> Result<(), ProtocolError> {
    let Some(map) = properties.as_object() else {
        return Err(ProtocolError::new(
            ErrorCode::InvalidRequest,
            "properties must be an object",
        ));
    };
    for (name, value) in map {
        let nested = value.is_object()
            || value
                .as_array()
                .is_some_and(|items| items.iter().any(serde_json::Value::is_object));
        if nested {
            return Err(ProtocolError::new(
                ErrorCode::InvalidRequest,
                format!(
                    "property `{name}` holds an object; properties hold scalars or arrays of \
                     them (ingest nested objects to store them as related records)"
                ),
            ));
        }
    }
    Ok(())
}
