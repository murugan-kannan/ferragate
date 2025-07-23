mod health;
mod logging;
mod config;
mod proxy;
mod cli;
mod server;
mod tls;

use cli::Cli;
use logging::init_default_logging;
use tracing::error;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging system
    if let Err(e) = init_default_logging() {
        eprintln!("Failed to initialize logging: {}", e);
        return Err(e);
    }

    // Parse CLI arguments and execute
    let cli = Cli::parse_args();
    
    if let Err(e) = cli.execute().await {
        error!("Application error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
