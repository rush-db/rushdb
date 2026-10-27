//! Continuous logical-WAL archiving and physical base backups (durability A).
//!
//! The durable state lives in one redb file. To reduce that single-file blast
//! radius to something Postgres-like, an operator can:
//!
//! 1. run a **base backup** (`rushdb serve --backup-base <dir>`) offline — a copy of
//!    the redb file(s) plus a CRC manifest pinned at a commit index; and
//! 2. let the server **archive the WAL** continuously with
//!    `--wal-archive-dir <dir>` — append-only `wal-<from>-<to>.seg` segment
//!    files plus an `archive.state` watermark.
//!
//! Base + archived segments reconstruct any point in the retained window
//! (`rushdb serve --restore-pitr`), so the data file is no longer "one file or
//! nothing" and the log can be shipped off-box.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crc32fast::Hasher;

use rushdb_storage::Store;

/// Manifest written beside a base backup.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BaseManifest {
    /// Manifest format version.
    pub format_version: u32,
    /// Canonical command-batch version encoded by the copied store.
    pub command_version: u32,
    /// Transport-neutral protocol version expected by restore tooling.
    pub protocol_version: u16,
    /// Commit index the base is pinned at.
    pub commit_index: u64,
    /// Backed-up files with sizes and CRCs.
    pub files: Vec<BaseFile>,
}

/// One file in a base backup.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BaseFile {
    /// File name (relative to the base directory).
    pub name: String,
    /// Size in bytes.
    pub bytes: u64,
    /// CRC32 of the file contents.
    pub crc32: u32,
}

const BASE_FORMAT_VERSION: u32 = 1;
const BASE_COMMAND_VERSION: u32 = rushdb_types::command::COMMAND_VERSION;
const BASE_PROTOCOL_VERSION: u16 = rushdb_protocol::PROTOCOL_VERSION;
const MANIFEST_NAME: &str = "manifest.json";
const STATE_NAME: &str = "archive.state";
const BASE_SUBDIR: &str = "base";

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct ArchiveState {
    last_lsn: u64,
}

fn file_size_crc32(path: &Path) -> Result<(u64, u32), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut reader = std::io::BufReader::with_capacity(1024 * 1024, file);
    let mut hasher = Hasher::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut bytes = 0u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        bytes = bytes.saturating_add(read as u64);
    }
    Ok((bytes, hasher.finalize()))
}

fn copy_file_streaming(src: &Path, dst: &Path) -> Result<(), String> {
    let mut input = std::fs::File::open(src).map_err(|e| format!("open source: {e}"))?;
    let mut output = std::fs::File::create_new(dst).map_err(|e| format!("create target: {e}"))?;
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|e| format!("read source: {e}"))?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(|e| format!("write target: {e}"))?;
    }
    output.sync_all().map_err(|e| format!("fsync target: {e}"))
}

fn sync_directory(path: &Path, context: &str) -> Result<(), String> {
    let handle = std::fs::File::open(path)
        .map_err(|error| format!("open {context} directory for fsync: {error}"))?;
    handle
        .sync_all()
        .map_err(|error| format!("fsync {context} directory: {error}"))
}

