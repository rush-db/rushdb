//! Typed rows and per-group accumulators for aggregation.
//!
//! A row is the properties an aggregate reads of one record, decoded once
//! from storage into [`Cell`]s. A group is found by its typed key tuple and
//! each metric accumulates into a typed slot; a group becomes JSON once, when
//! the aggregate finishes. A [`Partial`] covers one part of the candidates
//! and merges into the partial of the parts before it, so the groups, their
//! order of first appearance, counts, minima and maxima are what one pass in
//! part order gives. Float sums are summed per part, then the part sums in
//! part order.

use std::collections::{BTreeSet, HashMap};

use rushdb_types::encoding::decode_order_preserving;
use rushdb_types::occurrence::OccurrenceValue;

use crate::aggregate_catalog::too_many_groups;
use crate::aggregation::{exact_f64, MAX_AGGREGATE_BUCKETS};
use crate::{
    alias_field_values, canonical_scalar_cmp, count_related, normalize, AggregateRequest, Budget,
    GroupBucket, Json, Lmpg, Map, OutputExpr, QueryError, QuerySnapshot, RelatedSpec, Scalar,
    MISSING_SENTINEL,
};

/// One property value of a row.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Cell {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    /// What else a rebuilt record carries: an array, an object.
    Json(Json),
}

impl Cell {
    /// The value of a stored occurrence, as a rebuilt record carries it
    /// (`DateTime` as its epoch milliseconds; a non-finite float as null).
    pub(crate) fn decode(value: OccurrenceValue, payload: &[u8]) -> Result<Self, QueryError> {
        Ok(match value {
            OccurrenceValue::ExplicitNull | OccurrenceValue::EmptyArray => Self::Null,
            OccurrenceValue::Stored { ty } => match decode_order_preserving(ty, payload)
                .map_err(|_| rushdb_storage::StorageError::CorruptLog)?
            {
                Scalar::Boolean(b) => Self::Bool(b),
                Scalar::Int64(i) | Scalar::DateTime(i) => Self::Int(i),
                Scalar::Float64(f) if f.is_finite() => Self::Float(f),
                Scalar::Float64(_) => Self::Null,
                Scalar::Utf8(s) => Self::Str(s),
            },
        })
    }

    pub(crate) fn from_json(value: Json) -> Self {
        match value {
            Json::Null => Self::Null,
            Json::Bool(b) => Self::Bool(b),
            Json::Number(n) => match (n.as_i64(), n.is_f64()) {
                (Some(i), _) => Self::Int(i),
                (None, true) => Self::Float(n.as_f64().unwrap_or_default()),
                (None, false) => Self::Json(Json::Number(n)),
            },
            Json::String(s) => Self::Str(s),
            other => Self::Json(other),
        }
    }

    pub(crate) fn to_json(&self) -> Json {
        match self {
            Self::Null => Json::Null,
            Self::Bool(b) => Json::Bool(*b),
            Self::Int(i) => Json::Number((*i).into()),
            Self::Float(f) => serde_json::Number::from_f64(*f).map_or(Json::Null, Json::Number),
            Self::Str(s) => Json::String(s.clone()),
            Self::Json(v) => v.clone(),
        }
    }

    /// The canonical number, for `$min`/`$max` (`None`: not a number).
    fn numeric(&self) -> Option<Scalar> {
        match self {
            Self::Int(i) => Some(Scalar::Int64(*i)),
            Self::Float(f) => Some(Scalar::Float64(*f)),
            _ => None,
        }
    }

    /// `None` when not a number; an error for an integer `f64` cannot hold
    /// exactly.
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "the integer round-trip check rejects lossy widening"
    )]
    fn number(&self) -> Option<Result<f64, QueryError>> {
        let exact = |float: Option<f64>| float.ok_or_else(lossy);
        match self {
            Self::Int(i) => {
                let float = *i as f64;
                Some(exact(((float as i128) == i128::from(*i)).then_some(float)))
            }
            Self::Float(f) => Some(Ok(*f)),
            Self::Json(value @ Json::Number(_)) => Some(exact(exact_f64(value))),
            _ => None,
        }
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "a time bucket floors epoch milliseconds as JSON's as_f64 does"
    )]
    fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Int(i) => Some(*i as f64),
            Self::Float(f) => Some(*f),
            Self::Json(value) => value.as_f64(),
            _ => None,
        }
    }

    /// Bytes held beside the cell itself.
    fn heap_bytes(&self) -> usize {
        match self {
            Self::Str(s) => s.len(),
            Self::Json(value) => crate::aggregate_budget::value_bytes(value),
            _ => 0,
        }
    }
}

