//! Opt-in read accounting for profiling: counts the backend's point reads,
//! range opens and seeks per key family (`keys::family_label`). Off by
//! default; when off, a read costs one relaxed atomic load. [`take`] returns
//! every thread's reads (profile one workload at a time); [`take_local`]
//! returns only the calling thread's, which stays exact while other work
//! runs concurrently (tests).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static ENABLED: AtomicBool = AtomicBool::new(false);
static COUNTS: Mutex<BTreeMap<(&'static str, String), u64>> = Mutex::new(BTreeMap::new());

thread_local! {
    static LOCAL: RefCell<BTreeMap<(&'static str, String), u64>> =
        const { RefCell::new(BTreeMap::new()) };
}

/// Turn counting on or off.
pub fn enable(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

/// Count one backend read of kind `op` (`get`, `scan`, `seek`) at `key`.
pub(crate) fn record(op: &'static str, key: &[u8]) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let family = rushdb_types::keys::family_label(key);
    LOCAL.with(|local| *local.borrow_mut().entry((op, family.clone())).or_default() += 1);
    let mut counts = COUNTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *counts.entry((op, family)).or_default() += 1;
}

/// Take and reset the calling thread's counts.
#[must_use]
pub fn take_local() -> BTreeMap<(&'static str, String), u64> {
    LOCAL.with(|local| std::mem::take(&mut *local.borrow_mut()))
}

/// Take and reset the counts: `(op, family) -> reads`.
#[must_use]
pub fn take() -> BTreeMap<(&'static str, String), u64> {
    std::mem::take(
        &mut *COUNTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    )
}