/// Stream an artifact into a private temporary file and publish it with
/// exclusive-create semantics. The destination is never truncated or
/// replaced, and the completed file plus containing directory are durable
/// before this returns.
///
/// # Errors
/// Returns an error if the target path is invalid, staging or publication
/// fails, the callback fails, or either fsync cannot complete.
pub fn atomic_publish_stream<F>(target: &Path, write: F) -> Result<(), String>
where
    F: FnOnce(&mut std::fs::File) -> Result<(), String>,
{
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create artifact parent {}: {error}", parent.display()))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("invalid artifact target `{}`", target.display()))?;
    let temporary = parent.join(format!(".{name}.tmp-{}", uuid::Uuid::now_v7()));
    let result = (|| {
        let mut file = std::fs::File::create_new(&temporary)
            .map_err(|error| format!("create artifact staging file: {error}"))?;
        write(&mut file)?;
        file.sync_all()
            .map_err(|error| format!("fsync artifact staging file: {error}"))?;
        // A hard link is an atomic create-if-absent publication on the same
        // filesystem; unlike rename it cannot overwrite a pre-existing target.
        std::fs::hard_link(&temporary, target)
            .map_err(|error| format!("publish artifact `{}`: {error}", target.display()))?;
        std::fs::remove_file(&temporary)
            .map_err(|error| format!("remove artifact staging file: {error}"))?;
        sync_directory(parent, "artifact publication")
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

/// Write `bytes` to `dir/name` atomically (temp + fsync + rename) so a crash
/// never leaves a half-written segment or state file.
fn atomic_write_stream<F>(dir: &Path, name: &str, write: F) -> Result<(), String>
where
    F: FnOnce(&mut std::fs::File) -> Result<(), String>,
{
    std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let tmp = dir.join(format!(".{name}.tmp"));
    let final_path = dir.join(name);
    let result = (|| {
        let mut file = std::fs::File::create(&tmp).map_err(|e| format!("create temp: {e}"))?;
        write(&mut file)?;
        file.sync_all().map_err(|e| format!("fsync temp: {e}"))?;
        std::fs::rename(&tmp, &final_path).map_err(|e| format!("rename into place: {e}"))?;
        sync_directory(dir, "atomic write")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn atomic_write(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let tmp = dir.join(format!(".{name}.tmp"));
    let final_path = dir.join(name);
    {
        use std::io::Write as _;
        let mut file = std::fs::File::create(&tmp).map_err(|e| format!("create temp: {e}"))?;
        file.write_all(bytes)
            .map_err(|e| format!("write temp: {e}"))?;
        file.sync_all().map_err(|e| format!("fsync temp: {e}"))?;
    }
    std::fs::rename(&tmp, &final_path).map_err(|e| format!("rename into place: {e}"))?;
    sync_directory(dir, "atomic write")?;
    Ok(())
}

/// Last archived LSN from `<dir>/archive.state` (0 when absent).
///
/// # Errors
/// Returns a message when the state file exists but cannot be read or parsed.
pub fn read_watermark(dir: &Path) -> Result<u64, String> {
    let path = dir.join(STATE_NAME);
    let raw = match std::fs::read(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(format!("read archive state: {error}")),
    };
    let state: ArchiveState =
        serde_json::from_slice(&raw).map_err(|e| format!("corrupt archive state: {e}"))?;
    Ok(state.last_lsn)
}

/// Archive the WAL range `(last_archived, committed]` into `dir` as one
/// append-only segment, advancing the durable watermark. Returns the new
/// watermark, or `None` when there is nothing new to archive.
///
/// # Errors
/// Returns a message when the export or the durable write fails.
pub fn archive_once(store: &Store, dir: &Path) -> Result<Option<u64>, String> {
    let committed = store.committed_version();
    let last = read_watermark(dir)?;
    if committed <= last {
        return Ok(None);
    }
    let name = format!("wal-{last:020}-{committed:020}.seg");
    atomic_write_stream(dir, &name, |file| {
        store
            .export_wal_segments(file, last.saturating_add(1), committed)
            .map(|_| ())
            .map_err(|e| format!("export wal: {e}"))
    })?;
    let state = serde_json::to_vec(&ArchiveState {
        last_lsn: committed,
    })
    .map_err(|e| format!("encode archive state: {e}"))?;
    atomic_write(dir, STATE_NAME, &state)?;
    Ok(Some(committed))
}

/// Base backups registered in an archive directory, so archive retention
/// knows which WAL a restorable window still needs (SD-7.2).
const BASES_NAME: &str = "bases.json";

/// One base backup the archive can restore from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedBase {
    /// Commit index the base is pinned at; replay starts after it.
    pub commit_index: u64,
    /// When the base was taken (unix milliseconds).
    pub taken_at_ms: u64,
}

fn read_bases(dir: &Path) -> Result<Vec<ArchivedBase>, String> {
    match std::fs::read(dir.join(BASES_NAME)) {
        Ok(raw) => serde_json::from_slice(&raw).map_err(|e| format!("corrupt {BASES_NAME}: {e}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(format!("read {BASES_NAME}: {error}")),
    }
}

/// Register a base backup taken at `commit_index` with the archive in `dir`.
///
/// # Errors
/// Returns a message when the registry cannot be read or rewritten.
pub fn register_base(dir: &Path, commit_index: u64, taken_at_ms: u64) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("create archive dir: {e}"))?;
    let mut bases = read_bases(dir)?;
    bases.push(ArchivedBase {
        commit_index,
        taken_at_ms,
    });
    bases.sort_by_key(|base| (base.taken_at_ms, base.commit_index));
    let encoded = serde_json::to_vec(&bases).map_err(|e| format!("encode bases: {e}"))?;
    atomic_write(dir, BASES_NAME, &encoded)
}

/// What one archive-retention pass removed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ArchivePrune {
    /// Segment files deleted.
    pub segments: usize,
    /// Bytes reclaimed.
    pub bytes: u64,
    /// Segments removed by the byte cap although the window still needed
    /// them (the restorable window shrank).
    pub forced: usize,
}

/// Archive retention (SD-7.2): PITR is "any point within the
/// window". The window needs the newest base taken at or before its start
/// (or, failing that, the oldest base inside it) plus every segment after
/// that base. Older segments go once they are past the window by time; the
/// optional byte cap then removes the oldest segments regardless (and says
/// so). Segments are removed oldest first only, so the archive never has a
/// gap.
///
/// # Errors
/// Returns a message when the directory, a segment or the registry cannot be
/// read or a file cannot be removed.
pub fn prune_archive(
    dir: &Path,
    now_ms: u64,
    window: std::time::Duration,
    max_bytes: Option<u64>,
) -> Result<ArchivePrune, String> {
    let window_start = now_ms.saturating_sub(u64::try_from(window.as_millis()).unwrap_or(u64::MAX));
    let mut segments: Vec<(u64, u64, PathBuf, u64, u64)> = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("read archive dir: {e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((from, to)) = name
            .strip_prefix("wal-")
            .and_then(|rest| rest.strip_suffix(".seg"))
            .and_then(|stem| stem.split_once('-'))
            .and_then(|(from, to)| Some((from.parse::<u64>().ok()?, to.parse::<u64>().ok()?)))
        else {
            continue;
        };
        let meta = entry.metadata().map_err(|e| format!("stat {name}: {e}"))?;
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |at| u64::try_from(at.as_millis()).unwrap_or(u64::MAX));
        segments.push((from, to, entry.path(), meta.len(), modified_ms));
    }
    segments.sort_by_key(|segment| segment.0);
    let bases = read_bases(dir)?;
    // Replay for the window starts after this commit.
    let needed_after = bases
        .iter()
        .filter(|base| base.taken_at_ms <= window_start)
        .map(|base| base.commit_index)
        .max()
        .or_else(|| {
            bases
                .iter()
                .filter(|base| base.taken_at_ms > window_start)
                .map(|base| base.commit_index)
                .min()
        });
    let mut total: u64 = segments.iter().map(|segment| segment.3).sum();
    let mut report = ArchivePrune::default();
    for (_, to, path, bytes, modified_ms) in &segments {
        let expired = *modified_ms < window_start && needed_after.is_none_or(|after| *to <= after);
        let over_cap = max_bytes.is_some_and(|max| total > max);
        if !expired && !over_cap {
            break;
        }
        std::fs::remove_file(path).map_err(|e| format!("remove {}: {e}", path.display()))?;
        total -= bytes;
        report.segments += 1;
        report.bytes += bytes;
        if !expired {
            report.forced += 1;
        }
    }
    // Bases whose replay start was pruned can no longer be restored.
    let oldest_kept = segments.get(report.segments).map(|segment| segment.0);
    let kept: Vec<ArchivedBase> = bases
        .iter()
        .copied()
        .filter(|base| oldest_kept.is_none_or(|from| base.commit_index >= from))
        .collect();
    if kept.len() != bases.len() {
        let encoded = serde_json::to_vec(&kept).map_err(|e| format!("encode bases: {e}"))?;
        atomic_write(dir, BASES_NAME, &encoded)?;
    }
    Ok(report)
}

/// Take a physical base backup of `data_dir` into `out_dir`: copy every
/// `rushdb*.redb` file and write a CRC manifest pinned at `commit_index`.
/// Intended to run offline (`rushdb serve --backup-base`).
///
/// # Errors
/// Returns a message when the data directory has no store file, or a copy/CRC
/// write fails.
pub fn backup_base(
    data_dir: &Path,
    out_dir: &Path,
    commit_index: u64,
) -> Result<BaseManifest, String> {
    let layout_shards = validate_layout_inventory(data_dir)?;
    if out_dir.exists() {
        return Err(format!(
            "refusing to overwrite existing backup target `{}`",
            out_dir.display()
        ));
    }
    let staging = out_dir.with_extension(format!("backup-{}", uuid::Uuid::now_v7()));
    let base_dir = staging.join(BASE_SUBDIR);
    std::fs::create_dir_all(&base_dir).map_err(|e| format!("create base dir: {e}"))?;
    let mut files = Vec::new();
    let mut found = false;
    for entry in std::fs::read_dir(data_dir).map_err(|e| format!("read data dir: {e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        // The store file(s) plus `config.yml` (needed to bootstrap the restored
        // directory; it holds no secrets).
        let is_store = layout_shards.iter().any(|shard| shard == &name);
        let is_layout = name.starts_with("rushdb") && name.ends_with(".layout.json");
        let is_config = name == "config.yml";
        if !is_store && !is_layout && !is_config {
            continue;
        }
        found |= is_store;
        let src = entry.path();
        let dst = base_dir.join(&name);
        copy_file_streaming(&src, &dst).map_err(|e| format!("copy {name}: {e}"))?;
        let (bytes, checksum) = file_size_crc32(&dst)?;
        files.push(BaseFile {
            name,
            bytes,
            crc32: checksum,
        });
    }
    if !found {
        return Err(format!(
            "no `rushdb*.redb` store file in `{}`",
            data_dir.display()
        ));
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    let manifest = BaseManifest {
        format_version: BASE_FORMAT_VERSION,
        command_version: BASE_COMMAND_VERSION,
        protocol_version: BASE_PROTOCOL_VERSION,
        commit_index,
        files,
    };
    let encoded =
        serde_json::to_vec_pretty(&manifest).map_err(|e| format!("encode manifest: {e}"))?;
    atomic_write(&staging, MANIFEST_NAME, &encoded)?;
    if let Err(error) = std::fs::rename(&staging, out_dir) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("publish backup target: {error}"));
    }
    if let Some(parent) = out_dir.parent() {
        sync_directory(parent, "backup publication")?;
    }
    Ok(manifest)
}

/// Online counterpart of [`backup_base`]: runs in-process against the live,
/// already-open store instead of a second `Database::open`, which redb's
/// exclusive file lock would refuse while the server is running. The store
/// files are copied while quiescent (publication held, every shard's redb
/// pausing its writers, readers continuing), so the copy is consistent at
/// the commit index the manifest names; commits wait for the copy.
///
/// # Errors
/// Returns a message when the checkpoint, a file copy, or the manifest write
/// fails.
pub fn backup_base_online(
    store: &Store,
    data_dir: &Path,
    out_dir: &Path,
) -> Result<BaseManifest, String> {
    let layout_shards = validate_layout_inventory(data_dir)?;
    if out_dir.exists() {
        return Err(format!(
            "refusing to overwrite existing backup target `{}`",
            out_dir.display()
        ));
    }
    let staging = out_dir.with_extension(format!("backup-{}", uuid::Uuid::now_v7()));
    let base_dir = staging.join(BASE_SUBDIR);
    std::fs::create_dir_all(&base_dir).map_err(|e| format!("create base dir: {e}"))?;
    // The store files are copied while quiescent at a durable commit, so the
    // copy holds exactly the commit index the manifest names.
    let copied = store
        .with_quiescent_files(|commit_index| {
            copy_base_files(data_dir, &base_dir, &layout_shards).map(|files| (commit_index, files))
        })
        .map_err(|e| e.to_string())
        .and_then(std::convert::identity);
    let (commit_index, files) = match copied {
        Ok(copied) => copied,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }
    };
    let manifest = BaseManifest {
        format_version: BASE_FORMAT_VERSION,
        command_version: BASE_COMMAND_VERSION,
        protocol_version: BASE_PROTOCOL_VERSION,
        commit_index,
        files,
    };
    let encoded =
        serde_json::to_vec_pretty(&manifest).map_err(|e| format!("encode manifest: {e}"))?;
    atomic_write(&staging, MANIFEST_NAME, &encoded)?;
    if let Err(error) = std::fs::rename(&staging, out_dir) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("publish backup target: {error}"));
    }
    if let Some(parent) = out_dir.parent() {
        sync_directory(parent, "backup publication")?;
    }
    Ok(manifest)
}