/// Bytes of one row of cells.
pub(crate) fn row_bytes(row: &[Option<Cell>]) -> usize {
    row.iter().fold(0usize, |sum, cell| {
        sum.saturating_add(std::mem::size_of::<Option<Cell>>())
            .saturating_add(cell.as_ref().map_or(0, Cell::heap_bytes))
    })
}

/// One position of a group key. Two keys are equal exactly when their JSON
/// renderings are (integer `15` and float `15.0` are different groups).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Key {
    /// A per-root row's projected field the record does not hold.
    Absent,
    Null,
    Bool(bool),
    UInt(u64),
    Int(i64),
    /// A float by its bits (`-0.0` renders apart from `0.0`).
    Float(u64),
    Str(String),
    /// Anything else, by its JSON rendering.
    Other(String),
}

impl Key {
    fn of(cell: &Cell) -> Self {
        match cell {
            Cell::Null => Self::Null,
            Cell::Bool(b) => Self::Bool(*b),
            Cell::Int(i) => u64::try_from(*i).map_or(Self::Int(*i), Self::UInt),
            Cell::Float(f) => Self::Float(f.to_bits()),
            Cell::Str(s) => Self::Str(s.clone()),
            Cell::Json(value) => Self::Other(value.to_string()),
        }
    }

    /// A missing group field keys as the documented sentinel.
    fn of_group(cell: Option<&Cell>) -> Self {
        cell.map_or_else(|| Self::Str(MISSING_SENTINEL.into()), Self::of)
    }

    fn bytes(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(match self {
            Self::Str(s) | Self::Other(s) => s.len(),
            _ => 0,
        })
    }

    fn into_json(self) -> Option<Json> {
        Some(match self {
            Self::Absent => return None,
            Self::Null => Json::Null,
            Self::Bool(b) => Json::Bool(b),
            Self::UInt(u) => Json::Number(u.into()),
            Self::Int(i) => Json::Number(i.into()),
            Self::Float(bits) => {
                serde_json::Number::from_f64(f64::from_bits(bits)).map_or(Json::Null, Json::Number)
            }
            Self::Str(s) => Json::String(s),
            Self::Other(s) => serde_json::from_str(&s).unwrap_or(Json::Null),
        })
    }
}

/// Where a group key position comes from.
enum KeySource {
    /// A `groupBy` field (missing keys as the sentinel).
    Group(Option<usize>),
    /// A per-root row's projected field (missing is left out).
    Plain(Option<usize>),
    /// The time bucket of a field.
    TimeBucket(Option<usize>),
    /// A related record's field through a traversal alias (`"$c.city"`):
    /// its value, or the values of every bound record as an array. Missing
    /// keys as the sentinel when grouping and is left out of a per-root row.
    Alias { field: String, group: bool },
}

/// Where a metric reads its values.
enum Source<'r> {
    /// A property of the row.
    Slot(Option<usize>),
    /// A traversal alias's related records (`$count: "$battle"`,
    /// `$sum: "$employee.salary"`).
    Alias(&'r str),
}

