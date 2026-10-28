//! Live aggregate retention and per-record reconstruction admission.
use std::cell::Cell;
use std::sync::Arc;

use rushdb_lmpg::reader::TryRead;
use rushdb_storage::StorageError;
use serde_json::Value;

use crate::{GroupBucket, QueryError, QuerySnapshot};

pub(super) fn check(limit: Option<usize>, bytes: usize) -> Result<(), QueryError> {
    if limit.is_some_and(|limit| bytes > limit) {
        return Err(QueryError::ResourceLimit(
            "aggregate working byte budget exceeded".into(),
        ));
    }
    Ok(())
}

pub(super) fn value_bytes(value: &Value) -> usize {
    let payload = match value {
        Value::String(value) => value.len(),
        Value::Array(values) => values
            .iter()
            .fold(0usize, |sum, v| sum.saturating_add(value_bytes(v))),
        Value::Object(values) => object_bytes(values),
        _ => 0,
    };
    std::mem::size_of::<Value>().saturating_add(payload)
}

pub(super) fn object_bytes(values: &serde_json::Map<String, Value>) -> usize {
    values.iter().fold(0usize, |sum, (key, value)| {
        sum.saturating_add(96)
            .saturating_add(key.len())
            .saturating_add(value_bytes(value))
    })
}

pub(super) fn bucket_bytes(bucket: &GroupBucket) -> usize {
    [
        &bucket.keys,
        &bucket.sums,
        &bucket.counts,
        &bucket.mins,
        &bucket.maxs,
    ]
    .into_iter()
    .fold(std::mem::size_of::<GroupBucket>(), |sum, map| {
        sum.saturating_add(object_bytes(map))
    })
    .saturating_add(
        bucket
            .derived_outputs
            .iter()
            .fold(0usize, |sum, (key, value)| {
                sum.saturating_add(96)
                    .saturating_add(key.len())
                    .saturating_add(value_bytes(value))
            }),
    )
}

/// Charge reads before JSON decoding. The factor includes decoded scalar,
/// occurrence, family-cache and tree-node overhead. Each record gets its own
/// allowance, released when its reconstruction session is dropped.
pub(super) struct RecordReader {
    view: QuerySnapshot,
    remaining: Cell<Option<usize>>,
}

impl RecordReader {
    pub(super) fn new(view: &QuerySnapshot, limit: Option<usize>) -> Self {
        Self {
            view: Arc::clone(view),
            remaining: Cell::new(limit),
        }
    }

    fn view(&self) -> QuerySnapshot {
        self.remaining.get().map_or_else(
            || Arc::clone(&self.view),
            |bytes| self.view.with_materialization_limit(bytes / 16),
        )
    }

    fn charge(&self, bytes: usize) -> Result<(), StorageError> {
        if let Some(remaining) = self.remaining.get() {
            let charge = bytes.saturating_mul(16).saturating_add(128);
            let next = remaining.checked_sub(charge).ok_or_else(|| {
                StorageError::ResourceExhausted(
                    "aggregate reconstruction exceeds byte budget".into(),
                )
            })?;
            self.remaining.set(Some(next));
        }
        Ok(())
    }
}

impl TryRead for RecordReader {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        let value = self.view().get(key)?;
        if let Some(value) = &value {
            self.charge(key.len().saturating_add(value.len()))?;
        }
        Ok(value)
    }

    fn range_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        let rows = self.view().range_prefix(prefix)?;
        for (key, value) in &rows {
            self.charge(key.len().saturating_add(value.len()))?;
        }
        Ok(rows)
    }

    fn range(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        let rows = self.view().range(start, end)?;
        for (key, value) in &rows {
            self.charge(key.len().saturating_add(value.len()))?;
        }
        Ok(rows)
    }
}

pub(super) fn resource_error(error: QueryError) -> QueryError {
    match error {
        QueryError::Storage(StorageError::ResourceExhausted(message)) => {
            QueryError::ResourceLimit(message)
        }
        error => error,
    }
}
