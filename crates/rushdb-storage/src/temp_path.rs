//! Self-cleaning temporary locations for tests and benchmarks: each lives in
//! its own fresh directory, removed with everything in it (a store's WAL and
//! layout siblings included) when the guard drops, also while a failed test
//! unwinds.

use std::path::{Path, PathBuf};

/// A temporary store file or directory; see the module docs.
#[derive(Debug)]
pub struct TempPath {
    dir: PathBuf,
    path: PathBuf,
}

impl TempPath {
    /// A store file path (`<fresh dir>/store.redb`) named after `prefix`.
    ///
    /// # Panics
    /// Panics when the temporary directory cannot be created.
    #[must_use]
    pub fn store(prefix: &str) -> Self {
        let dir = Self::fresh(prefix);
        let path = dir.join("store.redb");
        Self { dir, path }
    }

    /// A fresh, empty directory named after `prefix`.
    ///
    /// # Panics
    /// Panics when the temporary directory cannot be created.
    #[must_use]
    pub fn dir(prefix: &str) -> Self {
        let dir = Self::fresh(prefix);
        Self {
            path: dir.clone(),
            dir,
        }
    }

    fn fresh(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rushdb-{prefix}-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&dir).expect("create a temporary directory");
        dir
    }

    /// The path itself.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl std::ops::Deref for TempPath {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for TempPath {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
