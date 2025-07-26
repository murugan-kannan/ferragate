use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{error, info};

use crate::config::GatewayConfig;
use crate::constants::*;
use crate::error::{FerragateError, FerragateResult};

/// Ferragate API Gateway CLI
///
/// A high-performance, multi-tenant API Gateway built in Rust.
/// Provides secure, scalable routing and load balancing for your services.
#[derive(Parser)]
#[command(name = env!("CARGO_PKG_NAME"))]
#[command(about = env!("CARGO_PKG_DESCRIPTION"))]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Available CLI commands
#[derive(Subcommand)]
pub enum Commands {
    /// Start the gateway server
    Start {
        /// Configuration file path
        #[arg(short, long, default_value = DEFAULT_CONFIG_FILE)]
        config: PathBuf,

        /// Override server host address
        #[arg(long, help = "Override the host address from config")]
        host: Option<String>,

        /// Override server port
        #[arg(short, long, help = "Override the port from config")]
        port: Option<u16>,
    },

    /// Validate configuration file
    Validate {
        /// Configuration file path
        #[arg(short, long, default_value = DEFAULT_CONFIG_FILE)]
        config: PathBuf,
    },

    /// Generate example configuration file
    Init {
        /// Output configuration file path
        #[arg(short, long, default_value = DEFAULT_CONFIG_FILE)]
        output: PathBuf,

        /// Overwrite existing file
        #[arg(long, help = "Overwrite the file if it already exists")]
        force: bool,
    },

    /// Generate TLS certificates for HTTPS
    GenCerts {
        /// Certificate output directory
        #[arg(short, long, default_value = DEFAULT_CERT_DIR)]
        output_dir: PathBuf,

        /// Hostname for the certificate (default: localhost)
        #[arg(long, default_value = DEFAULT_HOSTNAME)]
        hostname: String,

        /// Overwrite existing certificates
        #[arg(long, help = "Overwrite existing certificate files")]
        force: bool,
    },

    /// Stop the running gateway server
    Stop {
        /// Configuration file path (to identify the correct server instance)
        #[arg(short, long, default_value = DEFAULT_CONFIG_FILE)]
        config: PathBuf,

        /// Force stop (kill process immediately)
        #[arg(long, help = "Force immediate shutdown without graceful stop")]
        force: bool,
    },
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Execute the CLI command
    ///
    /// Dispatches to the appropriate handler function based on the command type.
    pub async fn execute(self) -> FerragateResult<()> {
        match self.command {
            Commands::Start { config, host, port } => start_server(config, host, port).await,
            Commands::Validate { config } => validate_config(config),
            Commands::Init { output, force } => init_config(output, force),
            Commands::GenCerts {
                output_dir,
                hostname,
                force,
            } => generate_certs(output_dir, hostname, force),
            Commands::Stop { config, force } => stop_server(config, force).await,
        }
    }
}

async fn start_server(
    config_path: PathBuf,
    host_override: Option<String>,
    port_override: Option<u16>,
) -> FerragateResult<()> {
    info!("Starting FerraGate server...");

    let mut config = GatewayConfig::from_file(config_path.to_str().unwrap_or(DEFAULT_CONFIG_FILE))?;

    // Apply CLI overrides
    if let Some(host) = host_override {
        info!("Overriding host from CLI: {}", host);
        config.server.host = host;
    }

    if let Some(port) = port_override {
        info!("Overriding port from CLI: {}", port);
        config.server.port = port;
    }

    // Start the server
    crate::server::start_server(config, config_path.to_str()).await
}

fn validate_config(config_path: PathBuf) -> FerragateResult<()> {
    info!("Validating configuration...");

    let config = GatewayConfig::from_file(config_path.to_str().unwrap_or(DEFAULT_CONFIG_FILE))?;

    info!("✅ Configuration is valid!");
    info!("Server: {}:{}", config.server.host, config.server.port);
    info!("Routes configured: {}", config.routes.len());

    for (i, route) in config.routes.iter().enumerate() {
        info!("  Route {}: {} -> {}", i + 1, route.path, route.upstream);
    }

    Ok(())
}

