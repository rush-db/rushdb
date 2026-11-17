//! `rushdb_resident_*{layer}`: one metric surface for every resident layer
//! (owner sets, graphs, catalogs, planner statistics, vector rows, columns).
//! The layers keep their own counters (`rushdb_storage::resident`); this
//! collector renders the snapshot taken at each scrape.

use prometheus::core::{Collector, Desc};
use prometheus::proto;

use rushdb_storage::resident::LayerSnapshot;

/// `(name, help, counter)`: a counter is cumulative, a gauge is occupancy.
const FAMILIES: [(&str, &str, bool); 11] = [
    (
        "rushdb_resident_bytes",
        "Bytes a resident layer holds",
        false,
    ),
    (
        "rushdb_resident_entries",
        "Entries a resident layer holds",
        false,
    ),
    (
        "rushdb_resident_hits_total",
        "Lookups answered from a resident entry",
        true,
    ),
    (
        "rushdb_resident_misses_total",
        "Lookups a resident layer left to storage",
        true,
    ),
    (
        "rushdb_resident_builds_total",
        "Resident entries built",
        true,
    ),
    (
        "rushdb_resident_build_refused_total",
        "Resident values refused by the layer budget",
        true,
    ),
    (
        "rushdb_resident_build_seconds",
        "Seconds spent building resident entries",
        true,
    ),
    (
        "rushdb_resident_invalidations_total",
        "Resident entries dropped because storage they reflect changed",
        true,
    ),
    (
        "rushdb_resident_evictions_total",
        "Resident entries dropped by LRU",
        true,
    ),
    (
        "rushdb_resident_verify_total",
        "Resident hits rebuilt from storage and compared",
        true,
    ),
    (
        "rushdb_resident_verify_mismatch_total",
        "Resident entries found different from storage (dropped)",
        true,
    ),
];

fn value(snapshot: &LayerSnapshot, family: usize) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "metric values; f64 is the exposition format"
    )]
    let count = |n: u64| n as f64;
    match family {
        0 => count(snapshot.bytes),
        1 => count(snapshot.entries),
        2 => count(snapshot.hits),
        3 => count(snapshot.misses),
        4 => count(snapshot.builds),
        5 => count(snapshot.build_refused),
        6 => snapshot.build_seconds,
        7 => count(snapshot.invalidations),
        8 => count(snapshot.evictions),
        9 => count(snapshot.verify),
        _ => count(snapshot.verify_mismatch),
    }
}

pub(super) struct ResidentCollector {
    descs: Vec<Desc>,
    snapshot: std::sync::Mutex<Vec<LayerSnapshot>>,
}

impl ResidentCollector {
    pub(super) fn new() -> Self {
        Self {
            descs: FAMILIES
                .iter()
                .map(|(name, help, _)| {
                    Desc::new(
                        (*name).into(),
                        (*help).into(),
                        vec!["layer".into()],
                        std::collections::HashMap::new(),
                    )
                    .expect("valid Prometheus metric")
                })
                .collect(),
            snapshot: std::sync::Mutex::default(),
        }
    }

    /// Replace the rendered snapshot with the store's layers now.
    pub(super) fn observe(&self, store: &rushdb_storage::Store) {
        *self
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = store.residents().snapshots();
    }
}

impl Collector for ResidentCollector {
    fn desc(&self) -> Vec<&Desc> {
        self.descs.iter().collect()
    }

    fn collect(&self) -> Vec<proto::MetricFamily> {
        let snapshot = self
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        FAMILIES
            .iter()
            .enumerate()
            .map(|(at, (name, help, counter))| {
                let mut family = proto::MetricFamily::default();
                family.set_name((*name).into());
                family.set_help((*help).into());
                family.set_field_type(if *counter {
                    proto::MetricType::COUNTER
                } else {
                    proto::MetricType::GAUGE
                });
                for layer in &snapshot {
                    let mut label = proto::LabelPair::default();
                    label.set_name("layer".into());
                    label.set_value(layer.layer.into());
                    let mut metric = proto::Metric::default();
                    metric.set_label(vec![label]);
                    if *counter {
                        let mut value_of = proto::Counter::default();
                        value_of.set_value(value(layer, at));
                        metric.set_counter(value_of);
                    } else {
                        let mut value_of = proto::Gauge::default();
                        value_of.set_value(value(layer, at));
                        metric.set_gauge(value_of);
                    }
                    family.mut_metric().push(metric);
                }
                family
            })
            .collect()
    }
}
