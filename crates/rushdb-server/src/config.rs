//! Data-directory configuration record (`config.yml`).
//!
//! `config.yml` lives in the data directory and is written on first start. It
//! records the effective configuration that the directory was created under and
//! documents the whole `rushdb serve` option surface with commented defaults, so an
//! operator can inspect and adjust it without reading the CLI reference.
//!
//! Precedence:
//! - `data_dir`, `token` and `capability_secret` are always supplied by the
//!   CLI / environment (they are needed to locate the file, and secrets are
//!   never persisted here).
//! - Ports and resource/transaction sections are read from `config.yml` when present (the file
//!   is the source of truth for a data directory).
//! - The tenancy `mode` is an **isolation invariant**: an existing data
//!   directory cannot switch between single-tenant and multitenant modes. A
//!   mismatch is refused, because that would silently reinterpret the stored
//!   graph under a different isolation regime. To change the mode, migrate to a
//!   fresh data directory.

use serde::{Deserialize, Serialize};

use crate::ServerConfig;

/// Borrowed pair of resource sections serialized into the rendered file.
#[derive(Serialize)]
struct Limits<'a> {
    resources: &'a crate::resource_config::ResourceConfig,
    transactions: &'a crate::resource_config::TransactionConfig,
}

/// Schema version written to `config.yml`; mismatches refuse to boot.
pub const CONFIG_VERSION: u32 = 1;

/// File name of the configuration record inside a data directory.
pub const CONFIG_FILE_NAME: &str = "config.yml";

/// Versioned language enablement section rendered into `config.yml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LanguageSection {
    languages: rushdb_protocol::LanguageSet,
}

/// The persisted configuration record. Security secrets are intentionally
/// absent: they are supplied at boot and never written to disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ConfigFile {
    /// Schema version of this record (see [`CONFIG_VERSION`]).
    pub version: u32,
    /// HTTP/Pgwire/gRPC/Bolt listener settings.
    pub server: ServerSection,
    /// Tenancy mode and HTTP scope header.
    pub tenancy: TenancySection,
    /// Absent in older v1 files: retain CLI/environment/embedded settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<crate::resource_config::ResourceConfig>,
    /// Transaction limits. Absent in older v1 files: retain the startup
    /// settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transactions: Option<crate::resource_config::TransactionConfig>,
    /// Startup query-language enablement. Absent = defaults
    /// (`searchquery` only; every statement language disabled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub languages: Option<rushdb_protocol::LanguageSet>,
}

/// Persisted listener settings (`server` section of `config.yml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ServerSection {
    /// HTTP protocol adapter port.
    pub port: u16,
    /// HTTP bind address (default `127.0.0.1`). Absent = retain the startup
    /// setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_addr: Option<String>,
    /// PEM certificate chain path for HTTPS. Both cert and key are required.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_cert: Option<String>,
    /// PEM private key path (PKCS#8) for HTTPS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_key: Option<String>,
    /// PostgreSQL wire-protocol port.
    pub pgwire_port: u16,
    /// Maintenance mode: rejects writes and marks `/health` degraded.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub maintenance: bool,
    /// Gate `/ready` on vector-index warmup (no unindexed/stale bindings).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub require_index_warmup: bool,
    /// gRPC transport port (`0` = disabled). Absent in older v1 files:
    /// retain the startup setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grpc_port: Option<u16>,
    /// Bolt (Neo4j wire) transport port (`0` = disabled). Absent in older v1
    /// files: retain the startup setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bolt_port: Option<u16>,
    /// Maximum protocol payload (HTTP request body, gRPC per-message, response
    /// budget). Bytes or B/KiB/MiB/GiB. Absent = 16MiB default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_limit: Option<crate::resource_config::ByteSize>,
    /// Persistent database directory (informational; the running server gets
    /// it from the CLI / `RUSHDB_DATA_DIR`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_dir: Option<String>,
}