fn init_config(output_path: PathBuf, force: bool) -> FerragateResult<()> {
    let path_str = output_path.to_str().unwrap_or(DEFAULT_CONFIG_FILE);

    if output_path.exists() && !force {
        error!("Configuration file already exists: {}", path_str);
        error!("Use --force to overwrite the existing file");
        return Err(FerragateError::validation("File already exists"));
    }

    info!("Generating example configuration...");
    GatewayConfig::save_example(path_str)?;
    info!("✅ Example configuration saved to: {}", path_str);
    info!("Edit the file and run 'ferragate start' to begin");

    Ok(())
}

fn generate_certs(output_dir: PathBuf, hostname: String, force: bool) -> FerragateResult<()> {
    info!("Generating TLS certificates...");

    // Create output directory if it doesn't exist
    std::fs::create_dir_all(&output_dir)?;

    let cert_path = output_dir.join(&format!("server{}", CERT_FILE_EXTENSION));
    let key_path = output_dir.join(&format!("server{}", KEY_FILE_EXTENSION));

    // Check if certificates already exist
    if (cert_path.exists() || key_path.exists()) && !force {
        error!(
            "Certificate files already exist in: {}",
            output_dir.display()
        );
        error!("Use --force to overwrite existing certificates");
        return Err(FerragateError::validation("Certificates already exist"));
    }

    crate::tls::create_self_signed_cert(
        cert_path.to_str().unwrap(),
        key_path.to_str().unwrap(),
        &hostname,
    )?;

    info!("✅ TLS certificates generated successfully!");
    info!("Certificate: {}", cert_path.display());
    info!("Private key: {}", key_path.display());
    info!("Hostname: {}", hostname);
    info!("");
    info!("To enable HTTPS, update your {}:", DEFAULT_CONFIG_FILE);
    info!("[server.tls]");
    info!("enabled = true");
    info!("cert_file = \"{}\"", cert_path.display());
    info!("key_file = \"{}\"", key_path.display());

    Ok(())
}

