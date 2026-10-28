//! `rushdb serve`: the server's CLI/environment configuration, the node boot
//! with every enabled transport adapter, and the offline maintenance exits.
use rushdb_server::{Node, RaftConfig, ServerConfig};

#[expect(
    clippy::struct_excessive_bools,
    reason = "CLI flag struct: each bool is an independent operator switch, not a state machine"
)]
#[derive(Debug, clap::Args)]
pub struct ServeArgs {
    /// HTTP protocol adapter port.
    #[arg(long, default_value_t = 3000)]
    pub port: u16,
    /// PostgreSQL wire-protocol port.
    #[arg(long, default_value_t = 5432)]
    pub pgwire_port: u16,
    /// gRPC transport port (0 = disabled).
    #[arg(long, default_value_t = 0)]
    grpc_port: u16,
    /// Bolt (Neo4j wire) transport port (0 = disabled; requires cypher/gql).
    #[arg(long, default_value_t = 0)]
    pub bolt_port: u16,
    /// Bootstrap bearer/password token.
    #[arg(long, default_value = rushdb_server::DEFAULT_TOKEN, env = "RUSHDB_TOKEN")]
    pub token: String,
    /// HTTP bind address (default 127.0.0.1). Use 0.0.0.0 to expose beyond
    /// loopback (pair with --tls-cert/--tls-key).
    #[arg(long, default_value = "127.0.0.1", env = "RUSHDB_LISTEN")]
    pub listen: String,
    /// PEM certificate chain path for HTTPS (requires --tls-key too).
    #[arg(long, env = "RUSHDB_TLS_CERT")]
    tls_cert: Option<std::path::PathBuf>,
    /// PEM private key path (PKCS#8) for HTTPS.
    #[arg(long, env = "RUSHDB_TLS_KEY")]
    tls_key: Option<std::path::PathBuf>,
    /// HMAC secret (hex) for signed capability tokens. When set, a valid
    /// capability presented via `X-RushDB-Token` is accepted (scope-gated);
    /// the boot token always remains a full-access fallback.
    #[arg(long, env = "RUSHDB_CAPABILITY_SECRET")]
    capability_secret: Option<String>,
    /// Persistent database directory; omit for an in-memory node.
    #[arg(long, env = "RUSHDB_DATA_DIR")]
    pub data_dir: Option<std::path::PathBuf>,
    /// Maintenance mode: reject writes and mark /health as maintenance until
    /// cleared. Reads stay available.
    #[arg(long, env = "RUSHDB_MAINTENANCE")]
    maintenance: bool,
    /// Explicitly allow pgwire/gRPC/Bolt (cleartext transports) to bind a
    /// non-loopback `--listen` address without TLS. Required whenever any of
    /// them is enabled with a non-loopback listen address.
    #[arg(long, env = "RUSHDB_ALLOW_INSECURE_LISTEN")]
    allow_insecure_listen: bool,
    /// Gate `/ready` on vector-index warmup: report 503 until every vector
    /// index has caught up with its source records. Single-tenant only.
    #[arg(long, env = "RUSHDB_REQUIRE_INDEX_WARMUP")]
    require_index_warmup: bool,
    /// Request execution deadline in milliseconds for every transport (0 = no
    /// deadline; a client deadline may only shorten it). Requests
    /// exceeding it are cancelled with a structured `Cancelled` error.
    #[arg(long, env = "RUSHDB_REQUEST_TIMEOUT_MS", default_value_t = 0)]
    request_timeout_ms: u64,
    /// Per-tenant concurrent request cap in multitenant mode (0 = unlimited).
    #[arg(long, env = "RUSHDB_MAX_CONCURRENT_PER_TENANT", default_value_t = 0)]
    max_concurrent_requests_per_tenant: usize,
    /// Enable fail-closed scope-keyed multi-tenancy: each request must present
    /// a scope (`X-RushDB-Tenant` on HTTP, `database` on pgwire) that selects
    /// the tenant keyspace. No discriminator property is stored on records.
    #[arg(long)]
    multitenant: bool,
    /// Enabled query languages (comma list: sql,gql,cypher,gremlin,sql-pgq,
    /// mongodb), replacing the default set sql,gremlin,sql-pgq,mongodb.
    /// `searchquery` is always enabled and cannot be listed.
    #[arg(long, value_delimiter = ',')]
    pub languages: Vec<String>,
    /// Enable raft replication with N in-process state machines (including
    /// this node). Every local commit routes through the elected leader and is
    /// applied deterministically on all nodes before the commit returns.
    ///
    /// EXPERIMENTAL: this is an in-process harness. The raft log, vote,
    /// membership and snapshot state live in memory and are lost on restart;
    /// there is no network transport, no durable snapshot, and no linearizable
    /// read. It is not a high-availability or durability feature. Requires
    /// `--experimental-raft`.
    #[arg(long, value_name = "N")]
    raft: Option<usize>,
    /// Acknowledge that `--raft` is an experimental in-process harness with no
    /// durability or HA guarantees. Required to enable `--raft`.
    #[arg(long)]
    experimental_raft: bool,
    /// Print the effective `config.yml` for this configuration and exit
    /// (no server is started).
    #[arg(long)]
    print_config: bool,
    /// Secure first-run flow: write an initial `config.yml` for the effective
    /// configuration into the data directory, validate it, and exit (no server
    /// is started). Re-running against the same directory is a no-op that
    /// re-validates the existing record. Pairs with `--print-config` to review
    /// the effective config before persisting it.
    #[arg(long, value_name = "DATA_DIR")]
    init: Option<std::path::PathBuf>,
    /// Stream a consistent *graph* snapshot export to a file and exit (no
    /// server started). Uses `--data-dir` for the source store. This is graph
    /// interchange, not full recovery: it carries records and relationships
    /// only. Vector spaces/bindings, ANN manifests, text profiles, security
    /// policy, transaction outcomes, and MVCC history are **not** included.
    /// Use `--backup-base` + `--restore-pitr` for complete state recovery.
    #[arg(long, value_name = "PATH")]
    export: Option<std::path::PathBuf>,
    /// Restore a graph snapshot stream into a FRESH `--data-dir` and exit (no
    /// server started). Restore replays as a sequence of commits;
    /// truncated/corrupted artifacts are rejected before any state is applied.
    /// Derived index/vector/text state is not restored (see `--export`); use
    /// `--restore-pitr` for full recovery.
    #[arg(long, value_name = "PATH")]
    restore: Option<std::path::PathBuf>,
    /// Compact the durable store in place and exit (no server started):
    /// reclaims the redb copy-on-write high-water mark and retained MVCC
    /// history. Needs exclusive access — stop the server first. Run it between
    /// campaigns so commit/scan cost does not creep up with file size.
    #[arg(long)]
    compact: bool,
    /// Take an offline physical base backup of `--data-dir` into PATH and exit
    /// (writes `PATH/base/*.redb` + `PATH/manifest.json`). Stop the server
    /// first. Combine with `--wal-archive-dir` segments for PITR.
    #[arg(long, value_name = "PATH")]
    backup_base: Option<std::path::PathBuf>,
    /// Restore a base backup (`BASE_DIR`) plus archived WAL into a FRESH
    /// `--data-dir` and exit. The archive directory is `--wal-archive-dir`;
    /// `--to-lsn` bounds the point in time.
    #[arg(long, value_name = "BASE_DIR")]
    restore_pitr: Option<std::path::PathBuf>,
    /// Stop at this commit index/LSN during `--restore-pitr` (default: latest).
    #[arg(long, value_name = "LSN")]
    to_lsn: Option<u64>,
    /// Logical integrity check of `--data-dir` (scrub every tenant) and exit.
    #[arg(long)]
    verify: bool,
    /// Verify a base backup's manifest and file CRCs (`BASE_DIR`) and exit.
    #[arg(long, value_name = "BASE_DIR")]
    verify_base: Option<std::path::PathBuf>,
    #[command(flatten)]
    resources: rushdb_server::resource_config::ResourceConfig,
    #[command(flatten)]
    transactions: rushdb_server::resource_config::TransactionConfig,
}

