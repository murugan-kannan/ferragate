use std::env;
use tracing::info;
use tracing_subscriber::{fmt::time::UtcTime, EnvFilter};

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

    let result = if config.json_format {
        builder.json().try_init()
    } else {
        builder.try_init()
    };

    // Handle the case where a global subscriber is already set
    match result {
        Ok(()) => {}
        Err(e) => {
            if e.to_string().contains("already been set") {
                // Subscriber already set, which is okay for tests
                // Just log a debug message if possible
                eprintln!("Warning: Global subscriber already set: {}", e);
            } else {
                return Err(anyhow::anyhow!("Failed to initialize logging: {}", e));
            }
        }
    }

    // If file logging is enabled, we'll set up a separate file appender
    if config.log_to_file {
        // Note: For simplicity, we're not implementing dual output (console + file) in this version
        // This would require a more complex subscriber setup with layered outputs
        // For now, we'll just log a message about the file logging configuration

        info!(
            "File logging configured: {}/{} (requires custom subscriber setup)",
            config.log_dir, config.log_file_prefix
        );
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
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

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
    use std::env;
    use tempfile::TempDir;

    #[test]
    #[serial_test::serial]
    fn test_default_config() {
        // Clear all relevant environment variables first
        env::remove_var("RUST_LOG");
        env::remove_var("LOG_JSON");
        env::remove_var("LOG_TO_FILE");
        env::remove_var("LOG_DIR");
        env::remove_var("LOG_FILE_PREFIX");
        env::remove_var("LOG_INCLUDE_LOCATION");

        let config = LoggingConfig::default();
        assert_eq!(config.level, "info");
        assert!(!config.json_format);
        assert!(config.log_to_file);
        assert_eq!(config.log_dir, "logs");
        assert_eq!(config.log_file_prefix, "ferragate");
        assert!(!config.include_location);
    }

    #[test]
    #[serial_test::serial]
    fn test_config_from_env_vars() {
        // Save original environment variables if they exist
        let orig_rust_log = env::var("RUST_LOG").ok();
        let orig_log_json = env::var("LOG_JSON").ok();
        let orig_log_to_file = env::var("LOG_TO_FILE").ok();
        let orig_log_dir = env::var("LOG_DIR").ok();
        let orig_log_file_prefix = env::var("LOG_FILE_PREFIX").ok();
        let orig_log_include_location = env::var("LOG_INCLUDE_LOCATION").ok();

        // Set test environment variables
        env::set_var("RUST_LOG", "debug");
        env::set_var("LOG_JSON", "true");
        env::set_var("LOG_TO_FILE", "false");
        env::set_var("LOG_DIR", "custom_logs");
        env::set_var("LOG_FILE_PREFIX", "custom_prefix");
        env::set_var("LOG_INCLUDE_LOCATION", "true");

        let config = LoggingConfig::default();

        assert_eq!(config.level, "debug");
        assert!(config.json_format);
        assert!(!config.log_to_file);
        assert_eq!(config.log_dir, "custom_logs");
        assert_eq!(config.log_file_prefix, "custom_prefix");
        assert!(config.include_location);

        // Restore original environment variables
        match orig_rust_log {
            Some(val) => env::set_var("RUST_LOG", val),
            None => env::remove_var("RUST_LOG"),
        }
        match orig_log_json {
            Some(val) => env::set_var("LOG_JSON", val),
            None => env::remove_var("LOG_JSON"),
        }
        match orig_log_to_file {
            Some(val) => env::set_var("LOG_TO_FILE", val),
            None => env::remove_var("LOG_TO_FILE"),
        }
        match orig_log_dir {
            Some(val) => env::set_var("LOG_DIR", val),
            None => env::remove_var("LOG_DIR"),
        }
        match orig_log_file_prefix {
            Some(val) => env::set_var("LOG_FILE_PREFIX", val),
            None => env::remove_var("LOG_FILE_PREFIX"),
        }
        match orig_log_include_location {
            Some(val) => env::set_var("LOG_INCLUDE_LOCATION", val),
            None => env::remove_var("LOG_INCLUDE_LOCATION"),
        }
    }

    #[test]
    #[serial_test::serial]
    fn test_config_with_invalid_boolean_env_vars() {
        // Save current env vars
        let original_json = env::var("LOG_JSON").ok();
        let original_file = env::var("LOG_TO_FILE").ok();
        let original_location = env::var("LOG_INCLUDE_LOCATION").ok();

        // Set invalid boolean values
        env::set_var("LOG_JSON", "invalid");
        env::set_var("LOG_TO_FILE", "not_a_bool");
        env::set_var("LOG_INCLUDE_LOCATION", "maybe");

        let config = LoggingConfig::default();

        // Should fall back to the unwrap_or() values when parsing fails
        assert!(!config.json_format); // parse("invalid") fails, falls back to false
        assert!(config.log_to_file); // parse("not_a_bool") fails, falls back to true (the unwrap_or value)
        assert!(!config.include_location); // parse("maybe") fails, falls back to false

        // Clean up
        env::remove_var("LOG_JSON");
        env::remove_var("LOG_TO_FILE");
        env::remove_var("LOG_INCLUDE_LOCATION");

        // Restore original env vars if they existed
        if let Some(val) = original_json {
            env::set_var("LOG_JSON", val);
        }
        if let Some(val) = original_file {
            env::set_var("LOG_TO_FILE", val);
        }
        if let Some(val) = original_location {
            env::set_var("LOG_INCLUDE_LOCATION", val);
        }
    }

    #[test]
    fn test_request_id_generation() {
        let id1 = create_request_id();
        let id2 = create_request_id();

        assert!(id1.starts_with("req_"));
        assert!(id2.starts_with("req_"));
        assert_ne!(id1, id2);

        // Verify format: req_{timestamp}_{counter}
        let parts: Vec<&str> = id1.split('_').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "req");
        assert!(parts[1].parse::<u128>().is_ok()); // timestamp
        assert!(parts[2].parse::<u32>().is_ok()); // counter
    }

    #[test]
    fn test_request_id_uniqueness() {
        let mut ids = std::collections::HashSet::new();

        // Generate multiple request IDs and ensure they're unique
        for _ in 0..100 {
            let id = create_request_id();
            assert!(ids.insert(id), "Request ID should be unique");
        }
    }

    #[test]
    fn test_init_default_logging() {
        // This should not panic
        let result = init_default_logging();
        // Either succeeds or fails with "already set" error
        assert!(result.is_ok() || result.unwrap_err().to_string().contains("already been set"));
    }

    #[test]
    fn test_init_logging_with_custom_config() {
        let config = LoggingConfig {
            level: "warn".to_string(),
            json_format: true,
            log_to_file: false,
            log_dir: "test_logs".to_string(),
            log_file_prefix: "test_prefix".to_string(),
            include_location: true,
        };

        // Since the global subscriber can only be set once,
        // we'll test that the function doesn't panic
        // The first test that runs will set the subscriber successfully
        let result = init_logging(config);
        // Either succeeds or fails with "already set" error
        assert!(result.is_ok() || result.unwrap_err().to_string().contains("already been set"));
    }

    #[test]
    fn test_init_logging_with_file_output() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path().to_str().unwrap().to_string();

        let config = LoggingConfig {
            level: "info".to_string(),
            json_format: false,
            log_to_file: true,
            log_dir: log_dir.clone(),
            log_file_prefix: "test".to_string(),
            include_location: false,
        };

        let result = init_logging(config);
        // Either succeeds or fails with "already set" error
        assert!(result.is_ok() || result.unwrap_err().to_string().contains("already been set"));

        // Verify the log directory was created
        assert!(std::path::Path::new(&log_dir).exists());
    }

    #[test]
    fn test_init_logging_invalid_log_level() {
        let config = LoggingConfig {
            level: "invalid_level".to_string(),
            json_format: false,
            log_to_file: false,
            log_dir: "logs".to_string(),
            log_file_prefix: "test".to_string(),
            include_location: false,
        };

        // Should still not panic and either succeed or fail with "already set"
        let result = init_logging(config);
        assert!(result.is_ok() || result.unwrap_err().to_string().contains("already been set"));
    }

    #[test]
    fn test_logging_config_clone() {
        let config = LoggingConfig::default();
        let cloned = config.clone();

        assert_eq!(config.level, cloned.level);
        assert_eq!(config.json_format, cloned.json_format);
        assert_eq!(config.log_to_file, cloned.log_to_file);
        assert_eq!(config.log_dir, cloned.log_dir);
        assert_eq!(config.log_file_prefix, cloned.log_file_prefix);
        assert_eq!(config.include_location, cloned.include_location);
    }

    #[test]
    fn test_logging_config_debug_format() {
        let config = LoggingConfig::default();
        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("LoggingConfig"));
        assert!(debug_str.contains("level"));
        assert!(debug_str.contains("json_format"));
    }

    #[test]
    fn test_create_log_directory_error_handling() {
        // Test with an invalid path (assuming this will fail on most systems)
        let config = LoggingConfig {
            level: "info".to_string(),
            json_format: false,
            log_to_file: true,
            log_dir: "/invalid/path/that/should/not/exist".to_string(),
            log_file_prefix: "test".to_string(),
            include_location: false,
        };

        let result = init_logging(config);
        // This should return an error due to inability to create the directory
        assert!(result.is_err());
    }

    #[test]
    fn test_different_log_levels() {
        let levels = vec!["trace", "debug", "info", "warn", "error"];

        for level in levels {
            let config = LoggingConfig {
                level: level.to_string(),
                json_format: false,
                log_to_file: false,
                log_dir: "logs".to_string(),
                log_file_prefix: "test".to_string(),
                include_location: false,
            };

            let result = init_logging(config);
            // Either succeeds or fails with "already set" error
            assert!(
                result.is_ok() || result.unwrap_err().to_string().contains("already been set"),
                "Failed to handle logging with level: {}",
                level
            );
        }
    }

    #[test]
    fn test_config_with_all_options_enabled() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path().to_str().unwrap().to_string();

        let config = LoggingConfig {
            level: "trace".to_string(),
            json_format: true,
            log_to_file: true,
            log_dir,
            log_file_prefix: "full_test".to_string(),
            include_location: true,
        };

        let result = init_logging(config);
        // Either succeeds or fails with "already set" error
        assert!(result.is_ok() || result.unwrap_err().to_string().contains("already been set"));
    }
}
