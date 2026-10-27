//! Copy-on-write opt-out for the data directory (SCALE-2, 2026-10-18).
//!
//! redb materializes commits without an fsync and makes them durable at each
//! checkpoint, which holds the single writer through the fsync. On a
//! copy-on-write filesystem (btrfs) that fsync re-writes and re-compresses
//! every dirty extent: measured 10–28 s per checkpoint under a bulk load,
//! stalling every tenant, against ≤ 1.13 s with copy-on-write off. The store
//! therefore sets `FS_NOCOW_FL` on the data directory before it creates any
//! file, so the redb files, the WAL directory and its segments inherit it.
//! The flag only takes effect on files created empty, so a file created
//! before the opt-out keeps copy-on-write until it is re-created (restore
//! from a backup). Filesystems without the flag (ext4, xfs, tmpfs) never
//! copy on write and report [`CopyOnWrite::NotApplicable`].

use std::path::Path;

/// Copy-on-write state of a store's data files, reported at boot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyOnWrite {
    /// The filesystem has no copy-on-write flag (ext4, xfs, tmpfs, …).
    NotApplicable,
    /// The data directory and every data file have copy-on-write off.
    Off,
    /// Some data files still copy on write: their paths, and why.
    On {
        /// Files or directories that still copy on write.
        paths: Vec<String>,
        /// Why the opt-out did not reach them.
        reason: String,
    },
}

/// Turn copy-on-write off for the directory of the store file `db` (new
/// files inherit it), then report the state of that directory, the shard
/// files (`db`, `db-N`) and the files of the WAL directory (`db.wal`).
pub(crate) fn opt_out(db: &Path) -> CopyOnWrite {
    imp::opt_out(db)
}

#[cfg(target_os = "linux")]
mod imp {
    use super::CopyOnWrite;
    use rustix::fs::{ioctl_getflags, ioctl_setflags, IFlags};
    use rustix::io::Errno;
    use std::path::{Path, PathBuf};

    pub(super) fn opt_out(db: &Path) -> CopyOnWrite {
        let Some(dir) = db.parent().filter(|dir| !dir.as_os_str().is_empty()) else {
            return CopyOnWrite::NotApplicable;
        };
        let Some(name) = db.file_name().and_then(|name| name.to_str()) else {
            return CopyOnWrite::NotApplicable;
        };
        let flags = match flags_of(dir) {
            Ok(flags) => flags,
            Err(error) if unsupported(error) => return CopyOnWrite::NotApplicable,
            Err(error) => return on(&[dir.to_path_buf()], format!("read flags: {error}")),
        };
        if !flags.contains(IFlags::NOCOW) {
            match set_nocow(dir, flags) {
                Ok(()) => {}
                Err(error) if unsupported(error) => return CopyOnWrite::NotApplicable,
                Err(error) => return on(&[dir.to_path_buf()], format!("set FS_NOCOW_FL: {error}")),
            }
        }
        let mut still_cow = Vec::new();
        for file in data_files(dir, name) {
            match flags_of(&file) {
                Ok(flags) if flags.contains(IFlags::NOCOW) => {}
                // A file created empty after the opt-out inherits the flag;
                // an empty one from before can still take it.
                Ok(flags) if std::fs::metadata(&file).is_ok_and(|meta| meta.len() == 0) => {
                    if set_nocow(&file, flags).is_err() {
                        still_cow.push(file);
                    }
                }
                Ok(_) | Err(_) => still_cow.push(file),
            }
        }
        if still_cow.is_empty() {
            CopyOnWrite::Off
        } else {
            on(
                &still_cow,
                "created before copy-on-write was turned off; re-create them (restore from a backup) to drop it".into(),
            )
        }
    }

    fn on(paths: &[PathBuf], reason: String) -> CopyOnWrite {
        CopyOnWrite::On {
            paths: paths
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            reason,
        }
    }

    fn unsupported(error: Errno) -> bool {
        matches!(
            error,
            Errno::NOTTY | Errno::OPNOTSUPP | Errno::INVAL | Errno::NOSYS
        )
    }