#[expect(
    clippy::too_many_lines,
    reason = "the CLI dispatch keeps all mutually exclusive maintenance exits together"
)]
pub fn run(args: ServeArgs, explicit_flags: std::collections::BTreeSet<String>) {
    rushdb_server::allocator::configure();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rushdb=info,rushdb_server=info,tower_http=info".into()),
        )
        .with_target(true)
        .compact()
        .init();
    if args.raft.is_some() && !args.experimental_raft {
        eprintln!(
            "--raft is an experimental in-process harness: raft log/vote/snapshot state is \
             in-memory and lost on restart, there is no network transport or linearizable read. \
             It provides no durability or high-availability guarantee. Pass --experimental-raft \
             to acknowledge this."
        );
        std::process::exit(2);
    }
    if args.raft.is_some() {
        tracing::warn!(
            "raft replication is EXPERIMENTAL: in-process only, volatile log/vote/snapshot, no linearizable reads"
        );
    }
    let languages = if args.languages.is_empty() {
        rushdb_protocol::LanguageSet::default()
    } else {
        rushdb_protocol::LanguageSet::from_list(&args.languages.join(",")).unwrap_or_else(|e| {
            eprintln!("invalid --languages: {e}");
            std::process::exit(2);
        })
    };

    let capability_secret = args.capability_secret.as_deref().map(|hex| {
        hex::decode(hex).unwrap_or_else(|_| {
            eprintln!("--capability-secret must be hex");
            std::process::exit(2)
        })
    });
    let cfg = ServerConfig {
        port: args.port,
        listen_addr: args.listen,
        tls_cert: args.tls_cert,
        tls_key: args.tls_key,
        pgwire_port: args.pgwire_port,
        grpc_port: args.grpc_port,
        bolt_port: args.bolt_port,
        token: args.token,
        data_dir: args.data_dir,
        multitenant: args.multitenant,
        maintenance: args.maintenance,
        allow_insecure_listen: args.allow_insecure_listen,
        require_index_warmup: args.require_index_warmup,
        request_timeout_ms: args.request_timeout_ms,
        max_concurrent_requests_per_tenant: args.max_concurrent_requests_per_tenant,
        capability_secret,
        raft: args.raft.map(|size| RaftConfig { size }),
        resources: args.resources,
        transactions: args.transactions,
        languages,
        max_request_bytes: 16 * 1024 * 1024,
        explicit_flags,
    };
    if args.print_config {
        println!("{}", rushdb_server::config::ConfigFile::render(&cfg));
        return;
    }
    if let Some(dir) = args.init {
        init_config(&cfg, &dir);
    }
    if let Some(out) = args.export {
        export_snapshot(&cfg, &out);
    }
    if let Some(inp) = args.restore {
        restore_snapshot(&cfg, &inp);
    }
    if args.compact {
        compact_store(&cfg);
    }
    if let Some(out) = args.backup_base {
        backup_base(&cfg, &out);
    }
    if let Some(base_dir) = args.restore_pitr {
        restore_pitr(&cfg, &base_dir, args.to_lsn);
    }
    if args.verify {
        verify_store(&cfg);
    }
    if let Some(base_dir) = args.verify_base {
        verify_base(&base_dir);
    }
    // Answer health/readiness probes from process start: resolve the
    // stored config first so the effective listen address is known, bind it,
    // and hand the same socket to the router once bootstrap completes. TLS
    // listeners are bound by the TLS stack itself, so they keep the old order.
    let mut cfg = cfg;
    if let Err(error) = rushdb_server::config::resolve_stored_config(&mut cfg) {
        eprintln!("server bootstrap: {error}");
        std::process::exit(1);
    }
    let boot_listener = if cfg.tls_cert.is_none() {
        match rushdb_server::boot::BootListener::start(&cfg.listen_addr, cfg.port) {
            Ok(boot) => Some(boot),
            Err(error) => {
                eprintln!(
                    "server bootstrap: cannot bind {}:{}: {error}",
                    cfg.listen_addr, cfg.port
                );
                std::process::exit(1);
            }
        }
    } else {
        None
    };
    let listen = cfg.listen_addr.clone();
    let (mut router, bound, mut pgwire, bound_pgwire, mut grpc, bound_grpc, mut bolt, bound_bolt) =
        match rushdb_server::pgwire::new_gateway_with_boot_listener(
            cfg,
            args.pgwire_port,
            boot_listener,
        ) {
            Ok(gateway) => gateway,
            Err(error) => {
                eprintln!("server bootstrap: {error}");
                std::process::exit(1);
            }
        };
    let mut transports = rushdb_server::lifecycle::TransportLifecycle::new(
        router.shutdown_handle(),
        std::time::Duration::from_secs(7),
    );
    transports.spawn("pgwire", move || pgwire.serve_forever());
    transports.spawn("grpc", move || grpc.serve_forever());
    transports.spawn("bolt", move || bolt.serve_forever());
    tracing::info!(
        listen = %listen,
        http_port = bound,
        pgwire_port = bound_pgwire,
        grpc_port = bound_grpc,
        bolt_port = bound_bolt,
        tenancy = if args.multitenant { "multitenant" } else { "single" },
        "rushdb listening"
    );
    let http_outcome = router.serve_forever();
    if let Err(failures) = transports.shutdown() {
        tracing::error!(?failures, "database shutdown incomplete");
        std::process::exit(1);
    }
    if let Err(error) = http_outcome {
        tracing::error!(%error, "HTTP shutdown incomplete");
        std::process::exit(1);
    }
}

