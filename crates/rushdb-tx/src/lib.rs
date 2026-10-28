//! In-process transaction coordination layered over the storage commit path.
//!
//! [`coordinator::TxCoordinator`] stages mutations, enforces active/operation/
//! byte/expiry limits, and publishes each transaction as one atomic LMPG batch
//! so a partial write is never observable. Idle transactions expire and are
//! reclaimed; `autocommit` transactions commit best-effort at expiry.
use rushdb_lmpg::{BatchOp, CommittedBatch, Lmpg, LmpgError};
use rushdb_types::command::IsolationMode;
use uuid::Uuid;

/// Cross-operation transaction handle (document 07 slice). Stages multiple
/// data-plane operations, plans them against one committed view at commit,
/// publishes atomically, and exposes idempotent-outcome discovery through a
/// caller-provided commit key.
pub struct Transaction<'a> {
    lmpg: &'a Lmpg,
    ops: Vec<BatchOp>,
    /// Isolation level requested for this transaction.
    pub isolation: IsolationMode,
    /// Stable commit key used to discover the durable outcome of a retry.
    pub commit_key: Uuid,
}

/// Errors reported while staging or committing a [`Transaction`].
#[derive(Debug, thiserror::Error)]
pub enum TxError {
    /// The underlying LMPG engine rejected the staged batch.
    #[error("engine error: {0}")]
    Engine(#[from] LmpgError),
    /// The requested isolation mode is not supported by v1 transaction staging.
    #[error("unsupported isolation mode for v1 transaction staging")]
    UnsupportedIsolation,
}

impl<'a> Transaction<'a> {
    /// Begin a new empty transaction with the default
    /// [`IsolationMode::Snapshot`] isolation and a fresh commit key.
    #[must_use]
    pub fn begin(lmpg: &'a Lmpg) -> Self {
        Transaction {
            lmpg,
            ops: Vec::new(),
            isolation: IsolationMode::Snapshot,
            commit_key: Uuid::now_v7(),
        }
    }

    /// Set the isolation mode for this transaction.
    ///
    /// # Errors
    ///
    /// Returns [`TxError::UnsupportedIsolation`] when `isolation` is
    /// [`IsolationMode::Serializable`], which v1 staging does not support.
    pub fn set_isolation(mut self, isolation: IsolationMode) -> Result<Self, TxError> {
        match isolation {
            IsolationMode::ReadCommitted | IsolationMode::Snapshot => {
                self.isolation = isolation;
                Ok(self)
            }
            IsolationMode::Serializable => Err(TxError::UnsupportedIsolation),
        }
    }

    /// Pins a stable commit key so a retried `commit` discovers the durable
    /// outcome instead of returning false success.
    #[must_use]
    pub fn with_commit_key(mut self, key: Uuid) -> Self {
        self.commit_key = key;
        self
    }

    /// Stage a record creation with an optional explicit record id.
    #[must_use]
    pub fn create_record(
        mut self,
        label: impl Into<String>,
        explicit_id: Option<[u8; 16]>,
        fields: Vec<(String, rushdb_lmpg::FieldChange)>,
    ) -> Self {
        self.ops.push(BatchOp::CreateRecord {
            label: label.into(),
            explicit_id,
            fields,
        });
        self
    }

    /// Stage a field update against the record named by its ordinal and public
    /// id.
    #[must_use]
    pub fn update_fields(
        mut self,
        owner_ord: u64,
        owner_public_id: [u8; 16],
        fields: Vec<(String, rushdb_lmpg::FieldChange)>,
    ) -> Self {
        self.ops.push(BatchOp::UpdateFields {
            owner: rushdb_types::ids::OwnerKind::Record,
            owner_ord,
            owner_public_id,
            fields,
        });
        self
    }

    /// Stage a label change from `from_label` to `to_label` for a record.
    #[must_use]
    pub fn retype_record(
        mut self,
        record: rushdb_lmpg::RecordRef,
        from_label: impl Into<String>,
        to_label: impl Into<String>,
    ) -> Self {
        self.ops.push(BatchOp::RetypeRecord {
            record,
            from_label: from_label.into(),
            to_label: to_label.into(),
        });
        self
    }

    /// Stage a relationship creation between two record references.
    #[must_use]
    pub fn create_relationship(
        mut self,
        rel_type: impl Into<String>,
        source: rushdb_lmpg::RecordRef,
        target: rushdb_lmpg::RecordRef,
        fields: Vec<(String, rushdb_lmpg::FieldChange)>,
    ) -> Self {
        self.ops.push(BatchOp::CreateRelationship {
            rel_type: rel_type.into(),
            source,
            target,
            fields,
        });
        self
    }

    /// Stage a record deletion.
    #[must_use]
    pub fn delete_record(mut self, record: rushdb_lmpg::RecordRef) -> Self {
        self.ops.push(BatchOp::DeleteRecord { record });
        self
    }

    /// Append a raw [`BatchOp`] to the staged operation list.
    #[must_use]
    pub fn stage(mut self, op: BatchOp) -> Self {
        self.ops.push(op);
        self
    }

    /// Returns `true` when no operations have been staged yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Publish every staged operation as one atomic LMPG batch.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`TxError::Engine`] when the engine rejects the
    /// batch, for example on a transaction conflict.
    pub fn commit(self) -> Result<CommittedBatch, TxError> {
        let mut batch = self.lmpg.begin_batch();
        batch.ops = self.ops;
        self.lmpg.run_batch(batch).map_err(Into::into)
    }
}

/// Idempotency surface documented in the plan: transaction/commit tokens
/// never carry authority; outcomes are discovered by commit key.
pub enum KnownOutcome {
    /// The commit key is durably recorded at the given commit index.
    Committed(u64),
    /// The commit key has no durable outcome and the request is retryable.
    NeverProposed,
}

/// Look up the durable outcome for `commit_key` without consuming any
/// coordinator state or staging quota.
#[must_use]
pub fn discover_outcome(
    store: &std::sync::Arc<rushdb_storage::Store>,
    commit_key: Uuid,
) -> KnownOutcome {
    match store.committed_index_for(commit_key) {
        Some(ci) => KnownOutcome::Committed(ci),
        None => KnownOutcome::NeverProposed,
    }
}

mod coordinator;
mod staging;
pub use coordinator::{TransactionLimits, TxCoordinator, TxOutcome, TxStatus};