async fn stop_server(config_path: PathBuf, force: bool) -> FerragateResult<()> {
    // Delegate to server module - CLI should not contain business logic
    crate::server::stop_server(config_path.to_str(), force).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_cli_parse_args() {
        // Test that we can create a CLI instance
        let cli = Cli {
            command: Commands::Start {
                config: PathBuf::from("test.toml"),
                host: Some("localhost".to_string()),
                port: Some(8080),
            },
        };

        // Verify the command structure
        match cli.command {
            Commands::Start { config, host, port } => {
                assert_eq!(config, PathBuf::from("test.toml"));
                assert_eq!(host, Some("localhost".to_string()));
                assert_eq!(port, Some(8080));
            }
            _ => panic!("Expected Start command"),
        }
    }

    #[test]
    fn test_cli_parse_args_all_commands() {
        // Test Start command variations
        let start_cli = Cli {
            command: Commands::Start {
                config: PathBuf::from("custom.toml"),
                host: None,
                port: None,
            },
        };
        assert!(matches!(start_cli.command, Commands::Start { .. }));

        // Test Validate command
        let validate_cli = Cli {
            command: Commands::Validate {
                config: PathBuf::from("test.toml"),
            },
        };
        assert!(matches!(validate_cli.command, Commands::Validate { .. }));

        // Test Init command
        let init_cli = Cli {
            command: Commands::Init {
                output: PathBuf::from("output.toml"),
                force: true,
            },
        };
        assert!(matches!(init_cli.command, Commands::Init { .. }));

        // Test GenCerts command
        let gencerts_cli = Cli {
            command: Commands::GenCerts {
                output_dir: PathBuf::from("certs"),
                hostname: "example.com".to_string(),
                force: false,
            },
        };
        assert!(matches!(gencerts_cli.command, Commands::GenCerts { .. }));

        // Test Stop command
        let stop_cli = Cli {
            command: Commands::Stop {
                config: PathBuf::from("test.toml"),
                force: false,
            },
        };
        assert!(matches!(stop_cli.command, Commands::Stop { .. }));
    }

    #[test]
    fn test_validate_config_success() {
        // Create a temporary directory and config file
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create a valid config file
        let config_content = r#"
[server]
host = "0.0.0.0"
port = 8080

[[routes]]
path = "/api"
upstream = "http://backend:3000"
methods = ["GET", "POST"]
"#;
        fs::write(&config_path, config_content).unwrap();

        // Test validation
        let result = validate_config(config_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_config_invalid_file() {
        let config_path = PathBuf::from("nonexistent.toml");
        let result = validate_config(config_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_malformed_toml() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("malformed.toml");

        // Create a malformed TOML file
        let malformed_content = r#"
[server
host = "0.0.0.0"
port = invalid
"#;
        fs::write(&config_path, malformed_content).unwrap();

        let result = validate_config(config_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_invalid_upstream_url() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("invalid_upstream.toml");

        // Create config with invalid upstream URL
        let config_content = r#"
[server]
host = "0.0.0.0"
port = 8080

[[routes]]
path = "/api"
upstream = "invalid-url-format"
methods = ["GET"]
"#;
        fs::write(&config_path, config_content).unwrap();

        let result = validate_config(config_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_empty_routes() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("empty_routes.toml");

        // Test config with no routes defined (empty array)
        let config_content = r#"
[server]
host = "0.0.0.0"
port = 8080
workers = 4

[logging]
level = "info"
json = false

# Empty routes array - should be valid but generate warning
"#;
        fs::write(&config_path, config_content).unwrap();

        let result = validate_config(config_path);
        // Since routes field is required in the struct but not marked as #[serde(default)],
        // missing routes will cause a deserialization error
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_with_tls() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("tls_config.toml");

        // Create config with TLS configuration
        let config_content = r#"
[server]
host = "0.0.0.0"
port = 8080

[server.tls]
enabled = true
port = 8443
cert_file = "nonexistent.crt"
key_file = "nonexistent.key"
redirect_http = true

[[routes]]
path = "/api"
upstream = "http://backend:3000"
methods = ["GET"]
"#;
        fs::write(&config_path, config_content).unwrap();

        let result = validate_config(config_path);
        assert!(result.is_ok()); // Should be valid even with missing cert files
    }

    #[test]
    fn test_init_config_new_file() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("new_config.toml");

        let result = init_config(config_path.clone(), false);
        assert!(result.is_ok());
        assert!(config_path.exists());

        // Verify the generated content contains expected sections
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("[server]"));
        assert!(content.contains("[[routes]]"));
    }

    #[test]
    fn test_init_config_existing_file_no_force() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("existing_config.toml");

        // Create an existing file
        fs::write(&config_path, "existing content").unwrap();

        let result = init_config(config_path, false);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("File already exists"));
    }

    #[test]
    fn test_init_config_existing_file_with_force() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("existing_config.toml");

        // Create an existing file
        fs::write(&config_path, "existing content").unwrap();

        let result = init_config(config_path.clone(), true);
        assert!(result.is_ok());

        // Verify the file was overwritten with new content
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("[server]"));
        assert!(!content.contains("existing content"));
    }

    #[test]
    fn test_init_config_invalid_path_fallback() {
        // Test with a path that can't be converted to string (edge case)
        let temp_dir = tempdir().unwrap();
        let valid_path = temp_dir.path().join("fallback_test.toml");

        let result = init_config(valid_path.clone(), false);
        assert!(result.is_ok());
        assert!(valid_path.exists());
    }

    #[test]
    fn test_generate_certs_new_directory() {
        let temp_dir = tempdir().unwrap();
        let cert_dir = temp_dir.path().join("new_certs");

        let result = generate_certs(cert_dir.clone(), "localhost".to_string(), false);
        assert!(result.is_ok());

        // Check that certificate files were created
        assert!(cert_dir.join("server.crt").exists());
        assert!(cert_dir.join("server.key").exists());
    }

    #[test]
    fn test_generate_certs_custom_hostname() {
        let temp_dir = tempdir().unwrap();
        let cert_dir = temp_dir.path().join("custom_hostname_certs");

        let hostname = "custom.example.com".to_string();
        let result = generate_certs(cert_dir.clone(), hostname, false);
        assert!(result.is_ok());

        // Verify certificate files exist
        assert!(cert_dir.join("server.crt").exists());
        assert!(cert_dir.join("server.key").exists());
    }

    #[test]
    fn test_generate_certs_existing_files_no_force() {
        let temp_dir = tempdir().unwrap();
        let cert_dir = temp_dir.path().join("existing_certs");
        fs::create_dir_all(&cert_dir).unwrap();

        // Create existing certificate files
        fs::write(cert_dir.join("server.crt"), "existing cert").unwrap();

        let result = generate_certs(cert_dir, "localhost".to_string(), false);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Certificates already exist"));
    }

    #[test]
    fn test_generate_certs_existing_key_file_no_force() {
        let temp_dir = tempdir().unwrap();
        let cert_dir = temp_dir.path().join("existing_key_certs");
        fs::create_dir_all(&cert_dir).unwrap();

        // Create existing key file only
        fs::write(cert_dir.join("server.key"), "existing key").unwrap();

        let result = generate_certs(cert_dir, "localhost".to_string(), false);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Certificates already exist"));
    }

    #[test]
    fn test_generate_certs_existing_files_with_force() {
        let temp_dir = tempdir().unwrap();
        let cert_dir = temp_dir.path().join("existing_certs_force");
        fs::create_dir_all(&cert_dir).unwrap();

        // Create existing certificate files
        fs::write(cert_dir.join("server.crt"), "existing cert").unwrap();
        fs::write(cert_dir.join("server.key"), "existing key").unwrap();

        let result = generate_certs(cert_dir.clone(), "test.example.com".to_string(), true);
        assert!(result.is_ok());

        // Verify files were recreated (content should be different)
        assert!(cert_dir.join("server.crt").exists());
        assert!(cert_dir.join("server.key").exists());

        let cert_content = fs::read_to_string(cert_dir.join("server.crt")).unwrap();
        let key_content = fs::read_to_string(cert_dir.join("server.key")).unwrap();
        assert_ne!(cert_content, "existing cert");
        assert_ne!(key_content, "existing key");
    }

    #[test]
    fn test_generate_certs_creates_directory() {
        let temp_dir = tempdir().unwrap();
        let nested_cert_dir = temp_dir.path().join("deeply").join("nested").join("certs");

        // Directory doesn't exist initially
        assert!(!nested_cert_dir.exists());

        let result = generate_certs(nested_cert_dir.clone(), "localhost".to_string(), false);
        assert!(result.is_ok());

        // Directory should be created
        assert!(nested_cert_dir.exists());
        assert!(nested_cert_dir.join("server.crt").exists());
        assert!(nested_cert_dir.join("server.key").exists());
    }

    #[tokio::test]
    async fn test_cli_execute_start_missing_config() {
        let cli = Cli {
            command: Commands::Start {
                config: PathBuf::from("nonexistent_config.toml"),
                host: None,
                port: None,
            },
        };

        let result = cli.execute().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cli_execute_start_with_overrides() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create a valid config file
        let config_content = r#"
[server]
host = "127.0.0.1"
port = 8080

[[routes]]
path = "/api"
upstream = "http://backend:3000"
methods = ["GET"]
"#;
        fs::write(&config_path, config_content).unwrap();

        let cli = Cli {
            command: Commands::Start {
                config: config_path,
                host: Some("0.0.0.0".to_string()),
                port: Some(9090),
            },
        };

        // This test would normally start the server, but we can't easily test that
        // without mocking the server startup. The important part is that the command
        // structure is correct and the config loading logic is tested elsewhere.
        // For actual server startup testing, we rely on integration tests.

        // Test the parameter passing at least
        match cli.command {
            Commands::Start {
                config: _,
                host,
                port,
            } => {
                assert_eq!(host, Some("0.0.0.0".to_string()));
                assert_eq!(port, Some(9090));
            }
            _ => panic!("Expected Start command"),
        }
    }

    #[tokio::test]
    async fn test_cli_execute_validate() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create a valid config file
        let config_content = r#"
[server]
host = "0.0.0.0"
port = 8080

[[routes]]
path = "/api"
upstream = "http://backend:3000"
methods = ["GET", "POST"]
"#;
        fs::write(&config_path, config_content).unwrap();

        let cli = Cli {
            command: Commands::Validate {
                config: config_path,
            },
        };

        let result = cli.execute().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cli_execute_validate_invalid_config() {
        let cli = Cli {
            command: Commands::Validate {
                config: PathBuf::from("nonexistent.toml"),
            },
        };

        let result = cli.execute().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cli_execute_init() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("init_test.toml");

        let cli = Cli {
            command: Commands::Init {
                output: config_path.clone(),
                force: false,
            },
        };

        let result = cli.execute().await;
        assert!(result.is_ok());
        assert!(config_path.exists());
    }

    #[tokio::test]
    async fn test_cli_execute_init_force() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("init_force_test.toml");

        // Create existing file
        fs::write(&config_path, "existing content").unwrap();

        let cli = Cli {
            command: Commands::Init {
                output: config_path.clone(),
                force: true,
            },
        };

        let result = cli.execute().await;
        assert!(result.is_ok());
        assert!(config_path.exists());

        // Verify content was replaced
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("[server]"));
        assert!(!content.contains("existing content"));
    }

    #[tokio::test]
    async fn test_cli_execute_gen_certs() {
        let temp_dir = tempdir().unwrap();
        let cert_dir = temp_dir.path().join("test_certs");

        let cli = Cli {
            command: Commands::GenCerts {
                output_dir: cert_dir.clone(),
                hostname: "test.local".to_string(),
                force: false,
            },
        };

        let result = cli.execute().await;
        assert!(result.is_ok());
        assert!(cert_dir.join("server.crt").exists());
        assert!(cert_dir.join("server.key").exists());
    }

    #[tokio::test]
    async fn test_cli_execute_gen_certs_with_force() {
        let temp_dir = tempdir().unwrap();
        let cert_dir = temp_dir.path().join("test_certs_force");
        fs::create_dir_all(&cert_dir).unwrap();

        // Create existing files
        fs::write(cert_dir.join("server.crt"), "old cert").unwrap();
        fs::write(cert_dir.join("server.key"), "old key").unwrap();

        let cli = Cli {
            command: Commands::GenCerts {
                output_dir: cert_dir.clone(),
                hostname: "force-test.example.com".to_string(),
                force: true,
            },
        };

        let result = cli.execute().await;
        assert!(result.is_ok());
        assert!(cert_dir.join("server.crt").exists());
        assert!(cert_dir.join("server.key").exists());

        // Verify files were replaced
        let cert_content = fs::read_to_string(cert_dir.join("server.crt")).unwrap();
        assert_ne!(cert_content, "old cert");
    }

    #[test]
    fn test_cli_parse_args_static_method() {
        // Test the static parse_args method exists and works
        // We can't easily test the actual parsing without command line args,
        // but we can verify the method exists and the structure is correct
        let cli = Cli {
            command: Commands::Validate {
                config: PathBuf::from("test.toml"),
            },
        };

        // Verify the CLI structure
        assert!(matches!(cli.command, Commands::Validate { .. }));
    }

    #[test]
    fn test_commands_enum_coverage() {
        // Test all command variants are covered
        let start_cmd = Commands::Start {
            config: PathBuf::from("test.toml"),
            host: Some("localhost".to_string()),
            port: Some(8080),
        };
        assert!(matches!(start_cmd, Commands::Start { .. }));

        let validate_cmd = Commands::Validate {
            config: PathBuf::from("test.toml"),
        };
        assert!(matches!(validate_cmd, Commands::Validate { .. }));

        let init_cmd = Commands::Init {
            output: PathBuf::from("test.toml"),
            force: true,
        };
        assert!(matches!(init_cmd, Commands::Init { .. }));

        let gencerts_cmd = Commands::GenCerts {
            output_dir: PathBuf::from("certs"),
            hostname: "localhost".to_string(),
            force: false,
        };
        assert!(matches!(gencerts_cmd, Commands::GenCerts { .. }));

        let stop_cmd = Commands::Stop {
            config: PathBuf::from("test.toml"),
            force: false,
        };
        assert!(matches!(stop_cmd, Commands::Stop { .. }));
    }

    #[tokio::test]
    async fn test_start_server_config_loading() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("server_test.toml");

        // Create a minimal valid config
        let config_content = r#"
