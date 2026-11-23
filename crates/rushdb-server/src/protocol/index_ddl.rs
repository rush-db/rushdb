//! Index DDL from statement languages (SD-4.6): Cypher/GQL
//! `CREATE INDEX … FOR … ON …` / `DROP INDEX` / `SHOW INDEXES`, SQL
//! `CREATE INDEX name ON label (prop)`, and Mongo `createIndex` without
//! `dimensions` all reach the same declared-index operations.

use rushdb_language::ddl::{DdlKind, IndexDdl};

use super::{execution_failed, ErrorCode, ProtocolError, ProtocolExecutor, Response};

/// SQL `CREATE [VECTOR] INDEX … ON <label>(…)` with the label spelled
/// as stored. A SQL table name finds its label case-insensitively (the
/// first stored spelling that matches, as a `SELECT` resolves it), so
/// the index is declared on the label a `SELECT` on that name reads. A
/// label no record has yet is kept as written.
pub(super) fn sql_index_label_as_stored<'s>(
    sql: &'s str,
    execution_context: &rushdb_engine::ExecutionContext,
) -> Result<std::borrow::Cow<'s, str>, ProtocolError> {
    let unchanged = Ok(std::borrow::Cow::Borrowed(sql));
    let upper = sql.to_ascii_uppercase();
    let ddl = upper.trim_start();
    if !(ddl.starts_with("CREATE INDEX") || ddl.starts_with("CREATE VECTOR INDEX")) {
        return unchanged;
    }
    let Some(on) = upper.find(" ON ") else {
        return unchanged;
    };
    let after_on = &sql[on + 4..];
    let Some(paren) = after_on.find('(') else {
        return unchanged;
    };
    let start = on + 4 + (after_on.len() - after_on.trim_start().len());
    let end = on + 4 + after_on[..paren].trim_end().len();
    if start >= end {
        return unchanged;
    }
    let written = sql[start..end].trim_matches('"');
    let labels = rushdb_lmpg::reader::try_access::labels(
        execution_context.read_view().as_ref(),
        execution_context.tenant(),
    )
    .map_err(|e| execution_failed(e.to_string()))?;
    if labels.iter().any(|label| label == written) {
        return unchanged;
    }
    let Some(stored) = labels
        .iter()
        .find(|label| label.eq_ignore_ascii_case(written))
    else {
        return unchanged;
    };
    Ok(std::borrow::Cow::Owned(format!(
        "{}{stored}{}",
        &sql[..start],
        &sql[end..]
    )))
}

impl ProtocolExecutor<'_> {
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
        // CREATE UNIQUE INDEX: the same form, values unique under the label.
        let unique_index = upper.starts_with("CREATE UNIQUE INDEX");
        let (trimmed, upper) = if unique_index {
            (
                format!("CREATE{}", &trimmed["CREATE UNIQUE".len()..]),
                format!("CREATE{}", &upper["CREATE UNIQUE".len()..]),
            )
        } else {
            (trimmed.to_owned(), upper)
        };
        let trimmed = trimmed.as_str();
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
                "" if unique_index => rushdb_language::ddl::DdlKind::Unique,
                "" => rushdb_language::ddl::DdlKind::Ordered,
                _ if unique_index => return Err(malformed()),
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

    /// Drop a declared index by name, or a vector index when no declared
    /// index has that name.
    pub(super) fn drop_any_index(
        &self,
        name: &str,
        execution_context: &mut rushdb_engine::ExecutionContext,
    ) -> Result<(), ProtocolError> {
        let declared = rushdb_lmpg::index::by_name(
            execution_context.read_view().as_ref(),
            execution_context.tenant(),
            name,
        )
        .map_err(|e| execution_failed(e.to_string()))?;
        if declared.is_some() {
            self.node.drop_index(name).map_err(execution_failed)
        } else {
            self.node
                .drop_vector_index_with_execution_context(execution_context, name)
                .map(drop)
                .map_err(execution_failed)
        }
    }

    /// Declare an index and answer like a statement mutation.
    pub(super) fn create_declared_index(
        &self,
        name: &str,
        label: &str,
        property: &str,
        kind: DdlKind,
    ) -> Result<Response, ProtocolError> {
        let (kind, consistency) = match kind {
            DdlKind::Ordered => ("ordered", None),
            DdlKind::Unique => ("unique", None),
            DdlKind::Fulltext { immediate: true } => ("text", Some("immediate".to_string())),
            DdlKind::Fulltext { immediate: false } => ("text", Some("eventual".to_string())),
        };
        let index = self
            .node
            .create_index(&rushdb_protocol::IndexDefinition {
                name: name.to_owned(),
                label: label.to_owned(),
                property: property.to_owned(),
                kind: kind.into(),
                consistency,
                dimensions: None,
            })
            .map_err(execution_failed)?;
        Ok(Response::StatementMutation {
            total: 0,
            rows: vec![],
            affected: 1,
            commit_index: self.node.lmpg().store.committed_version(),
            status: index.get("name").and_then(|v| v.as_str()).map(String::from),
        })
    }

    /// Execute parsed Neo4j-style index DDL.
    pub(super) fn index_ddl(
        &self,
        ddl: Result<IndexDdl, String>,
        execution_context: &mut rushdb_engine::ExecutionContext,
    ) -> Result<Response, ProtocolError> {
        let ddl = ddl.map_err(|message| ProtocolError::new(ErrorCode::InvalidRequest, message))?;
        let exists = |name: &str| {
            rushdb_lmpg::index::by_name(
                execution_context.read_view().as_ref(),
                execution_context.tenant(),
                name,
            )
            .map(|def| def.is_some())
            .map_err(|e| execution_failed(e.to_string()))
        };
        match ddl {
            IndexDdl::Create {
                name,
                label,
                property,
                kind,
                if_not_exists,
            } => {
                if if_not_exists && exists(&name)? {
                    return Ok(Response::StatementMutation {
                        total: 0,
                        rows: vec![],
                        affected: 0,
                        commit_index: self.node.lmpg().store.committed_version(),
                        status: Some(name),
                    });
                }
                self.create_declared_index(&name, &label, &property, kind)
            }
            IndexDdl::Drop { name, if_exists } => {
                if if_exists && !exists(&name)? {
                    return Ok(Response::StatementMutation {
                        total: 0,
                        rows: vec![],
                        affected: 0,
                        commit_index: self.node.lmpg().store.committed_version(),
                        status: None,
                    });
                }
                self.drop_any_index(&name, execution_context)?;
                Ok(Response::StatementMutation {
                    total: 0,
                    rows: vec![],
                    affected: 1,
                    commit_index: self.node.lmpg().store.committed_version(),
                    status: None,
                })
            }
            IndexDdl::Show => {
                let rows = self
                    .node
                    .list_indexes_in_view(execution_context.read_view())
                    .map_err(execution_failed)?;
                Ok(Response::Rows {
                    total: rows.len() as u64,
                    rows,
                })
            }
        }
    }
}
