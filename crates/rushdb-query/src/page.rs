//! Budgeted page reconstruction for the structured query read path.

use super::{
    attach_collects_with_budget, projection::Projection, reconstruct, Cap, Lmpg, QueryError,
    QuerySnapshot, SearchRequest,
};

/// The serialized size of a row: what it costs in the response.
pub(super) fn serialized_bytes(row: &serde_json::Value) -> usize {
    serde_json::to_vec(row).map_or(0, |bytes| bytes.len())
}

/// Reconstruct the page's records in order (or, for a projection, only the
/// projected fields of each), charging each row's real serialized bytes,
/// `$collect` expansion included, cumulatively against the budget, so N page
/// records cannot each retain the full remaining allowance.
///
/// The running total also grows a reservation against the shared graph-phase
/// budget: best-effort attribution, not a new rejection path — only `budget`
/// can fail the page, so a tight shared pool just means the gauge
/// under-reports.
pub(super) fn reconstruct_page_with_budget(
    lmpg: &Lmpg,
    req: &SearchRequest,
    is_cancelled: &dyn Fn() -> bool,
    snap: &QuerySnapshot,
    page_roots: Vec<u64>,
    budget: Option<Cap>,
) -> Result<Vec<serde_json::Value>, QueryError> {
    let mut charged = 0usize;
    let mut reservation = lmpg
        .store
        .graph_phase_budget()
        .and_then(|phase_budget| phase_budget.try_reserve(0).ok());
    let projection = Projection::resolve(snap, lmpg.tenant, &req.projection)?;
    let mut session = reconstruct::ReconstructSession::new(snap, lmpg.tenant);
    let mut records = Vec::with_capacity(page_roots.len());
    for ord in page_roots {
        if is_cancelled() {
            return Err(QueryError::Cancelled);
        }
        let row = match &projection {
            Some(projection) => projection.row(&mut session, snap, lmpg.tenant, ord)?,
            None => session.reconstruct_record(ord)?,
        };
        let Some(row) = row else {
            continue;
        };
        let bytes = budget.map_or(0, |_| serialized_bytes(&row));
        Cap::check_opt(budget, charged.saturating_add(bytes))?;
        // `$collect` expansion spends from what the page has left, so a
        // collect cannot pull an unbounded number of related records into
        // the response; the expanded row then replaces the bare one.
        let (record, bytes) = if req.collects.is_empty() {
            (row, bytes)
        } else {
            let record = attach_collects_with_budget(
                lmpg,
                snap,
                ord,
                row,
                &req.collects,
                budget.map(|cap| cap.after(charged.saturating_add(bytes))),
                is_cancelled,
            )?;
            let expanded = budget.map_or(0, |_| serialized_bytes(&record));
            (record, expanded)
        };
        charged = charged.saturating_add(bytes);
        Cap::check_opt(budget, charged)?;
        if let Some(reservation) = reservation.as_mut() {
            let _ = reservation.resize(charged);
        }
        records.push(record);
    }
    Ok(records)
}