/// Persisted tenancy settings (`tenancy` section of `config.yml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TenancySection {
    /// `single` or `multitenant`. Isolation invariant — see module docs.
    pub mode: String,
}

impl ConfigFile {
    fn mode_string(multitenant: bool) -> &'static str {
        if multitenant {
            "multitenant"
        } else {
            "single"
        }
    }

    /// Render a commented `config.yml` for the effective configuration.
    ///
    /// # Panics
    ///
    /// Panics only if a numeric resource setting or language set cannot be
    /// serialized to YAML, which cannot happen for the plain numeric/bool
    /// fields in these sections.
    #[must_use]
    pub fn render(cfg: &ServerConfig) -> String {
        let mode = Self::mode_string(cfg.multitenant);
        let data_dir = cfg
            .data_dir
            .as_ref()
            .map(|d| d.display().to_string())
            .unwrap_or_default();
        let data_dir_line = if data_dir.is_empty() {
            "  # data-dir: (in-memory node)".to_string()
        } else {
            // A JSON string is a YAML-compatible quoted scalar, including
            // paths containing colons, hashes, quotes, or newlines.
            format!("  data-dir: {}", serde_json::to_string(&data_dir).unwrap())
        };
        #[allow(clippy::format_in_format_args)]
        let mut rendered = format!(
            "# RushDB engine configuration.\n\
             # Generated on first start. Edit to change and restart to apply.\n\
             #\n\
             # Security secrets are never stored here. Set them via the CLI or\n\
             # environment:\n\
             #   token:             --token / RUSHDB_TOKEN\n\
             #   capability-secret: --capability-secret (HMAC secret enabling\n\
             #                       signed X-RushDB-Token capability auth)\n\
             #\n\
             # The tenancy mode is an isolation invariant: an existing data\n\
             # directory cannot switch between single-tenant and multitenant\n\
             # modes. To change it, migrate to a fresh data directory.\n\
             version: {}\n\
             server:\n\
             #   HTTP protocol adapter port.\n\
             {}\n\
             #   HTTP bind address (default 127.0.0.1). Use 0.0.0.0 to expose\n\
             #   beyond loopback (pair with TLS).\n\
             {}\n\
             #   HTTPS PEM certificate chain path (requires tls-key too).\n\
             {}\n\
             #   HTTPS PEM private key path (PKCS#8).\n\
             {}\n\
             #   PostgreSQL wire-protocol port.\n\
             {}\n\
             #   gRPC transport port (0 = disabled).\n\
             {}\n\
             #   Bolt (Neo4j wire) transport port (0 = disabled).\n\
             {}\n\
             #   Maximum protocol payload (HTTP request body, gRPC per-message,\n\
             #   response budget). Bytes or B/KiB/MiB/GiB. Default 16MiB.\n\
             {}\n\
             #   Maintenance mode (default false): reject writes and mark\n\
             #   /health as maintenance until cleared.\n\
             {}\n\
             #   Persistent database directory.\n\
             {}\n\
             tenancy:\n\
             #   single | multitenant\n\
             {}\n",
            CONFIG_VERSION,
            format!("  port: {}", cfg.port),
            format!("  listen-addr: {}", cfg.listen_addr),
            cfg.tls_cert.as_ref().map_or_else(
                || "  # tls-cert: (TLS disabled)".to_string(),
                |c| format!("  tls-cert: {}", serde_json::to_string(c).unwrap())
            ),
            cfg.tls_key.as_ref().map_or_else(
                || "  # tls-key: (TLS disabled)".to_string(),
                |k| format!("  tls-key: {}", serde_json::to_string(k).unwrap())
            ),
            format!("  pgwire-port: {}", cfg.pgwire_port),
            format!("  grpc-port: {}", cfg.grpc_port),
            format!("  bolt-port: {}", cfg.bolt_port),
            format!("  body-limit: {}B", cfg.max_request_bytes),
            format!("  maintenance: {}", cfg.maintenance),
            data_dir_line,
            format!("  mode: {mode}"),
        );
        rendered.push_str("# This file is the source of truth for this data directory: a startup flag or\n# environment variable that disagrees with a value here is a startup error.\n# Byte values are bytes; B/KiB/MiB/GiB strings are also accepted.\n# Managed reservations are NOT a process RSS cap.\n");
        rendered.push_str(
            &serde_yml::to_string(&Limits {
                resources: &cfg.resources,
                transactions: &cfg.transactions,
            })
            .expect("serializing numeric resource settings"),
        );
        // Languages section: searchquery is always enabled and not listed.
        let languages = cfg.languages;
        rendered.push('\n');
        rendered.push_str(
            &serde_yml::to_string(&LanguageSection { languages })
                .expect("serializing language settings"),
        );
        rendered.push('\n');
        rendered.push_str(
            "# searchquery (the native query document) is always enabled and cannot be disabled.\n",
        );
        rendered
    }

    /// Load and parse `config.yml` from the data directory, if it exists.
    ///
    /// # Errors
    ///
    /// Returns a message when the file exists but cannot be read, is not UTF-8,
    /// or does not parse as a [`ConfigFile`].
    pub fn load(data_dir: &std::path::Path) -> Result<Option<ConfigFile>, String> {
        let path = data_dir.join(CONFIG_FILE_NAME);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("read config `{}`: {error}", path.display()))?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|error| format!("config `{}` is not UTF-8: {error}", path.display()))?;
        serde_yml::from_str(text)
            .map(Some)
            .map_err(|error| format!("invalid config `{}`: {error}", path.display()))
    }
}

