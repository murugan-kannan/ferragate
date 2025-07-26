/// Application constants for Ferragate API Gateway
///
/// This module contains all the magic numbers, default values, and string constants
/// used throughout the application to improve maintainability and consistency.
// Default server configuration
pub const DEFAULT_HOST: &str = "0.0.0.0";
pub const DEFAULT_HTTP_PORT: u16 = 3000;
pub const DEFAULT_HTTPS_PORT: u16 = 443;
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
pub const DEFAULT_LOG_LEVEL: &str = "info";

// Client configuration
pub const CLIENT_USER_AGENT: &str = concat!("FerraGate/", env!("CARGO_PKG_VERSION"));
pub const CLIENT_POOL_IDLE_TIMEOUT_SECS: u64 = 60;
pub const CLIENT_POOL_MAX_IDLE_PER_HOST: usize = 10;

// Health check endpoints
pub const HEALTH_ENDPOINT: &str = "/health";
pub const LIVENESS_ENDPOINT: &str = "/health/live";
pub const READINESS_ENDPOINT: &str = "/health/ready";

// File paths and extensions
pub const DEFAULT_CONFIG_FILE: &str = "gateway.toml";
pub const DEFAULT_LOG_DIR: &str = "logs";
pub const DEFAULT_CERT_DIR: &str = "certs";
pub const DEFAULT_LOG_FILE_PREFIX: &str = "ferragate";
pub const CERT_FILE_EXTENSION: &str = ".crt";
pub const KEY_FILE_EXTENSION: &str = ".key";

// Control socket configuration
#[cfg(unix)]
pub const CONTROL_SOCKET_PREFIX: &str = "/tmp/ferragate_";
#[cfg(windows)]
pub const CONTROL_SOCKET_PREFIX: &str = "ferragate_";

// Certificate configuration
pub const CERT_ORGANIZATION: &str = "FerraGate";
pub const CERT_COUNTRY: &str = "US";
pub const DEFAULT_HOSTNAME: &str = "localhost";

// HTTP headers that should not be forwarded to upstream
pub const FILTERED_HEADERS: &[&str] = &[
    "connection",
    "upgrade",
    "proxy-connection",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
];

// Response messages
pub const MSG_ROUTE_NOT_FOUND: &str = "No matching route found";
pub const MSG_HEALTH_CHECK_FAILED: &str = "Health check failed";
pub const MSG_SERVER_NOT_READY: &str = "Server not ready";
pub const MSG_INVALID_REQUEST_BODY: &str = "Failed to read request body";

// Buffer sizes
pub const CONTROL_SOCKET_BUFFER_SIZE: usize = 1024;

// Log messages
pub const LOG_SERVER_STARTING: &str = "Starting Ferragate API Gateway";
pub const LOG_SERVER_SHUTDOWN: &str = "Shutting down Ferragate API Gateway";
pub const LOG_CONFIG_LOADED: &str = "Configuration loaded successfully";
pub const LOG_TLS_ENABLED: &str = "TLS configuration loaded successfully";
