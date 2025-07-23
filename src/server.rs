use axum::{
    routing::{any, get},
    Router,
    response::Redirect,
    extract::Request,
};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use tracing::{info, error, warn};
use anyhow::Result;

use crate::config::GatewayConfig;
use crate::proxy::{ProxyState, proxy_handler, handle_not_found};
use crate::health::{health_handler, liveness_handler, readiness_handler, AppState};
use crate::tls;

pub async fn start_server(config: GatewayConfig) -> Result<()> {
    info!("Starting FerraGate API Gateway v0.1.0");
    
    // Create proxy state
    let proxy_state = ProxyState::new(config.clone());
    
    // Create health state
    let health_state = AppState::new();
    
    info!("Application state initialized");

    // Start background health check task
    let health_check_state = health_state.clone();
    tokio::spawn(async move {
        info!("Starting background health check task");
        crate::health::health_check_background_task(health_check_state).await;
    });

    // Build the router
    let app = create_router_with_states(proxy_state, health_state);

    // Check if TLS is enabled
    if let Some(tls_config) = &config.server.tls {
        if tls_config.enabled {
            // Start both HTTP and HTTPS servers
            let http_config = config.clone();
            let https_config = config.clone();
            let app_clone = app.clone();

            let http_handle = tokio::spawn(async move {
                start_http_server(http_config, app_clone).await
            });

            let https_handle = tokio::spawn(async move {
                start_https_server(https_config, app).await
            });

            // Wait for either server to fail
            tokio::select! {
                result = http_handle => {
                    match result {
                        Ok(Ok(())) => info!("HTTP server shut down normally"),
                        Ok(Err(e)) => error!("HTTP server error: {}", e),
                        Err(e) => error!("HTTP server task panicked: {}", e),
                    }
                }
                result = https_handle => {
                    match result {
                        Ok(Ok(())) => info!("HTTPS server shut down normally"),
                        Ok(Err(e)) => error!("HTTPS server error: {}", e),
                        Err(e) => error!("HTTPS server task panicked: {}", e),
                    }
                }
            }
        } else {
            // TLS disabled, start only HTTP server
            start_http_server(config, app).await?;
        }
    } else {
        // No TLS configuration, start only HTTP server
        start_http_server(config, app).await?;
    }

    Ok(())
}

async fn start_http_server(config: GatewayConfig, app: Router) -> Result<()> {
    let addr = SocketAddr::from((
        config.server.host.parse::<std::net::IpAddr>()
            .unwrap_or_else(|_| std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0))),
        config.server.port,
    ));

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    
    // Log startup information
    info!("🌐 HTTP server running on http://{}", addr);
    log_routes_info(&config);
    log_health_endpoints(&addr, false);
    
    // Check if we should redirect HTTP to HTTPS
    let app = if let Some(tls_config) = &config.server.tls {
        if tls_config.enabled && tls_config.redirect_http {
            info!("🔀 HTTP to HTTPS redirect enabled");
            create_redirect_router(tls_config.port)
        } else {
            app
        }
    } else {
        app
    };

    // Start the HTTP server
    if let Err(e) = axum::serve(listener, app).await {
        error!("HTTP Server error: {}", e);
        return Err(e.into());
    }

    Ok(())
}

async fn start_https_server(config: GatewayConfig, app: Router) -> Result<()> {
    let tls_config = config.server.tls.as_ref()
        .ok_or_else(|| anyhow::anyhow!("TLS configuration not found"))?;

    let addr = SocketAddr::from((
        config.server.host.parse::<std::net::IpAddr>()
            .unwrap_or_else(|_| std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0))),
        tls_config.port,
    ));

    // Generate self-signed certificates if they don't exist
    if !std::path::Path::new(&tls_config.cert_file).exists() || 
       !std::path::Path::new(&tls_config.key_file).exists() {
        
        // Create certs directory if it doesn't exist
        if let Some(parent) = std::path::Path::new(&tls_config.cert_file).parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        warn!("Certificate files not found, generating self-signed certificate");
        tls::create_self_signed_cert(
            &tls_config.cert_file,
            &tls_config.key_file,
            "localhost"
        )?;
    }

    // Load TLS configuration
    let rustls_config = tls::load_tls_config(&tls_config.cert_file, &tls_config.key_file).await?;

    info!("🔒 HTTPS server running on https://{}", addr);
    log_routes_info(&config);
    log_health_endpoints(&addr, true);

    // Start the HTTPS server
    if let Err(e) = axum_server::bind_rustls(addr, rustls_config)
        .serve(app.into_make_service())
        .await {
        error!("HTTPS Server error: {}", e);
        return Err(e.into());
    }

    Ok(())
}

fn create_redirect_router(https_port: u16) -> Router {
    Router::new()
        .fallback(move |request: Request| async move {
            let host = request.headers()
                .get("host")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("localhost");
            
            // Remove port from host if present, then add HTTPS port
            let host = host.split(':').next().unwrap_or(host);
            let https_url = if https_port == 443 {
                format!("https://{}{}", host, request.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or("/"))
            } else {
                format!("https://{}:{}{}", host, https_port, request.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or("/"))
            };
            
            info!("Redirecting HTTP request to HTTPS: {}", https_url);
            Redirect::permanent(&https_url)
        })
}

fn create_router_with_states(proxy_state: ProxyState, health_state: AppState) -> Router {
    Router::new()
        // Health endpoints (using health state)
        .route("/health", get(health_handler))
        .route("/health/live", get(liveness_handler))
        .route("/health/ready", get(readiness_handler))
        .with_state(health_state)
        // Proxy routes (using proxy state)
        .route("/*path", any(proxy_handler))
        .with_state(proxy_state)
        // Request tracing
        .layer(TraceLayer::new_for_http())
        // Fallback for unmatched routes
        .fallback(handle_not_found)
}

fn log_routes_info(config: &GatewayConfig) {
    info!("📊 Routes configured: {}", config.routes.len());
    for (i, route) in config.routes.iter().enumerate() {
        info!("   Route {}: {} -> {}", i + 1, route.path, route.upstream);
    }
}

fn log_health_endpoints(addr: &SocketAddr, is_https: bool) {
    let protocol = if is_https { "https" } else { "http" };
    info!("🏥 Health endpoints:");
    info!("   - Health: {}://{}/health", protocol, addr);
    info!("   - Liveness: {}://{}/health/live", protocol, addr);
    info!("   - Readiness: {}://{}/health/ready", protocol, addr);
    info!("🔧 Background health checks running every 30 seconds");
}
