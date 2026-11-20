//! Test-only crash points: a child test process sets
//! `RUSHDB_TEST_KILL_AT=<point>` (and optionally `RUSHDB_TEST_KILL_AFTER=n`)
//! and is killed with SIGKILL the n-th time execution reaches that point.

use std::sync::atomic::{AtomicU64, Ordering};

static HITS: AtomicU64 = AtomicU64::new(0);

/// Kill this process here when the environment names this point.
pub(crate) fn hit(point: &str) {
    if std::env::var("RUSHDB_TEST_KILL_AT").as_deref() != Ok(point) {
        return;
    }
    let after: u64 = std::env::var("RUSHDB_TEST_KILL_AFTER")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(1);
    if HITS.fetch_add(1, Ordering::SeqCst) + 1 >= after {
        kill();
    }
}

/// SIGKILL this process: no unwinding, no destructors, no clean close.
pub(crate) fn kill() -> ! {
    let _ = std::process::Command::new("kill")
        .args(["-KILL", &std::process::id().to_string()])
        .status();
    std::process::abort();
}
