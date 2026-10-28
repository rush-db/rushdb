//! In-memory storage backend.
//!
//! The in-memory store is an ordinary [`StorageBackend`] whose MVCC maps are
//! the storage, so the durable and in-memory modes share one read path
//! ([`crate::backend::StorageReadView`]) and one write path
//! ([`StorageBackend::apply`]/`apply_many`). It is a test / small-data mode,
//! not a production scaling target.
//!
//! `Store` keeps shared `Arc` handles to the same maps for diagnostics and
//! replay; they are the *same* data, not a second copy.

use std::collections::BTreeMap;
use std::ops::Bound;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use crate::backend::{LoadedState, StorageBackend, StorageReadView};
use crate::read_view::{ReadLimits, ScanProjection};
use crate::{wal, Op, StorageError, StorageStatistics, VersionEntry, LAST_APPLIED_KEY, LOG_PREFIX};

/// Shared MVCC maps backing an in-memory store.
pub(crate) struct MemoryMaps {
    /// Current value per live key.
    pub(crate) data: Arc<RwLock<BTreeMap<Vec<u8>, Vec<u8>>>>,
    /// Creation commit index per live key (multi-version reads).
    pub(crate) created: Arc<RwLock<BTreeMap<Vec<u8>, u64>>>,
    /// Superseded values per key for time-travel reads.
    pub(crate) history: Arc<RwLock<BTreeMap<Vec<u8>, Vec<VersionEntry>>>>,
}

impl MemoryMaps {
    pub(crate) fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(BTreeMap::new())),
            created: Arc::new(RwLock::new(BTreeMap::new())),
            history: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }
}

pub(crate) struct MemoryBackend {
    health: Arc<crate::health::StoreHealth>,
    maps: MemoryMaps,
    abort: Arc<AtomicBool>,
}

impl MemoryBackend {
    pub(crate) fn new(
        health: Arc<crate::health::StoreHealth>,
        maps: MemoryMaps,
        abort: Arc<AtomicBool>,
    ) -> Self {
        Self {
            health,
            maps,
            abort,
        }
    }

    /// Apply one op-set's MVCC transition to the maps. Mirrors the durable
    /// backend's semantics: `Put`/`Del` retain a superseded version, `Purge`
    /// erases history, `Set` overwrites without history.
    fn apply_ops(&self, ops: &[Op], ci: u64) -> Result<(), String> {
        let mut data = self
            .maps
            .data
            .write()
            .map_err(|_| "memory data lock poisoned".to_string())?;
        {
            let mut hist = self
                .maps
                .history
                .write()
                .map_err(|_| "memory history lock poisoned".to_string())?;
            let mut created = self
                .maps
                .created
                .write()
                .map_err(|_| "memory created lock poisoned".to_string())?;
            for op in ops {
                match op {
                    Op::Put(k, _) => {
                        if let Some(prev_created) = created.get(k).copied() {
                            let old = data.get(k).cloned();
                            hist.entry(k.clone()).or_default().push(VersionEntry {
                                from_ci: prev_created,
                                to_ci: ci,
                                value: old,
                            });
                        }
                        created.insert(k.clone(), ci);
                    }
                    Op::Del(k) => {
                        if let Some(prev_created) = created.get(k).copied() {
                            let old = data.get(k).cloned();
                            hist.entry(k.clone()).or_default().push(VersionEntry {
                                from_ci: prev_created,
                                to_ci: ci,
                                value: old,
                            });
                        }
                        created.remove(k);
                    }
                    Op::Purge(k) => {
                        hist.remove(k);
                        created.remove(k);
                    }
                    Op::Set(k, _) => {
                        hist.remove(k);
                        created.insert(k.clone(), ci);
                    }
                }
            }
        }
        let mut usage = crate::usage::UsageBytes::default();
        for op in ops {
            match op {
                Op::Put(k, v) | Op::Set(k, v) => {
                    let old = data.insert(k.clone(), v.clone());
                    usage.record(k, old.as_ref().map(Vec::len), Some(v.len()));
                }
                Op::Del(k) | Op::Purge(k) => {
                    let old = data.remove(k);
                    usage.record(k, old.as_ref().map(Vec::len), None);
                }
            }
        }
        let rows = usage.rows(|key| Ok(data.get(key).cloned()))?;
        if rows.is_empty() {
            return Ok(());
        }
        let usage_ops: Vec<Op> = rows
            .into_iter()
            .map(|(key, value)| match value {
                Some(value) => Op::Put(key, value),
                None => Op::Del(key),
            })
            .collect();
        drop(data);
        self.apply_ops(&usage_ops, ci)
    }
}