/// Write an initial `config.yml` for the effective configuration into the data
/// directory, then round-trip-validate it so an invalid first-run record is
/// rejected before any server starts. Re-running is a re-validation no-op.
fn init_config(cfg: &ServerConfig, dir: &std::path::Path) -> ! {
    if let Err(error) = std::fs::create_dir_all(dir) {
        eprintln!(
            "--init: cannot create data directory `{}`: {error}",
            dir.display()
        );
        std::process::exit(2);
    }
    let path = dir.join(rushdb_server::config::CONFIG_FILE_NAME);
    let existing = match rushdb_server::config::ConfigFile::load(dir) {
        Ok(existing) => existing,
        Err(error) => {
            eprintln!("--init: {error}");
            std::process::exit(2);
        }
    };
    if let Some(existing) = existing {
        // Re-run against the same directory: re-validate the stored record.
        let mut requested = cfg.clone();
        if let Err(error) = rushdb_server::config::validate_config(&existing, &mut requested) {
            eprintln!("--init: existing config is not valid for this configuration: {error}");
            std::process::exit(2);
        }
        eprintln!(
            "--init: `{}` already exists and validates; nothing to write",
            path.display()
        );
        std::process::exit(0);
    }
    if let Err(error) = rushdb_server::config::write_config(dir, cfg) {
        eprintln!("--init: {error}");
        std::process::exit(2);
    }
    // Round-trip: the file we just wrote must load and validate.
    let mut requested = cfg.clone();
    match rushdb_server::config::ConfigFile::load(dir) {
        Ok(Some(file)) => {
            if let Err(error) = rushdb_server::config::validate_config(&file, &mut requested) {
                eprintln!("--init: written config failed validation: {error}");
                std::process::exit(2);
            }
        }
        Ok(None) => {
            eprintln!("--init: written config did not persist");
            std::process::exit(2);
        }
        Err(error) => {
            eprintln!("--init: {error}");
            std::process::exit(2);
        }
    }
    eprintln!("--init: wrote and validated `{}`", path.display());
    std::process::exit(0);
}

