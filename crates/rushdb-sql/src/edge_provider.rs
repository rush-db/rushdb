//! Streamed relationship tables (`rushdb_edges.<type>`, SQL-4). Resolving a
//! table reads the type's columns and exact row count from the relationship
//! catalog (per-shape counts and property counters), without reading a
//! relationship; each scan then streams the type's relationships in bounded
//! batches. No edge table is held in memory, so the old
//! 100,000-relationship ceiling no longer applies; the request's memory and
//! deadline limits do.
//!
//! The relationship index holds every type in ordinal order, so a scan still
//! walks all relationships to return one type's; a per-type index would
//! make it proportional to the type (deferred).

use std::collections::VecDeque;
use std::ops::Bound;
use std::sync::Arc;

use arrow::datatypes::{Schema, SchemaRef};
use arrow_json::reader::infer_json_schema_from_iterator;
use datafusion::catalog::Session;
use datafusion::common::stats::Precision;
use datafusion::common::{DataFusionError, Result as DFResult, Statistics};
use datafusion::datasource::TableProvider;
use datafusion::execution::TaskContext;
use datafusion::physical_plan::stream::RecordBatchStreamAdapter;
use datafusion::physical_plan::streaming::{PartitionStream, StreamingTableExec};
use datafusion::physical_plan::{
    DisplayAs, DisplayFormatType, ExecutionPlan, SendableRecordBatchStream,
};
use datafusion_expr::{Expr, TableType};
use rushdb_lmpg::reader::try_access as access;
use rushdb_lmpg::reconstruct::reconstruct_relationship;
use rushdb_storage::{ReadView, ScanBounds, ScanProjection, StorageError};
use rushdb_types::ids::OwnerKind;
use rushdb_types::ids::TenantId;
use rushdb_types::keys::{key, P_REL_BY_ORD};
use rushdb_types::model::{LogicalType, TypeState};
use rushdb_types::value::{ID_FIELD, SOURCE_FIELD, TARGET_FIELD, TYPE_FIELD};
use serde_json::Value as Json;

use crate::{dashed_id, SqlError};
use rushdb_lmpg::CatalogAccess;

/// Relationship index keys read per page.
const PAGE_KEYS: usize = 1024;
/// Rows per streamed batch.
const BATCH_ROWS: usize = 1024;

/// The relationships of one type, read page by page from the relationship
/// index (which holds every type, in ordinal order).
struct EdgeRows {
    snapshot: Arc<ReadView>,
    tenant: TenantId,
    rel_type: String,
    prefix: Vec<u8>,
    lower: Bound<Vec<u8>>,
    upper: Bound<Vec<u8>>,
    pending: VecDeque<Json>,
    done: bool,
    visible: CatalogAccess,
}

impl EdgeRows {
    fn new(
        snapshot: Arc<ReadView>,
        tenant: TenantId,
        rel_type: &str,
        visible: CatalogAccess,
    ) -> Self {
        let prefix = key(tenant, &[P_REL_BY_ORD]);
        let bounds = ScanBounds::prefix(&prefix);
        Self {
            snapshot,
            tenant,
            rel_type: rel_type.to_string(),
            prefix,
            lower: bounds.start,
            upper: bounds.end,
            pending: VecDeque::new(),
            done: false,
            visible,
        }
    }

