//! Resident typed columns: one scalar property of one label's records, held
//! in memory for grouped aggregates (a layer of `rushdb_storage::resident`).
//!
//! A column holds, for every member of the label at one snapshot (ascending
//! ordinals), the cell [`PointFields`](crate::PointFields) reads for it:
//! absent, null, a boolean, an integer, a float, a string or a value of
//! another type (kept by its tagged form). Values are copies of the stored
//! occurrences, decoded by [`Cell::decode`]; nothing is derived or rounded,
//! so an aggregate that reads cells from a column folds the same values in
//! the same order as one that reads them from storage.
//!
//! A column reflects the property's typed definitions (occurrences, array
//! markers), the label's membership and the tenant's definitions; a write to
//! any of them drops it. Properties stored as an array anywhere, system
//! fields, and labels marked suspect by self-heal have no column.

use std::sync::Arc;

use rushdb_storage::resident::{label_hash, Budget, BuildCx, BuildError, Dep, Resident};
use rushdb_storage::{ReadView, ScanBounds, ScanProjection, StorageError};
use rushdb_types::ids::TenantId;

use crate::aggregate_rows::Cell;
use crate::{normalize, QueryError};

/// Hashes the raw cell codes keying a pass's caches (`(tag, value)` pairs
/// the engine made, not client input): one multiply per word, where the
/// default hasher costs more than the work it keys.
#[derive(Default, Clone, Copy)]
pub(crate) struct CodeHasher(u64);

impl CodeHasher {
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

impl std::hash::Hasher for CodeHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.add(u64::from(*byte));
        }
    }

    fn write_u8(&mut self, word: u8) {
        self.add(u64::from(word));
    }

    fn write_u64(&mut self, word: u64) {
        self.add(word);
    }

    fn write_usize(&mut self, word: usize) {
        self.add(word as u64);
    }
}

/// A map keyed by cell codes.
pub(crate) type CodeMap<V> =
    std::collections::HashMap<Vec<(u8, u64)>, V, std::hash::BuildHasherDefault<CodeHasher>>;

/// Group keys a pass folds by code without allocating per row; a grouping
/// on more keys folds built rows.
pub(crate) const KEY_CODES: usize = 4;

/// The codes of a row's group keys (unused places stay `(0, 0)`).
pub(crate) type KeyCodes = [(u8, u64); KEY_CODES];

/// A map keyed by a row's group-key codes.
pub(crate) type KeyCodeMap<V> =
    std::collections::HashMap<KeyCodes, V, std::hash::BuildHasherDefault<CodeHasher>>;

/// One property of one label of one tenant.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ColumnKey {
    pub(crate) tenant: TenantId,
    pub(crate) label: String,
    /// The property name as the request spells it (probed normalized).
    pub(crate) name: String,
}

const ABSENT: u8 = 0;
const NULL: u8 = 1;
const BOOL: u8 = 2;
const INT: u8 = 3;
const FLOAT: u8 = 4;
const STR: u8 = 5;
/// A temporal value, duration or point: its tagged JSON form in `dict`.
const TYPED: u8 = 6;

/// The cells of one property for every member of a label, by ordinal.
#[derive(Debug, PartialEq)]
pub(crate) struct Column {
    ords: Vec<u64>,
    tags: Vec<u8>,
    /// `Bool` 0/1, `Int` as `i64` bits, `Float` as `f64` bits, `Str` as an
    /// index into `dict`.
    vals: Vec<u64>,
    dict: Vec<String>,
}

impl Column {
    /// The cell of member `at` (`None`: the record does not hold it).
    pub(crate) fn cell(&self, at: usize) -> Option<Cell> {
        let val = self.vals[at];
        match self.tags[at] {
            NULL => Some(Cell::Null),
            BOOL => Some(Cell::Bool(val != 0)),
            INT => Some(Cell::Int(i64::from_ne_bytes(val.to_ne_bytes()))),
            FLOAT => Some(Cell::Float(f64::from_bits(val))),
            STR => Some(Cell::Str(
                self.dict[usize::try_from(val).unwrap_or_default()].clone(),
            )),
            TYPED => serde_json::from_str(&self.dict[usize::try_from(val).unwrap_or_default()])
                .ok()
                .map(Cell::from_json),
            _ => None,
        }
    }

