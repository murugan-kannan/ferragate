use anyhow::{Context, Result};
use axum_server::tls_rustls::RustlsConfig;
use tracing::info;

pub async fn load_tls_config(cert_file: &str, key_file: &str) -> Result<RustlsConfig> {
    info!("Loading TLS configuration from cert: {}, key: {}", cert_file, key_file);

    let config = RustlsConfig::from_pem_file(cert_file, key_file)
        .await
        .with_context(|| format!("Failed to load TLS configuration from cert: {}, key: {}", cert_file, key_file))?;

    info!("TLS configuration loaded successfully");
    Ok(config)
}

pub fn create_self_signed_cert(cert_path: &str, key_path: &str, hostname: &str) -> Result<()> {
    use rcgen::{Certificate, CertificateParams, DistinguishedName};
    use std::fs;

    info!("Generating self-signed certificate for hostname: {}", hostname);

    let mut params = CertificateParams::new(vec![hostname.to_string()]);
    params.distinguished_name = DistinguishedName::new();
    params.distinguished_name.push(rcgen::DnType::CommonName, hostname);
    params.distinguished_name.push(rcgen::DnType::OrganizationName, "FerraGate");
    params.distinguished_name.push(rcgen::DnType::CountryName, "US");

    let cert = Certificate::from_params(params)
        .with_context(|| "Failed to generate self-signed certificate")?;

    // Write certificate to file
    fs::write(cert_path, cert.serialize_pem()?)
        .with_context(|| format!("Failed to write certificate to: {}", cert_path))?;

    // Write private key to file
    fs::write(key_path, cert.serialize_private_key_pem())
        .with_context(|| format!("Failed to write private key to: {}", key_path))?;

    info!("Self-signed certificate generated successfully");
    info!("Certificate: {}", cert_path);
    info!("Private key: {}", key_path);

    Ok(())
}