    fn flags_of(path: &Path) -> Result<IFlags, Errno> {
        let file = std::fs::File::open(path)
            .map_err(|error| Errno::from_io_error(&error).unwrap_or(Errno::IO))?;
        ioctl_getflags(&file)
    }

    fn set_nocow(path: &Path, flags: IFlags) -> Result<(), Errno> {
        let file = std::fs::File::open(path)
            .map_err(|error| Errno::from_io_error(&error).unwrap_or(Errno::IO))?;
        ioctl_setflags(&file, flags | IFlags::NOCOW)
    }

    /// The store's files in `dir`: shard files (`name`, `name-N`) and the
    /// regular files of the WAL directory (`name.wal`).
    fn data_files(dir: &Path, name: &str) -> Vec<PathBuf> {
        let shard = |file: &str| {
            file == name
                || file
                    .strip_prefix(name)
                    .and_then(|rest| rest.strip_prefix('-'))
                    .is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit()))
        };
        let wal = format!("{name}.wal");
        let mut files = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return files;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let file = entry.file_name();
            let file = file.to_string_lossy();
            match entry.file_type() {
                Ok(kind) if kind.is_file() && shard(&file) => files.push(path),
                Ok(kind) if kind.is_dir() && file == wal => {
                    if let Ok(inner) = std::fs::read_dir(&path) {
                        files.extend(
                            inner
                                .flatten()
                                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
                                .map(|entry| entry.path()),
                        );
                    }
                }
                _ => {}
            }
        }
        files
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    use super::CopyOnWrite;
    use std::path::Path;

    pub(super) fn opt_out(_db: &Path) -> CopyOnWrite {
        CopyOnWrite::NotApplicable
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{opt_out, CopyOnWrite};
    use rustix::fs::{ioctl_getflags, IFlags};
    use std::path::{Path, PathBuf};

    /// A fresh directory on the filesystem holding the build (`/tmp` is often
    /// tmpfs, which has no copy-on-write to turn off).
    struct BuildDir(PathBuf);

    impl BuildDir {
        fn new() -> Self {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/copy-on-write-tests")
                .join(uuid::Uuid::now_v7().to_string());
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for BuildDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn nocow(path: &Path) -> bool {
        ioctl_getflags(std::fs::File::open(path).unwrap())
            .unwrap()
            .contains(IFlags::NOCOW)
    }

    #[test]
    fn a_durable_store_creates_its_files_without_copy_on_write() {
        let dir = BuildDir::new();
        let db = dir.0.join("data").join("store.redb");
        let store = crate::Store::open_with_deriver(
            &db,
            crate::resources::StorageResources::default(),
            None,
        )
        .unwrap();
        match store.copy_on_write().expect("a durable store records it") {
            CopyOnWrite::NotApplicable => {}
            CopyOnWrite::Off => {
                assert!(nocow(db.parent().unwrap()));
                assert!(nocow(&db));
                let wal = PathBuf::from(format!("{}.wal", db.display()));
                for segment in std::fs::read_dir(wal).unwrap() {
                    assert!(nocow(&segment.unwrap().path()));
                }
            }
            CopyOnWrite::On { paths, reason } => panic!("{paths:?} still copy on write: {reason}"),
        }
    }

    #[test]
    fn a_file_written_before_the_opt_out_is_reported_by_path() {
        let dir = BuildDir::new();
        let db = dir.0.join("store.redb");
        std::fs::write(&db, b"written while the directory still copied on write").unwrap();
        let unrelated = dir.0.join("config.yml");
        std::fs::write(&unrelated, b"not a store file").unwrap();
        match opt_out(&db) {
            CopyOnWrite::NotApplicable => {}
            CopyOnWrite::On { paths, reason } => {
                assert_eq!(paths, vec![db.display().to_string()]);
                assert!(reason.contains("restore from a backup"), "{reason}");
                assert!(nocow(&dir.0), "new files still inherit the opt-out");
            }
            CopyOnWrite::Off => panic!("a non-empty file cannot drop copy-on-write in place"),
        }
    }
}