/// What one aggregate reads per row and where each metric accumulates.
pub(crate) struct RowPlan<'r> {
    req: &'r AggregateRequest,
    /// The properties read per row, as the request spells them; a row's
    /// cells follow this order.
    pub(crate) names: Vec<String>,
    keys: Vec<(String, KeySource)>,
    per_row: bool,
    alias_specs: Vec<RelatedSpec>,
    counts: Vec<(usize, Source<'r>)>,
    count_names: Vec<String>,
    sums: Vec<(usize, Source<'r>)>,
    expr_sums: Vec<(usize, &'r OutputExpr)>,
    expr_avgs: Vec<(usize, usize, &'r OutputExpr)>,
    avgs: Vec<(usize, usize, Source<'r>)>,
    /// Sum slots by output name: `$sum`, `$avg` and expression outputs
    /// sharing a name share a slot.
    sum_names: Vec<String>,
    avg_names: Vec<String>,
    mins: Vec<(usize, Source<'r>)>,
    min_names: Vec<String>,
    maxs: Vec<(usize, Source<'r>)>,
    max_names: Vec<String>,
}

/// The cell at `slot` of `row`, when the record holds it.
fn cell(row: &[Option<Cell>], slot: Option<usize>) -> Option<&Cell> {
    slot.and_then(|slot| row.get(slot)).and_then(Option::as_ref)
}

fn slot_of(names: &mut Vec<String>, name: &str) -> usize {
    names.iter().position(|n| n == name).unwrap_or_else(|| {
        names.push(name.to_owned());
        names.len() - 1
    })
}

impl<'r> RowPlan<'r> {
    #[expect(
        clippy::too_many_lines,
        reason = "one slot table per metric kind, built side by side"
    )]
    pub(crate) fn new(req: &'r AggregateRequest, alias_specs: Vec<RelatedSpec>) -> Self {
        let names: Vec<String> = referenced_fields(req).into_iter().collect();
        let field = |name: &str| {
            let name = name.strip_prefix("$record.").unwrap_or(name);
            names.iter().position(|n| n == name)
        };
        let is_alias = |name: &str| {
            alias_specs.iter().any(|s| {
                s.alias
                    .as_deref()
                    .is_some_and(|a| a.trim_start_matches('$') == name)
            })
        };
        let source = |name: &'r str| match name.split_once('.') {
            Some((alias, _)) if is_alias(alias) => Source::Alias(name),
            _ => Source::Slot(field(name)),
        };
        let per_row =
            req.group_by.is_empty() && (!req.plain_fields.is_empty() || !req.collects.is_empty());
        let mut keys = Vec::new();
        if per_row {
            for (output, name) in &req.plain_fields {
                let source = match name.split_once('.') {
                    Some((alias, _)) if is_alias(alias) => KeySource::Alias {
                        field: name.clone(),
                        group: false,
                    },
                    _ => KeySource::Plain(field(name)),
                };
                keys.push((output.clone(), source));
            }
        }
        for g in &req.group_by {
            // A group-by entry naming the time bucket is keyed by it; an
            // alias field is keyed by its property.
            match g.split_once('.') {
                Some((alias, prop)) if is_alias(alias) => keys.push((
                    prop.to_owned(),
                    KeySource::Alias {
                        field: g.clone(),
                        group: true,
                    },
                )),
                _ if req.time_bucket_alias.as_deref() == Some(g.as_str()) => {}
                _ => keys.push((g.clone(), KeySource::Group(field(g)))),
            }
        }
        if let Some(tb) = &req.time_bucket {
            // Output naming honors the `as` alias; the default key is the
            // documented `__bucket_<field>/<unit>` form.
            let name = req.time_bucket_alias.clone().unwrap_or_else(|| {
                format!("__bucket_{}/{}", normalize(&tb.field), tb.unit.unit_str())
            });
            keys.push((name, KeySource::TimeBucket(field(&tb.field))));
        }
        let mut count_names = Vec::new();
        let counts = req
            .counts
            .iter()
            .filter(|(out, f)| f != "*" && !req.hidden_outputs.contains(out))
            .map(|(out, f)| {
                let source = if is_alias(f) {
                    Source::Alias(f.as_str())
                } else {
                    Source::Slot(field(f))
                };
                (slot_of(&mut count_names, out), source)
            })
            .collect();
        let (mut sum_names, mut avg_names) = (Vec::new(), Vec::new());
        let sums = req
            .sums
            .iter()
            .map(|(out, f)| (slot_of(&mut sum_names, out), source(f)))
            .collect();
        let expr_sums = req
            .expr_sums
            .iter()
            .map(|(out, e)| (slot_of(&mut sum_names, out), e))
            .collect();
        let expr_avgs = req
            .expr_avgs
            .iter()
            .map(|(out, e)| {
                (
                    slot_of(&mut sum_names, out),
                    slot_of(&mut avg_names, out),
                    e,
                )
            })
            .collect();
        let avgs = req
            .avg_terms
            .iter()
            .map(|(out, f)| {
                (
                    slot_of(&mut sum_names, out),
                    slot_of(&mut avg_names, out),
                    source(f),
                )
            })
            .collect();
        let (mut min_names, mut max_names) = (Vec::new(), Vec::new());
        let mins = req
            .mins
            .iter()
            .map(|(out, f)| (slot_of(&mut min_names, out), source(f)))
            .collect();
        let maxs = req
            .maxs
            .iter()
            .map(|(out, f)| (slot_of(&mut max_names, out), source(f)))
            .collect();
        Self {
            req,
            names,
            keys,
            per_row,
            alias_specs,
            counts,
            count_names,
            sums,
            expr_sums,
            expr_avgs,
            avgs,
            sum_names,
            avg_names,
            mins,
            min_names,
            maxs,
            max_names,
        }
    }

    /// Each matched root is its own row (a `select` of root fields or
    /// collects beside metrics, without `groupBy`).
    pub(crate) fn per_row(&self) -> bool {
        self.per_row
    }

    /// Metrics a row feeds (the alias budget is shared among them).
    pub(crate) fn metric_count(&self) -> usize {
        let req = self.req;
        [
            req.counts.len(),
            req.sums.len(),
            req.mins.len(),
            req.maxs.len(),
            req.expr_avgs.len(),
            req.expr_sums.len(),
            req.avg_terms.len(),
            req.group_by.len(),
            req.plain_fields.len(),
        ]
        .into_iter()
        .fold(0usize, usize::saturating_add)
    }

    /// A per-row math expression over the row's numbers (`None`: a field is
    /// missing, not a number or not exactly an `f64`, or a division by zero).
    fn eval(&self, expr: &OutputExpr, row: &[Option<Cell>]) -> Option<f64> {
        let number = |name: &str| {
            let slot = self.names.iter().position(|n| n == name);
            cell(row, slot).and_then(Cell::number).and_then(Result::ok)
        };
        match expr {
            OutputExpr::Literal(l) => Some(*l),
            OutputExpr::Field(name) => number(name.strip_prefix("$record.").unwrap_or(name)),
            OutputExpr::Ref(name) => number(&normalize(name)),
            OutputExpr::Add(a, b) => Some(self.eval(a, row)? + self.eval(b, row)?),
            OutputExpr::Subtract(a, b) => Some(self.eval(a, row)? - self.eval(b, row)?),
            OutputExpr::Multiply(a, b) => Some(self.eval(a, row)? * self.eval(b, row)?),
            OutputExpr::Divide {
                dividend, divisor, ..
            } => {
                let d = self.eval(dividend, row)?;
                let q = self.eval(divisor, row)?;
                (q != 0.0).then(|| d / q)
            }
        }
    }
}

/// The record properties an aggregate reads per row, as the request spells
/// them (without the `$record.` prefix): group keys, the time-bucket field,
/// counted, summed, averaged and min/max fields, plain projections and the
/// fields of per-row expressions (a `$ref` by its normalized name). Alias
/// metrics (`$sum: "$employee.salary"`) read related records through their
/// traversal, not the row, so their names resolve to nothing here.
fn referenced_fields(req: &AggregateRequest) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut add = |field: &str| {
        names.insert(field.strip_prefix("$record.").unwrap_or(field).to_owned());
    };
    for g in &req.group_by {
        if req.time_bucket_alias.as_deref() != Some(g.as_str()) {
            add(g);
        }
    }
    if let Some(tb) = &req.time_bucket {
        add(&tb.field);
    }
    for (_, field) in req
        .counts
        .iter()
        .filter(|(_, field)| field != "*")
        .chain(&req.sums)
        .chain(&req.mins)
        .chain(&req.maxs)
        .chain(&req.avg_terms)
        .chain(&req.plain_fields)
    {
        add(field);
    }
    for (_, expr) in req.expr_sums.iter().chain(&req.expr_avgs) {
        expr_fields(expr, &mut add);
    }
    names
}

fn expr_fields(expr: &OutputExpr, add: &mut impl FnMut(&str)) {
    match expr {
        OutputExpr::Field(name) => add(name),
        OutputExpr::Ref(name) => add(&normalize(name)),
        OutputExpr::Add(a, b) | OutputExpr::Subtract(a, b) | OutputExpr::Multiply(a, b) => {
            expr_fields(a, add);
            expr_fields(b, add);
        }
        OutputExpr::Divide {
            dividend, divisor, ..
        } => {
            expr_fields(dividend, add);
            expr_fields(divisor, add);
        }
        OutputExpr::Literal(_) => {}
    }
}

/// A `$min`/`$max`: the first value seen sets the metric; when it is a
/// number, later numbers replace it when strictly smaller (larger), and
/// other values are ignored; a first value that is not a number stays.
#[derive(Debug, Clone, Default)]
pub(crate) struct Extreme {
    first: Option<Cell>,
    best: Option<Cell>,
}

impl Extreme {
    /// Bytes the extreme newly holds.
    pub(crate) fn offer(&mut self, value: &Cell, is_min: bool) -> usize {
        let mut added = 0;
        if self.first.is_none() {
            added = value.heap_bytes();
            self.first = Some(value.clone());
        }
        if Self::beats(Some(value), self.best.as_ref(), is_min) {
            self.best = Some(value.clone());
        }
        added
    }

    fn beats(next: Option<&Cell>, current: Option<&Cell>, is_min: bool) -> bool {
        let Some(next) = next.and_then(Cell::numeric) else {
            return false;
        };
        let Some(current) = current.and_then(Cell::numeric) else {
            return true;
        };
        let wanted = if is_min {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        };
        canonical_scalar_cmp(&next, &current) == Some(wanted)
    }

    fn absorb(&mut self, later: Self, is_min: bool) {
        if self.first.is_none() {
            self.first = later.first;
        }
        if Self::beats(later.best.as_ref(), self.best.as_ref(), is_min) {
            self.best = later.best;
        }
    }

    /// The metric's value: an integer `f64` holds exactly renders as a
    /// float, as every other number of the metric does.
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "the paired integer round-trip check preserves exact large integers"
    )]
    pub(crate) fn render(&self) -> Option<Json> {
        let first = self.first.as_ref()?;
        if first.numeric().is_none() {
            return Some(first.to_json());
        }
        Some(match self.best.as_ref()? {
            Cell::Int(i) => {
                let float = *i as f64;
                if (float as i128) == i128::from(*i) {
                    serde_json::Number::from_f64(float)
                        .map_or_else(|| Json::Number((*i).into()), Json::Number)
                } else {
                    Json::Number((*i).into())
                }
            }
            other => other.to_json(),
        })
    }
}

