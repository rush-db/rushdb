//! TLS listener: a TLS-configured HTTP router speaks real HTTPS — a rustls
//! client (accepting the self-signed cert) completes a handshake and receives
//! a 200 over TLS, proving the rustls acceptor is wired end-to-end.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use rushdb_server::{new_router, ServerConfig};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::{pki_types::ServerName, ClientConfig, ClientConnection, Stream};

#[derive(Debug)]
struct NoVerify;

impl ServerCertVerifier for NoVerify {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[test]
fn tls_listener_serves_https() {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_pem = cert.cert.pem();
    let key_pem = cert.key_pair.serialize_pem();

    let dir = rushdb_storage::TempPath::dir("tls");
    std::fs::create_dir_all(&dir).unwrap();
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    std::fs::write(&cert_path, cert_pem).unwrap();
    std::fs::write(&key_path, key_pem).unwrap();

    let cfg = ServerConfig {
        port: 0,
        listen_addr: "127.0.0.1".into(),
        tls_cert: Some(cert_path),
        tls_key: Some(key_path),
        pgwire_port: 0,
        grpc_port: 0,
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
        maintenance: false,
        allow_insecure_listen: false,
        require_index_warmup: false,
        request_timeout_ms: 0,
        max_concurrent_requests_per_tenant: 0,
        explicit_flags: std::collections::BTreeSet::new(),
    };
    // rustls 0.23 with both `ring` and (via axum-server) `aws-lc-rs` enabled
    // cannot auto-determine a provider; install one explicitly.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("install ring provider");
    let (mut router, port) = new_router(cfg).expect("bootstrap");
    std::thread::spawn(move || router.serve_forever());

    // Retry the connect: axum-server binds + loads the cert asynchronously.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut tcp = loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(s) => break s,
            Err(_) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => panic!("TLS listener must come up: {e}"),
        }
    };
    tcp.set_read_timeout(Some(Duration::from_secs(10))).unwrap();

    let mut config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoVerify))
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let config = Arc::new(config);
    let server_name = ServerName::try_from("localhost").unwrap();
    let mut conn = ClientConnection::new(config, server_name).unwrap();
    let mut stream = Stream::new(&mut conn, &mut tcp);

    stream
        .write_all(
            b"GET /health HTTP/1.1\r\nHost: localhost\r\nX-RushDB-Token: t\r\nConnection: close\r\n\r\n",
        )
        .unwrap();
    let mut resp = String::new();
    let _ = stream.read_to_string(&mut resp);
    assert!(
        resp.starts_with("HTTP/1.1 200"),
        "expected 200 over TLS, got: {resp}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn config_yml_renders_tls_and_listen() {
    let cfg = ServerConfig {
        port: 3000,
        listen_addr: "0.0.0.0".into(),
        tls_cert: Some("/tmp/cert.pem".into()),
        tls_key: Some("/tmp/key.pem".into()),
        pgwire_port: 5432,
        grpc_port: 0,
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
        maintenance: false,
        allow_insecure_listen: false,
        require_index_warmup: false,
        request_timeout_ms: 0,
        max_concurrent_requests_per_tenant: 0,
        explicit_flags: std::collections::BTreeSet::new(),
    };
    let rendered = rushdb_server::config::ConfigFile::render(&cfg);
    assert!(rendered.contains("listen-addr: 0.0.0.0"), "{rendered}");
    assert!(rendered.contains("tls-cert:"), "{rendered}");
    assert!(rendered.contains("tls-key:"), "{rendered}");
}
