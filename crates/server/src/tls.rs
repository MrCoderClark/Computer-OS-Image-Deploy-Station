//! TLS cert management for imaging-server.
//!
//! On first run, generates a self-signed cert covering localhost, 127.0.0.1,
//! and the machine's hostname. Subsequent runs reuse the stored PEM files.
//!
//! A SHA-256 fingerprint of the cert's DER encoding is logged on startup
//! (and available via the `fingerprint` subcommand). Clients should pin to
//! this fingerprint in production -- browser-style CA trust is a non-goal.

use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub struct CertFiles {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
    pub fingerprint_sha256: String,
}

/// Load the cert+key at the given paths, generating them first if missing.
/// Returns the fingerprint of the cert's DER encoding.
pub fn load_or_generate(cert_path: &Path, key_path: &Path) -> Result<CertFiles> {
    if !cert_path.exists() || !key_path.exists() {
        tracing::info!("Generating self-signed TLS cert at {}", cert_path.display());
        generate_self_signed(cert_path, key_path)
            .context("generating self-signed cert")?;
    }
    let fingerprint_sha256 = fingerprint(cert_path)
        .context("computing cert fingerprint")?;
    tracing::info!("TLS cert SHA-256 fingerprint: {}", fingerprint_sha256);
    Ok(CertFiles {
        cert_path: cert_path.to_path_buf(),
        key_path:  key_path.to_path_buf(),
        fingerprint_sha256,
    })
}

fn generate_self_signed(cert_path: &Path, key_path: &Path) -> Result<()> {
    use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, SanType};

    // No portable std::hostname -- COMPUTERNAME on Windows, HOSTNAME elsewhere.
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "localhost".to_string());

    let mut sans = vec![
        SanType::DnsName("localhost".to_string().try_into()?),
        SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
        SanType::IpAddress(std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)),
    ];
    if hostname != "localhost" {
        sans.push(SanType::DnsName(hostname.clone().try_into()?));
    }

    let mut params = CertificateParams::new(Vec::<String>::new())?;
    params.subject_alt_names = sans;
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, &hostname);
    dn.push(DnType::OrganizationName, "DeployStation");
    params.distinguished_name = dn;

    let key_pair = KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;

    if let Some(parent) = cert_path.parent() { std::fs::create_dir_all(parent).ok(); }
    if let Some(parent) = key_path.parent()  { std::fs::create_dir_all(parent).ok(); }
    std::fs::write(cert_path, cert.pem())?;
    std::fs::write(key_path,  key_pair.serialize_pem())?;

    // Lock down the private key (best-effort; Windows ACLs are a different
    // story but at least the Unix file mode is set when on Unix).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(key_path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn fingerprint(cert_path: &Path) -> Result<String> {
    let pem = std::fs::read(cert_path)?;
    let mut reader = std::io::BufReader::new(&pem[..]);
    let cert = rustls_pemfile::certs(&mut reader)
        .next()
        .ok_or_else(|| anyhow!("no certificate found in {}", cert_path.display()))??;
    let mut hasher = Sha256::new();
    hasher.update(cert.as_ref());
    let digest = hasher.finalize();
    // Colon-separated hex is the common display format
    let hex = hex::encode_upper(digest);
    let pairs: Vec<String> = (0..hex.len()).step_by(2)
        .map(|i| hex[i..i+2].to_string())
        .collect();
    Ok(pairs.join(":"))
}
