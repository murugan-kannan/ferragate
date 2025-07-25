use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub server: ServerConfig,
    pub routes: Vec<RouteConfig>,
    #[serde(default)]
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_workers")]
    pub workers: Option<usize>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub tls: Option<TlsConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub enabled: bool,
    #[serde(default = "default_https_port")]
    pub port: u16,
    pub cert_file: String,
    pub key_file: String,
    #[serde(default)]
    pub redirect_http: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    pub path: String,
    pub upstream: String,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub strip_path: bool,
    #[serde(default)]
    pub preserve_host: bool,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub json: bool,
    #[serde(default)]
    pub file: bool,
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
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    3000
}

fn default_workers() -> Option<usize> {
    None
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_https_port() -> u16 {
    443
}

impl GatewayConfig {
    pub fn from_file(path: &str) -> Result<Self> {
        debug!("Loading configuration from: {}", path);

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path))?;

        let config: GatewayConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path))?;

        info!("Configuration loaded successfully from: {}", path);
        debug!("Loaded config: {:#?}", config);

        config.validate()?;

        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
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

        for (i, route) in self.routes.iter().enumerate() {
            // Validate path
            if route.path.is_empty() {
                return Err(anyhow::anyhow!("Route {} has empty path", i));
            }

            // Validate upstream URL
            let _url = url::Url::parse(&route.upstream).with_context(|| {
                format!("Invalid upstream URL in route {}: {}", i, route.upstream)
            })?;

            // Validate methods
            for method in &route.methods {
                match method.to_uppercase().as_str() {
                    "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS" => {}
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Invalid HTTP method in route {}: {}",
                            i,
                            method
                        ))
                    }
                }
            }

            debug!(
                "Route {} validated: {} -> {}",
                i, route.path, route.upstream
            );
        }

        info!("Configuration validation successful");
        Ok(())
    }

    pub fn default_config() -> Self {
        Self {
            server: ServerConfig {
                host: default_host(),
                port: default_port(),
                workers: default_workers(),
                timeout_ms: Some(30000), // 30 seconds
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

    pub fn save_example(path: &str) -> Result<()> {
        let config = Self::default_config();
        let content =
            toml::to_string_pretty(&config).context("Failed to serialize example config")?;

        fs::write(path, content)
            .with_context(|| format!("Failed to write example config to: {}", path))?;

        info!("Example configuration saved to: {}", path);
        Ok(())
    }
}

impl RouteConfig {
    pub fn matches_path(&self, path: &str) -> bool {
        if self.path.ends_with("/*") {
            let prefix = &self.path[..self.path.len() - 2];
            path.starts_with(prefix)
        } else {
            path == self.path
        }
    }

    pub fn matches_method(&self, method: &str) -> bool {
        if self.methods.is_empty() {
            return true; // No method restriction
        }
        self.methods
            .iter()
            .any(|m| m.to_uppercase() == method.to_uppercase())
    }

    pub fn transform_path(&self, original_path: &str) -> String {
        if self.strip_path && self.path.ends_with("/*") {
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
}
