//! Folding a row from resident columns by code (L6): the group is found
//! from the key cells' raw codes without allocating, and each metric reads
//! its column directly instead of a built row. The values, their order and
//! the arithmetic are those of [`apply`](super::apply), so the groups are
//! bit-identical to folding the built row: a count counts what a built row
//! would hold, a sum adds `Cell::number` of the same cell, an extreme is
//! offered the same cell. Only text cells are never built where their text
//! is not read (a count, a sum).

use std::sync::Arc;

use super::{too_many_groups, CodedPlan, Group, Key, Partial, RowPlan, Source};
use crate::aggregation::MAX_AGGREGATE_BUCKETS;
use crate::columns::{Column, KeyCodeMap, KeyCodes};
use crate::QueryError;

impl Partial {
    /// Fold member `at` of resident `columns` (one per plan slot) in, as
    /// [`Self::accumulate`] folds the row built from the same cells: the
    /// group is found through `group_of_code` (this part's cache from the
    /// key cells' codes to groups). Returns the bytes the partial newly
    /// holds.
    ///
    /// # Errors
    /// Returns the group cap and a lossy integer sum.
    pub(crate) fn accumulate_coded(
        &mut self,
        (plan, coded): (&RowPlan<'_>, &CodedPlan),
        (columns, at): (&[Arc<Column>], usize),
        group_of_code: &mut KeyCodeMap<usize>,
    ) -> Result<usize, QueryError> {
        let mut code: KeyCodes = Default::default();
        for (place, slot) in code.iter_mut().zip(&coded.key_slots) {
            *place = slot.map_or((u8::MAX, 0), |slot| columns[slot].code(at));
        }
        let mut added = 0usize;
        let position = if let Some(position) = group_of_code.get(&code) {
            *position
        } else {
            self.scratch.clear();
            for slot in &coded.key_slots {
                let cell = slot.and_then(|slot| columns[slot].cell(at));
                self.scratch.push(Key::of_group(cell.as_ref()));
            }
            let position = if let Some(position) = self.index.get(self.scratch.as_slice()) {
                *position
            } else {
                if self.groups.len() >= MAX_AGGREGATE_BUCKETS {
                    return Err(too_many_groups());
                }
                let group = Group::new(plan, self.scratch.clone());
                self.index.insert(self.scratch.clone(), self.groups.len());
                added = group.bytes();
                self.groups.push(group);
                self.groups.len() - 1
            };
            group_of_code.insert(code, position);
            position
        };
        let group = &mut self.groups[position];
        added = added.saturating_add(apply_coded(group, plan, columns, at)?);
        Ok(added)
    }
}

/// [`apply`](super::apply) for a coded plan: every metric reads a property
/// of the row, so each reads member `at` of its column.
fn apply_coded(
    group: &mut Group,
    plan: &RowPlan<'_>,
    columns: &[Arc<Column>],
    at: usize,
) -> Result<usize, QueryError> {
    let column = |source: &Source<'_>| match source {
        Source::Slot(field) => field.map(|slot| &*columns[slot]),
        // A coded plan has no alias source.
        Source::Alias(_) => None,
    };
    group.count_roots += 1;
    for (slot, source) in &plan.counts {
        if column(source).is_some_and(|column| column.is_text(at) || column.cell(at).is_some()) {
            group.counts[*slot] += 1;
        }
    }
    for (slot, source) in &plan.sums {
        if let Some(value) = number_at(column(source), at) {
            let sum = &mut group.sums[*slot];
            *sum = Some(sum.unwrap_or(0.0) + value?);
        }
    }
    for (slot, avg, source) in &plan.avgs {
        if let Some(value) = number_at(column(source), at) {
            let sum = &mut group.sums[*slot];
            *sum = Some(sum.unwrap_or(0.0) + value?);
            group.avg_n[*avg] = Some(group.avg_n[*avg].unwrap_or(0) + 1);
        }
    }
    let mut added = 0usize;
    for (extremes, metrics, is_min) in [
        (&mut group.mins, &plan.mins, true),
        (&mut group.maxs, &plan.maxs, false),
    ] {
        for (slot, source) in metrics {
            if let Some(value) = column(source).and_then(|column| column.cell(at)) {
                added += extremes[*slot].offer(&value, is_min);
            }
        }
    }
    Ok(added)
}

/// `Cell::number` of member `at`'s cell; a string is never a number.
fn number_at(column: Option<&Column>, at: usize) -> Option<Result<f64, QueryError>> {
    let column = column?;
    if column.is_text(at) {
        return None;
    }
    column.cell(at)?.number()
}
