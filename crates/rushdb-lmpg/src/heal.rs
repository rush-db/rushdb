//! SELF-HEAL: derived rows are exact by construction; a read that finds them
//! disagreeing with base rows reports it ([`report`]), the label becomes
//! suspect (its reads use base rows only), and [`Lmpg::rederive_label`]
//! repairs it with a logged command every node applies through the deriver.

use rushdb_storage::derived_health::Finding;
use rushdb_types::command::Mutation;
use rushdb_types::ids::TenantId;

use crate::reader::TryRead;
use crate::Lmpg;

/// Records a label, and its tenant, may hold for its repair to run as one
/// commit at any time: the re-derive reads every record of the label (and,
/// for the null counts, every record of the tenant) while the commit holds
/// the writer. A larger repair waits until the store has been quiet for a
/// while, so the commit holds up no writer.
pub const REPAIR_MAX_RECORDS: u64 = 200_000;

/// Report that `label`'s derived rows disagree with its base rows, when `r`
/// reads committed state (a transaction's staged rows are derived as read).
pub fn report<R: TryRead + ?Sized>(
    r: &R,
    tenant: TenantId,
    label: &str,
    kind: &'static str,
    detail: String,
) {
    if let Some(health) = r.derived_health() {
        health.report(Finding {
            tenant,
            label: label.to_owned(),
            kind,
            detail,
        });
    }
}

/// Whether reads of `labels` (every label when empty) must not trust derived
/// rows.
pub fn untrusted<R: TryRead + ?Sized>(r: &R, tenant: TenantId, labels: &[String]) -> bool {
    r.derived_health().is_some_and(|health| {
        if labels.is_empty() {
            health.tenant_suspect(tenant)
        } else {
            labels.iter().any(|label| health.is_suspect(tenant, label))
        }
    })
}

/// Mark `tenant`'s labels with a stored suspect marker suspect in `store`'s
/// registry (after a restart, before their repair).
///
/// # Errors
/// Returns [`rushdb_storage::StorageError`] if the scan fails.
pub fn load_suspects(
    store: &rushdb_storage::Store,
    tenant: TenantId,
) -> Result<usize, rushdb_storage::StorageError> {
    let view = store.read_view()?;
    let health = rushdb_storage::Store::derived_health(store);
    let mut loaded = 0;
    for (key, _) in view.range_prefix(&rushdb_types::keys::rederive_prefix(tenant))? {
        let (_, label) = rushdb_types::keys::parse_rederive_key(&key)
            .ok_or(rushdb_storage::StorageError::CorruptLog)?;
        health.report(Finding {
            tenant,
            label: label.to_owned(),
            kind: "stored_suspect",
            detail: "marked suspect before the last restart".into(),
        });
        loaded += 1;
    }
    Ok(loaded)
}

impl Lmpg {
    /// Store `label`'s suspect marker, so it stays suspect across restarts
    /// and on every replica until its re-derive.
    ///
    /// # Errors
    /// Returns [`crate::LmpgError`] if the commit fails.
    pub fn mark_suspect(&self, label: &str) -> crate::LmpgResult<()> {
        let tenant = self.tenant;
        let mutation = Mutation::MarkSuspect {
            label: label.to_owned(),
        };
        crate::run_mutation(self, None, move |ci, _txn| {
            let mut batch = crate::empty_batch(tenant, ci);
            batch.mutations.push(mutation);
            Ok((batch, ()))
        })
    }

    /// Recompute `label`'s derived record rows from its base rows in one
    /// logged commit, and trust them again.
    ///
    /// # Errors
    /// Returns [`crate::LmpgError`] if the commit fails.
    pub fn rederive_label(&self, label: &str) -> crate::LmpgResult<()> {
        let tenant = self.tenant;
        let mutation = Mutation::RederiveLabel {
            label: label.to_owned(),
        };
        crate::run_mutation(self, None, move |ci, _txn| {
            let mut batch = crate::empty_batch(tenant, ci);
            batch.mutations.push(mutation);
            Ok((batch, ()))
        })?;
        rushdb_storage::Store::derived_health(&self.store).clear(tenant, label);
        Ok(())
    }
}