/// One group's accumulators.
struct Group {
    key: Vec<Key>,
    root: Option<u64>,
    count_roots: u64,
    counts: Vec<u64>,
    sums: Vec<Option<f64>>,
    avg_n: Vec<Option<u64>>,
    mins: Vec<Extreme>,
    maxs: Vec<Extreme>,
}

impl Group {
    fn new(plan: &RowPlan<'_>, key: Vec<Key>) -> Self {
        Self {
            key,
            root: None,
            count_roots: 0,
            counts: vec![0; plan.count_names.len()],
            sums: vec![None; plan.sum_names.len()],
            avg_n: vec![None; plan.avg_names.len()],
            mins: vec![Extreme::default(); plan.min_names.len()],
            maxs: vec![Extreme::default(); plan.max_names.len()],
        }
    }

    fn bytes(&self) -> usize {
        let slots = self.counts.len() * 8
            + self.sums.len() * std::mem::size_of::<Option<f64>>()
            + self.avg_n.len() * std::mem::size_of::<Option<u64>>()
            + (self.mins.len() + self.maxs.len()) * std::mem::size_of::<Extreme>();
        self.key
            .iter()
            .fold(std::mem::size_of::<Self>() + slots, |sum, key| {
                sum.saturating_add(key.bytes())
            })
    }

