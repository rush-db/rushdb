//! The reads a transaction recorded, checked at commit against what later
//! commits wrote (serializable conflict detection).

use crate::prefix_range;

/// Serializable-mode read dependencies recorded against a transaction.
/// Zero-width entries are point reads; otherwise half-open [start,end).
#[derive(Default, Clone, Debug)]
pub struct DependencySet {
    /// Recorded read ranges as `(start, end)` pairs; a zero-width pair is a
    /// point dependency.
    pub ranges: Vec<(Vec<u8>, Vec<u8>)>,
    pub(crate) whole_keyspace: bool,
}

impl DependencySet {
    /// Whether no point, range, or whole-keyspace dependency was recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty() && !self.whole_keyspace
    }

    /// Record a half-open `[start, end)` range dependency.
    pub fn record(&mut self, start: Vec<u8>, end: Vec<u8>) {
        self.ranges.push((start, end));
    }

    /// Prefix tracking is explicit so an empty prefix (all keys) is distinct
    /// from a point dependency on the empty key.
    pub fn record_prefix(&mut self, prefix: &[u8]) {
        if prefix.is_empty() {
            self.whole_keyspace = true;
        } else {
            let (start, end) = prefix_range(prefix);
            self.record(start, end);
        }
    }

    /// A mark to [`Self::roll_back_to`]: what was recorded so far.
    pub(crate) fn mark(&self) -> (usize, bool) {
        (self.ranges.len(), self.whole_keyspace)
    }

    /// Forget everything recorded after `mark`.
    pub(crate) fn roll_back_to(&mut self, (ranges, whole_keyspace): (usize, bool)) {
        self.ranges.truncate(ranges);
        self.whole_keyspace = whole_keyspace;
    }

    /// Merge dependencies captured by another preparation phase.
    pub fn extend(&mut self, other: Self) {
        self.ranges.extend(other.ranges);
        self.whole_keyspace |= other.whole_keyspace;
    }

    pub(crate) fn intersects(&self, key: &[u8]) -> bool {
        self.whole_keyspace
            || self.ranges.iter().any(|(s, e)| {
                if s == e {
                    key == s.as_slice()
                } else {
                    key >= s.as_slice() && (e.is_empty() || key < e.as_slice())
                }
            })
    }

    /// Whether `key` falls in any recorded read range (serializable conflict
    /// probe for transaction commit).
    #[must_use]
    pub fn intersects_key(&self, key: &[u8]) -> bool {
        self.intersects(key)
    }

    /// The set arranged for many probes (commit validation checks every key
    /// written since the snapshot): a binary search per probe instead of a
    /// pass over every recorded read.
    #[must_use]
    pub fn index(&self) -> DependencyIndex<'_> {
        let mut points = Vec::new();
        let mut ranges: Vec<(&[u8], &[u8])> = Vec::new();
        for (start, end) in &self.ranges {
            if start == end {
                points.push(start.as_slice());
            } else if end.is_empty() || start < end {
                ranges.push((start.as_slice(), end.as_slice()));
            }
        }
        points.sort_unstable();
        points.dedup();
        ranges.sort_unstable_by(|left, right| left.0.cmp(right.0));
        // Merge overlapping or touching ranges into disjoint ones (an empty
        // end is unbounded and absorbs everything after its start).
        let mut merged: Vec<(&[u8], &[u8])> = Vec::with_capacity(ranges.len());
        for (start, end) in ranges {
            if let Some((_, last_end)) = merged.last_mut() {
                if last_end.is_empty() {
                    continue;
                }
                if start <= *last_end {
                    if end.is_empty() || end > *last_end {
                        *last_end = end;
                    }
                    continue;
                }
            }
            merged.push((start, end));
        }
        DependencyIndex {
            whole_keyspace: self.whole_keyspace,
            points,
            ranges: merged,
        }
    }
}

/// A [`DependencySet`] arranged for probes: sorted point reads and disjoint
/// sorted ranges.
#[derive(Debug)]
pub struct DependencyIndex<'a> {
    whole_keyspace: bool,
    points: Vec<&'a [u8]>,
    ranges: Vec<(&'a [u8], &'a [u8])>,
}

impl DependencyIndex<'_> {
    /// Whether `key` falls in any recorded read, as
    /// [`DependencySet::intersects_key`] answers it.
    #[must_use]
    pub fn contains(&self, key: &[u8]) -> bool {
        if self.whole_keyspace || self.points.binary_search(&key).is_ok() {
            return true;
        }
        let after = self.ranges.partition_point(|(start, _)| *start <= key);
        after > 0 && {
            let (_, end) = self.ranges[after - 1];
            end.is_empty() || key < end
        }
    }
}
