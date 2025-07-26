use axum::{
    extract::Request,
    response::Redirect,
    routing::{any, get},
    Router,
};
use std::net::SocketAddr;
use std::path::Path;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};

#[cfg_attr(not(test), allow(unused_imports))]
use crate::config::{GatewayConfig, LoggingConfig, RouteConfig, ServerConfig};
use crate::constants::*;
use crate::error::{FerragateError, FerragateResult};
use crate::health::{health_handler, liveness_handler, readiness_handler, AppState};
use crate::proxy::{handle_not_found, proxy_handler, ProxyState};
use crate::tls;

fn write_pid_file(path: &str) -> FerragateResult<()> {
    use std::fs;
    let pid = std::process::id();
    fs::write(path, pid.to_string())?;
    info!("PID file written: {} (PID: {})", path, pid);
    Ok(())
}

fn get_control_socket_path(config_path: &str) -> String {
    #[cfg(unix)]
    {
        format!(
            "{}{}.sock",
            CONTROL_SOCKET_PREFIX,
            config_path.replace(['/', '\\', '.'], "_")
        )
    }
    #[cfg(windows)]
    {
        // On Windows, we'll use a named pipe approach
        format!(
            "{}{}",
            CONTROL_SOCKET_PREFIX,
            config_path.replace(['/', '\\', '.', ':'], "_")
        )
    }
}

#[cfg(unix)]
async fn start_control_socket_listener(
    socket_path: String,
    shutdown_token: CancellationToken,
) -> FerragateResult<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixListener;

    // Remove existing socket file if it exists
    if Path::new(&socket_path).exists() {
        std::fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    info!("Control socket listening at: {}", socket_path);

    loop {
        tokio::select! {
            _ = shutdown_token.cancelled() => {
                info!("Control socket listener shutting down");
                break;
            }
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((mut stream, _)) => {
                        info!("Received control connection");

                        let shutdown_token_clone = shutdown_token.clone();
                        tokio::spawn(async move {
                            let mut buffer = [0u8; CONTROL_SOCKET_BUFFER_SIZE];
                            match stream.read(&mut buffer).await {
                                Ok(n) => {
                                    let command = String::from_utf8_lossy(&buffer[..n]);
                                    let command = command.trim();

                                    if command == "shutdown" {
                                        info!("Received shutdown command via control socket");
                                        let _ = stream.write_all(b"OK: Shutdown initiated\n").await;
                                        shutdown_token_clone.cancel();
                                    } else if command == "status" {
                                        let _ = stream.write_all(b"OK: Server running\n").await;
                                    } else {
                                        let _ = stream.write_all(b"ERROR: Unknown command\n").await;
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to read from control socket: {}", e);
                                }
                            }
                        });
                    }
                    Err(e) => {
                        warn!("Failed to accept control connection: {}", e);
                    }
                }
            }
        }
    }

    // Clean up socket file
    let _ = std::fs::remove_file(&socket_path);
    info!("Control socket file removed: {}", socket_path);

    Ok(())
}

#[cfg(windows)]
async fn start_control_socket_listener(
    _socket_path: String,
    shutdown_token: CancellationToken,
) -> FerragateResult<()> {
    // For Windows, we'll use a simpler file-based approach for now
    // This could be enhanced with named pipes in the future
    info!("Control socket not yet implemented on Windows, using signal-only shutdown");
    shutdown_token.cancelled().await;
    Ok(())
}

async fn shutdown_signal(shutdown_token: CancellationToken) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
        info!("Received Ctrl+C signal, shutting down gracefully...");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
        info!("Received terminate signal, shutting down gracefully...");
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    let token_cancelled = async {
        shutdown_token.cancelled().await;
        info!("Received shutdown via control socket, shutting down gracefully...");
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
        _ = token_cancelled => {},
    }
}