    fn absorb(&mut self, later: Self) {
        self.count_roots += later.count_roots;
        for (a, b) in self.counts.iter_mut().zip(later.counts) {
            *a += b;
        }
        for (a, b) in self.sums.iter_mut().zip(later.sums) {
            *a = match (*a, b) {
                (Some(a), Some(b)) => Some(a + b),
                (a, b) => a.or(b),
            };
        }
        for (a, b) in self.avg_n.iter_mut().zip(later.avg_n) {
            *a = match (*a, b) {
                (Some(a), Some(b)) => Some(a + b),
                (a, b) => a.or(b),
            };
        }
        for (a, b) in self.mins.iter_mut().zip(later.mins) {
            a.absorb(b, true);
        }
        for (a, b) in self.maxs.iter_mut().zip(later.maxs) {
            a.absorb(b, false);
        }
    }
}

/// What a row's alias metrics read with.
pub(crate) struct RowCtx<'a> {
    pub(crate) lmpg: &'a Lmpg,
    pub(crate) snap: &'a QuerySnapshot,
    pub(crate) ord: u64,
    pub(crate) alias_budget: Option<usize>,
}

impl RowCtx<'_> {
    fn related(&self, plan: &RowPlan<'_>, alias: &str) -> Result<Option<Vec<u64>>, QueryError> {
        count_related(
            self.lmpg,
            self.snap,
            self.lmpg.tenant,
            self.ord,
            &plan.alias_specs,
            alias,
            self.alias_budget,
        )
    }

    fn values(&self, plan: &RowPlan<'_>, field: &str) -> Result<Option<Vec<Json>>, QueryError> {
        alias_field_values(
            self.lmpg,
            self.snap,
            self.lmpg.tenant,
            self.ord,
            &plan.alias_specs,
            field,
            self.alias_budget,
        )
    }
}

