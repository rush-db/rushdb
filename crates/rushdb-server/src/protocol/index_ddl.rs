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