impl StorageBackend for MemoryBackend {
    fn load(&self) -> Result<LoadedState, String> {
        let data = self
            .maps
            .data
            .read()
            .map_err(|_| "memory data lock poisoned".to_string())?
            .clone();
        let created = self
            .maps
            .created
            .read()
            .map_err(|_| "memory created lock poisoned".to_string())?
            .clone();
        let history = self
            .maps
            .history
            .read()
            .map_err(|_| "memory history lock poisoned".to_string())?
            .clone();
        Ok(LoadedState {
            data,
            created,
            history,
            gc_watermark: 0,
        })
    }

    fn apply(&self, ops: &[Op], commit_index: u64) -> Result<(), String> {
        self.apply_ops(ops, commit_index)
    }

    fn apply_many(&self, batches: &[(u64, &[Op])]) -> Result<(), String> {
        for (ci, ops) in batches {
            self.apply_ops(ops, *ci)?;
        }
        Ok(())
    }

    fn retire_before(&self, watermark: u64) -> Result<(), String> {
        let mut hist = self
            .maps
            .history
            .write()
            .map_err(|_| "memory history lock poisoned".to_string())?;
        for entries in hist.values_mut() {
            entries.retain(|e| e.to_ci > watermark || e.to_ci == 0);
        }
        hist.retain(|_, entries| !entries.is_empty());
        Ok(())
    }

    fn compact(&self) -> Result<(), String> {
        Ok(())
    }

    fn storage_stats(&self) -> Result<StorageStatistics, String> {
        let mut stats = StorageStatistics::default();
        let data = self
            .maps
            .data
            .read()
            .map_err(|_| "memory data lock poisoned".to_string())?;
        for (index, (key, value)) in data.iter().enumerate() {
            if index % 256 == 0 && self.abort.load(Ordering::Relaxed) {
                return Err("maintenance cancelled".to_string());
            }
            let bytes = (key.len() + value.len()) as u64;
            if key.starts_with(LOG_PREFIX) {
                stats.log_keys += 1;
                stats.log_bytes += bytes;
            } else if key.starts_with(crate::receipt::RECEIPT_FAMILY) {
                stats.receipt_keys += 1;
                stats.receipt_bytes += bytes;
            } else {
                stats.data_keys += 1;
                stats.data_bytes += bytes;
            }
        }
        drop(data);
        let history = self
            .maps
            .history
            .read()
            .map_err(|_| "memory history lock poisoned".to_string())?;
        for (key, versions) in history.iter() {
            stats.history_keys += 1;
            stats.history_bytes += key.len() as u64;
            for entry in versions {
                stats.history_bytes += entry.value.as_ref().map_or(0, Vec::len) as u64;
            }
        }
        drop(history);
        Ok(stats)
    }

    fn read_view(&self) -> Result<Box<dyn StorageReadView>, StorageError> {
        Ok(Box::new(MemoryReadView {
            health: Arc::clone(&self.health),
            maps: MemoryMaps {
                data: Arc::clone(&self.maps.data),
                created: Arc::clone(&self.maps.created),
                history: Arc::clone(&self.maps.history),
            },
        }))
    }
}

