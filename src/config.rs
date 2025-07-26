use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use tracing::{debug, info, warn};

use crate::constants::*;
use crate::error::{FerragateError, FerragateResult};

/// Main gateway configuration structure
///
/// This represents the complete configuration for the Ferragate API Gateway,
/// including server settings, routing rules, and logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Server configuration (host, port, workers, etc.)
    pub server: ServerConfig,
    /// List of routing rules
    pub routes: Vec<RouteConfig>,
    /// Logging configuration (defaults to basic settings if not specified)
    #[serde(default)]
    pub logging: LoggingConfig,
}

/// Server configuration structure
///
/// Defines how the gateway server should be configured, including
/// network settings, performance tuning, and TLS configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server bind address (default: "0.0.0.0")
    #[serde(default = "default_host")]
    pub host: String,
    /// Server port (default: 3000)
    #[serde(default = "default_port")]
    pub port: u16,
    /// Number of worker threads (default: auto-detected)
    #[serde(default = "default_workers")]
    pub workers: Option<usize>,
    /// Request timeout in milliseconds (default: 30000)
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// TLS configuration (optional)
    #[serde(default)]
    pub tls: Option<TlsConfig>,
}

/// TLS/SSL configuration structure
///
/// Defines HTTPS settings including certificate paths and behavior options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Whether TLS is enabled
    pub enabled: bool,
    /// HTTPS port (default: 443)
    #[serde(default = "default_https_port")]
    pub port: u16,
    /// Path to the certificate file
    pub cert_file: String,
    /// Path to the private key file
    pub key_file: String,
    /// Whether to redirect HTTP requests to HTTPS
    #[serde(default)]
    pub redirect_http: bool,
}

/// Route configuration structure
///
/// Defines a single routing rule that maps incoming requests to upstream services.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    /// URL path pattern to match (e.g., "/api/v1/*")
    pub path: String,
    /// Upstream service URL (e.g., "http://localhost:8080")
    pub upstream: String,
    /// Allowed HTTP methods (empty = all methods allowed)
    #[serde(default)]
    pub methods: Vec<String>,
    /// Additional headers to add to upstream requests
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Whether to strip the matched path prefix before forwarding
    #[serde(default)]
    pub strip_path: bool,
    /// Whether to preserve the original Host header
    #[serde(default)]
    pub preserve_host: bool,
    /// Route-specific timeout in milliseconds (overrides server default)
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// Logging configuration structure
///
/// Controls how the gateway handles logging output, including levels and formats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error) - default: "info"
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Whether to output logs in JSON format
    #[serde(default)]
    pub json: bool,
    /// Whether to write logs to files
    #[serde(default)]
    pub file: bool,
    /// Directory for log files (if file logging is enabled)
    #[serde(default)]
    pub dir: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            json: false,
            file: false,
            dir: None,
        }
    }
}

fn default_host() -> String {
    DEFAULT_HOST.to_string()
}

fn default_port() -> u16 {
    DEFAULT_HTTP_PORT
}

fn default_workers() -> Option<usize> {
    None
}

fn default_log_level() -> String {
    DEFAULT_LOG_LEVEL.to_string()
}

fn default_https_port() -> u16 {
    DEFAULT_HTTPS_PORT
}

impl GatewayConfig {
    /// Load configuration from a TOML file
    ///
    /// Reads and parses a TOML configuration file, validates the configuration,
    /// and returns a GatewayConfig instance.
    pub fn from_file(path: &str) -> FerragateResult<Self> {
        info!("Loading configuration from: {}", path);

        let content = fs::read_to_string(path).map_err(|e| {
            FerragateError::config(format!("Failed to read config file '{}': {}", path, e))
        })?;

        let config: GatewayConfig = toml::from_str(&content).map_err(|e| {
            FerragateError::config(format!("Failed to parse config file '{}': {}", path, e))
        })?;

        info!("{} from: {}", LOG_CONFIG_LOADED, path);
        debug!("Loaded config: {:#?}", config);

        config.validate()?;

        Ok(config)
    }

