use std::env;
use tracing::info;
use tracing_subscriber::{fmt::time::UtcTime, EnvFilter};
use tracing_appender::{non_blocking, rolling};

use crate::constants::*;
use crate::error::{FerragateError, FerragateResult};

/// Configuration for the Ferragate logging system
/// 
/// Provides comprehensive logging configuration including level control,
/// output formatting, and file logging options.
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
            level: env::var("RUST_LOG").unwrap_or_else(|_| DEFAULT_LOG_LEVEL.to_string()),
            json_format: parse_env_bool("LOG_JSON", false),
            log_to_file: parse_env_bool("LOG_TO_FILE", true),
            log_dir: env::var("LOG_DIR").unwrap_or_else(|_| DEFAULT_LOG_DIR.to_string()),
            log_file_prefix: env::var("LOG_FILE_PREFIX")
                .unwrap_or_else(|_| DEFAULT_LOG_FILE_PREFIX.to_string()),
            include_location: parse_env_bool("LOG_INCLUDE_LOCATION", false),
        }
    }
}

/// Parse a boolean environment variable with a default value
fn parse_env_bool(var_name: &str, default: bool) -> bool {
    env::var(var_name)
        .unwrap_or_else(|_| default.to_string())
        .parse()
        .unwrap_or(default)
}

/// Initialize the logging system with the given configuration
/// 
/// Sets up tracing subscriber with the specified level, format, and output options.
/// Handles cases where a global subscriber is already initialized (common in tests).
pub fn init_logging(config: LoggingConfig) -> FerragateResult<()> {
    // Create the log directory if it doesn't exist and file logging is enabled
    if config.log_to_file {
        std::fs::create_dir_all(&config.log_dir)
            .map_err(|e| FerragateError::io(format!("Failed to create log directory '{}': {}", config.log_dir, e)))?;
    }

    // Create environment filter from configuration
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&config.level))
        .unwrap_or_else(|_| {
            eprintln!("Warning: Invalid log level '{}', using 'info'", config.level);
            EnvFilter::new("info")
        });

    // Set up the subscriber based on whether file logging is enabled
    let result = if config.log_to_file {
        // Create file appender with daily rotation using the prefix
        let file_appender = rolling::daily(&config.log_dir, &config.log_file_prefix);
        let (non_blocking_appender, _guard) = non_blocking(file_appender);
        
        // We need to keep the guard alive for the lifetime of the program
        // In a real application, you'd want to store this somewhere
        std::mem::forget(_guard);
        
        if config.json_format {
            // For JSON format, we can only do file OR console due to type constraints
            // Prioritize file logging for JSON format
            tracing_subscriber::fmt()
                .with_timer(UtcTime::rfc_3339())
                .with_target(true)
                .with_file(config.include_location)
                .with_line_number(config.include_location)
                .with_writer(non_blocking_appender)
                .with_ansi(false) // No ANSI in files
                .with_env_filter(env_filter)
                .json()
                .try_init()
        } else {
            // For non-JSON format, we can use a tee writer to write to both
            use tracing_subscriber::fmt::writer::MakeWriterExt;
            
            // Create a tee writer that writes to both stdout and file
            let tee_writer = non_blocking_appender.and(std::io::stdout);
            
            tracing_subscriber::fmt()
                .with_timer(UtcTime::rfc_3339())
                .with_target(true)
                .with_file(config.include_location)
                .with_line_number(config.include_location)
                .with_writer(tee_writer)
                .with_ansi(false) // Disable ANSI to keep files clean (console will lose colors as tradeoff)
                .with_env_filter(env_filter)
                .try_init()
        }
    } else {
        // Console-only logging with ANSI colors
        let builder = tracing_subscriber::fmt()
            .with_timer(UtcTime::rfc_3339())
            .with_target(true)
            .with_file(config.include_location)
            .with_line_number(config.include_location)
            .with_ansi(true) // Enable ANSI colors for console
            .with_env_filter(env_filter);

        if config.json_format {
            builder.json().try_init()
        } else {
            builder.try_init()
        }
    };

    // Handle initialization result
    match result {
        Ok(()) => {
            // Log successful initialization
            if config.log_to_file {
                info!(
                    "Logging initialized: level={}, json={}, file_dir={}, file_prefix={}",
                    config.level, config.json_format, config.log_dir, config.log_file_prefix
                );
            } else {
                info!(
                    "Logging initialized: level={}, json={}",
                    config.level, config.json_format
                );
            }
        }
        Err(e) => {
            if e.to_string().contains("already been set") {
                // Subscriber already set, which is okay for tests
                eprintln!("Warning: Global subscriber already set: {}", e);
            } else {
                return Err(FerragateError::config(format!("Failed to initialize logging: {}", e)));
            }
        }
    }

    Ok(())
}

/// Initialize logging with default configuration
/// 
/// Convenience function that creates a default logging configuration and initializes the system.
/// This is the function used by the main application for simple setup.
pub fn init_default_logging() -> FerragateResult<()> {
    let config = LoggingConfig::default();
    init_logging(config)
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

    #[test]
    fn test_file_logging_with_prefix() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path().to_str().unwrap().to_string();
        let custom_prefix = "test_ferragate";

        let config = LoggingConfig {
            level: "info".to_string(),
            json_format: false,
            log_to_file: true,
            log_dir: log_dir.clone(),
            log_file_prefix: custom_prefix.to_string(),
            include_location: false,
        };

        let result = init_logging(config);
        // Either succeeds or fails with "already set" error
        assert!(result.is_ok() || result.unwrap_err().to_string().contains("already been set"));

        // Verify the log directory was created
        assert!(std::path::Path::new(&log_dir).exists());

        // Note: The actual log file creation happens when logs are written
        // and uses the format {prefix}.{date} (e.g., test_ferragate.2023-12-01)
        // Since we can't predict the exact filename without writing logs,
        // we just verify the directory exists and the config is properly used
    }
}
