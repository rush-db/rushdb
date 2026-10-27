//! Planner value-distribution summaries: estimates only,
//! never query results.

/// Number of equi-width buckets in a [`Histogram`].
pub const VALUE_DISTRIBUTION_BUCKETS: usize = 64;

/// Value-domain shape of one `(kind, pid)` for planner estimates:
/// exact extrema and distinct count, plus an equi-width histogram over the
/// order-preserving payload space. Computed on demand from the value
/// dictionary; never a query result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Histogram {
    /// Smallest payload.
    pub min: Vec<u8>,
    /// Largest payload.
    pub max: Vec<u8>,
    /// Distinct payloads.
    pub distinct: u64,
    /// Owner-occurrence counts per fixed bucket.
    pub buckets: [u32; VALUE_DISTRIBUTION_BUCKETS],
    /// The walk that built this stopped at its budget: buckets, distinct and
    /// `max` cover only the walked (smallest) prefix of the value domain.
    pub partial: bool,
}

impl Default for Histogram {
    fn default() -> Self {
        Self {
            min: Vec::new(),
            max: Vec::new(),
            distinct: 0,
            buckets: [0; VALUE_DISTRIBUTION_BUCKETS],
            partial: false,
        }
    }
}

impl Histogram {
    /// Fold one distinct payload held by `count` owner-occurrences.
    pub fn add(&mut self, payload: &[u8], count: u64) {
        if self.min.is_empty() || payload < self.min.as_slice() {
            self.min = payload.to_vec();
        }
        if payload > self.max.as_slice() {
            self.max = payload.to_vec();
        }
        self.distinct += 1;
        let bucket = &mut self.buckets[fixed_bucket_index(payload)];
        *bucket = bucket.saturating_add(u32::try_from(count).unwrap_or(u32::MAX));
    }

    /// Resident bytes, for cache accounting.
    #[must_use]
    pub fn resident_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.min.capacity() + self.max.capacity()
    }

    /// Total owner-occurrences across all buckets.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.buckets.iter().map(|&b| u64::from(b)).sum()
    }

    /// Estimated owners satisfying a range bound (`above` = `Gt`/`Gte`),
    /// interpolating uniformly within the bound's fixed bucket.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        reason = "selectivity estimate: a non-negative fraction of a u32 bucket count"
    )]
    pub fn estimate(&self, bound: &[u8], above: bool) -> u64 {
        let total = self.total();
        if total == 0 || self.max.is_empty() {
            return 0;
        }
        let (min_v, max_v, bound_v) = (
            encoded_to_u128(&self.min),
            encoded_to_u128(&self.max),
            encoded_to_u128(bound),
        );
        if above {
            if bound_v <= min_v {
                return total;
            }
            if bound_v > max_v {
                return 0;
            }
        } else {
            if bound_v > max_v {
                return total;
            }
            if bound_v <= min_v {
                return 0;
            }
        }
        let width = self.min.len().max(self.max.len()).max(bound.len());
        let bidx = fixed_bucket_index(bound);
        let (lo, hi) = fixed_bucket_bounds(width, bidx);
        let bucket_width = hi.saturating_sub(lo).max(1);
        let within = if above {
            hi.saturating_sub(bound_v)
        } else {
            bound_v.saturating_sub(lo)
        };
        let fraction = (within as f64 / bucket_width as f64).clamp(0.0, 1.0);
        let partial = (f64::from(self.buckets[bidx]) * fraction).round() as u32;
        let rest: u64 = if above {
            self.buckets[bidx + 1..].iter().map(|&b| u64::from(b)).sum()
        } else {
            self.buckets[..bidx].iter().map(|&b| u64::from(b)).sum()
        };
        rest.saturating_add(u64::from(partial)).min(total)
    }
}

fn encoded_to_u128(bytes: &[u8]) -> u128 {
    let mut buf = [0u8; 16];
    let take = bytes.len().min(16);
    buf[16 - take..].copy_from_slice(&bytes[bytes.len() - take..]);
    u128::from_be_bytes(buf)
}

fn fixed_bucket_index(encoded: &[u8]) -> usize {
    let bits = encoded.len().saturating_mul(8);
    let shift = bits.saturating_sub(6);
    ((encoded_to_u128(encoded) >> shift) & 0x3F) as usize
}

fn fixed_bucket_bounds(width: usize, idx: usize) -> (u128, u128) {
    let bits = width.saturating_mul(8);
    let shift = bits.saturating_sub(6);
    let lo = (idx as u128) << shift;
    let hi = if idx + 1 < VALUE_DISTRIBUTION_BUCKETS {
        (idx as u128 + 1) << shift
    } else if bits >= 128 {
        u128::MAX
    } else {
        (1u128 << bits) - 1
    };
    (lo, hi)
}