/// Write the config record into the data directory.
///
/// # Errors
///
/// Returns a message when the rendered `config.yml` cannot be written.
pub fn write_config(data_dir: &std::path::Path, cfg: &ServerConfig) -> Result<(), String> {
    let path = data_dir.join(CONFIG_FILE_NAME);
    std::fs::write(&path, ConfigFile::render(cfg))
        .map_err(|error| format!("write config `{}`: {error}", path.display()))
}

/// Rewrite `config.yml` without the node's replica role (after promotion, or
/// for a directory restored from another node's backup) and, when
/// `drop_archive` is set, without its WAL archive directory (a restored node
/// must not append to its source's archive). `cfg` is the effective config.
///
/// # Errors
/// Returns a message when `config.yml` cannot be written.
pub fn persist_without_roles(
    data_dir: &std::path::Path,
    cfg: &ServerConfig,
    drop_archive: bool,
) -> Result<(), String> {
    let mut detached = cfg.clone();
    detached.resources.replica_of = None;
    detached.resources.replica_token = None;
    if drop_archive {
        detached.resources.wal_archive_dir = None;
    }
    write_config(data_dir, &detached)
}

/// Settings that are this boot's role assignment rather than shared
/// directory configuration: an explicit flag sets them without being a
/// conflict with `config.yml` (which keeps them for flagless restarts).
const ROLE_FLAGS: [&str; 3] = ["replica-of", "replica-id", "replica-token"];

