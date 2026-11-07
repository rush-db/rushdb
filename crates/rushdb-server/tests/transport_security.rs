//! 7A — transport security guard: cleartext transports (pgwire/gRPC/Bolt) must
//! not bind a non-loopback `--listen` address unless the operator explicitly
//! acknowledges it with `--allow-insecure-listen`.

use std::sync::atomic::{AtomicU16, Ordering};

use rushdb_server::{new_gateway, new_router, ServerConfig};

static GRPC_PORT: AtomicU16 = AtomicU16::new(39040);

fn cfg(listen: &str, cleartext: bool, allow: bool) -> ServerConfig {
    ServerConfig {
        port: 0,
        listen_addr: listen.to_string(),
        pgwire_port: 0,
        grpc_port: if cleartext {
            GRPC_PORT.fetch_add(1, Ordering::Relaxed)
        } else {
            0
        },
        bolt_port: 0,
        token: "t".into(),
        data_dir: None,
        multitenant: false,
        capability_secret: None,
        raft: None,
        resources: rushdb_server::resource_config::ResourceConfig::default(),
        transactions: rushdb_server::resource_config::TransactionConfig::default(),
        languages: rushdb_protocol::LanguageSet::default(),
        max_request_bytes: 64 * 1024 * 1024,
        tls_cert: None,
        tls_key: None,
        maintenance: false,
        allow_insecure_listen: allow,
        require_index_warmup: false,
        request_timeout_ms: 0,
        max_concurrent_requests_per_tenant: 0,
        explicit_flags: std::collections::BTreeSet::new(),
    }
}

#[test]
fn non_loopback_cleartext_bind_is_refused_without_acknowledgement() {
    let cfg = cfg("0.0.0.0", true, false);
    let Err(err) = new_gateway(cfg, 0) else {
        panic!("expected refusal")
    };
    assert!(
        err.contains("cleartext") && err.contains("allow-insecure-listen"),
        "unexpected error: {err}"
    );
}

fn bind_ok(cfg: ServerConfig) {
    bind_ok_on(cfg, 0);
}

fn bind_ok_on(cfg: ServerConfig, pgwire_port: u16) {
    let (_router, _hp, _pg, _pp, grpc, _gp, _bolt, _bp) =
        new_gateway(cfg, pgwire_port).unwrap_or_else(|e| panic!("must bind: {e}"));
    drop(grpc);
}

#[test]
fn allow_insecure_listen_permits_non_loopback_bind() {
    bind_ok(cfg("0.0.0.0", true, true));
}

#[test]
fn loopback_cleartext_bind_needs_no_acknowledgement() {
    bind_ok(cfg("127.0.0.1", true, false));
}

#[test]
fn multitenant_cleartext_transport_cannot_bind_externally_even_with_acknowledgement() {
    let mut config = cfg("0.0.0.0", true, true);
    config.multitenant = true;
    let Err(error) = new_gateway(config, 0) else {
        panic!("must refuse global-token multitenant transport")
    };
    assert!(
        error.contains("tenant-bound credentials"),
        "unexpected error: {error}"
    );
}

#[test]
fn empty_boot_token_is_refused() {
    let mut config = cfg("127.0.0.1", false, false);
    config.token = String::new();
    let Err(error) = new_router(config) else {
        panic!("empty token must be refused")
    };
    assert!(error.contains("token must not be empty"), "{error}");
}

#[test]
fn short_capability_secret_is_refused() {
    let mut config = cfg("127.0.0.1", false, false);
    config.capability_secret = Some(vec![1, 2, 3]);
    let Err(error) = new_router(config) else {
        panic!("short capability secret must be refused")
    };
    assert!(error.contains("capability secret"), "{error}");
}

#[test]
fn half_configured_tls_is_refused() {
    let mut config = cfg("127.0.0.1", false, false);
    config.tls_cert = Some("/tmp/cert.pem".into());
    let Err(error) = new_router(config) else {
        panic!("half-configured TLS must be refused")
    };
    assert!(error.contains("certificate and a private key"), "{error}");
}

#[test]
fn non_loopback_cleartext_http_is_refused_without_acknowledgement() {
    let config = cfg("0.0.0.0", false, false);
    let Err(error) = new_router(config) else {
        panic!("cleartext HTTP on non-loopback must be refused")
    };
    assert!(error.contains("non-loopback"), "{error}");
}

#[test]
fn guard_counts_only_transports_that_will_bind() {
    // pgwire needs `sql` and Bolt needs `cypher`: with both off, their ports
    // alone must not trip the multitenant transport guard (which
    // `--allow-insecure-listen` does not lift).
    let mut config = cfg("0.0.0.0", false, true);
    config.multitenant = true;
    config.bolt_port = 7687;
    config.languages = rushdb_protocol::LanguageSet::NONE;
    config.languages.gql = true;
    bind_ok_on(config.clone(), 5432);
    config.languages.sql = true;
    let Err(error) = new_gateway(config, 5432) else {
        panic!("an enabled pgwire must trip the guard")
    };
    assert!(error.contains("tenant-bound credentials"), "{error}");
}