/// Copy the store file(s), layout manifests and `config.yml` of `data_dir`
/// into `base_dir`, with their sizes and CRCs, sorted by name.
fn copy_base_files(
    data_dir: &Path,
    base_dir: &Path,
    layout_shards: &[String],
) -> Result<Vec<BaseFile>, String> {
    let mut files = Vec::new();
    let mut found = false;
    for entry in std::fs::read_dir(data_dir).map_err(|e| format!("read data dir: {e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_store = layout_shards.iter().any(|shard| shard == &name);
        let is_layout = name.starts_with("rushdb") && name.ends_with(".layout.json");
        let is_config = name == "config.yml";
        if !is_store && !is_layout && !is_config {
            continue;
        }
        found |= is_store;
        let dst = base_dir.join(&name);
        copy_file_streaming(&entry.path(), &dst).map_err(|e| format!("copy {name}: {e}"))?;
        let (bytes, checksum) = file_size_crc32(&dst)?;
        files.push(BaseFile {
            name,
            bytes,
            crc32: checksum,
        });
    }
    if !found {
        return Err(format!(
            "no `rushdb*.redb` store file in `{}`",
            data_dir.display()
        ));
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}

/// Validate the persisted layout manifest and return the exact physical files
/// that belong to the store. Backups must not infer shard membership from a
/// filename glob: a stale or foreign shard would otherwise be silently copied.
fn validate_layout_inventory(data_dir: &Path) -> Result<Vec<String>, String> {
    let mut manifests = Vec::new();
    for entry in std::fs::read_dir(data_dir).map_err(|e| format!("read data dir: {e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("rushdb") && name.ends_with(".layout.json") {
            manifests.push(entry.path());
        }
    }
    if manifests.len() != 1 {
        return Err(format!(
            "expected exactly one storage layout manifest, found {}",
            manifests.len()
        ));
    }
    if !std::fs::symlink_metadata(&manifests[0])
        .map_err(|e| format!("stat layout manifest: {e}"))?
        .file_type()
        .is_file()
    {
        return Err("storage layout manifest must be a regular file".into());
    }
    let raw = std::fs::read(&manifests[0]).map_err(|e| format!("read layout manifest: {e}"))?;
    let manifest: serde_json::Value = serde_json::from_slice(&raw)
        .map_err(|e| format!("corrupt storage layout manifest: {e}"))?;
    let shard_count = manifest
        .get("shard_count")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "storage layout manifest has no shard_count".to_string())?;
    let shard_files = manifest
        .get("shard_files")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "storage layout manifest has no shard_files".to_string())?;
    if shard_files.is_empty()
        || shard_count == 0
        || usize::try_from(shard_count).ok() != Some(shard_files.len())
    {
        return Err("storage layout manifest has no shards".into());
    }
    let mut listed = std::collections::BTreeSet::new();
    for file in shard_files {
        let name = file
            .as_str()
            .ok_or_else(|| "storage layout shard file is not a string".to_string())?;
        if name.contains('/') || name.contains('\\') || name.is_empty() {
            return Err(format!("unsafe storage shard file `{name}`"));
        }
        if !listed.insert(name.to_owned()) {
            return Err(format!("duplicate storage layout shard `{name}`"));
        }
        let shard = data_dir.join(name);
        let metadata = std::fs::symlink_metadata(&shard)
            .map_err(|e| format!("stat storage shard `{name}`: {e}"))?;
        if !metadata.file_type().is_file() {
            return Err(format!("storage layout is missing shard `{name}`"));
        }
    }
    for entry in std::fs::read_dir(data_dir).map_err(|e| format!("read data dir: {e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("rushdb")
            && name.contains(".redb")
            && !name.ends_with(".layout.json")
            && entry
                .file_type()
                .map_err(|e| format!("stat data file: {e}"))?
                .is_file()
            && !listed.contains(&name)
        {
            return Err(format!("storage layout omits shard `{name}`"));
        }
    }
    Ok(shard_files
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_owned)
        .collect())
}