/// Stream a consistent snapshot (`Node::export_snapshot`) to a file. The
/// export is pinned at one commit index; writers keep serving until exit.
/// One-shot offline compaction: open the store, reclaim retained MVCC history
/// and the redb copy-on-write high-water mark, print the commit index, and
/// exit. Needs exclusive access — stop the server first.
fn compact_store(cfg: &ServerConfig) -> ! {
    let node = match Node::bootstrap(&mut cfg.clone()) {
        Ok(node) => node,
        Err(e) => {
            eprintln!("compact: {e}");
            std::process::exit(2);
        }
    };
    match node.compact_now() {
        Ok(ci) => {
            eprintln!("compact: store compacted at commit index {ci}");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("compact: {e} — compaction needs exclusive access; stop the server first");
            std::process::exit(2);
        }
    }
}

fn export_snapshot(cfg: &ServerConfig, out: &std::path::Path) -> ! {
    let node = match Node::bootstrap(&mut cfg.clone()) {
        Ok(node) => node,
        Err(e) => {
            eprintln!("export: {e}");
            std::process::exit(2);
        }
    };
    let mut commit_index = None;
    let result = rushdb_server::archive::atomic_publish_stream(out, |file| {
        let mut writer = std::io::BufWriter::new(file);
        let ci = node.export_snapshot(&mut writer, &[])?;
        std::io::Write::flush(&mut writer).map_err(|error| format!("flush snapshot: {error}"))?;
        commit_index = Some(ci);
        Ok(())
    });
    match result {
        Ok(()) => {
            eprintln!(
                "export: wrote snapshot at commit index {} to `{}`",
                commit_index.unwrap_or_default(),
                out.display()
            );
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("export: {e}");
            std::process::exit(2);
        }
    }
}