    /// Under an enforced policy, a relationship is visible only when both
    /// endpoints' labels are allowed.
    fn endpoints_visible(&self, source: u64, target: u64) -> Result<bool, SqlError> {
        if !self.visible.enforced() {
            return Ok(true);
        }
        for ordinal in [source, target] {
            let header = access::record_by_ord(self.snapshot.as_ref(), self.tenant, ordinal)?
                .ok_or(StorageError::CorruptLog)?;
            if !self.visible.label(&header.label) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Reads the next page of the index into `pending`.
    fn fill(&mut self) -> Result<(), SqlError> {
        let mut cursor = self.snapshot.scan(
            ScanBounds {
                start: self.lower.clone(),
                end: self.upper.clone(),
            },
            ScanProjection::Keys,
        )?;
        let page = cursor.next_batch(PAGE_KEYS, 1024 * 1024)?;
        if let Some(last) = page.rows.last() {
            self.lower = Bound::Excluded(last.key.clone());
        }
        self.done = page.done || page.rows.is_empty();
        for member in &page.rows {
            let ordinal = ordinal_suffix(&member.key, &self.prefix)?;
            let header = access::relationship_by_ord(self.snapshot.as_ref(), self.tenant, ordinal)?
                .ok_or(StorageError::CorruptLog)?;
            if header.rel_type != self.rel_type
                || !self.endpoints_visible(header.source_ord, header.target_ord)?
            {
                continue;
            }
            let mut row = reconstruct_relationship(self.snapshot.as_ref(), self.tenant, ordinal)?
                .ok_or(StorageError::CorruptLog)?;
            if let Json::Object(object) = &mut row {
                for field in [ID_FIELD, SOURCE_FIELD, TARGET_FIELD] {
                    let id = object.remove(field).unwrap_or(Json::Null);
                    object.insert(field.into(), dashed_id(id));
                }
                self.pending.push_back(row);
            }
        }
        Ok(())
    }

    /// The next row, or `None` at the end.
    fn next_row(&mut self) -> Result<Option<Json>, SqlError> {
        while self.pending.is_empty() && !self.done {
            self.fill()?;
        }
        Ok(self.pending.pop_front())
    }
}

/// The ordinal at the end of a relationship-index key.
fn ordinal_suffix(storage_key: &[u8], prefix: &[u8]) -> Result<u64, StorageError> {
    if storage_key.len() != prefix.len() + 9
        || !storage_key.starts_with(prefix)
        || storage_key[prefix.len()] != b'/'
    {
        return Err(StorageError::CorruptLog);
    }
    Ok(u64::from_be_bytes(
        storage_key[prefix.len() + 1..]
            .try_into()
            .map_err(|_| StorageError::CorruptLog)?,
    ))
}

/// One relationship type as a streamed table.
pub(crate) struct EdgeTable {
    snapshot: Arc<ReadView>,
    tenant: TenantId,
    rel_type: String,
    schema: SchemaRef,
    rows: usize,
    visible: CatalogAccess,
}

impl std::fmt::Debug for EdgeTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EdgeTable")
            .field("rel_type", &self.rel_type)
            .finish_non_exhaustive()
    }
}

impl EdgeTable {
    /// The type's columns and exact row count, from the relationship
    /// catalog.
    pub(crate) fn new(
        snapshot: Arc<ReadView>,
        tenant: TenantId,
        rel_type: &str,
        visible: CatalogAccess,
    ) -> Result<Self, SqlError> {
        let (schema, rows) = edge_columns(&snapshot, tenant, rel_type, &visible)?;
        Ok(Self {
            snapshot,
            tenant,
            rel_type: rel_type.to_string(),
            schema: Arc::new(schema),
            rows: usize::try_from(rows).unwrap_or(usize::MAX),
            visible,
        })
    }
}

/// A relationship type's columns and row count from its catalog shapes:
/// every `(type, source label, target label)` shape counts its
/// relationships and the properties they carry. Under an enforced policy
/// only shapes between allowed labels count, as only their relationships
/// are returned. Columns get the type each property's definitions store
/// (an array where a relationship of the type holds one), the same rule
/// label tables use.
fn edge_columns(
    snapshot: &ReadView,
    tenant: TenantId,
    rel_type: &str,
    visible: &CatalogAccess,
) -> Result<(Schema, u64), SqlError> {
    let mut rows = 0u64;
    let mut pids = std::collections::BTreeSet::new();
    for (shape_type, source, target, count) in access::relationship_catalog(snapshot, tenant)? {
        if shape_type != rel_type
            || count == 0
            || !visible.label(&source)
            || !visible.label(&target)
        {
            continue;
        }
        rows += count;
        for (pid, owners) in
            access::rel_prop_counts(snapshot, tenant, (&shape_type, &source, &target))?
        {
            if owners > 0 {
                pids.insert(pid);
            }
        }
    }
    let mut first = serde_json::Map::new();
    let mut extra: Vec<Json> = Vec::new();
    for pid in pids {
        let definition =
            access::definition(snapshot, tenant, pid)?.ok_or(StorageError::CorruptLog)?;
        let family = access::family(snapshot, tenant, definition.family_id)?
            .ok_or(StorageError::CorruptLog)?;
        let sample = match definition.type_state {
            TypeState::Unresolved => Json::Null,
            TypeState::Known(LogicalType::Boolean) => Json::Bool(true),
            // Datetimes read back as epoch milliseconds.
            TypeState::Known(LogicalType::Int64 | LogicalType::DateTime) => Json::from(0),
            TypeState::Known(LogicalType::Float64) => Json::from(0.5),
            TypeState::Known(LogicalType::Utf8) => Json::String(String::new()),
        };
        let array =
            rushdb_lmpg::derived::array_markers(snapshot, tenant, OwnerKind::Relationship, pid)?
                .1
                .iter()
                .any(|marked| marked == rel_type);
        let value = if array {
            Json::Array(vec![sample])
        } else {
            sample
        };
        if first.contains_key(&family.display_name) {
            let mut row = serde_json::Map::new();
            row.insert(family.display_name, value);
            extra.push(Json::Object(row));
        } else {
            first.insert(family.display_name, value);
        }
    }
    // The fields every relationship row carries (see `reconstruct_relationship`).
    for field in [ID_FIELD, TYPE_FIELD, SOURCE_FIELD, TARGET_FIELD] {
        first.insert(field.into(), Json::String(String::new()));
    }
    let schema =
        infer_json_schema_from_iterator(std::iter::once(Json::Object(first)).chain(extra).map(Ok))
            .map_err(|error| SqlError::Engine(error.to_string()))?;
    Ok((schema, rows))
}