/// Read and validate a base-backup manifest, returning it.
///
/// # Errors
/// Returns a message when the manifest is missing/corrupt or its format is
/// unsupported.
pub fn read_manifest(base_dir: &Path) -> Result<BaseManifest, String> {
    let raw =
        std::fs::read(base_dir.join(MANIFEST_NAME)).map_err(|e| format!("read manifest: {e}"))?;
    let manifest: BaseManifest =
        serde_json::from_slice(&raw).map_err(|e| format!("corrupt manifest: {e}"))?;
    if manifest.format_version != BASE_FORMAT_VERSION {
        return Err(format!(
            "unsupported base format {} (expected {BASE_FORMAT_VERSION})",
            manifest.format_version
        ));
    }
    if manifest.command_version != BASE_COMMAND_VERSION {
        return Err(format!(
            "unsupported command format {} (expected {BASE_COMMAND_VERSION})",
            manifest.command_version
        ));
    }
    if manifest.protocol_version != BASE_PROTOCOL_VERSION {
        return Err(format!(
            "unsupported protocol format {} (expected {BASE_PROTOCOL_VERSION})",
            manifest.protocol_version
        ));
    }
    let mut names = std::collections::BTreeSet::new();
    let mut has_store = false;
    for file in &manifest.files {
        let path = Path::new(&file.name);
        if file.name.is_empty()
            || path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
            || !names.insert(file.name.clone())
        {
            return Err(format!("unsafe or duplicate base filename `{}`", file.name));
        }
        has_store |= file.name.starts_with("rushdb")
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("redb"));
    }
    if !has_store {
        return Err("base manifest contains no rushdb redb store".into());
    }
    Ok(manifest)
}