/// Validate an existing config record against the requested configuration.
///
/// The tenancy mode is the isolation invariant: it must match exactly.
/// `port`/`pgwire_port` are applied back onto the requested config (the file is
/// authoritative for a data directory). Secrets and `data_dir` are never read
/// from the file.
///
/// # Errors
///
/// Returns a message when the file's schema version or tenancy mode does not
/// match the running engine, when only one half of the TLS key pair is present,
/// or when the stored resource/transaction limits are invalid.
pub fn validate_config(file: &ConfigFile, requested: &mut ServerConfig) -> Result<(), String> {
    if file.version != CONFIG_VERSION {
        return Err(format!(
            "config version mismatch: file v{}, engine v{}; migrate to a new data directory",
            file.version, CONFIG_VERSION
        ));
    }
    let expected_mode = ConfigFile::mode_string(requested.multitenant);
    if file.tenancy.mode != expected_mode {
        return Err(format!(
            "data directory tenancy mismatch: stored {}, requested {}; migrate into a new data directory",
            file.tenancy.mode, expected_mode
        ));
    }
    let mut resources = file
        .resources
        .clone()
        .unwrap_or_else(|| requested.resources.clone());
    // Replica identity is this boot's role assignment, not durable shared
    // cluster config: a data directory restored from another node's base
    // backup carries that node's own `config.yml` (e.g. a leader's, with no
    // `replica_of` set at all), and letting it win here would silently
    // strip the follower role before `bind_router` ever sees it. An
    // explicit `requested.resources.replica_of` for this boot always wins;
    // absent that, the stored file's value (set on a follower's own first
    // boot) is kept so a plain restart need not repeat the setting.
    if requested.resources.replica_of.is_some() {
        resources
            .replica_of
            .clone_from(&requested.resources.replica_of);
        resources
            .replica_id
            .clone_from(&requested.resources.replica_id);
        resources
            .replica_token
            .clone_from(&requested.resources.replica_token);
    }
    let conflicts = explicit_flag_conflicts(file, requested);
    if !conflicts.is_empty() {
        return Err(format!(
            "config.yml disagrees with explicitly supplied startup flags ({}); \
             config.yml is the source of truth for this data directory: edit it or drop the flag",
            conflicts.join("; ")
        ));
    }
    let transactions = file.transactions.unwrap_or(requested.transactions);
    resources.storage()?;
    transactions.validate(&resources)?;
    // Apply only after all checks succeed. Present sections are authoritative;
    // absent sections preserve old-v1 compatibility and startup overrides.
    requested.port = file.server.port;
    requested.pgwire_port = file.server.pgwire_port;
    if let Some(grpc_port) = file.server.grpc_port {
        requested.grpc_port = grpc_port;
    }
    if let Some(bolt_port) = file.server.bolt_port {
        requested.bolt_port = bolt_port;
    }
    if let Some(listen_addr) = &file.server.listen_addr {
        requested.listen_addr.clone_from(listen_addr);
    }
    if let (Some(cert), Some(key)) = (&file.server.tls_cert, &file.server.tls_key) {
        requested.tls_cert = Some(cert.into());
        requested.tls_key = Some(key.into());
    } else if file.server.tls_cert.is_some() || file.server.tls_key.is_some() {
        return Err("tls-cert and tls-key must be set together".to_string());
    }
    if let Some(body_limit) = file.server.body_limit {
        requested.max_request_bytes = body_limit.0;
    }
    // These are independent of `body-limit`; applying them only when a body
    // limit was present silently ignored `maintenance: true` in a config file.
    requested.maintenance = file.server.maintenance;
    requested.require_index_warmup = file.server.require_index_warmup;
    requested.resources = resources;
    requested.transactions = transactions;
    if let Some(languages) = file.languages {
        requested.languages = languages;
    }
    Ok(())
}

/// Apply the data directory's stored `config.yml` (if any) to `cfg` before the
/// node boots, so the binary knows the effective listen address and port
/// early enough to answer probes during a long bootstrap. Idempotent:
/// `Node::bootstrap` applies the same file again.
///
/// # Errors
/// Returns the same validation errors as [`validate_config`].
pub fn resolve_stored_config(cfg: &mut ServerConfig) -> Result<(), String> {
    let Some(dir) = cfg.data_dir.as_deref() else {
        return Ok(());
    };
    if let Some(file) = ConfigFile::load(dir)? {
        validate_config(&file, cfg)?;
    }
    Ok(())
}

