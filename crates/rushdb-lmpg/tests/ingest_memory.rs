//! Transient heap of a plain-create ingest, per row.
//!
//! One ingest request is one atomic commit, so its rows pass through the
//! import plan, the commit's expanded ops, the sequencer, derivation and the
//! backend write while all of them are live. Each stage must hand its rows
//! on (move or share) rather than hold its own copy. A counting allocator
//! measures the heap the ingest needs above what it leaves behind: the peak
//! minus the bytes still live afterwards (the stored rows).

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rushdb_lmpg::import::{import_bfs_with_cancel, serialize_bfs_owned, ImportOptions};
use rushdb_lmpg::Lmpg;
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::{json, Value};

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grew(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

// SAFETY: every method forwards to the system allocator with the caller's
// arguments unchanged; the counters do not affect allocation.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        grew(layout.size());
        // SAFETY: forwarded contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        grew(layout.size());
        // SAFETY: forwarded contract.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: forwarded contract.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if new_size >= layout.size() {
            grew(new_size - layout.size());
        } else {
            LIVE.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
        }
        // SAFETY: forwarded contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Taxi-trip-shaped rows, typed as a CSV import types them.
fn rows(count: usize) -> Value {
    let vendors = ["CMT", "VTS", "DDS"];
    let payments = ["card", "cash", "dispute"];
    Value::Array(
        (0..count)
            .map(|i| {
                let n = u32::try_from(i).expect("row index fits u32");
                json!({
                    "trip_id": i,
                    "vendor": vendors[i % 3],
                    "pickup_zone": (i * 7) % 263 + 1,
                    "dropoff_zone": (i * 13) % 263 + 1,
                    "passengers": i % 5 + 1,
                    "distance_km": f64::from(n % 1000) / 100.0 + 0.5,
                    "fare": f64::from(n % 5000) / 100.0 + 3.25,
                    "tip": f64::from(n % 300) / 100.0 + 0.5,
                    "payment": payments[i % 3],
                    "pickup_hour": i % 24,
                })
            })
            .collect(),
    )
}

#[test]
fn plain_create_ingest_holds_one_copy_of_its_rows() {
    const ROWS: usize = 20_000;
    let lmpg = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(41));
    let options = ImportOptions::default();
    let documents = rows(ROWS);

    let before = LIVE.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);
    let plan = serialize_bfs_owned(documents, "Trip", &options);
    let idmap = import_bfs_with_cancel(&lmpg, plan, &options, &|| false).unwrap();
    assert_eq!(idmap.len(), ROWS);
    drop(idmap);
    let transient = PEAK
        .load(Ordering::Relaxed)
        .saturating_sub(LIVE.load(Ordering::Relaxed));
    let per_row = transient / ROWS;
    println!("transient heap per row: {per_row} bytes");
    // Measured: about 3.1 KB per row once the plan moves its rows into the
    // commit and the sequencer, publisher and deriver share the commit's
    // ops instead of copying them; 10.3 KB per row before.
    assert!(
        per_row < 5 * 1024,
        "transient heap per row {per_row} bytes: a stage copies the rows again"
    );
}
