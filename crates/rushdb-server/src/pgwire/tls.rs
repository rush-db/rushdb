//! TLS for the PostgreSQL listener, from the same certificate and key as
//! HTTP (`--tls-cert` / `--tls-key`). `pgwire` handles both negotiations
//! itself: the classic `SSLRequest` and PostgreSQL 17's direct TLS (ALPN
//! `postgresql`).

use std::sync::Arc;

use pgwire::tokio::tokio_rustls::rustls;

/// The listener's TLS acceptor, or `None` when TLS is not configured.
///
/// # Errors
/// Returns a message when the certificate or key cannot be read or parsed.
pub(super) fn acceptor(
    cfg: &crate::ServerConfig,
) -> Result<Option<pgwire::tokio::TlsAcceptor>, String> {
    let (Some(cert_path), Some(key_path)) = (&cfg.tls_cert, &cfg.tls_key) else {
        return Ok(None);
    };
    let cert_file = std::fs::File::open(cert_path).map_err(|error| {
        format!(
            "pgwire TLS: cannot read certificate {}: {error}",
            cert_path.display()
        )
    })?;
    let certs = rustls_pemfile::certs(&mut std::io::BufReader::new(cert_file))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "pgwire TLS: invalid certificate {}: {error}",
                cert_path.display()
            )
        })?;
    if certs.is_empty() {
        return Err(format!(
            "pgwire TLS: no certificate in {}",
            cert_path.display()
        ));
    }
    let key_file = std::fs::File::open(key_path).map_err(|error| {
        format!(
            "pgwire TLS: cannot read private key {}: {error}",
            key_path.display()
        )
    })?;
    let key = rustls_pemfile::private_key(&mut std::io::BufReader::new(key_file))
        .map_err(|error| {
            format!(
                "pgwire TLS: invalid private key {}: {error}",
                key_path.display()
            )
        })?
        .ok_or_else(|| format!("pgwire TLS: no private key in {}", key_path.display()))?;
    // The same provider as HTTP (`ring`); named so two linked providers
    // never leave rustls without a default.
    let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|error| format!("pgwire TLS: {error}"))?
    .with_no_client_auth()
    .with_single_cert(certs, key)
    .map_err(|error| format!("pgwire TLS: {error}"))?;
    config.alpn_protocols = vec![b"postgresql".to_vec()];
    Ok(Some(pgwire::tokio::TlsAcceptor::from(Arc::new(config))))
}
