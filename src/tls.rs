use anyhow::{Context, Result};
use axum_server::tls_rustls::RustlsConfig;
use tracing::info;

pub async fn load_tls_config(cert_file: &str, key_file: &str) -> Result<RustlsConfig> {
    info!(
        "Loading TLS configuration from cert: {}, key: {}",
        cert_file, key_file
    );

    let config = RustlsConfig::from_pem_file(cert_file, key_file)
        .await
        .with_context(|| {
            format!(
                "Failed to load TLS configuration from cert: {}, key: {}",
                cert_file, key_file
            )
        })?;

    info!("TLS configuration loaded successfully");
    Ok(config)
}

pub fn create_self_signed_cert(cert_path: &str, key_path: &str, hostname: &str) -> Result<()> {
    use rcgen::{Certificate, CertificateParams, DistinguishedName};
    use std::fs;

    info!(
        "Generating self-signed certificate for hostname: {}",
        hostname
    );

    let mut params = CertificateParams::new(vec![hostname.to_string()]);
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, hostname);
    params
        .distinguished_name
        .push(rcgen::DnType::OrganizationName, "FerraGate");
    params
        .distinguished_name
        .push(rcgen::DnType::CountryName, "US");

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_create_self_signed_cert() {
        let temp_dir = tempdir().unwrap();
        let cert_path = temp_dir.path().join("test.crt");
        let key_path = temp_dir.path().join("test.key");

        let result = create_self_signed_cert(
            cert_path.to_str().unwrap(),
            key_path.to_str().unwrap(),
            "test.example.com",
        );

        assert!(result.is_ok());
        assert!(cert_path.exists());
        assert!(key_path.exists());

        // Verify the certificate contains expected content
        let cert_content = fs::read_to_string(&cert_path).unwrap();
        assert!(cert_content.contains("-----BEGIN CERTIFICATE-----"));
        assert!(cert_content.contains("-----END CERTIFICATE-----"));

        // Verify the key contains expected content
        let key_content = fs::read_to_string(&key_path).unwrap();
        assert!(key_content.contains("-----BEGIN PRIVATE KEY-----"));
        assert!(key_content.contains("-----END PRIVATE KEY-----"));
    }

    #[test]
    fn test_create_self_signed_cert_invalid_path() {
        // Try to write to a non-existent directory without creating it
        let result = create_self_signed_cert(
            "/nonexistent/directory/test.crt",
            "/nonexistent/directory/test.key",
            "test.example.com",
        );

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_tls_config_nonexistent_files() {
        let result = load_tls_config("nonexistent.crt", "nonexistent.key").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_tls_config_valid_files() {
        let temp_dir = tempdir().unwrap();
        let cert_path = temp_dir.path().join("test.crt");
        let key_path = temp_dir.path().join("test.key");

        // First create valid certificate files
        let result = create_self_signed_cert(
            cert_path.to_str().unwrap(),
            key_path.to_str().unwrap(),
            "test.local",
        );
        assert!(result.is_ok());

        // Then test loading the TLS config
        let tls_result =
            load_tls_config(cert_path.to_str().unwrap(), key_path.to_str().unwrap()).await;

        assert!(tls_result.is_ok());
    }

    #[tokio::test]
    async fn test_load_tls_config_invalid_certificate_format() {
        let temp_dir = tempdir().unwrap();
        let cert_path = temp_dir.path().join("invalid.crt");
        let key_path = temp_dir.path().join("invalid.key");

        // Create invalid certificate files
        fs::write(&cert_path, "invalid certificate content").unwrap();
        fs::write(&key_path, "invalid key content").unwrap();

        let result = load_tls_config(cert_path.to_str().unwrap(), key_path.to_str().unwrap()).await;

        assert!(result.is_err());
    }
}