    /// The members (ordinals, ascending) whose cells the column holds.
    pub(crate) fn members(&self) -> &[u64] {
        &self.ords
    }

    /// Whether member `at` holds a string (its cell would copy it).
    pub(crate) fn is_text(&self, at: usize) -> bool {
        self.tags[at] == STR
    }

    /// The raw code of member `at`'s cell: equal codes are equal cells.
    pub(crate) fn code(&self, at: usize) -> (u8, u64) {
        (self.tags[at], self.vals[at])
    }

    /// The heap bytes member `at`'s cell holds once built
    /// (`Cell::heap_bytes`), without building it.
    pub(crate) fn heap_bytes(&self, at: usize) -> usize {
        match self.tags[at] {
            STR | TYPED => self.dict[usize::try_from(self.vals[at]).unwrap_or_default()].len(),
            _ => 0,
        }
    }

    /// Whether this column holds the same members as `other`.
    pub(crate) fn same_members(&self, other: &Self) -> bool {
        self.ords == other.ords
    }

    /// The position of `ord`, searching forward from `from` (candidates of
    /// a part ascend, so a cursor rarely moves far).
    pub(crate) fn seek(&self, ord: u64, from: usize) -> Option<usize> {
        let rest = self.ords.get(from..)?;
        // Gallop: most steps are to the next member or a near one.
        let mut step = 1usize;
        let mut low = 0usize;
        while low + step < rest.len() && rest[low + step] < ord {
            low += step;
            step = step.saturating_mul(2);
        }
        let high = (low + step + 1).min(rest.len());
        let found = rest[low..high].partition_point(|&member| member < ord) + low;
        (rest.get(found) == Some(&ord)).then_some(from + found)
    }

    fn push(&mut self, cell: Option<Cell>, strings: &mut std::collections::HashMap<String, u64>) {
        let (tag, val) = match cell {
            // `Cell::decode` never yields JSON; keep the shape exhaustive.
            None | Some(Cell::Json(_)) => (ABSENT, 0),
            Some(Cell::Null) => (NULL, 0),
            Some(Cell::Bool(b)) => (BOOL, u64::from(b)),
            Some(Cell::Int(i)) => (INT, u64::from_ne_bytes(i.to_ne_bytes())),
            Some(Cell::Float(f)) => (FLOAT, f.to_bits()),
            Some(Cell::Str(s)) => (STR, self.intern(s, strings)),
            Some(Cell::Typed(scalar)) => {
                // Tagged JSON never equals a plain string's text: the dict
                // keys stay apart.
                let text = rushdb_types::types::to_doc_json(&scalar).to_string();
                (TYPED, self.intern(text, strings))
            }
        };
        self.tags.push(tag);
        self.vals.push(val);
    }

    fn intern(&mut self, s: String, strings: &mut std::collections::HashMap<String, u64>) -> u64 {
        let next = self.dict.len() as u64;
        *strings.entry(s).or_insert_with_key(|s| {
            self.dict.push(s.clone());
            next
        })
    }

    fn bytes(&self) -> usize {
        self.ords.len() * 8
            + self.tags.len()
            + self.vals.len() * 8
            + self.dict.iter().map(|s| s.len() + 24).sum::<usize>()
    }
}

/// The typed columns of every label's properties.
#[derive(Default)]
pub(crate) struct Columns;

impl Resident for Columns {
    type Key = ColumnKey;
    type Value = Column;
    const NAME: &'static str = "column";
    const BUDGET: Budget = Budget::Columns;

    fn deps(&self, key: &ColumnKey, view: &ReadView) -> Result<Vec<Dep>, StorageError> {
        let mut deps = vec![
            Dep::Label(key.tenant, label_hash(&key.label)),
            Dep::Definitions(key.tenant),
        ];
        let norm = normalize(&key.name);
        if let Some(fid) = crate::planner::family_id(view, key.tenant, &norm).map_err(storage)? {
            for pid in crate::planner::pids_of(view, key.tenant, fid).map_err(storage)? {
                deps.push(Dep::Pid(key.tenant, pid));
            }
        }
        Ok(deps)
    }

