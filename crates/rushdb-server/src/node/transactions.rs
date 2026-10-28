//! Node transactions operations. Context entry points retain caller-owned
//! snapshot, tenant, cancellation, and budget semantics.

use super::{bytes_from_hex, OwnerKind, RequestScope};

impl RequestScope<'_> {
    // ---- cross-request transactions (document 07) ----------------------
    /// Begin a cross-service transaction with the default TTL.
    ///
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn tx_begin(&self) -> Result<String, String> {
        self.tx_begin_with(None, false)
    }

    /// Begin a cross-service transaction with an optional per-transaction TTL
    /// override and the autocommit-on-expiry flag.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn tx_begin_with(&self, ttl_ms: Option<u64>, autocommit: bool) -> Result<String, String> {
        self.tx_coordinator()?.begin_with(ttl_ms, autocommit)
    }

    pub(crate) fn tx_begin_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        ttl_ms: Option<u64>,
        autocommit: bool,
    ) -> Result<String, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let result = self.tx_begin_with(ttl_ms, autocommit)?;
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        Ok(result)
    }

    /// Stage one typed protocol mutation onto a transaction.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    #[expect(
        clippy::too_many_lines,
        reason = "single linear operation/reader; splitting it would scatter shared borrows and hurt auditability"
    )]
    pub fn tx_stage(
        &self,
        tx_id: &str,
        mutation: &rushdb_protocol::TransactionMutation,
    ) -> Result<(), String> {
        use rushdb_lmpg::plan::FieldChange;
        let coordinator = self
            .existing_tx_coordinator()
            .ok_or("unknown transaction id")?;
        let record = |reference: &rushdb_protocol::RecordReference| -> Result<_, String> {
            let record_id = bytes_from_hex(&reference.id)?;
            Ok(rushdb_lmpg::RecordRef {
                record_id,
                ordinal: coordinator.resolve_record(tx_id, record_id)?,
            })
        };
        let batch_op = match mutation {
            rushdb_protocol::TransactionMutation::CreateRecord {
                label,
                id,
                properties,
                options,
            } => {
                rushdb_lmpg::import::ImportOptions::validate_json(options)?;
                let options = rushdb_lmpg::import::ImportOptions::from_json(options);
                let fields = properties
                    .as_object()
                    .ok_or("transaction properties must be an object")?
                    .iter()
                    .map(|(name, value)| {
                        let change = rushdb_lmpg::import::coerce_change(value, &options)
                            .unwrap_or_else(|| FieldChange::Set(value.clone()));
                        (name.clone(), change)
                    })
                    .collect();
                rushdb_lmpg::BatchOp::CreateRecord {
                    label: label.clone(),
                    explicit_id: id.as_deref().map(bytes_from_hex).transpose()?,
                    fields,
                }
            }
            rushdb_protocol::TransactionMutation::UpdateRecord {
                record: target,
                properties,
                replace,
            } => {
                let target = record(target)?;
                let mut fields = Self::props_from_properties(properties)?;
                if *replace {
                    // The record as this transaction sees it; the read joins
                    // the transaction's conflict checks.
                    let current = coordinator.get_record(tx_id, target.record_id)?;
                    let named: std::collections::HashSet<String> =
                        fields.iter().map(|(name, _)| name.clone()).collect();
                    let unnamed: Vec<String> = current["properties"]
                        .as_object()
                        .map(|existing| {
                            existing
                                .keys()
                                .filter(|name| !named.contains(*name))
                                .cloned()
                                .collect()
                        })
                        .unwrap_or_default();
                    fields.extend(unnamed.into_iter().map(|name| (name, FieldChange::Delete)));
                }
                rushdb_lmpg::BatchOp::UpdateFields {
                    owner: OwnerKind::Record,
                    owner_ord: target.ordinal,
                    owner_public_id: target.record_id,
                    fields,
                }
            }
            rushdb_protocol::TransactionMutation::RetypeRecord {
                record: target,
                from_label,
                to_label,
            } => rushdb_lmpg::BatchOp::RetypeRecord {
                record: record(target)?,
                from_label: from_label.clone(),
                to_label: to_label.clone(),
            },
            rushdb_protocol::TransactionMutation::DeleteRecord { record: target } => {
                rushdb_lmpg::BatchOp::DeleteRecord {
                    record: record(target)?,
                }
            }
            rushdb_protocol::TransactionMutation::CreateRelationship {
                relation_type,
                source,
                target,
                properties,
            } => rushdb_lmpg::BatchOp::CreateRelationship {
                rel_type: relation_type.clone(),
                source: record(source)?,
                target: record(target)?,
                fields: Self::props_from_properties(properties)?,
            },
            rushdb_protocol::TransactionMutation::GetRecord { .. } => {
                return Err("GetRecord is handled by tx_get_record".into());
            }
            rushdb_protocol::TransactionMutation::Ingest { .. } => {
                return Err("Ingest is handled by tx_stage_ingest".into());
            }
            rushdb_protocol::TransactionMutation::Statement {
                language,
                statement,
                parameters,
            } => {
                let language = match language {
                    rushdb_protocol::StatementLanguage::Gql => rushdb_language::QueryLanguage::Gql,
                    rushdb_protocol::StatementLanguage::Cypher => {
                        rushdb_language::QueryLanguage::Cypher
                    }
                    rushdb_protocol::StatementLanguage::Gremlin => {
                        rushdb_language::QueryLanguage::Gremlin
                    }
                    other => {
                        return Err(format!(
                            "transaction statement language not supported: {other:?}"
                        ))
                    }
                };
                // Plan the statement against the transaction overlay so it
                // sees this transaction's earlier staged writes, then
                // stage the resulting delta back into the same overlay.
                let overlay = coordinator.overlay_view(tx_id)?;
                let batch = rushdb_language::stage_statement_mutation_in_overlay(
                    &self.lmpg(),
                    language,
                    statement,
                    parameters.as_ref(),
                    rushdb_language::MUTATION_MAX_NODES,
                    &overlay,
                    Some(self.text_indexes()),
                )
                .map_err(|e| e.to_string())?;
                // Statement planning still does not capture its predicate
                // read-set, so conservatively order the transaction against
                // every writer to prevent phantoms. Precise predicate locking
                // is future work.
                coordinator.record_whole_keyspace_dependency(tx_id)?;
                return coordinator.stage_batch(tx_id, batch);
            }
        };
        coordinator.stage(tx_id, batch_op)
    }

    /// Stage a plain-create import into a transaction: every record of the
    /// plan with its draft id pinned, then the relationships between them
    /// (resolved inside the transaction, duplicates dropped). Returns the
    /// records and relationships staged.
    ///
    /// # Errors
    ///
    /// Returns an error message for a merge import or inline vectors (neither
    /// can be staged), an invalid plan, or a failed stage.
    pub fn tx_stage_ingest(
        &self,
        tx_id: &str,
        label: Option<&str>,
        documents: &serde_json::Value,
        options: &serde_json::Value,
    ) -> Result<(u64, u64), String> {
        use rushdb_lmpg::import::{serialize_bfs, ImportOptions};
        let coordinator = self
            .existing_tx_coordinator()
            .ok_or("unknown transaction id")?;
        ImportOptions::validate_json(options)?;
        let options = ImportOptions::from_json(options);
        if options.merge_by.is_some() || options.merge_strategy.is_some() {
            return Err("unsupported in a transaction: an import with mergeBy or mergeStrategy cannot be staged; run it outside the transaction".into());
        }
        let plan = serialize_bfs(documents, label.unwrap_or_default(), &options);
        if let Some(error) = plan.validation_errors.first() {
            return Err(error.clone());
        }
        if !plan.vector_drafts.is_empty() {
            return Err("unsupported in a transaction: inline vectors cannot be staged; write them outside the transaction".into());
        }
        let pinned: std::collections::HashMap<[u8; 16], [u8; 16]> = plan
            .records
            .iter()
            .map(|record| {
                (
                    record.draft_id,
                    record.explicit_id.unwrap_or(record.draft_id),
                )
            })
            .collect();
        let persisted = |draft: [u8; 16]| pinned.get(&draft).copied().unwrap_or(draft);
        let mut batch = self.lmpg().begin_batch();
        for record in &plan.records {
            batch.ops.push(rushdb_lmpg::BatchOp::CreateRecord {
                label: record.label.clone(),
                explicit_id: Some(record.explicit_id.unwrap_or(record.draft_id)),
                fields: record.properties.clone(),
            });
        }
        coordinator.stage_batch(tx_id, batch)?;
        let mut seen = std::collections::BTreeSet::new();
        let mut relationships = 0u64;
        for relation in &plan.relations {
            let (source, target) = (persisted(relation.source), persisted(relation.target));
            if !seen.insert((source, relation.rel_type.clone(), target)) {
                continue;
            }
            let reference = |record_id| -> Result<rushdb_lmpg::RecordRef, String> {
                Ok(rushdb_lmpg::RecordRef {
                    record_id,
                    ordinal: coordinator.resolve_record(tx_id, record_id)?,
                })
            };
            coordinator.stage(
                tx_id,
                rushdb_lmpg::BatchOp::CreateRelationship {
                    rel_type: relation.rel_type.clone(),
                    source: reference(source)?,
                    target: reference(target)?,
                    fields: vec![],
                },
            )?;
            relationships += 1;
        }
        Ok((plan.records.len() as u64, relationships))
    }

    pub(crate) fn tx_stage_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        tx_id: &str,
        mutation: &rushdb_protocol::TransactionMutation,
    ) -> Result<(), String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        self.tx_stage(tx_id, mutation)?;
        context.check_deadline().map_err(|error| error.to_string())
    }

    /// Read one record inside a transaction (read-your-writes); the read's key
    /// footprint becomes the transaction's conflict dependencies.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn tx_get_record(
        &self,
        tx_id: &str,
        reference: &rushdb_protocol::RecordReference,
    ) -> Result<serde_json::Value, String> {
        let coordinator = self
            .existing_tx_coordinator()
            .ok_or("unknown transaction id")?;
        coordinator.get_record(tx_id, bytes_from_hex(&reference.id)?)
    }

    pub(crate) fn tx_get_record_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        tx_id: &str,
        reference: &rushdb_protocol::RecordReference,
    ) -> Result<serde_json::Value, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let mut result = self.tx_get_record(tx_id, reference)?;
        let active = rushdb_authz::load_active_policy_in_view(context.read_view(), self.tenant())
            .map_err(|error| error.to_string())?;
        let authz = rushdb_lmpg::AuthorizedView::new(
            context.read_view().as_ref(),
            self.tenant(),
            active.as_ref(),
        );
        let label = result
            .get("__label")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let allowed = match result
            .get_mut("properties")
            .and_then(serde_json::Value::as_object_mut)
        {
            Some(properties) => authz.authorize_properties(&label, properties),
            None => authz.label_allowed(&label),
        };
        if !allowed {
            return Err("record denied by policy".to_string());
        }
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        Ok(result)
    }

    /// Commit a transaction, returning its outcome.
    #[must_use]
    pub fn tx_commit(&self, tx_id: &str) -> rushdb_tx::TxOutcome {
        self.try_tx_commit(tx_id)
            .unwrap_or(rushdb_tx::TxOutcome::UnknownRetryable)
    }

    /// Fallible transaction commit for embedded callers that need to
    /// distinguish durable discovery failures from an unknown retry outcome.
    ///
    /// # Errors
    /// Returns an error when retry discovery cannot read the durable outcome
    /// record. An in-memory coordinator always returns its typed outcome.
    pub fn try_tx_commit(&self, tx_id: &str) -> Result<rushdb_tx::TxOutcome, String> {
        match self.existing_tx_coordinator() {
            Some(coordinator) => Ok(coordinator.commit(tx_id)),
            None => rushdb_tx::TxCoordinator::discover(&self.store, tx_id)
                .map(|outcome| outcome.unwrap_or(rushdb_tx::TxOutcome::UnknownRetryable))
                .map_err(|error| error.to_string()),
        }
    }

    pub(crate) fn try_tx_commit_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        tx_id: &str,
    ) -> Result<rushdb_tx::TxOutcome, String> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let result = self.try_tx_commit(tx_id)?;
        Ok(result)
    }

    /// Where a transaction id stands; a tenant without a coordinator has no
    /// transactions.
    pub(crate) fn tx_status(&self, tx_id: &str) -> rushdb_tx::TxStatus {
        self.existing_tx_coordinator()
            .map_or(rushdb_tx::TxStatus::Unknown, |coordinator| {
                coordinator.status(tx_id)
            })
    }

    /// Roll back a transaction.
    ///
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn tx_rollback(&self, tx_id: &str) -> Result<(), String> {
        self.existing_tx_coordinator()
            .ok_or("unknown transaction id")?
            .rollback(tx_id)
    }

    pub(crate) fn tx_rollback_with_execution_context(
        &self,
        _context: &mut rushdb_engine::ExecutionContext,
        tx_id: &str,
    ) -> Result<(), String> {
        // Rollback intentionally ignores an expired request deadline so a
        // caller can always release transaction state.
        self.tx_rollback(tx_id)
    }
}
