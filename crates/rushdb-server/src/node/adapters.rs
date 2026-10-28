//! Node adapters operations. Context entry points retain caller-owned
//! snapshot, tenant, cancellation, and budget semantics.

use super::{
    access, bytes_from_hex, key, Arc, OwnerKind, RecordId, RelationshipId, RequestScope,
    P_FAMILY_SIG,
};

impl RequestScope<'_> {
    /// Execute a SQL statement, returning rows as JSON maps.
    ///
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn sql(&self, sql: &str) -> Result<Vec<rushdb_sql::Row>, String> {
        rushdb_sql::reset_work_counters();
        let result = rushdb_sql::execute_sql(&self.lmpg(), sql).map_err(|e| e.to_string());
        self.observe_sql_work();
        result
    }

    /// Execute a parameterized SQL statement, returning rows as JSON maps.
    ///
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn sql_with_parameters(
        &self,
        sql: &str,
        parameters: Option<&serde_json::Value>,
    ) -> Result<Vec<rushdb_sql::Row>, String> {
        self.sql_with_parameters_budget(sql, parameters, None)
    }

    /// Execute parameterized SQL with an optional response working-byte
    /// budget supplied by the caller's execution context.
    ///
    /// # Errors
    /// Returns a string when SQL parsing, execution, storage, or the response
    /// byte budget rejects the operation.
    pub fn sql_with_parameters_budget(
        &self,
        sql: &str,
        parameters: Option<&serde_json::Value>,
        max_result_bytes: Option<usize>,
    ) -> Result<Vec<rushdb_sql::Row>, String> {
        let snapshot = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        self.sql_with_parameters_budget_in_view(snapshot, sql, parameters, max_result_bytes)
    }

    /// Execute parameterized SQL against a caller-owned pinned view.
    pub(crate) fn sql_with_parameters_budget_in_view(
        &self,
        snapshot: Arc<rushdb_storage::ReadView>,
        sql: &str,
        parameters: Option<&serde_json::Value>,
        max_result_bytes: Option<usize>,
    ) -> Result<Vec<rushdb_sql::Row>, String> {
        self.sql_with_parameters_budget_in_view_cancellable(
            snapshot,
            sql,
            parameters,
            max_result_bytes,
            &|| false,
            None,
        )
        .map_err(|e| e.to_string())
    }

    /// Execute parameterized SQL against a caller-owned pinned view, polling
    /// `is_cancelled` between result batches so a long scan stops promptly.
    pub(crate) fn sql_with_parameters_budget_in_view_cancellable(
        &self,
        snapshot: Arc<rushdb_storage::ReadView>,
        sql: &str,
        parameters: Option<&serde_json::Value>,
        max_result_bytes: Option<usize>,
        is_cancelled: &(dyn Fn() -> bool + Send + Sync),
        policy: Option<Arc<rushdb_types::security::ActivePolicy>>,
    ) -> Result<Vec<rushdb_sql::Row>, rushdb_sql::SqlError> {
        rushdb_sql::reset_work_counters();
        let result = match policy {
            // Under an enforced policy the statement sees only what it
            // allows.
            Some(policy) => rushdb_sql::execute_sql_under_policy_in_view(
                &self.lmpg(),
                snapshot,
                sql,
                parameters,
                max_result_bytes,
                is_cancelled,
                Some(self.text_indexes()),
                policy,
            ),
            None => rushdb_sql::execute_sql_with_parameters_budget_in_view_cancellable(
                &self.lmpg(),
                snapshot,
                sql,
                parameters,
                max_result_bytes,
                is_cancelled,
                Some(self.text_indexes()),
            ),
        };
        self.observe_sql_work();
        result
    }

    pub(super) fn observe_sql_work(&self) {
        let work = rushdb_sql::take_work_counters();
        self.observe_work("sql_scan", "candidate_rows", work.candidate_rows);
        self.observe_work("sql_scan", "reconstructed_rows", work.reconstructed_rows);
        self.observe_work("sql_scan", "filtered_rows", work.filtered_rows);
        self.observe_work("sql_scan", "posting_bytes", work.posting_bytes);
    }

    // ---- lexical ---------------------------------------------------------

    /// Run a lexical top-k search and resolve each hit to its public record id,
    /// resolving against the caller's admitted read view so hits are consistent
    /// with the rest of the transaction's snapshot. Also returns the index's
    /// changes not yet applied when the search ran.
    ///
    /// # Errors
    /// Returns an error message when the search or record lookup fails.
    pub fn text_search_in_view(
        &self,
        index: &str,
        query: &str,
        opts: rushdb_textsearch::LexicalOptions,
        limit: usize,
        view: &rushdb_storage::ReadView,
    ) -> Result<(Vec<serde_json::Value>, u64), String> {
        let found = self.text.search(
            self.tenant(),
            view,
            rushdb_textsearch::TextQuery {
                index,
                query,
                opts,
                top_k: limit,
            },
            None,
        )?;
        let mut rows = Vec::with_capacity(found.hits.len());
        for hit in found.hits {
            let Some(header) = access::record_by_ord(view, self.tenant(), hit.owner_ord)
                .map_err(|e| e.to_string())?
            else {
                continue;
            };
            rows.push(serde_json::json!({
                "id": uuid::Uuid::from_bytes(header.record_id).to_string(),
                "score": hit.score,
            }));
        }
        Ok((rows, found.pending))
    }

    // ---- vectors -----------------------------------------------------------

    /// Create a vector space with the default similarity metric.
    ///
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn ensure_space(&self, alias: &str, dims: u32) -> Result<(), String> {
        self.vector_engine()?
            .ensure_space(alias, dims)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Ensure a vector space with an explicit similarity metric. The engine
    /// carries vectors only — no embedding source/model concept.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn ensure_space_similarity(
        &self,
        alias: &str,
        dims: u32,
        similarity: rushdb_types::occurrence::Similarity,
    ) -> Result<(), String> {
        self.vector_engine()?
            .ensure_space_similarity(alias, dims, similarity)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Resolves the family id for a record-owned field from the catalog
    /// signature index and binds the caller's vector through the shared
    /// deterministic path.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn bind_field_vector(
        &self,
        alias: &str,
        record_hex: &str,
        field: &str,
        vector: &[f32],
    ) -> Result<u64, String> {
        let id = RecordId(bytes_from_hex(record_hex)?);
        let tenant = self.tenant();
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        let hdr = access::record_by_id(&snap, tenant, &id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("record `{record_hex}` not found"))?;

        let norm = rushdb_types::value::normalize_name(field);
        let sig = key(
            tenant,
            &[
                P_FAMILY_SIG,
                b"default",
                &[OwnerKind::Record.as_byte()],
                norm.as_bytes(),
            ],
        );
        let raw = snap
            .get(&sig)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("family `{field}` does not exist yet"))?;
        let pid = u64::from_be_bytes(
            raw.as_slice()
                .try_into()
                .map_err(|_| "corrupt property family signature")?,
        );

        self.vector_engine()?
            .bind(
                alias,
                rushdb_vector::SourceRef {
                    kind: OwnerKind::Record,
                    owner_ord: hdr.ordinal,
                    pid,
                    elem: 0,
                },
                vector,
            )
            .map_err(|e| e.to_string())
    }

    /// Bind a managed vector only when the source still matches the hash
    /// captured before an external embedding provider was invoked. This keeps
    /// a late `NestJS` completion from attaching an embedding to a newer source
    /// value; the engine owns the conditional commit, while `NestJS` owns the
    /// provider call and retry policy.
    /// # Errors
    ///
    /// Returns an error when the record, field, source hash, or vector is
    /// invalid, or when the binding commit conflicts.
    pub fn bind_field_vector_if_source_hash(
        &self,
        alias: &str,
        record_hex: &str,
        field: &str,
        expected_source_hash: u64,
        vector: &[f32],
    ) -> Result<u64, String> {
        let id = RecordId(bytes_from_hex(record_hex)?);
        let tenant = self.tenant();
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        let hdr = access::record_by_id(&snap, tenant, &id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("record `{record_hex}` not found"))?;
        let norm = rushdb_types::value::normalize_name(field);
        let sig = key(
            tenant,
            &[
                P_FAMILY_SIG,
                b"default",
                &[OwnerKind::Record.as_byte()],
                norm.as_bytes(),
            ],
        );
        let raw = snap
            .get(&sig)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("family `{field}` does not exist yet"))?;
        let pid = u64::from_be_bytes(
            raw.as_slice()
                .try_into()
                .map_err(|_| "corrupt property family signature")?,
        );
        self.vector_engine()?
            .bind_if_source_hash(
                alias,
                rushdb_vector::SourceRef {
                    kind: OwnerKind::Record,
                    owner_ord: hdr.ordinal,
                    pid,
                    elem: 0,
                },
                expected_source_hash,
                vector,
            )
            .map_err(|e| e.to_string())
    }

    /// Bind an external vector to record `record_hex`'s property `field`.
    ///
    /// # Errors
    ///
    /// Returns an error message when the record or property family is
    /// missing or the binding commit fails.
    pub fn bind_field_vector_external(
        &self,
        alias: &str,
        record_hex: &str,
        field: &str,
        vector: &[f32],
    ) -> Result<u64, String> {
        Ok(self
            .bind_field_vectors_external(alias, field, &[(record_hex, vector)])?
            .first()
            .copied()
            .unwrap_or_default())
    }

    /// Bind many external record vectors for one `(alias, field)` in a single
    /// commit (C9, cloud-capacity F10: one durable commit per batch, not per
    /// vector).
    ///
    /// # Errors
    ///
    /// Returns an error message when a record or the property family is
    /// missing or the binding commit fails; nothing is bound in that case.
    pub fn bind_field_vectors_external(
        &self,
        alias: &str,
        field: &str,
        items: &[(&str, &[f32])],
    ) -> Result<Vec<u64>, String> {
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        let pid = self.record_family_pid(&snap, field)?;
        let tenant = self.tenant();
        let mut batch = Vec::with_capacity(items.len());
        for (record_hex, vector) in items {
            let id = RecordId(bytes_from_hex(record_hex)?);
            let hdr = access::record_by_id(&snap, tenant, &id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("record `{record_hex}` not found"))?;
            batch.push((
                rushdb_vector::SourceRef {
                    kind: OwnerKind::Record,
                    owner_ord: hdr.ordinal,
                    pid,
                    elem: 0,
                },
                vector.to_vec(),
            ));
        }
        self.vector_engine()?
            .bind_external_many(alias, batch)
            .map_err(|e| e.to_string())
    }

    /// Family id of record property `field` (it must already exist).
    fn record_family_pid(
        &self,
        snap: &rushdb_storage::ReadView,
        field: &str,
    ) -> Result<u64, String> {
        let norm = rushdb_types::value::normalize_name(field);
        let sig = key(
            self.tenant(),
            &[
                P_FAMILY_SIG,
                b"default",
                &[OwnerKind::Record.as_byte()],
                norm.as_bytes(),
            ],
        );
        let raw = snap
            .get(&sig)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("family `{field}` does not exist yet"))?;
        Ok(u64::from_be_bytes(
            raw.as_slice()
                .try_into()
                .map_err(|_| "corrupt property family signature")?,
        ))
    }

    /// Bind an external inline vector to a relationship owner without requiring
    /// a stored occurrence: resolves the relationship and its relationship-scoped
    /// family, then binds through the shared owner-kind-generic path.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn bind_relationship_vector_external(
        &self,
        alias: &str,
        relationship_hex: &str,
        field: &str,
        vector: &[f32],
    ) -> Result<u64, String> {
        let id = RelationshipId(bytes_from_hex(relationship_hex)?);
        let tenant = self.tenant();
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        let hdr = access::relationship_by_id(&snap, tenant, &id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("relationship `{relationship_hex}` not found"))?;
        let norm = rushdb_types::value::normalize_name(field);
        let sig = key(
            tenant,
            &[
                P_FAMILY_SIG,
                b"default",
                &[OwnerKind::Relationship.as_byte()],
                norm.as_bytes(),
            ],
        );
        let raw = snap
            .get(&sig)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("family `{field}` does not exist yet"))?;
        let pid = u64::from_be_bytes(
            raw.as_slice()
                .try_into()
                .map_err(|_| "corrupt property family signature")?,
        );
        self.vector_engine()?
            .bind_external(
                alias,
                rushdb_vector::SourceRef {
                    kind: OwnerKind::Relationship,
                    owner_ord: hdr.ordinal,
                    pid,
                    elem: 0,
                },
                vector,
            )
            .map_err(|e| e.to_string())
    }

    /// Bind a managed relationship vector only when its captured source hash
    /// still matches the live relationship property.
    /// # Errors
    ///
    /// Returns an error when the relationship, field, source hash, or vector
    /// is invalid, or when the binding commit conflicts.
    pub fn bind_relationship_vector_if_source_hash(
        &self,
        alias: &str,
        relationship_hex: &str,
        field: &str,
        expected_source_hash: u64,
        vector: &[f32],
    ) -> Result<u64, String> {
        let id = RelationshipId(bytes_from_hex(relationship_hex)?);
        let tenant = self.tenant();
        let snap = Arc::new(self.store.read_view().map_err(|e| e.to_string())?);
        let hdr = access::relationship_by_id(&snap, tenant, &id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("relationship `{relationship_hex}` not found"))?;
        let norm = rushdb_types::value::normalize_name(field);
        let sig = key(
            tenant,
            &[
                P_FAMILY_SIG,
                b"default",
                &[OwnerKind::Relationship.as_byte()],
                norm.as_bytes(),
            ],
        );
        let raw = snap
            .get(&sig)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("family `{field}` does not exist yet"))?;
        let pid = u64::from_be_bytes(
            raw.as_slice()
                .try_into()
                .map_err(|_| "corrupt property family signature")?,
        );
        self.vector_engine()?
            .bind_if_source_hash(
                alias,
                rushdb_vector::SourceRef {
                    kind: OwnerKind::Relationship,
                    owner_ord: hdr.ordinal,
                    pid,
                    elem: 0,
                },
                expected_source_hash,
                vector,
            )
            .map_err(|e| e.to_string())
    }
}
