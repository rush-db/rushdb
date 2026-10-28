//! Durable storage-layout identity and shard-set validation.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAGIC: &str = "RUSHDB_LAYOUT";
/// v2 (2026-10-07): data rows carry the commit index that wrote them as an
/// 8-byte header and the separate `created` table is gone; conflict
/// summaries are no longer stored.
/// v3: tenant keys start with `[TENANT_NS][tenant u32]` from the
/// tenant directory instead of `T/<32-byte digest>`.
/// v4: layout v3 — slim occurrence rows, label-scoped presence,
/// value dictionary and counters derived at apply time (DC10–DC12); no
/// posting rows.
/// v5 (lever C, 2026-10-10): record-header, label-member, occurrence,
/// presence and value-dictionary keys encode ordinals and property ids as
/// order-preserving variable-length integers (`keys::push_ord`).
/// v6 (2026-10-27): values too long for the value dictionary are refcounted
/// by digest at apply time (`C_LONG_VALUE`, `C_LONG_DISTINCT`), so their
/// distinct counts are exact without a scan; a v5 store lacks those rows.
const FORMAT_VERSION: u32 = 6;
/// v2: 4-byte directory ordinals replace 32-byte scope digests.
const TENANT_ENCODING_VERSION: u32 = 2;
const HISTORY_VERSION: u32 = 2;

/// Validated persisted layout identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayoutInfo {
    /// Stable database identity from the manifest.
    pub database_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LayoutManifest {
    magic: String,
    format_version: u32,
    database_id: String,
    tenant_encoding_version: u32,
    history_version: u32,
    shard_count: usize,
    shard_files: Vec<String>,
}

fn manifest_path(path: &Path) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(".layout.json");
    PathBuf::from(value)
}

fn shard_name(path: &Path, shard: usize) -> String {
    if shard == 0 {
        path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    } else {
        format!(
            "{}-{shard}",
            path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into_owned()
            )
        )
    }
}

fn shard_path(path: &Path, shard: usize) -> PathBuf {
    if shard == 0 {
        path.to_path_buf()
    } else {
        let mut value = path.as_os_str().to_os_string();
        value.push(format!("-{shard}"));
        PathBuf::from(value)
    }
}

/// Validate an existing layout before opening any writable shard.
pub(crate) fn validate_layout(
    path: &Path,
    shard_count: usize,
) -> Result<Option<LayoutInfo>, String> {
    let manifest = manifest_path(path);
    let raw = match std::fs::read(&manifest) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if path.exists() {
                return Err(format!(
                    "storage at {} has no layout manifest; existing stores must use the current format",
                    path.display()
                ));
            }
            return Ok(None);
        }
        Err(error) => {
            return Err(format!(
                "read storage layout manifest {}: {error}",
                manifest.display()
            ));
        }
    };
    let value: LayoutManifest = serde_json::from_slice(&raw)
        .map_err(|error| format!("corrupt storage layout manifest: {error}"))?;
    if value.magic != MAGIC {
        return Err("unsupported storage layout manifest".into());
    }
    if value.format_version != FORMAT_VERSION {
        return Err(format!(
            "unsupported storage format version {}; this build uses version {FORMAT_VERSION} \
             (stores written by older builds must be recreated or restored from a logical export)",
            value.format_version
        ));
    }
    if value.tenant_encoding_version != TENANT_ENCODING_VERSION
        || value.history_version != HISTORY_VERSION
    {
        return Err("incompatible storage encoding/history version".into());
    }
    if value.shard_count != shard_count {
        return Err(format!(
            "storage shard count {} does not match configured {shard_count}",
            value.shard_count
        ));
    }
    let expected: Vec<String> = (0..shard_count)
        .map(|shard| shard_name(path, shard))
        .collect();
    if value.shard_files != expected {
        return Err("storage layout shard file set does not match the manifest".into());
    }
    for shard in 0..shard_count {
        if !shard_path(path, shard).is_file() {
            return Err(format!("storage layout is missing shard {shard}"));
        }
    }
    Ok(Some(LayoutInfo {
        database_id: value.database_id,
    }))
}

/// Persist a new layout manifest after all shard backends have opened.
pub fn create_manifest(path: &Path, shard_count: usize) -> Result<String, String> {
    let database_id = uuid::Uuid::now_v7().to_string();
    let value = LayoutManifest {
        magic: MAGIC.into(),
        format_version: FORMAT_VERSION,
        database_id: database_id.clone(),
        tenant_encoding_version: TENANT_ENCODING_VERSION,
        history_version: HISTORY_VERSION,
        shard_count,
        shard_files: (0..shard_count)
            .map(|shard| shard_name(path, shard))
            .collect(),
    };
    let bytes = serde_json::to_vec_pretty(&value).map_err(|error| error.to_string())?;
    let manifest = manifest_path(path);
    let temporary = manifest.with_extension("json.tmp");
    if let Some(parent) = manifest.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&temporary).map_err(|error| error.to_string())?;
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
    }
    std::fs::rename(&temporary, &manifest).map_err(|error| error.to_string())?;
    sync_parent_directory(&manifest)?;
    Ok(database_id)
}

fn sync_parent_directory(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "layout manifest has no parent directory".to_string())?;
    let file = std::fs::File::open(parent)
        .map_err(|error| format!("open layout parent for fsync: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("fsync layout parent directory: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_manifest_rejects_shard_mismatch_and_missing_files() {
        let root = crate::TempPath::dir("layout");
        let path = root.join("rushdb.redb");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&path, []).unwrap();
        std::fs::write(root.join("rushdb.redb-1"), []).unwrap();
        create_manifest(&path, 2).unwrap();
        assert!(validate_layout(&path, 1).is_err());
        std::fs::remove_file(root.join("rushdb.redb-1")).unwrap();
        assert!(validate_layout(&path, 2).is_err());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn existing_store_without_manifest_is_rejected() {
        let root = crate::TempPath::dir("layout");
        let path = root.join("legacy.redb");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&path, []).unwrap();
        let error = validate_layout(&path, 1).unwrap_err();
        assert!(error.contains("no layout manifest"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn layout_manifest_read_errors_do_not_look_like_missing_manifests() {
        let root = crate::TempPath::dir("layout");
        let path = root.join("io-error.redb");
        let manifest = manifest_path(&path);
        std::fs::create_dir_all(&manifest).unwrap();
        std::fs::write(&path, []).unwrap();
        let error = validate_layout(&path, 1).unwrap_err();
        assert!(error.contains("read storage layout manifest"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn pre_layout_v3_stores_are_rejected() {
        let root = crate::TempPath::dir("layout");
        let path = root.join("marked.redb");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&path, []).unwrap();
        create_manifest(&path, 1).unwrap();
        let manifest = manifest_path(&path);
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
        assert!(validate_layout(&path, 1).unwrap().is_some());
        // A format-3 store has posting rows and no derived rows.
        value["format_version"] = serde_json::json!(3);
        std::fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(validate_layout(&path, 1)
            .unwrap_err()
            .contains("unsupported storage format version 3"));
        std::fs::remove_dir_all(root).ok();
    }
}
