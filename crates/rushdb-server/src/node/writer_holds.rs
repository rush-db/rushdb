//! `rushdb_writer_hold_seconds{holder}`: how long each kind of work held the
//! storage backend's single writer. The store keeps the histogram
//! itself (atomic buckets per backend, `rushdb_storage::WriterHolds`); this
//! collector renders the snapshot taken at each scrape.

use prometheus::core::{Collector, Desc};
use prometheus::proto;

const NAME: &str = "rushdb_writer_hold_seconds";
const HELP: &str = "Time one write transaction held the storage writer, by holder";

pub(super) struct WriterHoldCollector {
    desc: Desc,
    snapshot: std::sync::Mutex<Vec<(rushdb_storage::WriterHolder, rushdb_storage::WriterHolds)>>,
}

impl WriterHoldCollector {
    pub(super) fn new() -> Self {
        Self {
            desc: Desc::new(
                NAME.into(),
                HELP.into(),
                vec!["holder".into()],
                std::collections::HashMap::new(),
            )
            .expect("valid Prometheus metric"),
            snapshot: std::sync::Mutex::default(),
        }
    }

    /// Replace the rendered snapshot with the store's current holds.
    pub(super) fn observe(&self, store: &rushdb_storage::Store) {
        *self
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = store.writer_holds();
    }
}

impl Collector for WriterHoldCollector {
    fn desc(&self) -> Vec<&Desc> {
        vec![&self.desc]
    }

    fn collect(&self) -> Vec<proto::MetricFamily> {
        let snapshot = self
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut family = proto::MetricFamily::default();
        family.set_name(NAME.into());
        family.set_help(HELP.into());
        family.set_field_type(proto::MetricType::HISTOGRAM);
        for (holder, holds) in snapshot {
            let mut histogram = proto::Histogram::default();
            let mut buckets = Vec::with_capacity(rushdb_storage::WRITER_HOLD_BOUNDS.len());
            let mut cumulative = 0u64;
            for (bound, count) in rushdb_storage::WRITER_HOLD_BOUNDS.iter().zip(holds.counts) {
                cumulative = cumulative.saturating_add(count);
                let mut bucket = proto::Bucket::default();
                bucket.set_upper_bound(*bound);
                bucket.set_cumulative_count(cumulative);
                buckets.push(bucket);
            }
            histogram.set_bucket(buckets);
            let total = cumulative.saturating_add(holds.counts[holds.counts.len() - 1]);
            histogram.set_sample_count(total);
            histogram.set_sample_sum(holds.sum_seconds);
            let mut label = proto::LabelPair::default();
            label.set_name("holder".into());
            label.set_value(holder.label().into());
            let mut metric = proto::Metric::default();
            metric.set_label(vec![label]);
            metric.set_histogram(histogram);
            family.mut_metric().push(metric);
        }
        vec![family]
    }
}