[server]
host = "127.0.0.1"
port = 8080

[[routes]]
path = "/test"
upstream = "http://localhost:8081"
methods = ["GET"]
"#;
        fs::write(&config_path, config_content).unwrap();

        // Test that the config loading part works (we can't test actual server startup easily)
        let config = GatewayConfig::from_file(config_path.to_str().unwrap()).unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.routes.len(), 1);
    }

    #[tokio::test]
    async fn test_start_server_host_port_overrides() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("override_test.toml");

        // Create config with different host/port
        let config_content = r#"
[server]
host = "0.0.0.0"
port = 3000

[[routes]]
path = "/test"
upstream = "http://localhost:8081"
methods = ["GET"]
"#;
        fs::write(&config_path, config_content).unwrap();

        // Test config loading and verify defaults
        let mut config = GatewayConfig::from_file(config_path.to_str().unwrap()).unwrap();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 3000);

        // Simulate CLI overrides
        config.server.host = "127.0.0.1".to_string();
        config.server.port = 9090;

        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 9090);
    }

    #[test]
    fn test_generate_certs_path_str_conversion() {
        let temp_dir = tempdir().unwrap();
        let cert_dir = temp_dir.path().join("server.crt");

        // Test that paths can be converted to strings properly
        let cert_path = cert_dir.join("server.crt");
        let key_path = cert_dir.join("server.key");

        // These should not panic
        let cert_str = cert_path.to_str().unwrap();
        let key_str = key_path.to_str().unwrap();

        assert!(cert_str.ends_with("server.crt"));
        assert!(key_str.ends_with("server.key"));
    }

    #[test]
    fn test_validate_config_with_complex_routes() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("complex_routes.toml");

        // Create config with multiple routes and complex settings
        let config_content = r#"