/// Every explicitly supplied startup flag whose value disagrees with the
/// stored `config.yml`. Formatted as `--flag (flag X, config.yml Y)`.
fn explicit_flag_conflicts(file: &ConfigFile, requested: &ServerConfig) -> Vec<String> {
    fn differs<T: PartialEq + std::fmt::Debug>(flag: &str, cli: &T, stored: &T) -> Option<String> {
        (cli != stored).then(|| format!("--{flag} (flag {cli:?}, config.yml {stored:?})"))
    }
    let section = |value: Option<serde_json::Value>| match value {
        Some(serde_json::Value::Object(map)) => Some(map),
        _ => None,
    };
    let stored_resources = section(
        file.resources
            .as_ref()
            .and_then(|r| serde_json::to_value(r).ok()),
    );
    let requested_resources = section(serde_json::to_value(&requested.resources).ok());
    let stored_transactions = section(
        file.transactions
            .as_ref()
            .and_then(|t| serde_json::to_value(t).ok()),
    );
    let requested_transactions = section(serde_json::to_value(requested.transactions).ok());
    let mut conflicts = Vec::new();
    for flag in &requested.explicit_flags {
        if ROLE_FLAGS.contains(&flag.as_str()) {
            continue;
        }
        let conflict = match flag.as_str() {
            "port" => differs(flag, &requested.port, &file.server.port),
            "pgwire-port" => differs(flag, &requested.pgwire_port, &file.server.pgwire_port),
            "grpc-port" => file
                .server
                .grpc_port
                .and_then(|stored| differs(flag, &requested.grpc_port, &stored)),
            "bolt-port" => file
                .server
                .bolt_port
                .and_then(|stored| differs(flag, &requested.bolt_port, &stored)),
            "listen" => file
                .server
                .listen_addr
                .as_ref()
                .and_then(|stored| differs(flag, &requested.listen_addr, stored)),
            "maintenance" => differs(flag, &requested.maintenance, &file.server.maintenance),
            "require-index-warmup" => differs(
                flag,
                &requested.require_index_warmup,
                &file.server.require_index_warmup,
            ),
            "languages" => file
                .languages
                .as_ref()
                .and_then(|stored| differs(flag, &requested.languages, stored)),
            other => {
                let (stored, current, key) = match other.strip_prefix("tx-") {
                    Some(key) => (&stored_transactions, &requested_transactions, key),
                    None => (&stored_resources, &requested_resources, other),
                };
                match (stored, current) {
                    (Some(stored), Some(current)) => match (stored.get(key), current.get(key)) {
                        (Some(stored), Some(current)) => differs(flag, current, stored),
                        _ => None,
                    },
                    _ => None,
                }
            }
        };
        conflicts.extend(conflict);
    }
    conflicts
}

