use axum_server::tls_rustls::RustlsConfig;
use tracing::info;

use crate::constants::*;
use crate::error::{FerragateError, FerragateResult};

/// Load TLS configuration from certificate and key files
///
/// Reads the certificate and private key files and creates a RustlsConfig
/// that can be used with the Axum server for HTTPS support.
///
/// # Arguments
/// * `cert_file` - Path to the certificate file (PEM format)
/// * `key_file` - Path to the private key file (PEM format)
///
/// # Returns
/// * `FerragateResult<RustlsConfig>` - TLS configuration on success, error on failure
pub async fn load_tls_config(cert_file: &str, key_file: &str) -> FerragateResult<RustlsConfig> {
    info!(
        "Loading TLS configuration from cert: {}, key: {}",
        cert_file, key_file
    );

    // Validate that both files exist before attempting to load
    if !std::path::Path::new(cert_file).exists() {
        return Err(FerragateError::tls(format!(
            "Certificate file not found: {}",
            cert_file
        )));
    }

    if !std::path::Path::new(key_file).exists() {
        return Err(FerragateError::tls(format!(
            "Private key file not found: {}",
            key_file
        )));
    }

    let config = RustlsConfig::from_pem_file(cert_file, key_file)
        .await
        .map_err(|e| {
            FerragateError::tls(format!(
                "Failed to load TLS configuration from cert: {}, key: {}: {}",
                cert_file, key_file, e
            ))
        })?;

    info!("{}", LOG_TLS_ENABLED);
    Ok(config)
}

/// Generate a self-signed certificate for development and testing
///
/// Creates a self-signed X.509 certificate and private key for the given hostname.
/// This is useful for development environments where you need HTTPS but don't have
/// a certificate from a Certificate Authority.
///
/// # Arguments
/// * `cert_path` - Output path for the certificate file
/// * `key_path` - Output path for the private key file
/// * `hostname` - Hostname/domain name for the certificate
///
/// # Returns
/// * `Result<()>` - Success or error information
///
/// # Security Note
/// Self-signed certificates should only be used for development and testing.
/// Production deployments should use certificates from a trusted CA.
pub fn create_self_signed_cert(
    cert_path: &str,
    key_path: &str,
    hostname: &str,
) -> FerragateResult<()> {
    use rcgen::{Certificate, CertificateParams, DistinguishedName};
    use std::fs;

    info!(
        "Generating self-signed certificate for hostname: {}",
        hostname
    );

    // Create certificate parameters
    let mut params = CertificateParams::new(vec![hostname.to_string()]);

    // Set up distinguished name
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, hostname);
    params
        .distinguished_name
        .push(rcgen::DnType::OrganizationName, CERT_ORGANIZATION);
    params
        .distinguished_name
        .push(rcgen::DnType::CountryName, CERT_COUNTRY);

    // Generate the certificate
    let cert = Certificate::from_params(params).map_err(|e| {
        FerragateError::tls(format!("Failed to generate self-signed certificate: {}", e))
    })?;

    // Write certificate to file
    let cert_pem = cert
        .serialize_pem()
        .map_err(|e| FerragateError::tls(format!("Failed to serialize certificate: {}", e)))?;
    fs::write(cert_path, cert_pem).map_err(|e| {
        FerragateError::tls(format!(
            "Failed to write certificate to '{}': {}",
            cert_path, e
        ))
    })?;

    // Write private key to file
    fs::write(key_path, cert.serialize_private_key_pem()).map_err(|e| {
        FerragateError::tls(format!(
            "Failed to write private key to '{}': {}",
            key_path, e
        ))
    })?;

    info!("Self-signed certificate generated successfully");
    info!("Certificate written to: {}", cert_path);
    info!("Private key written to: {}", key_path);

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