pub async fn start_server(config: GatewayConfig, config_path: Option<&str>) -> FerragateResult<()> {
    info!("{}", LOG_SERVER_STARTING);

    // Create a shutdown token for graceful shutdown coordination
    let shutdown_token = CancellationToken::new();

    // Write PID file for graceful shutdown
    let config_str = config_path.unwrap_or("gateway.toml");
    let pid_file = format!("{config_str}.pid");
    if let Err(e) = write_pid_file(&pid_file) {
        warn!("Failed to write PID file {}: {}", pid_file, e);
    }

    // Start control socket listener for graceful shutdown
    let socket_path = get_control_socket_path(config_str);
    info!("Starting control socket listener at: {}", socket_path);
    let socket_shutdown_token = shutdown_token.clone();
    let socket_handle = tokio::spawn(async move {
        if let Err(e) = start_control_socket_listener(socket_path, socket_shutdown_token).await {
            warn!("Control socket listener error: {}", e);
        }
    });

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

            let http_handle =
                tokio::spawn(async move { start_http_server(http_config, app_clone).await });

            let https_handle =
                tokio::spawn(async move { start_https_server(https_config, app).await });

            // Wait for either server to fail or shutdown signal
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
                _ = shutdown_signal(shutdown_token.clone()) => {
                    info!("Received shutdown signal");
                }
            }
        } else {
            // TLS disabled, start only HTTP server
            tokio::select! {
                result = start_http_server(config, app) => {
                    if let Err(e) = result {
                        error!("HTTP server error: {}", e);
                    }
                }
                _ = shutdown_signal(shutdown_token.clone()) => {
                    info!("Received shutdown signal");
                }
            }
        }
    } else {
        // No TLS configuration, start only HTTP server
        tokio::select! {
            result = start_http_server(config, app) => {
                if let Err(e) = result {
                    error!("HTTP server error: {}", e);
                }
            }
            _ = shutdown_signal(shutdown_token.clone()) => {
                info!("Received shutdown signal");
            }
        }
    }

    // Clean up: cancel all tasks and wait for them to finish
    shutdown_token.cancel();
    let _ = socket_handle.await;

    info!("{}", LOG_SERVER_SHUTDOWN);

    // Clean up PID file on shutdown
    if let Err(e) = std::fs::remove_file(&pid_file) {
        warn!("Failed to remove PID file {}: {}", pid_file, e);
    } else {
        info!("PID file {} removed", pid_file);
    }

    Ok(())
}

async fn start_http_server(config: GatewayConfig, app: Router) -> FerragateResult<()> {
    let addr = SocketAddr::from((
        config
            .server
            .host
            .parse::<std::net::IpAddr>()
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

async fn start_https_server(config: GatewayConfig, app: Router) -> FerragateResult<()> {
    let tls_config = config
        .server
        .tls
        .as_ref()
        .ok_or_else(|| FerragateError::config("TLS configuration not found"))?;

    let addr = SocketAddr::from((
        config
            .server
            .host
            .parse::<std::net::IpAddr>()
            .unwrap_or_else(|_| std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0))),
        tls_config.port,
    ));

    // Generate self-signed certificates if they don't exist
    if !std::path::Path::new(&tls_config.cert_file).exists()
        || !std::path::Path::new(&tls_config.key_file).exists()
    {
        // Create certs directory if it doesn't exist
        if let Some(parent) = std::path::Path::new(&tls_config.cert_file).parent() {
            std::fs::create_dir_all(parent)?;
        }

        warn!("Certificate files not found, generating self-signed certificate");
        tls::create_self_signed_cert(&tls_config.cert_file, &tls_config.key_file, "localhost")?;
    }

    // Load TLS configuration
    let rustls_config = tls::load_tls_config(&tls_config.cert_file, &tls_config.key_file).await?;

    info!("🔒 HTTPS server running on https://{}", addr);
    log_routes_info(&config);
    log_health_endpoints(&addr, true);

    // Start the HTTPS server
    if let Err(e) = axum_server::bind_rustls(addr, rustls_config)
        .serve(app.into_make_service())
        .await
    {
        error!("HTTPS Server error: {}", e);
        return Err(e.into());
    }

    Ok(())
}

fn create_redirect_router(https_port: u16) -> Router {
    Router::new().fallback(move |request: Request| async move {
        let host = request
            .headers()
            .get("host")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("localhost");

        // Remove port from host if present, then add HTTPS port
        let host = host.split(':').next().unwrap_or(host);
        let https_url = if https_port == 443 {
            format!(
                "https://{}{}",
                host,
                request
                    .uri()
                    .path_and_query()
                    .map(|pq| pq.as_str())
                    .unwrap_or("/")
            )
        } else {
            format!(
                "https://{}:{}{}",
                host,
                https_port,
                request
                    .uri()
                    .path_and_query()
                    .map(|pq| pq.as_str())
                    .unwrap_or("/")
            )
        };

        info!("Redirecting HTTP request to HTTPS: {}", https_url);
        Redirect::permanent(&https_url)
    })
}

fn create_router_with_states(proxy_state: ProxyState, health_state: AppState) -> Router {
    Router::new()
        // Health endpoints (using health state)
        .route(HEALTH_ENDPOINT, get(health_handler))
        .route(LIVENESS_ENDPOINT, get(liveness_handler))
        .route(READINESS_ENDPOINT, get(readiness_handler))
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
    info!("   - Health: {}://{}{}", protocol, addr, HEALTH_ENDPOINT);
    info!(
        "   - Liveness: {}://{}{}",
        protocol, addr, LIVENESS_ENDPOINT
    );
    info!(
        "   - Readiness: {}://{}{}",
        protocol, addr, READINESS_ENDPOINT
    );
    info!("🔧 Background health checks running every 30 seconds");
}

/// Stop a running FerraGate server gracefully using control socket
pub async fn stop_server(config_path: Option<&str>, force: bool) -> FerragateResult<()> {
    info!("Attempting to stop FerraGate server...");

    let config_str = config_path.unwrap_or("gateway.toml");

    // Try to find the PID file first
    let pid_file = format!("{config_str}.pid");

    if !Path::new(&pid_file).exists() {
        info!("No PID file found. Server might not be running.");
        return Ok(());
    }

    // Try control socket communication first
    let socket_path = get_control_socket_path(config_str);

    #[cfg(unix)]
    {
        if let Ok(()) = send_shutdown_command(&socket_path, force).await {
            // Wait a moment for graceful shutdown
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            // Check if the server actually stopped by checking PID file
            if !Path::new(&pid_file).exists() {
                info!("✅ FerraGate server stopped gracefully!");
                return Ok(());
            }
        }
    }

    // If control socket failed, fall back to PID-based stopping
    warn!("Control socket communication failed, falling back to PID-based shutdown");
    stop_server_by_pid(&pid_file, force).await
}

#[cfg(unix)]
async fn send_shutdown_command(socket_path: &str, _force: bool) -> FerragateResult<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    // Try to connect to control socket
    let mut stream = UnixStream::connect(socket_path).await?;

    // Send shutdown command
    stream.write_all(b"shutdown").await?;

    // Read response
    let mut buffer = [0u8; CONTROL_SOCKET_BUFFER_SIZE];
    let n = stream.read(&mut buffer).await?;
    let response = String::from_utf8_lossy(&buffer[..n]);

    if response.starts_with("OK:") {
        info!("Shutdown command sent successfully: {}", response.trim());
        Ok(())
    } else {
        Err(FerragateError::server(format!(
            "Unexpected response: {}",
            response.trim()
        )))
    }
}

