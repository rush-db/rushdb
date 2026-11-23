//! Folding a row from resident columns by code (L6): the group is found
//! from the key cells' raw codes without allocating, and each metric reads
//! its column directly instead of a built row. The values, their order and
//! the arithmetic are those of [`apply`](super::apply), so the groups are
//! bit-identical to folding the built row: a count counts what a built row
//! would hold, a sum adds `Cell::number` of the same value, an extreme is
//! offered the same value and compares it as `Extreme::offer` does. Cells
//! are built only where a metric needs one (a text or typed extreme, a
//! typed number).

use std::sync::Arc;

use super::{too_many_groups, Cell, CodedPlan, Extreme, Group, Key, Partial, RowPlan, Source};
use crate::aggregation::MAX_AGGREGATE_BUCKETS;
use crate::columns::{CodeHasher, Column, KeyCodeMap, KeyCodes, Number, DENSE_CODES};
use crate::QueryError;

/// One part's cache from a row's key codes to its group.
///
/// Rows whose keys all have a dense code ([`Column::dense_code`]: absent,
/// null, booleans and strings of the column's dictionary) find their group
/// by a table index (one key) or one packed word (two to four); others by
/// their full codes. A code seen for the first time is resolved through the
/// keys themselves, as before, so group identity does not depend on which
/// cache found it.
#[derive(Default)]
pub(crate) struct GroupCache {
    /// One key: group position + 1 by dense code (0: not seen).
    direct: Vec<u32>,
    /// Two to four keys: groups by packed dense codes.
    packed: std::collections::HashMap<u64, usize, std::hash::BuildHasherDefault<CodeHasher>>,
    /// Any key without a dense code: groups by full codes.
    codes: KeyCodeMap<usize>,
}

/// Bits per dense code in a packed word: four keys fit a `u64`.
const DENSE_BITS: u32 = 16;
const _: () = assert!(DENSE_CODES <= 1 << DENSE_BITS);

impl GroupCache {
    /// The cached group of member `at`, or the key to cache it under.
    fn find(
        &self,
        key_slots: &[Option<usize>],
        columns: &[Arc<Column>],
        at: usize,
    ) -> Result<usize, CacheKey> {
        let dense = |slot: &Option<usize>| slot.and_then(|slot| columns[slot].dense_code(at));
        if let [slot] = key_slots {
            if let Some(code) = dense(slot) {
                return match self.direct.get(code).copied().unwrap_or(0) {
                    0 => Err(CacheKey::Direct(code)),
                    position => Ok(position as usize - 1),
                };
            }
        } else {
            let mut word = 0u64;
            let mut all_dense = true;
            for (place, slot) in key_slots.iter().enumerate() {
                let Some(code) = dense(slot) else {
                    all_dense = false;
                    break;
                };
                // At most four keys reach here.
                let shift = DENSE_BITS * u32::try_from(place).unwrap_or(u32::MAX);
                word |= (code as u64) << shift;
            }
            if all_dense {
                return self
                    .packed
                    .get(&word)
                    .copied()
                    .ok_or(CacheKey::Packed(word));
            }
        }
        let mut code: KeyCodes = Default::default();
        for (place, slot) in code.iter_mut().zip(key_slots) {
            *place = slot.map_or((u8::MAX, 0), |slot| columns[slot].code(at));
        }
        self.codes.get(&code).copied().ok_or(CacheKey::Codes(code))
    }

    fn insert(&mut self, key: CacheKey, position: usize) {
        match key {
            CacheKey::Direct(code) => {
                if self.direct.len() <= code {
                    self.direct.resize(code + 1, 0);
                }
                self.direct[code] = u32::try_from(position + 1).unwrap_or(u32::MAX);
            }
            CacheKey::Packed(word) => {
                self.packed.insert(word, position);
            }
            CacheKey::Codes(code) => {
                self.codes.insert(code, position);
            }
        }
    }
}

/// Where a group not yet cached goes in the [`GroupCache`].
#[derive(Clone, Copy)]
enum CacheKey {
    Direct(usize),
    Packed(u64),
    Codes(KeyCodes),
}

impl Partial {
    /// Fold member `at` of resident `columns` (one per plan slot) in, as
    /// [`Self::accumulate`] folds the row built from the same cells: the
    /// group is found through `cache` (this part's). Returns the bytes the
    /// partial newly holds.
    ///
    /// # Errors
    /// Returns the group cap and a lossy integer sum.
    pub(crate) fn accumulate_coded(
        &mut self,
        (plan, coded): (&RowPlan<'_>, &CodedPlan),
        (columns, at): (&[Arc<Column>], usize),
        cache: &mut GroupCache,
    ) -> Result<usize, QueryError> {
        let mut added = 0usize;
        let position = match cache.find(&coded.key_slots, columns, at) {
            Ok(position) => position,
            Err(key) => {
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
                cache.insert(key, position);
                position
            }
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
        if column(source).is_some_and(|column| column.holds(at)) {
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
            let Some(column) = column(source) else {
                continue;
            };
            if let Some(number) = column.number(at) {
                extremes[*slot].offer_number(number, is_min);
            } else if let Some(value) = column.cell(at) {
                added += extremes[*slot].offer(&value, is_min);
            }
        }
    }
    Ok(added)
}

/// `Cell::number` of member `at`'s cell, building the cell only for a value
/// of another type.
fn number_at(column: Option<&Column>, at: usize) -> Option<Result<f64, QueryError>> {
    let column = column?;
    match column.number(at) {
        Some(Number::Float(float)) => Some(Ok(float)),
        Some(Number::Int(int)) => Cell::Int(int).number(),
        None if column.is_typed(at) => column.cell(at)?.number(),
        None => None,
    }
}

impl Extreme {
    /// [`Extreme::offer`] of an integer or a float: the same first value and
    /// the same comparison (`Cell::ordered`, then the canonical order),
    /// without cloning cells to compare them. Numbers hold no heap bytes.
    fn offer_number(&mut self, number: Number, is_min: bool) {
        let cell = || match number {
            Number::Int(int) => Cell::Int(int),
            Number::Float(float) => Cell::Float(float),
        };
        if self.first.is_none() {
            self.first = Some(cell());
        }
        let next = match number {
            Number::Int(int) => rushdb_types::value::Scalar::Int64(int),
            Number::Float(float) => rushdb_types::value::Scalar::Float64(float),
        };
        let beats = match self.best.as_ref().and_then(Cell::ordered) {
            None => true,
            Some(current) => {
                let wanted = if is_min {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                };
                super::canonical_scalar_cmp(&next, &current) == Some(wanted)
            }
        };
        if beats {
            self.best = Some(cell());
        }
    }
}
