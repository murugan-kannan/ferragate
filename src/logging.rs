use std::env;
use tracing::info;
use tracing_subscriber::{
    fmt::time::UtcTime,
    EnvFilter,
};

/// Configuration for the logging system
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,
    /// Whether to output logs in JSON format
    pub json_format: bool,
    /// Whether to log to file
    pub log_to_file: bool,
    /// Directory for log files
    pub log_dir: String,
    /// Log file prefix
    pub log_file_prefix: String,
    /// Whether to include file and line numbers in logs
    pub include_location: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
            json_format: env::var("LOG_JSON")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            log_to_file: env::var("LOG_TO_FILE")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            log_dir: env::var("LOG_DIR").unwrap_or_else(|_| "logs".to_string()),
            log_file_prefix: env::var("LOG_FILE_PREFIX")
                .unwrap_or_else(|_| "ferragate".to_string()),
            include_location: env::var("LOG_INCLUDE_LOCATION")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
        }
    }
}

/// Initialize the logging system with the given configuration
pub fn init_logging(config: LoggingConfig) -> anyhow::Result<()> {
    // Create the log directory if it doesn't exist
    if config.log_to_file {
        std::fs::create_dir_all(&config.log_dir)?;
    }

    // Create environment filter
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&config.level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let builder = tracing_subscriber::fmt()
        .with_timer(UtcTime::rfc_3339())
        .with_target(true)
        .with_file(config.include_location)
        .with_line_number(config.include_location)
        .with_env_filter(env_filter);

    if config.json_format {
        builder.json().init();
    } else {
        builder.init();
    }

    // If file logging is enabled, we'll set up a separate file appender
    if config.log_to_file {
        // Note: For simplicity, we're not implementing dual output (console + file) in this version
        // This would require a more complex subscriber setup with layered outputs
        // For now, we'll just log a message about the file logging configuration
        
        info!("File logging configured: {}/{} (requires custom subscriber setup)", 
              config.log_dir, config.log_file_prefix);
    }

    Ok(())
}

/// Initialize logging with default configuration
pub fn init_default_logging() -> anyhow::Result<()> {
    init_logging(LoggingConfig::default())
}

/// Create a structured logging context for request tracing
#[allow(dead_code)]
pub fn create_request_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    use std::sync::atomic::{AtomicU32, Ordering};
    
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("req_{}_{}", timestamp, counter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, "info");
        assert!(!config.json_format);
        assert!(config.log_to_file);
        assert_eq!(config.log_dir, "logs");
        assert_eq!(config.log_file_prefix, "ferragate");
    }

    #[test]
    fn test_request_id_generation() {
        let id1 = create_request_id();
        let id2 = create_request_id();
        assert!(id1.starts_with("req_"));
        assert!(id2.starts_with("req_"));
        assert_ne!(id1, id2);
    }
}
