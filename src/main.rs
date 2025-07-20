mod health;
mod logging;

use axum::{
    routing::get,
    Router,
};
use health::{health_handler, liveness_handler, readiness_handler, AppState};
use tower_http::trace::TraceLayer;
use tracing::{info, error};
use logging::init_default_logging;

// Simple root endpoint
pub async fn root_handler() -> &'static str {
    info!("Root endpoint accessed");
    "Ferragate API is running!"
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging system
    if let Err(e) = init_default_logging() {
        eprintln!("Failed to initialize logging: {}", e);
        return Err(e);
    }

    info!("Starting Ferragate application");
    
    let state = AppState::new();
    info!("Application state initialized");

    // Start background health check task
    let health_check_state = state.clone();
    tokio::spawn(async move {
        info!("Starting background health check task");
        health::health_check_background_task(health_check_state).await;
    });

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/health", get(health_handler))
        .route("/health/live", get(liveness_handler))
        .route("/health/ready", get(readiness_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    
    // Log startup information
    info!("Ferragate server running on http://0.0.0.0:3000");
    info!("Health endpoints:");
    info!("   - Health: http://localhost:3000/health");
    info!("   - Liveness: http://localhost:3000/health/live");
    info!("   - Readiness: http://localhost:3000/health/ready");
    info!("Background health checks running every 30 seconds");

    if let Err(e) = axum::serve(listener, app).await {
        error!("Server error: {}", e);
        return Err(e.into());
    }

    Ok(())
}