/// Restore a snapshot stream into a fresh data directory. Restore replays as a
/// sequence of commits; a truncated/corrupted artifact is rejected before any
/// state is applied (restore never makes a partially validated store ready).
fn restore_snapshot(cfg: &ServerConfig, inp: &std::path::Path) -> ! {
    let node = match Node::bootstrap(&mut cfg.clone()) {
        Ok(node) => node,
        Err(e) => {
            eprintln!("restore: {e}");
            std::process::exit(2);
        }
    };
    let file = match std::fs::File::open(inp) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("restore: cannot open `{}`: {e}", inp.display());
            std::process::exit(2);
        }
    };
    let mut reader = std::io::BufReader::new(file);
    let mut progress = |index: usize, records: u64, relationships: u64| {
        eprintln!(
            "restore: chunk {index} -> {records} records, {relationships} relationships applied"
        );
    };
    match node.restore_snapshot_progress(&mut reader, &mut progress) {
        Ok((records, relationships, chunks)) => {
            eprintln!(
                "restore: applied {records} records and {relationships} relationships across {chunks} chunks into `{}`",
                cfg.data_dir.as_ref().map_or_else(|| "<memory>".into(), |d| d.display().to_string())
            );
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("restore: {e}");
            std::process::exit(2);
        }
    }
}

/// Offline physical base backup: copy the store file(s) into `out` with a CRC
/// manifest pinned at the current commit index. Requires `--data-dir` and
/// exclusive access (stop the server first).
fn backup_base(cfg: &ServerConfig, out: &std::path::Path) -> ! {
    if cfg.data_dir.is_none() {
        eprintln!("--backup-base requires --data-dir");
        std::process::exit(2);
    }
    let node = match Node::bootstrap(&mut cfg.clone()) {
        Ok(node) => node,
        Err(e) => {
            eprintln!("backup-base: {e} — needs exclusive access; stop the server first");
            std::process::exit(2);
        }
    };
    match node.backup_base_offline(out) {
        Ok(manifest) => {
            eprintln!(
                "backup-base: {} file(s) at commit index {} -> `{}`",
                manifest.files.len(),
                manifest.commit_index,
                out.display()
            );
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("backup-base: {e}");
            std::process::exit(2);
        }
    }
}