fn lossy() -> QueryError {
    QueryError::InvalidPredicate("aggregate integer is not exactly representable as f64".into())
}

/// The sum of an alias's numeric values (other values are skipped) and how
/// many there were.
fn sum_numbers(values: &[Json], start: f64) -> Result<(f64, u64), QueryError> {
    let mut sum = start;
    let mut n = 0u64;
    for value in values.iter().filter(|v| v.is_number()) {
        sum += exact_f64(value).ok_or_else(lossy)?;
        n += 1;
    }
    Ok((sum, n))
}

/// The key of an alias field's `values` across a root's bound records: the
/// value of one, the array of several (an array keys as its JSON text, as a
/// stored array does), and when none holds it the missing sentinel
/// (grouping) or nothing (a per-root field).
fn alias_key(mut values: Vec<Json>, group: bool) -> Key {
    let value = match values.len() {
        0 if group => return Key::Str(MISSING_SENTINEL.into()),
        0 => return Key::Absent,
        1 => values.remove(0),
        _ => Json::Array(values),
    };
    match &value {
        Json::Null => Key::Null,
        Json::Bool(b) => Key::Bool(*b),
        Json::String(s) => Key::Str(s.clone()),
        Json::Number(n) => n
            .as_u64()
            .map(Key::UInt)
            .or_else(|| n.as_i64().map(Key::Int))
            .or_else(|| n.as_f64().map(|f| Key::Float(f.to_bits())))
            .unwrap_or_else(|| Key::Other(value.to_string())),
        _ => Key::Other(value.to_string()),
    }
}

/// The groups of one part of the candidates (or of all of them).
#[derive(Default)]
pub(crate) struct Partial {
    groups: Vec<Group>,
    index: HashMap<Vec<Key>, usize>,
    scratch: Vec<Key>,
}

impl Partial {
    /// Fold one row (record `ctx.ord`) in; returns the bytes the partial
    /// newly holds.
    ///
    /// # Errors
    /// Returns the group cap, a lossy integer sum, alias read and budget
    /// errors.
    pub(crate) fn accumulate(
        &mut self,
        plan: &RowPlan<'_>,
        row: &[Option<Cell>],
        ctx: &RowCtx<'_>,
    ) -> Result<usize, QueryError> {
        self.scratch.clear();
        for (_, source) in &plan.keys {
            self.scratch.push(match source {
                KeySource::Group(slot) => Key::of_group(cell(row, *slot)),
                KeySource::Plain(slot) => cell(row, *slot).map_or(Key::Absent, Key::of),
                KeySource::TimeBucket(slot) => {
                    let tb = plan.req.time_bucket.as_ref();
                    match (tb, cell(row, *slot).and_then(Cell::as_f64)) {
                        (Some(tb), Some(ms)) => Key::UInt(bucket_key(tb.floor_millis(ms))),
                        _ => Key::Str(MISSING_SENTINEL.into()),
                    }
                }
                KeySource::Alias { field, group } => {
                    alias_key(ctx.values(plan, field)?.unwrap_or_default(), *group)
                }
            });
        }
        let mut added = 0usize;
        let position = if plan.per_row {
            None
        } else {
            self.index.get(self.scratch.as_slice()).copied()
        };
        let position = if let Some(position) = position {
            position
        } else {
            if !plan.per_row && self.groups.len() >= MAX_AGGREGATE_BUCKETS {
                return Err(too_many_groups());
            }
            let mut group = Group::new(plan, self.scratch.clone());
            if plan.per_row {
                group.root = Some(ctx.ord);
            } else {
                self.index.insert(self.scratch.clone(), self.groups.len());
            }
            added = group.bytes();
            self.groups.push(group);
            self.groups.len() - 1
        };
        let group = &mut self.groups[position];
        added = added.saturating_add(apply(group, plan, row, ctx)?);
        Ok(added)
    }