    fn build(
        &self,
        key: &ColumnKey,
        view: &ReadView,
        cx: &BuildCx,
    ) -> Result<Option<Column>, BuildError> {
        build(key, view, cx).map_err(|error| match error {
            QueryError::Storage(error) => BuildError::Storage(error),
            QueryError::ResourceLimit(_) => BuildError::Refused,
            other => BuildError::Storage(StorageError::InvalidRead(other.to_string())),
        })
    }

    fn bytes(column: &Column) -> usize {
        column.bytes()
    }
}

fn storage(error: QueryError) -> StorageError {
    match error {
        QueryError::Storage(error) => error,
        other => StorageError::InvalidRead(other.to_string()),
    }
}

/// Rows read between budget checks while a column is built (and between
/// cancellation checks while a predicate is evaluated from columns).
const BUILD_CHECK_ROWS: usize = 65_536;

/// Distinct cell combinations a predicate is evaluated for before it is left
/// to the planner.
const PREDICATE_COMBINATIONS: usize = 4_096;

fn build(key: &ColumnKey, view: &ReadView, cx: &BuildCx) -> Result<Option<Column>, QueryError> {
    if view.derived_health().is_suspect(key.tenant, &key.label) {
        return Ok(None);
    }
    let labels = [key.label.clone()];
    let Some(pids) = crate::point_fields::scalar_pids(view, key.tenant, &labels, &key.name)? else {
        return Ok(None);
    };
    let prefix = rushdb_types::keys::label_member_prefix(key.tenant, &key.label);
    let mut cursor = view.scan(ScanBounds::prefix(&prefix), ScanProjection::Keys)?;
    let mut column = Column {
        ords: Vec::new(),
        tags: Vec::new(),
        vals: Vec::new(),
        dict: Vec::new(),
    };
    let mut strings = std::collections::HashMap::new();
    loop {
        view.check_cancelled()?;
        let batch = cursor.next_batch(4_096, 1 << 20)?;
        for row in batch.rows {
            let (_, ord) = rushdb_types::keys::parse_label_member_key(&row.key)
                .ok_or(StorageError::CorruptLog)?;
            let cell = match crate::point_fields::held(view, key.tenant, ord, &pids)? {
                Some(occ) => Some(Cell::decode(occ.value, &occ.payload, &occ.suffix)?),
                None => None,
            };
            column.ords.push(ord);
            column.push(cell, &mut strings);
            if column.ords.len().is_multiple_of(BUILD_CHECK_ROWS) {
                cx.admit(column.bytes())
                    .map_err(|_| QueryError::ResourceLimit("column exceeds its budget".into()))?;
            }
        }
        if batch.done {
            break;
        }
    }
    cx.admit(column.bytes())
        .map_err(|_| QueryError::ResourceLimit("column exceeds its budget".into()))?;
    Ok(Some(column))
}

/// The properties a pass reads, and their resident columns when every one
/// is resident.
pub(crate) struct Served {
    label: String,
    names: Vec<String>,
    columns: Option<Vec<Arc<Column>>>,
}

impl Served {
    /// What a pass over `labels` reading `fields` can be served: one label,
    /// no traversal branch, stored properties only (not the record id).
    pub(crate) fn lookup(
        view: &ReadView,
        tenant: TenantId,
        (labels, traverses): (&[String], bool),
        fields: Option<&crate::PointFields>,
    ) -> Option<Self> {
        let [label] = labels else {
            return None;
        };
        if traverses {
            return None;
        }
        let names = fields?.property_names()?;
        let columns = resident(view, tenant, label, &names);
        Some(Self {
            label: label.clone(),
            names,
            columns,
        })
    }

