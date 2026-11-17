//! Progress of redb's full repair while a store file opens.
//!
//! A file that was not closed cleanly, and whose newest durable commit saved
//! no allocator state, is checked in full before it opens (three scans of
//! the file). That can take minutes on a large file; the boot listener
//! reports it as the `repairing-storage` phase with this progress.

use std::sync::atomic::{AtomicU64, Ordering};

/// The progress as `f64` bits, or `NONE` when no repair runs.
static PROGRESS: AtomicU64 = AtomicU64::new(NONE);
const NONE: u64 = u64::MAX;

pub(crate) fn report(progress: f64) {
    PROGRESS.store(progress.clamp(0.0, 1.0).to_bits(), Ordering::Release);
}

pub(crate) fn finish() {
    PROGRESS.store(NONE, Ordering::Release);
}

/// The progress (0.0 to 1.0) of the full repair a store open is running in
/// this process, if one is.
#[must_use]
pub fn progress() -> Option<f64> {
    match PROGRESS.load(Ordering::Acquire) {
        NONE => None,
        bits => Some(f64::from_bits(bits)),
    }
}