    /// Merge the partial of the parts after this one.
    ///
    /// # Errors
    /// Returns the group cap.
    pub(crate) fn merge(&mut self, later: Self) -> Result<(), QueryError> {
        for group in later.groups {
            if let Some(&position) = self.index.get(&group.key) {
                self.groups[position].absorb(group);
            } else {
                if self.groups.len() >= MAX_AGGREGATE_BUCKETS {
                    return Err(too_many_groups());
                }
                self.index.insert(group.key.clone(), self.groups.len());
                self.groups.push(group);
            }
        }
        Ok(())
    }

    /// The groups as buckets, in order of first appearance.
    pub(crate) fn finish(self, plan: &RowPlan<'_>) -> Vec<GroupBucket> {
        self.groups
            .into_iter()
            .map(|group| {
                let mut bucket = GroupBucket {
                    count_roots: group.count_roots,
                    root: group.root,
                    ..GroupBucket::default()
                };
                for ((name, _), key) in plan.keys.iter().zip(group.key) {
                    if let Some(value) = key.into_json() {
                        bucket.keys.insert(name.clone(), value);
                    }
                }
                for (name, n) in plan.count_names.iter().zip(group.counts) {
                    if n > 0 {
                        bucket.counts.insert(name.clone(), Json::Number(n.into()));
                    }
                }
                for (name, n) in plan.avg_names.iter().zip(group.avg_n) {
                    if let Some(n) = n {
                        bucket
                            .counts
                            .insert(format!("__avg_n_{name}"), Json::Number(n.into()));
                    }
                }
                for (name, sum) in plan.sum_names.iter().zip(group.sums) {
                    if let Some(sum) = sum {
                        bucket.sums.insert(name.clone(), serde_json::json!(sum));
                    }
                }
                render_extremes(&mut bucket.mins, &plan.min_names, &group.mins);
                render_extremes(&mut bucket.maxs, &plan.max_names, &group.maxs);
                bucket
            })
            .collect()
    }
}

#[expect(
    clippy::cast_sign_loss,
    reason = "the bucket key is the floored epoch-ms as an unsigned JSON integer, as it always was"
)]
fn bucket_key(floor: i64) -> u64 {
    floor as u64
}

fn render_extremes(out: &mut Map<String, Json>, names: &[String], extremes: &[Extreme]) {
    for (name, extreme) in names.iter().zip(extremes) {
        if let Some(value) = extreme.render() {
            out.insert(name.clone(), value);
        }
    }
}