    /// Validate the configuration
    ///
    /// Performs comprehensive validation of the configuration including
    /// route validation, TLS setup, and basic sanity checks.
    pub fn validate(&self) -> FerragateResult<()> {
        if self.routes.is_empty() {
            warn!("No routes configured - gateway will only serve health endpoints");
        }

        // Validate TLS configuration if enabled
        if let Some(tls) = &self.server.tls {
            if tls.enabled {
                // Check if certificate files exist (but allow auto-generation)
                if !std::path::Path::new(&tls.cert_file).exists() {
                    warn!(
                        "TLS certificate file not found: {} (will be auto-generated)",
                        tls.cert_file
                    );
                }
                if !std::path::Path::new(&tls.key_file).exists() {
                    warn!(
                        "TLS private key file not found: {} (will be auto-generated)",
                        tls.key_file
                    );
                }
                info!(
                    "TLS configuration validated: cert={}, key={}",
                    tls.cert_file, tls.key_file
                );
            }
        }

        // Validate each route
        for (i, route) in self.routes.iter().enumerate() {
            route
                .validate()
                .map_err(|e| FerragateError::config(format!("Route {}: {}", i, e)))?;
        }

        info!("Configuration validation completed successfully");
        Ok(())
    }

    pub fn default_config() -> Self {
        Self {
            server: ServerConfig {
                host: default_host(),
                port: default_port(),
                workers: default_workers(),
                timeout_ms: Some(DEFAULT_TIMEOUT_MS), // 30 seconds
                tls: Some(TlsConfig {
                    enabled: true,
                    port: default_https_port(),
                    cert_file: "/etc/ssl/certs/ssl-cert-snakeoil.pem".to_string(),
                    key_file: "/etc/ssl/private/ssl-cert-snakeoil.key".to_string(),
                    redirect_http: true,
                }),
            },
            routes: vec![
                RouteConfig {
                    path: "/get/*".to_string(),
                    upstream: "https://httpbin.org".to_string(),
                    methods: vec!["GET".to_string()],
                    headers: HashMap::new(),
                    strip_path: true,
                    preserve_host: false,
                    timeout_ms: Some(30000),
                },
                RouteConfig {
                    path: "/post/*".to_string(),
                    upstream: "https://httpbin.org".to_string(),
                    methods: vec!["POST".to_string()],
                    headers: HashMap::new(),
                    strip_path: true,
                    preserve_host: false,
                    timeout_ms: Some(30000),
                },
                RouteConfig {
                    path: "/json/*".to_string(),
                    upstream: "https://httpbin.org".to_string(),
                    methods: vec!["GET".to_string()],
                    headers: HashMap::new(),
                    strip_path: true,
                    preserve_host: false,
                    timeout_ms: Some(30000),
                },
                RouteConfig {
                    path: "/status/*".to_string(),
                    upstream: "https://httpbin.org".to_string(),
                    methods: vec!["GET".to_string()],
                    headers: HashMap::new(),
                    strip_path: true,
                    preserve_host: false,
                    timeout_ms: Some(30000),
                },
            ],
            logging: LoggingConfig::default(),
        }
    }

    pub fn save_example(path: &str) -> FerragateResult<()> {
        let config = Self::default_config();
        let content = toml::to_string_pretty(&config)?;

        fs::write(path, &content)?;

        info!("Example configuration saved to: {}", path);
        Ok(())
    }
}

impl RouteConfig {
    /// Check if this route matches the given path
    ///
    /// Supports wildcard matching with "/*" suffix for prefix matching.
    /// Returns true if the path matches this route's pattern.
    pub fn matches_path(&self, path: &str) -> bool {
        if self.path.ends_with("/*") {
            let prefix = &self.path[..self.path.len() - 2];
            path.starts_with(prefix)
        } else {
            path == self.path
        }
    }

    /// Check if this route allows the given HTTP method
    ///
    /// Returns true if:
    /// - No methods are specified (allows all methods)
    /// - The method is explicitly listed in allowed methods
    pub fn matches_method(&self, method: &str) -> bool {
        if self.methods.is_empty() {
            return true; // No method restriction
        }
        self.methods
            .iter()
            .any(|m| m.to_uppercase() == method.to_uppercase())
    }

    /// Transform the original request path for upstream forwarding
    ///
    /// If `strip_path` is enabled and the route uses wildcard matching,
    /// the matched prefix will be removed from the path before forwarding.
    pub fn transform_path(&self, original_path: &str) -> String {
        if !self.strip_path {
            return original_path.to_string();
        }

        if self.path.ends_with("/*") {
            let prefix = &self.path[..self.path.len() - 2];
            if original_path.starts_with(prefix) {
                let remaining = &original_path[prefix.len()..];
                if remaining.is_empty() {
                    "/".to_string()
                } else {
                    remaining.to_string()
                }
            } else {
                original_path.to_string()
            }
        } else {
            original_path.to_string()
        }
    }

