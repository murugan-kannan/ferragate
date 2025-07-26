/// Custom error types for Ferragate API Gateway
/// 
/// This module provides structured error types that improve error handling
/// and debugging throughout the application.

use thiserror::Error;

/// Main error type for Ferragate operations
#[derive(Error, Debug)]
pub enum FerragateError {
    /// Configuration-related errors
    #[error("Configuration error: {message}")]
    Config { message: String },

    /// Network and connection errors  
    #[error("Network error: {message}")]
    Network { message: String },

    /// TLS/SSL related errors
    #[error("TLS error: {message}")]
    Tls { message: String },

    /// Proxy operation errors
    #[error("Proxy error: {message}")]
    Proxy { message: String },

    /// Health check errors
    #[error("Health check error: {message}")]
    Health { message: String },

    /// File I/O errors
    #[error("File I/O error: {message}")]
    Io { message: String },

    /// Validation errors
    #[error("Validation error: {message}")]
    Validation { message: String },

    /// Server startup/shutdown errors
    #[error("Server error: {message}")]
    Server { message: String },
}

/// Result type alias for Ferragate operations
pub type FerragateResult<T> = Result<T, FerragateError>;

impl FerragateError {
    /// Create a new configuration error
    pub fn config<S: Into<String>>(message: S) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    /// Create a new network error
    pub fn network<S: Into<String>>(message: S) -> Self {
        Self::Network {
            message: message.into(),
        }
    }

    /// Create a new TLS error
    pub fn tls<S: Into<String>>(message: S) -> Self {
        Self::Tls {
            message: message.into(),
        }
    }

    /// Create a new proxy error
    pub fn proxy<S: Into<String>>(message: S) -> Self {
        Self::Proxy {
            message: message.into(),
        }
    }

    /// Create a new health check error
    pub fn health<S: Into<String>>(message: S) -> Self {
        Self::Health {
            message: message.into(),
        }
    }

    /// Create a new I/O error
    pub fn io<S: Into<String>>(message: S) -> Self {
        Self::Io {
            message: message.into(),
        }
    }

    /// Create a new validation error
    pub fn validation<S: Into<String>>(message: S) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    /// Create a new server error
    pub fn server<S: Into<String>>(message: S) -> Self {
        Self::Server {
            message: message.into(),
        }
    }
}

/// Convert standard I/O errors to Ferragate errors
impl From<std::io::Error> for FerragateError {
    fn from(err: std::io::Error) -> Self {
        Self::Io {
            message: err.to_string(),
        }
    }
}

/// Convert serde JSON errors to Ferragate errors
impl From<serde_json::Error> for FerragateError {
    fn from(err: serde_json::Error) -> Self {
        Self::Config {
            message: format!("JSON parsing error: {}", err),
        }
    }
}

/// Convert TOML deserialization errors to Ferragate errors
impl From<toml::de::Error> for FerragateError {
    fn from(err: toml::de::Error) -> Self {
        Self::Config {
            message: format!("TOML parsing error: {}", err),
        }
    }
}

/// Convert TOML serialization errors to Ferragate errors
impl From<toml::ser::Error> for FerragateError {
    fn from(err: toml::ser::Error) -> Self {
        Self::Config {
            message: format!("TOML serialization error: {}", err),
        }
    }
}

/// Convert reqwest errors to Ferragate errors
impl From<reqwest::Error> for FerragateError {
    fn from(err: reqwest::Error) -> Self {
        Self::Network {
            message: err.to_string(),
        }
    }
}
