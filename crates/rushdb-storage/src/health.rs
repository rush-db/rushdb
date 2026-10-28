//! Poisoning fences the store until reopen. Cleanup may recover a guard to
//! release resources, but data operations must propagate `Unhealthy`.

use crate::StorageError;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    LockResult,
};

#[derive(Default)]
pub(crate) struct StoreHealth(AtomicBool);

impl StoreHealth {
    pub(crate) fn fence(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub(crate) fn check(&self) -> Result<(), StorageError> {
        if self.0.load(Ordering::Acquire) {
            Err(StorageError::Unhealthy)
        } else {
            Ok(())
        }
    }

    pub(crate) fn guard<T>(&self, guard: LockResult<T>) -> Result<T, StorageError> {
        guard.map_err(|_| {
            self.fence();
            StorageError::Unhealthy
        })
    }

    pub(crate) fn cleanup<T>(&self, guard: LockResult<T>) -> T {
        guard.unwrap_or_else(|poison| {
            self.fence();
            poison.into_inner()
        })
    }
}
