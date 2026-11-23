//! Request phase tokens: the shared phase pool, its FIFO admission and the
//! few tokens reserved at open for requests that find the managed budget
//! exhausted by retained caches.

use std::sync::Arc;

use crate::{resources, Store};

/// The bytes a request's phase token reserves from the phase pool.
pub const PHASE_TOKEN_BYTES: usize = 8 * 1024 * 1024;
/// The phase pool's budget name.
pub const PHASE_POOL_NAME: &str = "query-phases";
/// Phase tokens reserved from the managed budget at open.
pub(crate) const PHASE_FLOOR_TOKENS: usize = 4;

impl Store {
    /// FIFO admission to [`Self::phase_budget`]: a request waits here for
    /// its phase token while the pool is full, instead of being refused.
    #[must_use]
    pub fn phase_admission(&self) -> Option<Arc<resources::FairAdmission>> {
        self.phase_admission.clone()
    }

    /// Bytes of the managed budget the phase tokens reserved at open hold.
    #[must_use]
    pub fn phase_floor_bytes(&self) -> usize {
        self.phase_floor
            .as_ref()
            .map_or(0, |(floor, _)| floor.bytes())
    }

    /// FIFO admission to the few phase tokens reserved at open, for a
    /// request that finds the managed budget (not the phase pool) exhausted
    /// by retained caches: some requests always make progress.
    #[must_use]
    pub fn phase_floor_admission(&self) -> Option<Arc<resources::FairAdmission>> {
        self.phase_floor
            .as_ref()
            .map(|(_, admission)| Arc::clone(admission))
    }
}