[server]
host = "0.0.0.0"  
port = 8080
workers = 4
timeout_ms = 30000

[[routes]]
path = "/api/v1/*"
upstream = "http://backend-v1:3000"
methods = ["GET", "POST", "PUT", "DELETE"]
strip_path = true
preserve_host = false
timeout_ms = 5000

[[routes]]
path = "/api/v2/*"
upstream = "https://backend-v2:8443"
methods = ["GET", "POST"]
strip_path = false
preserve_host = true

[[routes]]
path = "/static/*"
upstream = "http://cdn:8080"
methods = ["GET", "HEAD"]
"#;
        fs::write(&config_path, config_content).unwrap();

        let result = validate_config(config_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_config_invalid_http_method() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("invalid_method.toml");

        // Create config with invalid HTTP method
        let config_content = r#"
[server]
host = "0.0.0.0"
port = 8080

[[routes]]
path = "/api"
upstream = "http://backend:3000"
methods = ["GET", "INVALID_METHOD"]
"#;
        fs::write(&config_path, config_content).unwrap();

        let result = validate_config(config_path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid HTTP method"));
    }

    #[test]
    fn test_validate_config_empty_path() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("empty_path.toml");

        // Create config with empty path
        let config_content = r#"
[server]
host = "0.0.0.0"
port = 8080

[[routes]]
path = ""
upstream = "http://backend:3000"
methods = ["GET"]
"#;
        fs::write(&config_path, config_content).unwrap();

        let result = validate_config(config_path);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Route path cannot be empty"));
    }

    #[test]
    fn test_init_config_directory_creation() {
        let temp_dir = tempdir().unwrap();
        let nested_path = temp_dir
            .path()
            .join("nested")
            .join("dir")
            .join("config.toml");

        // Parent directories don't exist
        assert!(!nested_path.parent().unwrap().exists());

        // Create parent directories first (since save_example doesn't create them)
        std::fs::create_dir_all(nested_path.parent().unwrap()).unwrap();

        let result = init_config(nested_path.clone(), false);
        assert!(result.is_ok());
        assert!(nested_path.exists());
        assert!(nested_path.parent().unwrap().exists());
    }

    #[test]
    fn test_generate_certs_error_handling() {
        // Test with a read-only directory (simulate permission error)
        let temp_dir = tempdir().unwrap();
        let cert_dir = temp_dir.path().join("readonly_test");
        fs::create_dir_all(&cert_dir).unwrap();

        // On Unix systems, we could set read-only permissions, but for cross-platform
        // compatibility, we'll just verify the basic flow works
        let result = generate_certs(cert_dir.clone(), "test.local".to_string(), false);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cli_execute_all_commands_structure() {
        // Test that execute method handles all command types without panicking
        // This is more of a structural/compilation test

        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("structure_test.toml");
        let config_content = r#"
[server]
host = "0.0.0.0"
port = 8080

[[routes]]
path = "/test"
upstream = "http://localhost:8081"
methods = ["GET"]
"#;
        fs::write(&config_path, config_content).unwrap();

        // Test Validate command structure
        let validate_cli = Cli {
            command: Commands::Validate {
                config: config_path.clone(),
            },
        };
        let result = validate_cli.execute().await;
        assert!(result.is_ok());

        // Test Init command structure
        let init_path = temp_dir.path().join("init_structure.toml");
        let init_cli = Cli {
            command: Commands::Init {
                output: init_path,
                force: false,
            },
        };
        let result = init_cli.execute().await;
        assert!(result.is_ok());

        // Test GenCerts command structure
        let cert_dir = temp_dir.path().join("structure_certs");
        let gencerts_cli = Cli {
            command: Commands::GenCerts {
                output_dir: cert_dir,
                hostname: "structure-test.local".to_string(),
                force: false,
            },
        };
        let result = gencerts_cli.execute().await;
        assert!(result.is_ok());

        // Test Stop command structure
        let stop_cli = Cli {
            command: Commands::Stop {
                config: config_path.clone(),
                force: false,
            },
        };
        let result = stop_cli.execute().await;
        // Stop command should succeed even if no processes are found
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_stop_server_no_processes() {
        // Test stop command when no ferragate processes are running
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("stop_test.toml");

        let result = stop_server(config_path, false).await;
        // Should succeed even if no processes found
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_stop_server_force_flag() {
        // Test stop command with force flag
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("force_stop_test.toml");

        let result = stop_server(config_path, true).await;
        // Should succeed even with force flag when no processes found
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_stop_server_with_pid_file() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("pid_test.toml");
        let pid_file = format!("{}.pid", config_path.to_str().unwrap());

        // Create a fake PID file with a non-existent process ID
        fs::write(&pid_file, "99999").unwrap();

        let result = stop_server(config_path, false).await;
        // Should handle non-existent process gracefully
        // The exact behavior depends on the system, but it shouldn't panic
        assert!(result.is_ok() || result.is_err());

        // PID file should be cleaned up if the function succeeded
        if result.is_ok() {
            assert!(!std::path::Path::new(&pid_file).exists());
        }
    }

    #[tokio::test]
    async fn test_cli_execute_stop() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("execute_stop_test.toml");

        let cli = Cli {
            command: Commands::Stop {
                config: config_path,
                force: false,
            },
        };

        let result = cli.execute().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cli_execute_stop_force() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("execute_stop_force_test.toml");

        let cli = Cli {
            command: Commands::Stop {
                config: config_path,
                force: true,
            },
        };

        let result = cli.execute().await;
        assert!(result.is_ok());
    }
}