#[test]
fn the_default_token_cannot_guard_a_non_loopback_listener() {
    let mut config = cfg("0.0.0.0", false, true);
    config.token = rushdb_server::DEFAULT_TOKEN.into();
    config.tls_cert = Some("/tmp/cert.pem".into());
    config.tls_key = Some("/tmp/key.pem".into());
    let Err(error) = new_router(config) else {
        panic!("the default token must be refused off loopback, TLS or not")
    };
    assert!(error.contains("default token"), "{error}");
    let mut local = cfg("127.0.0.1", false, false);
    local.token = rushdb_server::DEFAULT_TOKEN.into();
    assert!(
        local.validate_boot_security().is_ok(),
        "loopback keeps the default"
    );
}

/// A self-signed certificate and key as PEM files, alive as long as the
/// returned directory handle.
fn tls_files() -> (
    std::path::PathBuf,
    std::path::PathBuf,
    rushdb_storage::TempPath,
) {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let dir = rushdb_storage::TempPath::dir("transport-tls");
    std::fs::create_dir_all(&dir).unwrap();
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    std::fs::write(&cert_path, cert.cert.pem()).unwrap();
    std::fs::write(&key_path, cert.key_pair.serialize_pem()).unwrap();
    (cert_path, key_path, dir)
}

/// A port nothing listens on right now, for a non-ephemeral pgwire bind.
fn free_port() -> u16 {
    std::net::TcpListener::bind(("0.0.0.0", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn sql_only(config: &mut ServerConfig) {
    config.languages = rushdb_protocol::LanguageSet::NONE;
    config.languages.sql = true;
}

#[test]
fn multitenant_pgwire_binds_externally_with_tls_and_capabilities() {
    // pgwire speaks only TLS once configured and authenticates tenant-bound
    // capabilities, so it may face the network on a multitenant server.
    let mut config = cfg("0.0.0.0", false, false);
    config.multitenant = true;
    sql_only(&mut config);
    let (cert, key, _dir) = tls_files();
    config.tls_cert = Some(cert);
    config.tls_key = Some(key);
    config.capability_secret = Some(vec![7u8; 32]);
    bind_ok_on(config, free_port());
}

#[test]
fn multitenant_pgwire_with_tls_but_no_capability_secret_is_refused() {
    let mut config = cfg("0.0.0.0", false, true);
    config.multitenant = true;
    sql_only(&mut config);
    let (cert, key, _dir) = tls_files();
    config.tls_cert = Some(cert);
    config.tls_key = Some(key);
    let Err(error) = new_gateway(config, free_port()) else {
        panic!("a multitenant TLS pgwire without capabilities must be refused")
    };
    assert!(error.contains("capability-secret"), "{error}");
}

#[test]
fn single_tenant_pgwire_with_tls_needs_no_acknowledgement() {
    let mut config = cfg("0.0.0.0", false, false);
    sql_only(&mut config);
    let (cert, key, _dir) = tls_files();
    config.tls_cert = Some(cert);
    config.tls_key = Some(key);
    bind_ok_on(config, free_port());
}

#[test]
fn unreadable_tls_files_fail_the_pgwire_bind() {
    let mut config = cfg("127.0.0.1", false, false);
    sql_only(&mut config);
    config.tls_cert = Some("/nonexistent/cert.pem".into());
    config.tls_key = Some("/nonexistent/key.pem".into());
    let Err(error) = new_gateway(config, 0) else {
        panic!("missing TLS files must fail the bind")
    };
    assert!(error.contains("pgwire TLS"), "{error}");
}

#[test]
fn multitenant_grpc_and_bolt_bind_externally_with_tls_and_capabilities() {
    let mut config = cfg("0.0.0.0", true, false);
    config.multitenant = true;
    config.languages = rushdb_protocol::LanguageSet::NONE;
    config.languages.cypher = true;
    config.bolt_port = free_port();
    let (cert, key, _dir) = tls_files();
    config.tls_cert = Some(cert);
    config.tls_key = Some(key);
    config.capability_secret = Some(vec![9u8; 32]);
    bind_ok_on(config, 0);
}

#[test]
fn multitenant_grpc_with_tls_but_no_capability_secret_is_refused() {
    let mut config = cfg("0.0.0.0", true, true);
    config.multitenant = true;
    config.languages = rushdb_protocol::LanguageSet::NONE;
    let (cert, key, _dir) = tls_files();
    config.tls_cert = Some(cert);
    config.tls_key = Some(key);
    let Err(error) = new_gateway(config, 0) else {
        panic!("a multitenant TLS gRPC without capabilities must be refused")
    };
    assert!(error.contains("capability-secret"), "{error}");
}