#[async_trait::async_trait]
impl TableProvider for EdgeTable {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    /// The row count from the relationship catalog: exact without a policy
    /// (every relationship is in one shape). Under a policy it is an
    /// estimate: shapes keep their creation-time endpoint labels, while rows
    /// are filtered by the endpoints' current ones.
    fn statistics(&self) -> Option<Statistics> {
        Some(Statistics {
            num_rows: if self.visible.enforced() {
                Precision::Inexact(self.rows)
            } else {
                Precision::Exact(self.rows)
            },
            ..Default::default()
        })
    }

    async fn scan(
        &self,
        _state: &dyn Session,
        projection: Option<&Vec<usize>>,
        _filters: &[Expr],
        limit: Option<usize>,
    ) -> DFResult<Arc<dyn ExecutionPlan>> {
        let partition = Arc::new(EdgePartition {
            snapshot: Arc::clone(&self.snapshot),
            tenant: self.tenant,
            rel_type: self.rel_type.clone(),
            schema: Arc::clone(&self.schema),
            visible: self.visible.clone(),
        });
        Ok(Arc::new(StreamingTableExec::try_new(
            Arc::clone(&self.schema),
            vec![partition],
            projection,
            Vec::<datafusion::physical_expr::LexOrdering>::new(),
            false,
            limit,
        )?))
    }
}

struct EdgePartition {
    snapshot: Arc<ReadView>,
    tenant: TenantId,
    rel_type: String,
    schema: SchemaRef,
    visible: CatalogAccess,
}

impl std::fmt::Debug for EdgePartition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EdgePartition").finish_non_exhaustive()
    }
}

impl DisplayAs for EdgePartition {
    fn fmt_as(&self, _t: DisplayFormatType, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EdgePartition({})", self.rel_type)
    }
}

impl PartitionStream for EdgePartition {
    fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    fn execute(&self, _ctx: Arc<TaskContext>) -> SendableRecordBatchStream {
        let rows = EdgeRows::new(
            Arc::clone(&self.snapshot),
            self.tenant,
            &self.rel_type,
            self.visible.clone(),
        );
        let schema = Arc::clone(&self.schema);
        Box::pin(RecordBatchStreamAdapter::new(
            Arc::clone(&self.schema),
            futures::stream::unfold(Some(rows), move |state| {
                let schema = Arc::clone(&schema);
                async move {
                    let mut rows = state?;
                    let mut batch = Vec::new();
                    while batch.len() < BATCH_ROWS {
                        match rows.next_row() {
                            Ok(Some(row)) => batch.push(row),
                            Ok(None) => break,
                            Err(error) => {
                                return Some((
                                    Err(DataFusionError::External(Box::new(error))),
                                    None,
                                ));
                            }
                        }
                    }
                    if batch.is_empty() {
                        return None;
                    }
                    let next = (!rows.done || !rows.pending.is_empty()).then_some(rows);
                    Some((crate::scan_provider::batch_from_rows(&schema, batch), next))
                }
            }),
        ))
    }
}