    /// Validate this route configuration
    ///
    /// Checks that the route has valid path, upstream URL, and HTTP methods.
    pub fn validate(&self) -> FerragateResult<()> {
        // Validate path
        if self.path.is_empty() {
            return Err(FerragateError::validation("Route path cannot be empty"));
        }

        // Validate upstream URL
        url::Url::parse(&self.upstream).map_err(|e| {
            FerragateError::validation(format!("Invalid upstream URL '{}': {}", self.upstream, e))
        })?;

        // Validate methods
        for method in &self.methods {
            match method.to_uppercase().as_str() {
                "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS" => {}
                _ => {
                    return Err(FerragateError::validation(format!(
                        "Invalid HTTP method: {}",
                        method
                    )))
                }
            }
        }

        Ok(())
    }

    /// Get the effective timeout for this route
    ///
    /// Returns the route-specific timeout if set, otherwise the provided default.
    pub fn effective_timeout(&self, default_timeout_ms: u64) -> u64 {
        self.timeout_ms.unwrap_or(default_timeout_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_matching() {
        let route = RouteConfig {
            path: "/get/*".to_string(),
            upstream: "https://httpbin.org".to_string(),
            methods: vec!["GET".to_string()],
            headers: HashMap::new(),
            strip_path: true,
            preserve_host: false,
            timeout_ms: None,
        };

        assert!(route.matches_path("/get/anything"));
        assert!(route.matches_path("/get/"));
        assert!(!route.matches_path("/post/anything"));
        assert!(!route.matches_path("/health"));

        assert!(route.matches_method("GET"));
        assert!(route.matches_method("get"));
        assert!(!route.matches_method("POST"));
    }

    #[test]
    fn test_path_transformation() {
        let route = RouteConfig {
            path: "/status/*".to_string(),
            upstream: "https://httpbin.org".to_string(),
            methods: vec![],
            headers: HashMap::new(),
            strip_path: true,
            preserve_host: false,
            timeout_ms: None,
        };

        assert_eq!(route.transform_path("/status/200"), "/200");
        assert_eq!(route.transform_path("/status/"), "/");
        assert_eq!(route.transform_path("/status"), "/");
    }

    #[test]
    fn test_effective_timeout_with_route_specific() {
        let route = RouteConfig {
            path: "/api/*".to_string(),
            upstream: "http://example.com".to_string(),
            methods: vec![],
            headers: std::collections::HashMap::new(),
            strip_path: false,
            preserve_host: false,
            timeout_ms: Some(5000), // Route-specific timeout
        };

        // Should return route-specific timeout, ignoring default
        assert_eq!(route.effective_timeout(30000), 5000);
        assert_eq!(route.effective_timeout(10000), 5000);
    }

    #[test]
    fn test_effective_timeout_with_server_default() {
        let route = RouteConfig {
            path: "/api/*".to_string(),
            upstream: "http://example.com".to_string(),
            methods: vec![],
            headers: std::collections::HashMap::new(),
            strip_path: false,
            preserve_host: false,
            timeout_ms: None, // No route-specific timeout
        };

        // Should return server default timeout
        assert_eq!(route.effective_timeout(30000), 30000);
        assert_eq!(route.effective_timeout(15000), 15000);
        assert_eq!(
            route.effective_timeout(DEFAULT_TIMEOUT_MS),
            DEFAULT_TIMEOUT_MS
        );
    }

    #[test]
    fn test_effective_timeout_zero_values() {
        let route_with_zero = RouteConfig {
            path: "/api/*".to_string(),
            upstream: "http://example.com".to_string(),
            methods: vec![],
            headers: std::collections::HashMap::new(),
            strip_path: false,
            preserve_host: false,
            timeout_ms: Some(0), // Zero timeout (valid but unusual)
        };

        // Should return zero if explicitly set
        assert_eq!(route_with_zero.effective_timeout(30000), 0);

        let route_no_timeout = RouteConfig {
            path: "/api/*".to_string(),
            upstream: "http://example.com".to_string(),
            methods: vec![],
            headers: std::collections::HashMap::new(),
            strip_path: false,
            preserve_host: false,
            timeout_ms: None,
        };

        // Should handle zero default gracefully
        assert_eq!(route_no_timeout.effective_timeout(0), 0);
    }
}