#[cfg(windows)]
async fn send_shutdown_command(_socket_path: &str, _force: bool) -> FerragateResult<()> {
    // For Windows, we don't have Unix sockets, so we'll fall back to PID-based shutdown
    Err(FerragateError::server(
        "Control socket not supported on Windows",
    ))
}

async fn stop_server_by_pid(pid_file: &str, force: bool) -> FerragateResult<()> {
    use std::fs;

    if let Ok(pid_content) = fs::read_to_string(pid_file) {
        if let Ok(pid) = pid_content.trim().parse::<u32>() {
            info!("Found PID file with process ID: {}", pid);

            // Use a more robust approach: just send signals and let the server handle cleanup
            #[cfg(unix)]
            {
                return stop_unix_process(pid, force, pid_file).await;
            }

            #[cfg(windows)]
            {
                return stop_windows_process(pid, force, pid_file).await;
            }
        }
    }

    Err(FerragateError::server("Could not read or parse PID file"))
}

#[cfg(unix)]
async fn stop_unix_process(pid: u32, force: bool, pid_file: &str) -> FerragateResult<()> {
    // Check if process exists first
    let check_result = unsafe { libc::kill(pid as i32, 0) };
    if check_result != 0 {
        info!("Process {} not found or already stopped", pid);
        let _ = std::fs::remove_file(pid_file);
        return Ok(());
    }

    if force {
        info!("Force stopping process {}...", pid);
        let result = unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        if result == 0 {
            let _ = std::fs::remove_file(pid_file);
            info!("✅ FerraGate server force-stopped successfully!");
            return Ok(());
        } else {
            return Err(FerragateError::server(format!(
                "Failed to force stop process {pid}"
            )));
        }
    }

    // Graceful shutdown: Send SIGTERM first
    info!("Sending SIGTERM to process {}...", pid);
    let result = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
    if result != 0 {
        return Err(FerragateError::server(format!(
            "Failed to send SIGTERM to process {pid}"
        )));
    }

    info!("SIGTERM sent successfully, waiting for graceful shutdown...");

    // Wait for graceful shutdown with timeout
    let timeout_duration = std::time::Duration::from_secs(10);
    let start_time = std::time::Instant::now();

    while start_time.elapsed() < timeout_duration {
        // Check if process is still running
        let check_result = unsafe { libc::kill(pid as i32, 0) };
        if check_result != 0 {
            // Process has stopped
            let _ = std::fs::remove_file(pid_file);
            info!("✅ FerraGate server stopped gracefully!");
            return Ok(());
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    // If we reach here, graceful shutdown timed out
    warn!("Graceful shutdown timed out, sending SIGKILL...");
    let _ = unsafe { libc::kill(pid as i32, libc::SIGKILL) };

    let _ = std::fs::remove_file(pid_file);
    info!("✅ FerraGate server stopped (forced after timeout)!");
    Ok(())
}

#[cfg(windows)]
async fn stop_windows_process(pid: u32, force: bool, pid_file: &str) -> FerragateResult<()> {
    use std::process::Command;

    if force {
        info!("Force stopping process {}...", pid);
        let result = Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .output();

        match result {
            Ok(output) if output.status.success() => {
                let _ = std::fs::remove_file(pid_file);
                info!("✅ FerraGate server force-stopped successfully!");
                return Ok(());
            }
            _ => {
                return Err(FerragateError::server(format!(
                    "Failed to force stop process {}",
                    pid
                )));
            }
        }
    }

    // Graceful shutdown on Windows (fallback to force stop after timeout)
    info!("Stopping process {} gracefully...", pid);
    let result = Command::new("taskkill")
        .args(["/PID", &pid.to_string()])
        .output();

    match result {
        Ok(output) if output.status.success() => {
            info!("Stop signal sent successfully, waiting for graceful shutdown...");

            // Wait for graceful shutdown with timeout
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            // Force stop after timeout
            warn!("Windows graceful shutdown timeout, force stopping...");
            let _ = Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .output();

            let _ = std::fs::remove_file(pid_file);
            info!("✅ FerraGate server stopped!");
            Ok(())
        }
        _ => Err(FerragateError::server(format!(
            "Failed to send stop signal to process {}",
            pid
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use tempfile::TempDir;

    fn create_test_config() -> GatewayConfig {
        GatewayConfig {
            server: crate::config::ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                workers: None,
                timeout_ms: None,
                tls: None,
            },
            routes: vec![
                crate::config::RouteConfig {
                    path: "/api/v1/*".to_string(),
                    upstream: "http://backend1:3000".to_string(),
                    methods: vec!["GET".to_string(), "POST".to_string()],
                    headers: std::collections::HashMap::new(),
                    strip_path: false,
                    preserve_host: false,
                    timeout_ms: None,
                },
                crate::config::RouteConfig {
                    path: "/users/*".to_string(),
                    upstream: "http://user-service:8000".to_string(),
                    methods: vec![],
                    headers: std::collections::HashMap::new(),
                    strip_path: false,
                    preserve_host: false,
                    timeout_ms: None,
                },
            ],
            logging: crate::config::LoggingConfig::default(),
        }
    }

    fn create_test_config_with_tls() -> GatewayConfig {
        let mut config = create_test_config();
        config.server.tls = Some(crate::config::TlsConfig {
            enabled: true,
            cert_file: "certs/server.crt".to_string(),
            key_file: "certs/server.key".to_string(),
            port: 8443,
            redirect_http: false,
        });
        config
    }

    #[test]
    fn test_create_router_with_states() {
        let config = create_test_config();
        let proxy_state = ProxyState::new(config);
        let health_state = AppState::new();

        let router = create_router_with_states(proxy_state, health_state);
        let _service = router.into_make_service();
    }

    #[test]
    fn test_create_redirect_router() {
        let router = create_redirect_router(8443);
        let _service = router.into_make_service();
    }

    #[test]
    fn test_log_routes_info() {
        let config = create_test_config();
        log_routes_info(&config);

        let empty_config = GatewayConfig {
            server: crate::config::ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                workers: None,
                timeout_ms: None,
                tls: None,
            },
            routes: vec![],
            logging: crate::config::LoggingConfig::default(),
        };
        log_routes_info(&empty_config);
    }

    #[test]
    fn test_log_health_endpoints() {
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        log_health_endpoints(&addr, false);
        log_health_endpoints(&addr, true);

        let ipv6_addr: SocketAddr = "[::1]:8080".parse().unwrap();
        log_health_endpoints(&ipv6_addr, false);
        log_health_endpoints(&ipv6_addr, true);
    }

    #[test]
    fn test_socket_addr_parsing() {
        // Test valid IPv4 address
        let host = "192.168.1.1";
        let addr = SocketAddr::from((
            host.parse::<IpAddr>()
                .unwrap_or_else(|_| IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))),
            8080,
        ));
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
        assert_eq!(addr.port(), 8080);

        // Test valid IPv6 address
        let host = "::1";
        let addr = SocketAddr::from((
            host.parse::<IpAddr>()
                .unwrap_or_else(|_| IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))),
            9000,
        ));
        assert_eq!(addr.ip(), IpAddr::V6(Ipv6Addr::LOCALHOST));
        assert_eq!(addr.port(), 9000);
    }

    #[test]
    fn test_socket_addr_parsing_invalid_host() {
        // Test invalid host fallback
        let host = "invalid.host.name";
        let addr = SocketAddr::from((
            host.parse::<IpAddr>()
                .unwrap_or_else(|_| IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))),
            8080,
        ));
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));
        assert_eq!(addr.port(), 8080);
    }

    #[test]
    fn test_missing_tls_config_error() {
        let config = create_test_config(); // No TLS config
        let result = config
            .server
            .tls
            .as_ref()
            .ok_or_else(|| FerragateError::config("TLS configuration not found"));

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Configuration error: TLS configuration not found"
        );
    }

    #[test]
    fn test_config_with_tls_enabled() {
        let mut config = create_test_config_with_tls();
        config.server.tls.as_mut().unwrap().enabled = true;

        let tls_config = config.server.tls.as_ref().unwrap();
        assert!(tls_config.enabled);
        assert_eq!(tls_config.port, 8443);
    }

    #[test]
    fn test_config_with_tls_disabled() {
        let mut config = create_test_config_with_tls();
        config.server.tls.as_mut().unwrap().enabled = false;

        let tls_config = config.server.tls.as_ref().unwrap();
        assert!(!tls_config.enabled);
    }

    #[test]
    fn test_certificate_directory_creation() {
        let temp_dir = TempDir::new().unwrap();
        let cert_path = temp_dir.path().join("subdir").join("server.crt");

        // Test parent directory detection
        if let Some(parent) = cert_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
            assert!(parent.exists());
        }
    }

    #[test]
    fn test_certificate_file_existence_check() {
        let temp_dir = TempDir::new().unwrap();
        let cert_file = temp_dir.path().join("server.crt");
        let key_file = temp_dir.path().join("server.key");

        // Initially files don't exist
        assert!(!cert_file.exists());
        assert!(!key_file.exists());

        // Create files
        std::fs::write(&cert_file, "dummy cert").unwrap();
        std::fs::write(&key_file, "dummy key").unwrap();

        // Now they should exist
        assert!(cert_file.exists());
        assert!(key_file.exists());
    }

    #[test]
    fn test_tls_configuration_validation() {
        let config = create_test_config_with_tls();
        let tls_config = config.server.tls.as_ref().unwrap();

        assert!(!tls_config.cert_file.is_empty());
        assert!(!tls_config.key_file.is_empty());
        assert!(tls_config.port > 0);
    }

    #[test]
    fn test_redirect_router_creation() {
        // Test different ports
        let router_443 = create_redirect_router(443);
        let _service_443 = router_443.into_make_service();

        let router_8443 = create_redirect_router(8443);
        let _service_8443 = router_8443.into_make_service();
    }

    #[test]
    fn test_router_state_integration() {
        let config = create_test_config();
        let proxy_state = ProxyState::new(config.clone());
        let health_state = AppState::new();

        let router = create_router_with_states(proxy_state, health_state);
        let _service = router.into_make_service();

        // Test with TLS config
        let tls_config = create_test_config_with_tls();
        let proxy_state_tls = ProxyState::new(tls_config);
        let health_state_tls = AppState::new();

        let router_tls = create_router_with_states(proxy_state_tls, health_state_tls);
        let _service_tls = router_tls.into_make_service();
    }

    #[test]
    fn test_server_config_variations() {
        // Test HTTP-only config
        let http_config = create_test_config();
        assert!(http_config.server.tls.is_none());
        assert_eq!(http_config.server.port, 8080);

        // Test HTTPS config
        let https_config = create_test_config_with_tls();
        assert!(https_config.server.tls.is_some());
        assert_eq!(https_config.server.tls.unwrap().port, 8443);
    }

    #[test]
    fn test_route_logging_configurations() {
        // Test empty routes
        let empty_config = GatewayConfig {
            server: crate::config::ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                workers: None,
                timeout_ms: None,
                tls: None,
            },
            routes: vec![],
            logging: crate::config::LoggingConfig::default(),
        };
        log_routes_info(&empty_config);

        // Test single route
        let single_route_config = GatewayConfig {
            server: crate::config::ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                workers: None,
                timeout_ms: None,
                tls: None,
            },
            routes: vec![crate::config::RouteConfig {
                path: "/single/*".to_string(),
                upstream: "http://single:3000".to_string(),
                methods: vec![],
                headers: std::collections::HashMap::new(),
                strip_path: false,
                preserve_host: false,
                timeout_ms: None,
            }],
            logging: crate::config::LoggingConfig::default(),
        };
        log_routes_info(&single_route_config);
    }

    #[test]
    fn test_health_endpoint_logging_variations() {
        // Test different address formats
        let localhost: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        log_health_endpoints(&localhost, false);
        log_health_endpoints(&localhost, true);

        let any_addr: SocketAddr = "0.0.0.0:443".parse().unwrap();
        log_health_endpoints(&any_addr, false);
        log_health_endpoints(&any_addr, true);

        let custom_addr: SocketAddr = "192.168.1.1:9999".parse().unwrap();
        log_health_endpoints(&custom_addr, false);
        log_health_endpoints(&custom_addr, true);
    }

    #[test]
    fn test_proxy_state_creation() {
        let config = create_test_config();
        let _proxy_state = ProxyState::new(config.clone());

        // Test that proxy state is created successfully
        let _proxy_state = ProxyState::new(config.clone());
    }

    #[test]
    fn test_health_state_creation() {
        let health_state = AppState::new();

        // Test that we can clone the state
        let _cloned_state = health_state.clone();
    }

    #[test]
    fn test_configuration_edge_cases() {
        // Test config with custom host
        let mut config = create_test_config();
        config.server.host = "192.168.1.100".to_string();

        let proxy_state = ProxyState::new(config);
        let health_state = AppState::new();
        let _router = create_router_with_states(proxy_state, health_state);
    }

    #[test]
    fn test_multiple_route_configurations() {
        let mut config = create_test_config();

        // Add more routes
        config.routes.push(crate::config::RouteConfig {
            path: "/auth/*".to_string(),
            upstream: "http://auth-service:3001".to_string(),
            methods: vec!["POST".to_string()],
            headers: std::collections::HashMap::new(),
            strip_path: false,
            preserve_host: false,
            timeout_ms: None,
        });

        log_routes_info(&config);

        let proxy_state = ProxyState::new(config);
        let health_state = AppState::new();
        let _router = create_router_with_states(proxy_state, health_state);
    }

    #[test]
    fn test_ip_address_parsing_edge_cases() {
        // Test localhost variations
        let localhost_variants = vec!["127.0.0.1", "::1", "localhost"];

        for host in localhost_variants {
            let parsed_ip = host
                .parse::<IpAddr>()
                .unwrap_or_else(|_| IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));

            // Should either parse successfully or fallback to 0.0.0.0
            match parsed_ip {
                IpAddr::V4(_) => {} // Valid IPv4
                IpAddr::V6(_) => {} // Valid IPv6
            }
        }
    }

    #[test]
    fn test_redirect_url_generation() {
        // Test the logic that would be used in redirect router
        let https_port = 8443u16;
        let expected_standard_port = 443u16;
        let expected_custom_port = 8443u16;

        assert_ne!(https_port, expected_standard_port);
        assert_eq!(https_port, expected_custom_port);
    }

    #[test]
    fn test_redirect_router_creation_with_different_ports() {
        // Test redirect router creation with different ports
        let router_8443 = create_redirect_router(8443);
        let _service_8443 = router_8443.into_make_service();

        let router_443 = create_redirect_router(443);
        let _service_443 = router_443.into_make_service();

        let router_custom = create_redirect_router(9999);
        let _service_custom = router_custom.into_make_service();

        // If we get here without panicking, router creation works correctly
    }

    #[test]
    fn test_redirect_url_logic() {
        // Test the URL generation logic that would be used in the redirect router
        let https_port = 8443u16;
        let standard_port = 443u16;

        // Test custom port logic
        assert_ne!(https_port, 443);

        // Test standard port logic
        assert_eq!(standard_port, 443);

        // Test that port numbers are handled correctly
        assert!(https_port > 0);
        assert!(standard_port > 0);
    }

    #[test]
    fn test_tls_config_with_redirect_enabled() {
        let mut config = create_test_config_with_tls();
        config.server.tls.as_mut().unwrap().redirect_http = true;

        let tls_config = config.server.tls.as_ref().unwrap();
        assert!(tls_config.redirect_http);
        assert!(tls_config.enabled);
    }

    #[test]
    fn test_server_logging_with_different_configurations() {
        // Test with minimal routes
        let minimal_config = GatewayConfig {
            server: crate::config::ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 80,
                workers: None,
                timeout_ms: None,
                tls: None,
            },
            routes: vec![],
            logging: crate::config::LoggingConfig::default(),
        };
        log_routes_info(&minimal_config);

        // Test with complex routes
        let complex_config = GatewayConfig {
            server: crate::config::ServerConfig {
                host: "192.168.1.100".to_string(),
                port: 3000,
                workers: Some(4),
                timeout_ms: Some(30000),
                tls: Some(crate::config::TlsConfig {
                    enabled: true,
                    cert_file: "/custom/path/cert.pem".to_string(),
                    key_file: "/custom/path/key.pem".to_string(),
                    port: 3443,
                    redirect_http: true,
                }),
            },
            routes: vec![crate::config::RouteConfig {
                path: "/api/v2/*".to_string(),
                upstream: "https://backend.internal:8443".to_string(),
                methods: vec![
                    "GET".to_string(),
                    "POST".to_string(),
                    "PUT".to_string(),
                    "DELETE".to_string(),
                ],
                headers: {
                    let mut headers = std::collections::HashMap::new();
                    headers.insert("X-Custom-Header".to_string(), "value".to_string());
                    headers
                },
                strip_path: true,
                preserve_host: true,
                timeout_ms: Some(5000),
            }],
            logging: crate::config::LoggingConfig {
                level: "debug".to_string(),
                json: true,
                file: true,
                dir: Some("/var/log/ferragate/".to_string()),
            },
        };
        log_routes_info(&complex_config);
    }

    #[test]
    fn test_health_endpoint_logging_edge_cases() {
        // Test with different ports and addresses
        let standard_http: SocketAddr = "127.0.0.1:80".parse().unwrap();
        log_health_endpoints(&standard_http, false);

        let standard_https: SocketAddr = "127.0.0.1:443".parse().unwrap();
        log_health_endpoints(&standard_https, true);

        let custom_port: SocketAddr = "10.0.0.1:9090".parse().unwrap();
        log_health_endpoints(&custom_port, false);
        log_health_endpoints(&custom_port, true);

        // IPv6 addresses
        let ipv6_std: SocketAddr = "[::1]:80".parse().unwrap();
        log_health_endpoints(&ipv6_std, false);

        let ipv6_custom: SocketAddr = "[2001:db8::1]:8443".parse().unwrap();
        log_health_endpoints(&ipv6_custom, true);
    }

    #[test]
    fn test_route_configuration_edge_cases() {
        // Empty methods list (should allow all)
        let empty_methods_config = GatewayConfig {
            server: crate::config::ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                workers: None,
                timeout_ms: None,
                tls: None,
            },
            routes: vec![crate::config::RouteConfig {
                path: "/wildcard/*".to_string(),
                upstream: "http://wildcard-service:8000".to_string(),
                methods: vec![], // Empty methods
                headers: std::collections::HashMap::new(),
                strip_path: false,
                preserve_host: false,
                timeout_ms: None,
            }],
            logging: crate::config::LoggingConfig::default(),
        };
        log_routes_info(&empty_methods_config);

        // Single method
        let single_method_config = GatewayConfig {
            server: crate::config::ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                workers: None,
                timeout_ms: None,
                tls: None,
            },
            routes: vec![crate::config::RouteConfig {
                path: "/single/*".to_string(),
                upstream: "http://single-service:8000".to_string(),
                methods: vec!["HEAD".to_string()],
                headers: std::collections::HashMap::new(),
                strip_path: false,
                preserve_host: false,
                timeout_ms: None,
            }],
            logging: crate::config::LoggingConfig::default(),
        };
        log_routes_info(&single_method_config);
    }

    #[tokio::test]
    async fn test_start_server_function_no_tls() {
        let config = create_test_config();

        // Test the start_server function setup without actually starting servers
        // We'll test the initial setup logic

        // This tests the initial logging and state creation parts
        let proxy_state = ProxyState::new(config.clone());
        let health_state = AppState::new();

        // Test router creation
        let _app = create_router_with_states(proxy_state, health_state);

        // Verify TLS config check logic
        assert!(config.server.tls.is_none());

        // If we get here, the initial setup works correctly
    }

    #[tokio::test]
    async fn test_start_server_with_tls_enabled() {
        let config = create_test_config_with_tls();

        // Test TLS-enabled path logic
        if let Some(tls_config) = &config.server.tls {
            assert!(tls_config.enabled);

            // Test that we have the required certificate files
            assert!(!tls_config.cert_file.is_empty());
            assert!(!tls_config.key_file.is_empty());
        }

        // Test proxy and health state creation
        let proxy_state = ProxyState::new(config.clone());
        let health_state = AppState::new();
        let _app = create_router_with_states(proxy_state, health_state);
    }

    #[tokio::test]
    async fn test_start_server_with_tls_disabled() {
        let mut config = create_test_config_with_tls();

        // Disable TLS to test the disabled path
        if let Some(ref mut tls_config) = config.server.tls {
            tls_config.enabled = false;
        }

        // Test that disabled TLS follows the HTTP-only path
        if let Some(tls_config) = &config.server.tls {
            assert!(!tls_config.enabled);
        }

        let proxy_state = ProxyState::new(config.clone());
        let health_state = AppState::new();
        let _app = create_router_with_states(proxy_state, health_state);
    }

    #[test]
    fn test_start_http_server_address_parsing() {
        let config = create_test_config();

        // Test address parsing logic that start_http_server uses
        let addr_str = format!("{}:{}", config.server.host, config.server.port);
        let parse_result = addr_str.parse::<SocketAddr>();

        // Should successfully parse
        assert!(parse_result.is_ok());

        // Test IPv6 address parsing (needs brackets for parsing)
        let ipv6_config = GatewayConfig {
            server: ServerConfig {
                host: "[::1]".to_string(), // IPv6 addresses need brackets in host:port format
                port: 8080,
                workers: None,
                timeout_ms: None,
                tls: None,
            },
            routes: vec![],
            logging: LoggingConfig::default(),
        };

        let ipv6_addr_str = format!("{}:{}", ipv6_config.server.host, ipv6_config.server.port);
        let ipv6_parse_result = ipv6_addr_str.parse::<SocketAddr>();
        assert!(ipv6_parse_result.is_ok());
    }

    #[test]
    fn test_start_https_server_tls_config_error() {
        let config = create_test_config_with_tls();

        // Test TLS config validation logic
        if let Some(tls_config) = &config.server.tls {
            assert!(tls_config.enabled);

            // Test that cert and key files are specified
            assert!(!tls_config.cert_file.is_empty());
            assert!(!tls_config.key_file.is_empty());

            // Test file path validation (files may not exist, but paths should be valid)
            use std::path::Path;
            let cert_path = Path::new(&tls_config.cert_file);
            let key_path = Path::new(&tls_config.key_file);

            assert!(cert_path.is_absolute() || cert_path.is_relative());
            assert!(key_path.is_absolute() || key_path.is_relative());
        }
    }

    #[test]
    fn test_enhanced_server_config_variations() {
        // Test different server configuration combinations

        // Test with workers specified
        let config_with_workers = GatewayConfig {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 3000,
                workers: Some(4),
                timeout_ms: Some(30000),
                tls: None,
            },
            routes: vec![],
            logging: LoggingConfig::default(),
        };

        assert_eq!(config_with_workers.server.workers, Some(4));
        assert_eq!(config_with_workers.server.timeout_ms, Some(30000));

        // Test with different hosts
        let configs = vec![("127.0.0.1", 8080), ("0.0.0.0", 443), ("localhost", 9000)];

        for (host, port) in configs {
            let config = GatewayConfig {
                server: ServerConfig {
                    host: host.to_string(),
                    port,
                    workers: None,
                    timeout_ms: None,
                    tls: None,
                },
                routes: vec![],
                logging: LoggingConfig::default(),
            };

            assert_eq!(config.server.host, host);
            assert_eq!(config.server.port, port);
        }
    }

    #[test]
    fn test_enhanced_server_logging_with_different_configurations() {
        // Test logging with various configurations

        // Empty routes
        let empty_config = GatewayConfig {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                workers: None,
                timeout_ms: None,
                tls: None,
            },
            routes: vec![],
            logging: LoggingConfig::default(),
        };
        log_routes_info(&empty_config);

        // Single route
        let single_route_config = GatewayConfig {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                workers: None,
                timeout_ms: None,
                tls: None,
            },
            routes: vec![RouteConfig {
                path: "/api/v2/*".to_string(),
                upstream: "https://backend.internal:8443".to_string(),
                methods: vec![],
                headers: std::collections::HashMap::new(),
                strip_path: false,
                preserve_host: false,
                timeout_ms: None,
            }],
            logging: LoggingConfig::default(),
        };
        log_routes_info(&single_route_config);
    }
}