/// Point-in-time restore: copy a base backup into a FRESH `--data-dir`, then
/// replay the archived WAL segments (from `--wal-archive-dir`) up to `to_lsn`.
fn restore_pitr(cfg: &ServerConfig, base_dir: &std::path::Path, to_lsn: Option<u64>) -> ! {
    let Some(data_dir) = cfg.data_dir.clone() else {
        eprintln!("--restore-pitr requires --data-dir");
        std::process::exit(2);
    };
    // The archive is this command's input; check it before copying anything.
    let Some(archive_dir) = cfg.resources.wal_archive_dir.clone() else {
        eprintln!("restore-pitr: --wal-archive-dir is required");
        std::process::exit(2);
    };
    let mut stream = match rushdb_server::archive::segment_stream(&archive_dir) {
        Ok(stream) => stream,
        Err(e) => {
            eprintln!("restore-pitr: {e}");
            std::process::exit(2);
        }
    };
    let base_ci = match rushdb_server::archive::restore_base_into(base_dir, &data_dir) {
        Ok(ci) => ci,
        Err(e) => {
            eprintln!("restore-pitr: {e}");
            std::process::exit(2);
        }
    };
    // The restored directory carries its source's `config.yml`: boot it as
    // stored (the archive flag is an input here, not this node's setting),
    // then drop the source's archive and replica roles so the restored node
    // never appends to the source's archive or follows its leader.
    let mut boot = cfg.clone();
    boot.resources.wal_archive_dir = None;
    boot.explicit_flags.remove("wal-archive-dir");
    let node = match Node::bootstrap(&mut boot) {
        Ok(node) => node,
        Err(e) => {
            eprintln!("restore-pitr: {e}");
            std::process::exit(2);
        }
    };
    if let Err(e) = rushdb_server::config::persist_without_roles(&data_dir, &boot, true) {
        eprintln!("restore-pitr: {e}");
        std::process::exit(2);
    }
    match node.restore_pitr_to(&mut stream, Some(base_ci), to_lsn) {
        Ok(ci) => {
            eprintln!("restore-pitr: replayed archived WAL to commit index {ci}");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("restore-pitr: {e}");
            std::process::exit(2);
        }
    }
}

/// Logical integrity check of `--data-dir`: scrub every tenant's graph.
fn verify_store(cfg: &ServerConfig) -> ! {
    let node = match Node::bootstrap(&mut cfg.clone()) {
        Ok(node) => node,
        Err(e) => {
            eprintln!("verify: {e}");
            std::process::exit(2);
        }
    };
    match node.verify() {
        Ok(0) => {
            eprintln!("verify: no violations");
            std::process::exit(0);
        }
        Ok(n) => {
            eprintln!("verify: {n} invariant violation(s) found");
            std::process::exit(2);
        }
        Err(e) => {
            eprintln!("verify: {e}");
            std::process::exit(2);
        }
    }
}

/// Verify a base backup's manifest and file CRCs.
fn verify_base(dir: &std::path::Path) -> ! {
    match rushdb_server::archive::verify_base(dir) {
        Ok(ci) => {
            eprintln!("verify-base: OK at commit index {ci}");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("verify-base: {e}");
            std::process::exit(2);
        }
    }
}

/// Long names of every flag the operator supplied on the command line or via
/// its environment variable (not clap defaults) to `command`. Used to reject a
/// disagreeing `config.yml` instead of silently overriding the operator.
pub fn explicit_flags(
    command: &clap::Command,
    matches: &clap::ArgMatches,
) -> std::collections::BTreeSet<String> {
    matches
        .ids()
        .filter(|id| {
            matches!(
                matches.value_source(id.as_str()),
                Some(
                    clap::parser::ValueSource::CommandLine | clap::parser::ValueSource::EnvVariable
                )
            )
        })
        .filter_map(|id| {
            command
                .get_arguments()
                .find(|arg| arg.get_id() == id)
                .and_then(clap::Arg::get_long)
                .map(str::to_string)
        })
        .collect()
}