/// Verify a base backup's files against the manifest (size + CRC). Returns the
/// pinned commit index.
///
/// # Errors
/// Returns a message when a file is missing, the wrong size, or fails CRC.
pub fn verify_base(base_dir: &Path) -> Result<u64, String> {
    let manifest = read_manifest(base_dir)?;
    for file in &manifest.files {
        let path = base_dir.join(BASE_SUBDIR).join(&file.name);
        let (bytes, checksum) = file_size_crc32(&path)?;
        if bytes != file.bytes {
            return Err(format!(
                "base file `{}` size {} != manifest {}",
                file.name, bytes, file.bytes
            ));
        }
        if checksum != file.crc32 {
            return Err(format!("base file `{}` CRC mismatch", file.name));
        }
    }
    Ok(manifest.commit_index)
}

/// Copy a base backup's files into `data_dir` (which must not already hold a
/// store). Returns the pinned commit index.
///
/// # Errors
/// Returns a message when the base is invalid, `data_dir` already has a store,
/// or the copy fails.
pub fn restore_base_into(base_dir: &Path, data_dir: &Path) -> Result<u64, String> {
    let manifest = read_manifest(base_dir)?;
    verify_base(base_dir)?;
    if data_dir.exists() {
        return Err(format!(
            "refusing to overwrite existing `{}`; restore into a fresh data directory",
            data_dir.display()
        ));
    }
    let temporary = data_dir.with_extension(format!("restore-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&temporary).map_err(|e| format!("create restore staging dir: {e}"))?;
    for file in &manifest.files {
        let src = base_dir.join(BASE_SUBDIR).join(&file.name);
        let dst = temporary.join(&file.name);
        if let Err(error) = copy_file_streaming(&src, &dst) {
            let _ = std::fs::remove_dir_all(&temporary);
            return Err(format!("copy {}: {error}", file.name));
        }
    }
    if let Err(error) = sync_directory(&temporary, "restore staging") {
        let _ = std::fs::remove_dir_all(&temporary);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&temporary, data_dir) {
        let _ = std::fs::remove_dir_all(&temporary);
        return Err(format!("publish restored data dir: {error}"));
    }
    if let Some(parent) = data_dir.parent() {
        sync_directory(parent, "restore publication")?;
    }
    Ok(manifest.commit_index)
}

/// A `Read` stream over every archived segment in `dir`, in LSN order, so a
/// PITR restore can replay them as one logical WAL.
///
/// # Errors
/// Returns a message when the directory cannot be read.
#[expect(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "the engine controls these file names and always writes them lowercase"
)]
pub fn segment_stream(dir: &Path) -> Result<Box<dyn Read>, String> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("read archive dir: {e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("wal-") && name.ends_with(".seg") {
            paths.push(entry.path());
        }
    }
    paths.sort();
    let mut expected_from = None;
    for path in &paths {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("invalid archive segment path `{}`", path.display()))?;
        let stem = name
            .strip_prefix("wal-")
            .and_then(|name| name.strip_suffix(".seg"))
            .ok_or_else(|| format!("invalid archive segment name `{name}`"))?;
        let (from, to) = stem
            .split_once('-')
            .ok_or_else(|| format!("invalid archive segment name `{name}`"))?;
        let from = from
            .parse::<u64>()
            .map_err(|_| format!("invalid archive segment start `{name}`"))?;
        let to = to
            .parse::<u64>()
            .map_err(|_| format!("invalid archive segment end `{name}`"))?;
        if from > to || expected_from.is_some_and(|expected| from != expected) {
            return Err(format!("archive WAL gap or overlap at `{name}`"));
        }
        expected_from = Some(to);
    }
    let mut reader: Box<dyn Read> = Box::new(std::io::empty());
    for path in paths {
        let file =
            std::fs::File::open(&path).map_err(|e| format!("open {}: {e}", path.display()))?;
        reader = Box::new(reader.chain(std::io::BufReader::new(file)));
    }
    Ok(reader)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY_MS: u64 = 24 * 3600 * 1000;

    fn segment(dir: &Path, from: u64, to: u64, bytes: usize, age_days: u64, now_ms: u64) {
        let path = dir.join(format!("wal-{from:020}-{to:020}.seg"));
        std::fs::write(&path, vec![0u8; bytes]).unwrap();
        let at =
            std::time::UNIX_EPOCH + std::time::Duration::from_millis(now_ms - age_days * DAY_MS);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(at)
            .unwrap();
    }

    fn remaining(dir: &Path) -> Vec<u64> {
        let mut froms: Vec<u64> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|entry| {
                let name = entry.unwrap().file_name().to_string_lossy().into_owned();
                name.strip_prefix("wal-")?.split('-').next()?.parse().ok()
            })
            .collect();
        froms.sort_unstable();
        froms
    }

    #[test]
    fn archive_prunes_outside_window_but_keeps_restorable_base() {
        let dir = rushdb_storage::TempPath::dir("archive-prune");
        std::fs::create_dir_all(&dir).unwrap();
        let now = 100 * DAY_MS;
        // Segments (from, to] aged 20, 15, 10, 5 and 1 days.
        segment(&dir, 0, 10, 100, 20, now);
        segment(&dir, 10, 20, 100, 15, now);
        segment(&dir, 20, 30, 100, 10, now);
        segment(&dir, 30, 40, 100, 5, now);
        segment(&dir, 40, 50, 100, 1, now);
        // Bases at commit 10 (16 days old) and 25 (9 days old). A 7-day
        // window starts 7 days ago: the base covering its start is the one at
        // 25, so every segment after 25 must stay, including (20, 30].
        register_base(&dir, 10, now - 16 * DAY_MS).unwrap();
        register_base(&dir, 25, now - 9 * DAY_MS).unwrap();
        let window = std::time::Duration::from_millis(7 * DAY_MS);
        let pruned = prune_archive(&dir, now, window, None).unwrap();
        assert_eq!(pruned.segments, 2);
        assert_eq!(pruned.forced, 0);
        assert_eq!(remaining(&dir), vec![20, 30, 40]);
        // The base at 10 can no longer be restored and is unregistered; 25
        // stays.
        assert_eq!(
            read_bases(&dir).unwrap(),
            vec![ArchivedBase {
                commit_index: 25,
                taken_at_ms: now - 9 * DAY_MS
            }]
        );
        // Idempotent.
        assert_eq!(prune_archive(&dir, now, window, None).unwrap().segments, 0);
        // A byte cap is hard: it removes the oldest segments even inside
        // the window, and reports them as forced.
        let capped = prune_archive(&dir, now, window, Some(150)).unwrap();
        assert_eq!((capped.segments, capped.forced), (2, 2));
        assert_eq!(remaining(&dir), vec![40]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn layout_inventory_rejects_missing_or_foreign_shards() {
        let root = rushdb_storage::TempPath::dir("archive");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("rushdb.redb"), []).unwrap();
        std::fs::write(
            root.join("rushdb.redb.layout.json"),
            r#"{"shard_count":2,"shard_files":["rushdb.redb","rushdb.redb-1"]}"#,
        )
        .unwrap();
        assert!(validate_layout_inventory(&root).is_err());
        std::fs::write(root.join("rushdb.redb-1"), []).unwrap();
        let files = validate_layout_inventory(&root).unwrap();
        assert_eq!(files, vec!["rushdb.redb", "rushdb.redb-1"]);
        std::fs::write(root.join("rushdb.redb-2"), []).unwrap();
        assert!(validate_layout_inventory(&root)
            .unwrap_err()
            .contains("omits shard"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn watermark_propagates_non_not_found_read_errors() {
        let root = rushdb_storage::TempPath::dir("archive-watermark");
        std::fs::create_dir_all(root.join(STATE_NAME)).unwrap();
        let error = read_watermark(&root).unwrap_err();
        assert!(error.contains("read archive state"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn atomic_publish_stream_refuses_existing_targets_and_cleans_staging() {
        let root = rushdb_storage::TempPath::dir("archive-publish");
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("snapshot.jsonl");
        atomic_publish_stream(&target, |file| {
            file.write_all(b"first").map_err(|error| error.to_string())
        })
        .unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"first");
        let error = atomic_publish_stream(&target, |file| {
            file.write_all(b"second").map_err(|error| error.to_string())
        })
        .unwrap_err();
        assert!(error.contains("publish artifact"));
        assert_eq!(std::fs::read(&target).unwrap(), b"first");
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        std::fs::remove_dir_all(root).ok();
    }
}