/// Ensure a data directory has a valid `config.yml`.
///
/// - missing + committed data → refuse (the directory was created by a version
///   that predates the config record);
/// - missing + fresh → write the effective configuration;
/// - present → validate (tenancy mode must match) and apply stored ports.
///
/// # Errors
///
/// Returns a message when a committed data directory has no `config.yml`, when
/// loading or validating the stored record fails, or when the fresh record
/// cannot be written.
pub fn sync_config(
    data_dir: &std::path::Path,
    cfg: &mut ServerConfig,
    committed_version: u64,
) -> Result<(), String> {
    if let Some(file) = ConfigFile::load(data_dir)? {
        validate_config(&file, cfg)
    } else {
        if committed_version > 0 {
            return Err(
                "existing data directory has no config.yml; migrate it explicitly before reuse"
                    .to_string(),
            );
        }
        cfg.resources.storage()?;
        cfg.transactions.validate(&cfg.resources)?;
        write_config(data_dir, cfg)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn print_languages_render() {
        let cfg = crate::ServerConfig::default();
        eprintln!(
            "===RENDERED===\n{}\n===END===",
            crate::config::ConfigFile::render(&cfg)
        );
    }
    use super::*;

    fn cfg(multitenant: bool) -> ServerConfig {
        ServerConfig {
            port: 3100,
            pgwire_port: 5544,
            grpc_port: 0,
            bolt_port: 0,
            token: "t".into(),
            data_dir: None,
            multitenant,
            capability_secret: None,
            raft: None,
            resources: crate::resource_config::ResourceConfig::default(),
            transactions: crate::resource_config::TransactionConfig::default(),
            languages: rushdb_protocol::LanguageSet::default(),
            max_request_bytes: 64 * 1024 * 1024,
            listen_addr: "127.0.0.1".into(),
            tls_cert: None,
            tls_key: None,
            maintenance: false,
            allow_insecure_listen: false,
            require_index_warmup: false,
            request_timeout_ms: 0,
            max_concurrent_requests_per_tenant: 0,
            explicit_flags: std::collections::BTreeSet::new(),
        }
    }

    fn temp_dir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "rushdb-config-test-{}-{}",
            std::process::id(),
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn writes_and_round_trips_the_effective_config() {
        let dir = temp_dir();
        let mut requested = cfg(false);
        sync_config(&dir, &mut requested, 0).unwrap();

        let loaded = ConfigFile::load(&dir).unwrap().unwrap();
        assert_eq!(loaded.version, CONFIG_VERSION);
        assert_eq!(loaded.server.port, 3100);
        assert_eq!(loaded.server.pgwire_port, 5544);
        assert_eq!(loaded.tenancy.mode, "single");

        // restart applies stored ports back onto the requested config
        let mut restart = ServerConfig {
            port: 9999,
            pgwire_port: 8888,
            max_request_bytes: 64 * 1024 * 1024,
            listen_addr: "127.0.0.1".into(),
            tls_cert: None,
            tls_key: None,
            ..cfg(false)
        };
        sync_config(&dir, &mut restart, 99).unwrap();
        assert_eq!(restart.port, 3100);
        assert_eq!(restart.pgwire_port, 5544);
        assert!(ConfigFile::load(&dir).unwrap().is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn multitenant_mode_is_an_isolation_invariant() {
        let dir = temp_dir();
        let mut single = cfg(false);
        sync_config(&dir, &mut single, 0).unwrap();

        // trying to boot the same directory as multitenant is refused
        let mut multi = cfg(true);
        let err = sync_config(&dir, &mut multi, 99).unwrap_err();
        assert!(err.contains("tenancy mismatch"), "{err}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_config_on_committed_data_is_refused() {
        let dir = temp_dir();
        let mut requested = cfg(true);
        let err = sync_config(&dir, &mut requested, 5).unwrap_err();
        assert!(err.contains("no config.yml"), "{err}");
        // nothing was written
        assert!(!dir.join(CONFIG_FILE_NAME).exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn render_documents_security_options_as_comments() {
        let rendered = ConfigFile::render(&cfg(true));
        assert!(rendered.contains("mode: multitenant"));
        assert!(rendered.contains("--capability-secret"));
        assert!(rendered.contains("--token"));
        // secrets are never persisted
        assert!(!rendered.contains("capability_secret: t"));
    }

    #[test]
    fn old_v1_config_without_resource_sections_preserves_startup_settings() {
        let file: ConfigFile = serde_yml::from_str(
            "version: 1\nserver:\n  port: 1\n  pgwire-port: 2\ntenancy:\n  mode: single\n",
        )
        .unwrap();
        let mut requested = cfg(false);
        requested.resources.storage_cache = crate::resource_config::ByteSize(64 * 1024 * 1024);
        requested.transactions.max_active = 3;
        let resources = requested.resources.clone();
        validate_config(&file, &mut requested).unwrap();
        assert_eq!(requested.resources, resources);
        assert_eq!(requested.transactions.max_active, 3);
        assert_eq!(requested.port, 1);
    }

    #[test]
    fn stored_resource_sections_override_startup_and_round_trip() {
        let mut stored = cfg(false);
        stored.resources.storage_cache = crate::resource_config::ByteSize(64 * 1024 * 1024);
        stored.resources.rss_max = crate::resource_config::ByteSize(2 * 1024 * 1024 * 1024);
        stored.resources.checkpoint_interval_seconds = 15;
        stored.resources.tx_log_retention_commits = 4242;
        stored.resources.logs_query_threshold_ms = 750;
        stored.resources.metrics_prefix = Some("rushdb_".into());
        stored.resources.metrics_filter = Some("budget".into());
        stored.resources.disk_space_threshold_percent = 90;
        stored.resources.threads = 8;
        stored.resources.advertised_address = Some("db.internal".into());
        stored.transactions.max_active = 3;
        stored.data_dir = Some(std::path::PathBuf::from(
            "path: # with \"quotes\"\nand newline",
        ));
        let file: ConfigFile = serde_yml::from_str(&ConfigFile::render(&stored)).unwrap();
        assert_eq!(
            file.server.data_dir.as_deref(),
            stored.data_dir.as_ref().unwrap().to_str()
        );
        let mut requested = cfg(false);
        validate_config(&file, &mut requested).unwrap();
        assert_eq!(requested.resources, stored.resources);
        assert_eq!(requested.transactions, stored.transactions);
        assert!(requested.data_dir.is_none());
    }

    #[test]
    fn invalid_stored_limits_do_not_partially_apply_config() {
        let mut file: ConfigFile = serde_yml::from_str(&ConfigFile::render(&cfg(false))).unwrap();
        file.server.port = 42;
        file.transactions.as_mut().unwrap().max_active = 0;
        let mut requested = cfg(false);
        assert!(validate_config(&file, &mut requested).is_err());
        assert_eq!(requested.port, 3100);
        assert_eq!(requested.transactions.max_active, 128);
    }

    #[test]
    fn explicit_flag_disagreeing_with_config_yml_is_a_startup_error() {
        // D6: config.yml is the single source of truth; a conflicting,
        // explicitly supplied flag must fail loudly, never be ignored.
        let stored = cfg(false);
        let file: ConfigFile = serde_yml::from_str(&ConfigFile::render(&stored)).unwrap();
        let mut requested = cfg(false);
        requested.port = stored.port + 1;
        requested.resources.threads = 7;
        requested.explicit_flags = ["port".to_string(), "threads".to_string()].into();
        let error = validate_config(&file, &mut requested).unwrap_err();
        assert!(error.contains("--port"), "{error}");
        assert!(error.contains("--threads"), "{error}");
        // The same values without explicit flags take the stored config.
        let mut defaulted = cfg(false);
        defaulted.port = stored.port + 1;
        validate_config(&file, &mut defaulted).unwrap();
        assert_eq!(defaulted.port, stored.port);
    }

    #[test]
    fn a_replica_role_flag_is_this_boots_role_not_a_conflict() {
        // A follower built from a leader's base backup carries the leader's
        // `config.yml`, which names no leader.
        let stored = cfg(false);
        let file: ConfigFile = serde_yml::from_str(&ConfigFile::render(&stored)).unwrap();
        let mut follower = cfg(false);
        follower.resources.replica_of = Some("http://leader:3000".into());
        follower.explicit_flags = ["replica-of".to_string()].into();
        validate_config(&file, &mut follower).unwrap();
        assert_eq!(
            follower.resources.replica_of.as_deref(),
            Some("http://leader:3000")
        );
    }

    #[test]
    fn persisting_without_roles_drops_replica_and_archive() {
        let dir = rushdb_storage::TempPath::dir("roles");
        std::fs::create_dir_all(&dir).unwrap();
        let mut node = cfg(false);
        node.resources.replica_of = Some("http://leader:3000".into());
        node.resources.wal_archive_dir = Some(dir.join("archive"));
        write_config(&dir, &node).unwrap();
        persist_without_roles(&dir, &node, false).unwrap();
        let file = ConfigFile::load(&dir).unwrap().unwrap();
        let resources = file.resources.unwrap();
        assert_eq!(resources.replica_of, None, "a promoted node follows no one");
        assert!(
            resources.wal_archive_dir.is_some(),
            "promotion keeps archiving"
        );
        persist_without_roles(&dir, &node, true).unwrap();
        let resources = ConfigFile::load(&dir).unwrap().unwrap().resources.unwrap();
        assert_eq!(
            resources.wal_archive_dir, None,
            "a restored node archives nowhere yet"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