/// Backend-agnostic read snapshot over the in-memory store's MVCC maps.
struct MemoryReadView {
    health: Arc<crate::health::StoreHealth>,
    maps: MemoryMaps,
}

impl StorageReadView for MemoryReadView {
    fn read_value(
        &self,
        key: &[u8],
        ci: u64,
        current: bool,
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        limits.check_item(key.len(), 0)?;
        // `current` is only ever true for an unpinned latest read (Store-level
        // `try_get`); a pinned `ReadView` passes `current = native_current &&
        // snapshot_pinned`, which is false for the live memory maps. So a
        // current read returns the live value; otherwise `ci` is authoritative.
        if current {
            let data = self.health.guard(self.maps.data.read())?;
            return data
                .get(key)
                .map(|value| {
                    if projection == ScanProjection::Keys {
                        return Ok(Vec::new());
                    }
                    limits.check_item(key.len(), value.len())?;
                    Ok(value.clone())
                })
                .transpose();
        }
        let data = self.health.guard(self.maps.data.read())?;
        let history = self.health.guard(self.maps.history.read())?;
        let created = self.health.guard(self.maps.created.read())?;
        let value = if created.get(key).is_some_and(|created| *created <= ci) {
            data.get(key).map(Vec::as_slice)
        } else {
            history
                .get(key)
                .and_then(|versions| {
                    versions
                        .iter()
                        .rev()
                        .find(|v| v.from_ci <= ci && (v.to_ci == 0 || ci < v.to_ci))
                })
                .and_then(|v| v.value.as_deref())
        };
        value
            .map(|value| {
                if projection == ScanProjection::Keys {
                    return Ok(Vec::new());
                }
                limits.check_item(key.len(), value.len())?;
                Ok(value.to_vec())
            })
            .transpose()
    }

    fn next_key(
        &self,
        lower: &Bound<Vec<u8>>,
        upper: &Bound<Vec<u8>>,
        limits: ReadLimits,
        _include_history: bool,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        let data = self.health.guard(self.maps.data.read())?;
        let history = self.health.guard(self.maps.history.read())?;
        let bounds = (
            lower.as_ref().map(Vec::as_slice),
            upper.as_ref().map(Vec::as_slice),
        );
        let current = data.range::<[u8], _>(bounds).next().map(|(key, _)| key);
        let old = history.range::<[u8], _>(bounds).next().map(|(key, _)| key);
        let key = match (current, old) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        key.map(|key| {
            limits.check_item(key.len(), 0)?;
            Ok(key.clone())
        })
        .transpose()
    }

    fn current_range<'a>(
        &'a self,
        start: &[u8],
        end: &[u8],
        limits: ReadLimits,
        projection: ScanProjection,
    ) -> Result<Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), StorageError>> + 'a>, StorageError>
    {
        let data = self.health.guard(self.maps.data.read())?;
        let bounds = (
            Bound::Included(start),
            if end.is_empty() {
                Bound::Unbounded
            } else {
                Bound::Excluded(end)
            },
        );
        let mut rows = Vec::new();
        for (key, value) in data.range::<[u8], _>(bounds) {
            if projection == ScanProjection::Keys {
                limits.check_item(key.len(), 0)?;
                rows.push(Ok((key.clone(), Vec::new())));
            } else {
                limits.check_item(key.len(), value.len())?;
                rows.push(Ok((key.clone(), value.clone())));
            }
        }
        Ok(Box::new(rows.into_iter()))
    }

    fn visible_ci(&self) -> Result<u64, StorageError> {
        let data = self.health.guard(self.maps.data.read())?;
        match data.get(LAST_APPLIED_KEY) {
            Some(value) => wal::decode_last_applied(value).map_err(|_| StorageError::CorruptLog),
            None => Ok(0),
        }
    }

    fn history_floor(&self) -> Result<u64, StorageError> {
        Ok(0)
    }
}