/// Feed one row's metrics to its group; returns the bytes newly held.
fn apply(
    group: &mut Group,
    plan: &RowPlan<'_>,
    row: &[Option<Cell>],
    ctx: &RowCtx<'_>,
) -> Result<usize, QueryError> {
    // `$count: "*"` counts distinct roots; each row is one root.
    group.count_roots += 1;
    for (slot, source) in &plan.counts {
        match source {
            Source::Alias(alias) => {
                if let Some(related) = ctx.related(plan, alias)? {
                    Budget::WorkingSet.check(ctx.alias_budget, related.len().saturating_mul(64))?;
                    group.counts[*slot] += related.len() as u64;
                }
            }
            Source::Slot(field) => {
                if cell(row, *field).is_some() {
                    group.counts[*slot] += 1;
                }
            }
        }
    }
    for (slot, source) in &plan.sums {
        let sum = &mut group.sums[*slot];
        match source {
            Source::Alias(field) => {
                if let Some(values) = ctx.values(plan, field)? {
                    *sum = Some(sum_numbers(&values, sum.unwrap_or(0.0))?.0);
                }
            }
            Source::Slot(field) => {
                if let Some(value) = cell(row, *field).and_then(Cell::number) {
                    *sum = Some(sum.unwrap_or(0.0) + value?);
                }
            }
        }
    }
    for (slot, expr) in &plan.expr_sums {
        if let Some(value) = plan.eval(expr, row) {
            let sum = &mut group.sums[*slot];
            *sum = Some(sum.unwrap_or(0.0) + value);
        }
    }
    for (slot, avg, expr) in &plan.expr_avgs {
        if let Some(value) = plan.eval(expr, row) {
            let sum = &mut group.sums[*slot];
            *sum = Some(sum.unwrap_or(0.0) + value);
            group.avg_n[*avg] = Some(group.avg_n[*avg].unwrap_or(0) + 1);
        }
    }
    for (slot, avg, source) in &plan.avgs {
        let (value, n) = match source {
            Source::Alias(field) => match ctx.values(plan, field)? {
                Some(values) => sum_numbers(&values, 0.0)?,
                None => continue,
            },
            Source::Slot(field) => match cell(row, *field).and_then(Cell::number) {
                Some(value) => (value?, 1),
                None => continue,
            },
        };
        let sum = &mut group.sums[*slot];
        *sum = Some(sum.unwrap_or(0.0) + value);
        group.avg_n[*avg] = Some(group.avg_n[*avg].unwrap_or(0) + n);
    }
    let mut added = 0usize;
    for (extremes, metrics, is_min) in [
        (&mut group.mins, &plan.mins, true),
        (&mut group.maxs, &plan.maxs, false),
    ] {
        for (slot, source) in metrics {
            match source {
                Source::Alias(field) => {
                    if let Some(values) = ctx.values(plan, field)? {
                        for value in values {
                            added += extremes[*slot].offer(&Cell::from_json(value), is_min);
                        }
                    }
                }
                Source::Slot(field) => {
                    if let Some(value) = cell(row, *field) {
                        added += extremes[*slot].offer(value, is_min);
                    }
                }
            }
        }
    }
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_and_float_keys_stay_apart_as_their_json_does() {
        let keys = [
            Key::of(&Cell::Int(15)),
            Key::of(&Cell::Float(15.0)),
            Key::of(&Cell::Float(-0.0)),
            Key::of(&Cell::Float(0.0)),
            Key::of(&Cell::Str("15".into())),
        ];
        for (i, a) in keys.iter().enumerate() {
            for (j, b) in keys.iter().enumerate() {
                let rendered = |k: &Key| serde_json::to_string(&k.clone().into_json()).unwrap();
                assert_eq!(a == b, i == j);
                assert_eq!(rendered(a) == rendered(b), i == j);
            }
        }
        assert_eq!(Key::of(&Cell::Int(7)), Key::UInt(7));
    }

    #[test]
    fn extremes_keep_large_integers_and_merge_in_part_order() {
        let exact = Cell::Int(9_007_199_254_740_993);
        let rounded = Cell::Float(9_007_199_254_740_992.0);
        let mut max = Extreme::default();
        max.offer(&rounded, false);
        max.offer(&exact, false);
        assert_eq!(
            max.render(),
            Some(serde_json::json!(9_007_199_254_740_993i64))
        );

        // A first value that is not a number stays, across parts too.
        let mut first = Extreme::default();
        first.offer(&Cell::Str("n/a".into()), true);
        let mut later = Extreme::default();
        later.offer(&Cell::Int(1), true);
        first.absorb(later, true);
        assert_eq!(first.render(), Some(serde_json::json!("n/a")));

        // Numbers win over a later part's text; a tie keeps the earlier.
        let mut a = Extreme::default();
        a.offer(&Cell::Int(5), true);
        let mut b = Extreme::default();
        b.offer(&Cell::Str("x".into()), true);
        b.offer(&Cell::Float(5.0), true);
        b.offer(&Cell::Int(2), true);
        a.absorb(b, true);
        assert_eq!(a.render(), Some(serde_json::json!(2.0)));
    }

    #[test]
    fn a_row_expression_rejects_lossy_integer_widening() {
        let req = AggregateRequest {
            expr_sums: vec![("x".into(), OutputExpr::Field("value".into()))],
            ..AggregateRequest::default()
        };
        let plan = RowPlan::new(&req, Vec::new());
        let row = vec![Some(Cell::Int(9_007_199_254_740_993))];
        assert_eq!(plan.eval(&OutputExpr::Field("value".into()), &row), None);
        let row = vec![Some(Cell::Int(42))];
        assert_eq!(
            plan.eval(&OutputExpr::Field("value".into()), &row),
            Some(42.0)
        );
    }
}