    /// The row source, when every column is resident.
    pub(crate) fn row(&self) -> Option<crate::aggregate_scan::ColumnRow<'_>> {
        self.columns
            .as_deref()
            .map(|columns| crate::aggregate_scan::ColumnRow {
                names: &self.names,
                columns,
            })
    }

    /// The members of the label for which `expr` holds, from the resident
    /// columns: each distinct combination of the predicate's cells is
    /// evaluated once, by the evaluator every read path uses, on the row
    /// point reads would build for it. `None` when the columns are not
    /// resident, or when more than [`PREDICATE_COMBINATIONS`] distinct
    /// combinations occur (a filter over many distinct values: the
    /// planner's candidates cost less).
    ///
    /// # Errors
    /// Predicate evaluation errors and cancellation.
    pub(crate) fn candidates(
        &self,
        ctx: &crate::EvalCtx<'_>,
        expr: &crate::Expr,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<Vec<u64>>, QueryError> {
        let Some(columns) = self.columns.as_deref() else {
            return Ok(None);
        };
        let Some(first) = columns.first() else {
            return Ok(None);
        };
        // Columns valid at one view hold the same members.
        if columns.iter().any(|column| column.ords != first.ords) {
            return Ok(None);
        }
        let mut verdicts: CodeMap<bool> = CodeMap::default();
        let mut key = Vec::with_capacity(columns.len());
        let mut matched = Vec::new();
        for (at, &ord) in first.ords.iter().enumerate() {
            if at.is_multiple_of(BUILD_CHECK_ROWS) && is_cancelled() {
                return Err(QueryError::Cancelled);
            }
            key.clear();
            key.extend(
                columns
                    .iter()
                    .map(|column| (column.tags[at], column.vals[at])),
            );
            let holds = if let Some(verdict) = verdicts.get(&key) {
                *verdict
            } else {
                if verdicts.len() >= PREDICATE_COMBINATIONS {
                    return Ok(None);
                }
                let mut row = serde_json::Map::new();
                for (name, column) in self.names.iter().zip(columns) {
                    if let Some(cell) = column.cell(at) {
                        row.insert(name.clone(), cell.to_json());
                    }
                }
                let verdict = crate::eval_expr_ctx(ctx, expr, &row, ord)?;
                verdicts.insert(key.clone(), verdict);
                verdict
            };
            if holds {
                matched.push(ord);
            }
        }
        Ok(Some(matched))
    }

    /// `rows` read from storage pay toward the columns (nothing when they
    /// were resident); `members` is the label's size, one build's cost.
    pub(crate) fn pay(
        &self,
        view: &ReadView,
        tenant: TenantId,
        rows: u64,
        members: impl FnOnce() -> Result<u64, QueryError>,
    ) -> Result<(), QueryError> {
        if self.columns.is_none() && rows > 0 {
            rent(view, tenant, &self.label, &self.names, rows, members()?);
        }
        Ok(())
    }
}

/// The resident columns of `names` for `label` at `view`: all of them, or
/// `None` when one is not resident (the caller reads storage and rents).
fn resident(
    view: &ReadView,
    tenant: TenantId,
    label: &str,
    names: &[String],
) -> Option<Vec<Arc<Column>>> {
    if view.derived_health().is_suspect(tenant, label) {
        return None;
    }
    let layer = view.residents().layer::<Columns>();
    names
        .iter()
        .map(|name| {
            layer.get(
                &ColumnKey {
                    tenant,
                    label: label.to_owned(),
                    name: name.clone(),
                },
                view,
            )
        })
        .collect()
}

/// Report `rows` point reads of each of `names` for `label` toward building
/// its column; one build costs about one read per member.
fn rent(view: &ReadView, tenant: TenantId, label: &str, names: &[String], rows: u64, members: u64) {
    if rows == 0 || members == 0 {
        return;
    }
    let layer = view.residents().layer::<Columns>();
    if !layer.enabled() {
        return;
    }
    for name in names {
        let key = ColumnKey {
            tenant,
            label: label.to_owned(),
            name: name.clone(),
        };
        // A resident column pays nothing (`rent` checks).
        layer.rent(&key, view, rows, members);
    }
}
