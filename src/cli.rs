use clap::{Parser, Subcommand};
use std::path::PathBuf;
use anyhow::Result;
use tracing::{info, error};

use crate::config::GatewayConfig;

#[derive(Parser)]
#[command(name = env!("CARGO_PKG_NAME"))]
#[command(about = env!("CARGO_PKG_DESCRIPTION"))]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the gateway server
    Start {
        /// Configuration file path
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
        
        /// Override server host
        #[arg(long)]
        host: Option<String>,
        
        /// Override server port
        #[arg(short, long)]
        port: Option<u16>,
    },
    
    /// Validate configuration file
    Validate {
        /// Configuration file path
        #[arg(short, long, default_value = "gateway.toml")]
        config: PathBuf,
    },
    
    /// Generate example configuration
    Init {
        /// Output configuration file path
        #[arg(short, long, default_value = "gateway.toml")]
        output: PathBuf,
        
        /// Overwrite existing file
        #[arg(long)]
        force: bool,
    },
    
    /// Generate TLS certificates
    GenCerts {
        /// Certificate output directory
        #[arg(short, long, default_value = "certs")]
        output_dir: PathBuf,
        
        /// Hostname for the certificate
        #[arg(long, default_value = "localhost")]
        hostname: String,
        
        /// Overwrite existing certificates
        #[arg(long)]
        force: bool,
    },
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }

    pub async fn execute(self) -> Result<()> {
        match self.command {
            Commands::Start { config, host, port } => {
                start_server(config, host, port).await
            }
            Commands::Validate { config } => {
                validate_config(config)
            }
            Commands::Init { output, force } => {
                init_config(output, force)
            }
            Commands::GenCerts { output_dir, hostname, force } => {
                generate_certs(output_dir, hostname, force)
            }
        }
    }
}

async fn start_server(
    config_path: PathBuf,
    host_override: Option<String>,
    port_override: Option<u16>,
) -> Result<()> {
    info!("Starting FerraGate server...");
    
    let mut config = GatewayConfig::from_file(
        config_path.to_str().unwrap_or("gateway.toml")
    )?;

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
    crate::server::start_server(config).await
}

fn validate_config(config_path: PathBuf) -> Result<()> {
    info!("Validating configuration...");
    
    let config = GatewayConfig::from_file(
        config_path.to_str().unwrap_or("gateway.toml")
    )?;

    info!("✅ Configuration is valid!");
    info!("Server: {}:{}", config.server.host, config.server.port);
    info!("Routes configured: {}", config.routes.len());
    
    for (i, route) in config.routes.iter().enumerate() {
        info!("  Route {}: {} -> {}", i + 1, route.path, route.upstream);
    }

    Ok(())
}

fn init_config(output_path: PathBuf, force: bool) -> Result<()> {
    let path_str = output_path.to_str().unwrap_or("gateway.toml");
    
    if output_path.exists() && !force {
        error!("Configuration file already exists: {}", path_str);
        error!("Use --force to overwrite the existing file");
        return Err(anyhow::anyhow!("File already exists"));
    }

    info!("Generating example configuration...");
    GatewayConfig::save_example(path_str)?;
    info!("✅ Example configuration saved to: {}", path_str);
    info!("Edit the file and run 'ferragate start' to begin");

    Ok(())
}

fn generate_certs(output_dir: PathBuf, hostname: String, force: bool) -> Result<()> {
    info!("Generating TLS certificates...");
    
    // Create output directory if it doesn't exist
    std::fs::create_dir_all(&output_dir)?;
    
    let cert_path = output_dir.join("server.crt");
    let key_path = output_dir.join("server.key");
    
    // Check if certificates already exist
    if (cert_path.exists() || key_path.exists()) && !force {
        error!("Certificate files already exist in: {}", output_dir.display());
        error!("Use --force to overwrite existing certificates");
        return Err(anyhow::anyhow!("Certificates already exist"));
    }

    crate::tls::create_self_signed_cert(
        cert_path.to_str().unwrap(),
        key_path.to_str().unwrap(),
        &hostname
    )?;
    
    info!("✅ TLS certificates generated successfully!");
    info!("Certificate: {}", cert_path.display());
    info!("Private key: {}", key_path.display());
    info!("Hostname: {}", hostname);
    info!("");
    info!("To enable HTTPS, update your gateway.toml:");
    info!("[server.tls]");
    info!("enabled = true");
    info!("cert_file = \"{}\"", cert_path.display());
    info!("key_file = \"{}\"", key_path.display());

    Ok(())
}
