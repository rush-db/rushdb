//! Node backup operations. Context entry points retain caller-owned
//! snapshot, tenant, cancellation, and budget semantics.

use std::sync::Arc;

use rushdb_types::ids::TenantId;

use super::Node;

impl Node {
    /// Export a framed, manifest-verified snapshot stream (W6, D9/P17): a
    /// `RDBWALSNAP` manifest chunk (LSN range pinned to the snapshot commit
    /// index, CRC over all tenants), then one independently checksummed chunk
    /// per tenant's JSONL. Truncated or corrupted artifacts are rejected on
    /// restore. Returns the pinned commit index.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn export_snapshot<W: std::io::Write>(
        &self,
        writer: &mut W,
        scopes: &[String],
    ) -> Result<u64, String> {
        let snap = self.store.read_view().map_err(|e| e.to_string())?;
        let commit_index = snap.commit_index();

        let tenants = self.export_tenants(&snap, scopes)?;

        let mut chunks: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(tenants.len());
        for (tenant, scope) in tenants {
            let (jsonl, _records, _relationships) =
                rushdb_backup::export_tenant_jsonl(&snap, tenant)
                    .map_err(|e| format!("export tenant {tenant:?}: {e}"))?;
            chunks.push((scope.0.to_vec(), jsonl.into_bytes()));
        }
        rushdb_storage::wal::write_snapshot(writer, 0, commit_index, commit_index, &chunks)
            .map_err(|e| format!("write snapshot stream: {e}"))?;
        Ok(commit_index)
    }

    /// Restore a framed snapshot stream written by [`Self::export_snapshot`].
    /// Validates the manifest magic/version, verifies every chunk CRC, and
    /// rejects truncated/corrupted input (greenfield: unframed legacy backups
    /// are not accepted). Returns (records, relationships) materialized.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn restore_snapshot<R: std::io::Read>(&self, reader: &mut R) -> Result<(u64, u64), String> {
        self.restore_snapshot_progress(reader, |_, _, _| {})
            .map(|(r, rel, _)| (r, rel))
    }

    /// Streaming restore: reads the manifest, then applies each tenant chunk as
    /// it arrives (bounded memory) while reporting per-chunk progress via
    /// `progress(chunk_index, records_so_far, relationships_so_far)`. A
    /// truncated/corrupted stream is rejected before any state is applied.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "these casts narrow engine counters and sizes that are bounded by the process address space or clamped before conversion, so truncation cannot occur on supported 64-bit targets"
    )]
    pub fn restore_snapshot_progress<R: std::io::Read>(
        &self,
        reader: &mut R,
        mut progress: impl FnMut(usize, u64, u64),
    ) -> Result<(u64, u64, usize), String> {
        use rushdb_storage::wal::{decode_tenant_chunk, read_framed_chunk, read_snapshot_manifest};
        use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
        if self.store.committed_version() != 0 {
            return Err("restore target is not fresh; use an empty data directory".into());
        }
        let manifest = read_snapshot_manifest(reader).map_err(|e| format!("restore: {e}"))?;
        // Validate and stage every tenant before mutating the destination. A
        // later corrupt or semantically invalid chunk can therefore never
        // leave a partially restored target. The temporary file keeps the
        // bounded-memory property of the streaming format.
        let validation_store = Arc::new(rushdb_storage::Store::new_in_memory());
        let mut staged = tempfile::NamedTempFile::new()
            .map_err(|error| format!("restore: create staging file: {error}"))?;
        for index in 0..manifest.tenant_count {
            let Some(raw) = read_framed_chunk(reader).map_err(|e| format!("restore: {e}"))? else {
                return Err(format!(
                    "restore: snapshot truncated: {} of {} tenant chunks present",
                    index, manifest.tenant_count
                ));
            };
            let chunk = decode_tenant_chunk(&raw).map_err(|e| format!("restore: {e}"))?;
            let jsonl = String::from_utf8(chunk.jsonl).map_err(|e| e.to_string())?;
            if chunk.tenant.len() != 32 {
                return Err(format!(
                    "restore: invalid tenant identity length {}",
                    chunk.tenant.len()
                ));
            }
            let scope = rushdb_types::ids::TenantScope(
                chunk
                    .tenant
                    .as_slice()
                    .try_into()
                    .map_err(|_| "restore: invalid tenant identity")?,
            );
            let validation_tenant =
                rushdb_lmpg::tenancy::resolve_or_register(&validation_store, &scope)
                    .map_err(|e| format!("restore: register tenant: {e}"))?;
            let validation_target =
                rushdb_lmpg::Lmpg::new(Arc::clone(&validation_store), validation_tenant);
            rushdb_backup::restore_backup(&validation_target, &jsonl)
                .map_err(|e| format!("restore tenant: {e}"))?;
            staged
                .write_all(&scope.0)
                .and_then(|()| staged.write_all(&(jsonl.len() as u64).to_le_bytes()))
                .and_then(|()| staged.write_all(jsonl.as_bytes()))
                .map_err(|error| format!("restore: stage tenant chunk: {error}"))?;
        }
        staged
            .as_file_mut()
            .seek(SeekFrom::Start(0))
            .map_err(|error| format!("restore: rewind staging file: {error}"))?;
        let mut records = 0u64;
        let mut relationships = 0u64;
        for index in 0..manifest.tenant_count {
            let mut tenant_bytes = [0u8; 32];
            staged
                .read_exact(&mut tenant_bytes)
                .and_then(|()| {
                    let mut len = [0u8; 8];
                    staged.read_exact(&mut len)?;
                    let length = usize::try_from(u64::from_le_bytes(len))
                        .map_err(|_| std::io::Error::other("tenant JSONL length overflow"))?;
                    let mut jsonl = vec![0u8; length];
                    staged.read_exact(&mut jsonl)?;
                    let jsonl = String::from_utf8(jsonl)
                        .map_err(|_| std::io::Error::other("tenant JSONL is not UTF-8"))?;
                    Ok((rushdb_types::ids::TenantScope(tenant_bytes), jsonl))
                })
                .map_err(|error| format!("restore: read staged tenant chunk: {error}"))
                .and_then(|(scope, jsonl)| {
                    // A single-tenant restore intentionally maps every source chunk
                    // into its fixed target tenant. Multitenant restores preserve the
                    // framed scope identity (registered in this store's directory)
                    // so chunks cannot silently cross scopes.
                    let target_tenant = if self.multitenant {
                        rushdb_lmpg::tenancy::resolve_or_register(&self.store, &scope)
                            .map_err(|e| format!("restore: register tenant: {e}"))?
                    } else {
                        self.base_tenant
                    };
                    let target = rushdb_lmpg::Lmpg::new(Arc::clone(&self.store), target_tenant);
                    let (r, rel) = rushdb_backup::restore_backup(&target, &jsonl)
                        .map_err(|e| format!("restore tenant: {e}"))?;
                    records += r;
                    relationships += rel;
                    progress(index as usize, records, relationships);
                    Ok(())
                })?;
        }
        Ok((records, relationships, manifest.tenant_count as usize))
    }

    /// P17: export a consistent snapshot AT an explicit commit index (offline
    /// point-in-time backup). Reads the retained view for that CI, so a backup
    /// captures exactly the state as of that commit. Manifest
    /// `from_lsn == to_lsn == the requested CI`. A CI older than the history
    /// floor or newer than the committed version is rejected by the read path.
    ///
    /// **Internal, not a client contract:** this is not exposed through
    /// the protocol or CLI; arbitrary "as of commit N" reads are not a product
    /// feature. PITR remains an offline WAL operation.
    ///
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    #[doc(hidden)]
    pub fn export_snapshot_at<W: std::io::Write>(
        &self,
        writer: &mut W,
        commit_index: u64,
        scopes: &[String],
    ) -> Result<u64, String> {
        let snap = self
            .store
            .read_at(commit_index)
            .map_err(|e| e.to_string())?;

        let tenants = self.export_tenants(&snap, scopes)?;

        let mut chunks: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(tenants.len());
        for (tenant, scope) in tenants {
            let (jsonl, _records, _relationships) =
                rushdb_backup::export_tenant_jsonl(&snap, tenant)
                    .map_err(|e| format!("export tenant {tenant:?}: {e}"))?;
            chunks.push((scope.0.to_vec(), jsonl.into_bytes()));
        }
        rushdb_storage::wal::write_snapshot(writer, 0, commit_index, commit_index, &chunks)
            .map_err(|e| format!("write snapshot stream: {e}"))?;
        Ok(commit_index)
    }

    /// Export the committed logical WAL as a checksummed segment stream
    /// (manifest + framed records) for the given LSN range. Foundation for
    /// PITR / CDC / incremental backups.
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub fn export_wal_segments<W: std::io::Write>(
        &self,
        writer: &mut W,
        from_lsn: u64,
        to_lsn: u64,
    ) -> Result<u64, String> {
        self.store
            .export_wal_segments(writer, from_lsn, to_lsn)
            .map_err(|e| e.to_string())
    }

    /// Export an incremental snapshot: the WAL delta `(base_ci, to_ci]` as a
    /// checksummed segment stream (V2 PITR). Combine with a base at `base_ci`
    /// (a physical checkpoint or an equivalent store) to reconstruct `to_ci`.
    /// # Errors
    ///
    /// Returns an error message when `base_ci > to_ci` or `to_ci` is newer than
    /// the committed version, or the export fails.
    pub fn export_incremental_snapshot<W: std::io::Write>(
        &self,
        writer: &mut W,
        base_ci: u64,
        to_ci: u64,
    ) -> Result<u64, String> {
        let committed = self.store.committed_version();
        if base_ci > to_ci {
            return Err(format!("base commit {base_ci} exceeds target {to_ci}"));
        }
        if to_ci > committed {
            return Err(format!(
                "target commit {to_ci} exceeds committed version {committed}"
            ));
        }
        self.store
            .export_wal_segments(writer, base_ci.saturating_add(1), to_ci)
            .map_err(|e| e.to_string())?;
        Ok(to_ci)
    }

    /// Restore an incremental snapshot onto this node: replay the WAL delta
    /// produced by [`Self::export_incremental_snapshot`] over the base state.
    /// Returns the final commit index.
    /// # Errors
    ///
    /// Returns an error message when the segment is corrupt or a batch cannot
    /// be applied.
    pub fn restore_incremental_snapshot<R: std::io::Read>(
        &self,
        reader: &mut R,
    ) -> Result<u64, String> {
        self.store
            .replay_wal_segments(reader)
            .map_err(|e| e.to_string())
    }

    /// Archive the committed WAL range since the last archived LSN into `dir`
    /// (durability A1). Returns the new watermark, or `None` when nothing is
    /// new.
    /// # Errors
    ///
    /// Returns a message when the export or the durable write fails.
    pub fn archive_wal(&self, dir: &std::path::Path) -> Result<Option<u64>, String> {
        crate::archive::archive_once(&self.store, dir)
    }

    /// Force a durability checkpoint: fsync the materialization and durably
    /// record the committed LSN. Under log-first this must run before an
    /// offline physical base backup (`--backup-base`) copies the store file.
    ///
    /// # Errors
    /// Returns a message when the checkpoint fails.
    pub fn checkpoint(&self) -> Result<(), String> {
        self.store.checkpoint().map_err(|e| e.to_string())
    }

    /// Offline physical base backup into `out_dir`: the caller owns the
    /// store exclusively (no server running). Opening the store replays the
    /// WAL into the store file, so the copy holds every commit even after an
    /// unclean stop.
    ///
    /// # Errors
    /// Returns a message when the node has no data directory, or the copy or
    /// retention bookkeeping fails.
    pub fn backup_base_offline(
        &self,
        out_dir: &std::path::Path,
    ) -> Result<crate::archive::BaseManifest, String> {
        let Some(data_dir) = self.data_dir.clone() else {
            return Err("base backup requires a data directory".into());
        };
        let ci = self.committed_version();
        let manifest = crate::archive::backup_base(&data_dir, out_dir, ci)?;
        // The base artifact is fully published before acknowledging its
        // coverage. Persisting this marker earlier would create a new commit
        // and could make the copied store no longer match the advertised
        // commit index.
        self.acknowledge_retention_consumer("backup", manifest.commit_index)
            .map_err(|error| error.to_string())?;
        if let Some(dir) = &self.wal_archive_dir {
            crate::archive::register_base(dir, manifest.commit_index, crate::retention::unix_ms())?;
        }
        Ok(manifest)
    }

    /// Online physical base backup: the running-server counterpart of the
    /// offline `--backup-base` CLI flag (`POST /internal/backup/base`). The
    /// store files are copied while quiescent at a durable commit (readers
    /// continue, commits wait for the copy), then a `"backup"` retention
    /// consumer is registered at that commit. Combine with
    /// `--wal-archive-dir` for a point-in-time-restorable backup window.
    ///
    /// # Errors
    /// Returns a message when there is no data directory (memory store),
    /// the checkpoint fails, or the copy/manifest write fails.
    pub fn backup_base_online(
        &self,
        out_dir: &std::path::Path,
    ) -> Result<crate::archive::BaseManifest, String> {
        let Some(data_dir) = self.data_dir.clone() else {
            return Err("online base backup requires a data directory".into());
        };
        let manifest = crate::archive::backup_base_online(&self.store, &data_dir, out_dir)?;
        // Registered once the copy is published. While the copy ran,
        // publication (and so every checkpoint and WAL truncation) waited.
        self.acknowledge_retention_consumer("backup", manifest.commit_index)
            .map_err(|error| error.to_string())?;
        if let Some(dir) = &self.wal_archive_dir {
            crate::archive::register_base(dir, manifest.commit_index, crate::retention::unix_ms())?;
        }
        Ok(manifest)
    }

    /// Replay archived WAL segments onto this node up to `to_lsn` (inclusive;
    /// `None` = all). Point-in-time restore after a base backup.
    /// # Errors
    ///
    /// Returns a message when a segment is corrupt or a batch cannot be applied.
    pub fn restore_pitr_to<R: std::io::Read>(
        &self,
        reader: &mut R,
        from_lsn: Option<u64>,
        to_lsn: Option<u64>,
    ) -> Result<u64, String> {
        self.store
            .replay_wal_segments_to(reader, from_lsn, to_lsn)
            .map_err(|e| e.to_string())
    }

    /// Tenants a snapshot exports, with the scope digest framed into each
    /// chunk (portable across stores; ordinals are local to a store).
    fn export_tenants(
        &self,
        snap: &rushdb_storage::ReadView,
        scopes: &[String],
    ) -> Result<Vec<(TenantId, rushdb_types::ids::TenantScope)>, String> {
        if !self.multitenant {
            return Ok(vec![(
                self.base_tenant,
                rushdb_types::ids::TenantScope::single(),
            )]);
        }
        if scopes.is_empty() {
            return Self::enumerate_tenants(snap);
        }
        let mut out = Vec::new();
        for scope in scopes {
            let digest = rushdb_types::ids::TenantScope::of(scope);
            if let Some(tenant) = self
                .store
                .resolve_tenant(&digest)
                .map_err(|e| e.to_string())?
            {
                out.push((tenant, digest));
            }
        }
        Ok(out)
    }

    /// Logical integrity check: scrub every tenant's graph for invariant
    /// violations. Returns the number of violations found.
    /// # Errors
    ///
    /// Returns a message when the store cannot be read.
    pub fn verify(&self) -> Result<usize, String> {
        let view = self.store.read_view().map_err(|e| e.to_string())?;
        let tenants = Self::enumerate_tenants(&view)?;
        let mut violations = 0usize;
        for (tenant, _) in tenants {
            let report =
                rushdb_lmpg::scrub::scrub_report(&view, tenant).map_err(|e| e.to_string())?;
            if !report.complete {
                return Err("scrub did not complete".into());
            }
            violations += report.violations.len();
        }
        Ok(violations)
    }
}
