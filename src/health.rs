use axum::{extract::State, http::StatusCode, response::Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use tracing::{debug, info, instrument, warn};

use crate::constants::{MSG_HEALTH_CHECK_FAILED, MSG_SERVER_NOT_READY};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    pub last_checked: DateTime<Utc>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
    Unknown,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub timestamp: DateTime<Utc>,
    pub uptime_seconds: u64,
    pub checks: Vec<HealthCheck>,
}

#[derive(Serialize)]
pub struct LivenessResponse {
    pub status: &'static str,
    pub timestamp: DateTime<Utc>,
    pub uptime_seconds: u64,
}

#[derive(Serialize)]
pub struct ReadinessResponse {
    pub status: HealthStatus,
    pub timestamp: DateTime<Utc>,
    pub ready: bool,
    pub checks: Vec<HealthCheck>,
}

#[derive(Debug, Clone)]
pub struct AppState {
    startup_time: SystemTime,
    ready: Arc<RwLock<bool>>,
    health_checks: Arc<RwLock<Vec<HealthCheck>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            startup_time: SystemTime::now(),
            ready: Arc::new(RwLock::new(true)),
            health_checks: Arc::new(RwLock::new(vec![])), // Start with no health checks
        }
    }

    pub fn get_uptime_seconds(&self) -> u64 {
        self.startup_time.elapsed().unwrap_or_default().as_secs()
    }

    pub fn is_ready(&self) -> bool {
        *self.ready.read().unwrap()
    }

    /// Set the application readiness state
    /// This is part of the public health API and may be called from other modules
    #[allow(dead_code)] // Public API method
    pub fn set_ready(&self, ready: bool) {
        *self.ready.write().unwrap() = ready;
    }

    pub fn get_health_checks(&self) -> Vec<HealthCheck> {
        self.health_checks.read().unwrap().clone()
    }

    /// Update the status of an existing health check
    /// This is part of the public health API and is used by the background health checker
    pub fn update_health_check(&self, name: &str, status: HealthStatus, message: Option<String>) {
        let mut checks = self.health_checks.write().unwrap();
        if let Some(check) = checks.iter_mut().find(|c| c.name == name) {
            let old_status = check.status.clone();
            check.status = status.clone();
            check.last_checked = Utc::now();
            check.message = message.clone();

            // Log status changes
            match (&old_status, &status) {
                (HealthStatus::Healthy, HealthStatus::Unhealthy) => {
                    warn!(
                        "Health check '{}' changed from healthy to unhealthy: {:?}",
                        name, message
                    );
                }
                (HealthStatus::Unhealthy, HealthStatus::Healthy) => {
                    info!(
                        "Health check '{}' recovered from unhealthy to healthy: {:?}",
                        name, message
                    );
                }
                _ => {
                    debug!(
                        "Health check '{}' status updated: {:?} - {:?}",
                        name, status, message
                    );
                }
            }
        } else {
            warn!("Attempted to update non-existent health check: {}", name);
        }
    }

    /// Register a new health check
    /// This is part of the public health API and is used by the background health checker
    pub fn register_health_check(
        &self,
        name: String,
        status: HealthStatus,
        message: Option<String>,
    ) {
        let mut checks = self.health_checks.write().unwrap();
        // Check if health check already exists
        if !checks.iter().any(|c| c.name == name) {
            info!(
                "Registering new health check: {} with status: {:?}",
                name, status
            );
            checks.push(HealthCheck {
                name,
                status,
                last_checked: Utc::now(),
                message,
            });
        } else {
            warn!("Attempted to register duplicate health check: {}", name);
        }
    }

    /// Remove a health check
    /// This is part of the public health API and may be called from other modules
    #[allow(dead_code)] // Public API method
    pub fn unregister_health_check(&self, name: &str) {
        let mut checks = self.health_checks.write().unwrap();
        let initial_count = checks.len();
        checks.retain(|c| c.name != name);
        if checks.len() < initial_count {
            info!("Unregistered health check: {}", name);
        } else {
            warn!(
                "Attempted to unregister non-existent health check: {}",
                name
            );
        }
    }

    /// Run all registered health checks
    /// This method can be extended to actually execute health check functions
    /// For now, it just refreshes the timestamp of existing checks
    #[allow(dead_code)] // Public API method - can be called manually to trigger health checks
    pub async fn run_all_health_checks(&self) {
        let mut checks = self.health_checks.write().unwrap();
        for check in checks.iter_mut() {
            check.last_checked = Utc::now();
            // Future: Here you could call actual health check functions
            // based on the check name or type
        }
    }

    /// Initialize default health checks for the application
    pub fn initialize_default_health_checks(&self) {
        info!("Initializing default health checks...");

        // Register system health check
        self.register_health_check(
            "system".to_string(),
            HealthStatus::Healthy,
            Some("System is operational".to_string()),
        );

        // Register memory health check
        self.register_health_check(
            "memory".to_string(),
            HealthStatus::Healthy,
            Some("Memory usage within limits".to_string()),
        );
    }

    /// Perform a graceful shutdown by updating readiness state
    #[allow(dead_code)] // Public API method - called during application shutdown
    pub fn prepare_for_shutdown(&self) {
        info!("Preparing application for shutdown...");

        // Set application as not ready
        self.set_ready(false);

        // Update all health checks to reflect shutdown state
        let check_names: Vec<String> = self
            .get_health_checks()
            .into_iter()
            .map(|check| check.name)
            .collect();

        for name in check_names {
            self.update_health_check(
                &name,
                HealthStatus::Unhealthy,
                Some("Application shutting down".to_string()),
            );
        }
    }

    /// Remove all health checks (useful for testing or reset scenarios)
    #[allow(dead_code)] // Public API method - useful for testing and cleanup
    pub fn clear_all_health_checks(&self) {
        let check_names: Vec<String> = self
            .get_health_checks()
            .into_iter()
            .map(|check| check.name)
            .collect();

        for name in check_names {
            self.unregister_health_check(&name);
        }
    }
}

// Health endpoint - comprehensive health check including all dependencies
#[instrument(skip(state))]
pub async fn health_handler(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    debug!("Health endpoint accessed");
    let checks = state.get_health_checks();

    // If no health checks are registered, consider the application healthy
    let all_healthy = if checks.is_empty() {
        true // Application is running, no external dependencies to check
    } else {
        checks
            .iter()
            .all(|check| matches!(check.status, HealthStatus::Healthy))
    };

    let overall_status = if all_healthy {
        HealthStatus::Healthy
    } else {
        HealthStatus::Unhealthy
    };

    let status_code = if all_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let response = HealthResponse {
        status: overall_status.clone(),
        timestamp: Utc::now(),
        uptime_seconds: state.get_uptime_seconds(),
        checks,
    };

    match overall_status {
        HealthStatus::Healthy => debug!("Health check passed - all systems healthy"),
        HealthStatus::Unhealthy => warn!("{} - some systems unhealthy", MSG_HEALTH_CHECK_FAILED),
        HealthStatus::Unknown => warn!("Health check returned unknown status"),
    }

    (status_code, Json(response))
}

// Liveness endpoint - indicates if the application is running
#[instrument(skip(state))]
pub async fn liveness_handler(State(state): State<AppState>) -> Json<LivenessResponse> {
    debug!("Liveness endpoint accessed");
    Json(LivenessResponse {
        status: "alive",
        timestamp: Utc::now(),
        uptime_seconds: state.get_uptime_seconds(),
    })
}

// Readiness endpoint - indicates if the application is ready to serve traffic
#[instrument(skip(state))]
pub async fn readiness_handler(
    State(state): State<AppState>,
) -> (StatusCode, Json<ReadinessResponse>) {
    debug!("Readiness endpoint accessed");
    let checks = state.get_health_checks();

    // If no health checks are registered, rely only on the ready flag
    let checks_healthy = if checks.is_empty() {
        true
    } else {
        checks
            .iter()
            .all(|check| matches!(check.status, HealthStatus::Healthy))
    };

    let is_ready = state.is_ready() && checks_healthy;

    let status = if is_ready {
        HealthStatus::Healthy
    } else {
        HealthStatus::Unhealthy
    };

    let status_code = if is_ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let response = ReadinessResponse {
        status: status.clone(),
        timestamp: Utc::now(),
        ready: is_ready,
        checks,
    };

    match status {
        HealthStatus::Healthy => debug!("Readiness check passed - application ready"),
        HealthStatus::Unhealthy => warn!("Readiness check failed - {}", MSG_SERVER_NOT_READY),
        HealthStatus::Unknown => warn!("Readiness check returned unknown status"),
    }

    (status_code, Json(response))
}

/// Execute a single health check and update its status
fn execute_health_check(state: &AppState, name: &str) {
    // Simulate health check logic (in real implementation, this would check actual services)
    let is_healthy = match name {
        "system" => {
            // Simulate system health check - always healthy if AppState exists
            true
        }
        "memory" => {
            // Simulate memory check (always healthy for demo)
            true
        }
        _ => {
            // Default health check
            true
        }
    };

    let (status, message) = if is_healthy {
        (HealthStatus::Healthy, Some(format!("{name} check passed")))
    } else {
        (
            HealthStatus::Unhealthy,
            Some(format!("{name} check failed")),
        )
    };

    state.update_health_check(name, status, message);
}

/// Background task to periodically run health checks
#[instrument(skip(state))]
pub async fn health_check_background_task(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    info!("Background health check task started");

    loop {
        interval.tick().await;
        let checks_count = state.get_health_checks().len();

        if checks_count > 0 {
            info!("Running {} background health checks...", checks_count);

            // Simulate health checks and update their status
            let check_names: Vec<String> = state
                .get_health_checks()
                .into_iter()
                .map(|check| check.name)
                .collect();

            for name in check_names {
                execute_health_check(&state, &name);
            }

            debug!("Background health checks completed");
        } else {
            // Initialize default health checks if none exist
            debug!("No health checks found, initializing defaults");
            state.initialize_default_health_checks();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::http::StatusCode;
    use chrono::Utc;
    use std::time::Duration;
    use tokio::time;

    #[test]
    fn test_health_status_serialization() {
        // Test HealthStatus enum serialization/deserialization
        let healthy = HealthStatus::Healthy;
        let unhealthy = HealthStatus::Unhealthy;
        let unknown = HealthStatus::Unknown;

        // Test equality
        assert_eq!(healthy, HealthStatus::Healthy);
        assert_eq!(unhealthy, HealthStatus::Unhealthy);
        assert_eq!(unknown, HealthStatus::Unknown);

        // Test clone
        let cloned_healthy = healthy.clone();
        assert_eq!(healthy, cloned_healthy);
    }

    #[test]
    fn test_health_check_creation() {
        let check = HealthCheck {
            name: "test-check".to_string(),
            status: HealthStatus::Healthy,
            last_checked: Utc::now(),
            message: Some("Test message".to_string()),
        };

        assert_eq!(check.name, "test-check");
        assert_eq!(check.status, HealthStatus::Healthy);
        assert!(check.message.is_some());
        assert_eq!(check.message.unwrap(), "Test message");
    }

    #[test]
    fn test_app_state_new() {
        let state = AppState::new();

        // Test initial state
        assert!(state.is_ready());
        assert!(state.get_health_checks().is_empty());
        // Just check that uptime is a non-negative value
        let _uptime = state.get_uptime_seconds();
    }

    #[test]
    fn test_app_state_default() {
        let state = AppState::default();

        // Default should be same as new()
        assert!(state.is_ready());
        assert!(state.get_health_checks().is_empty());
    }

    #[test]
    fn test_app_state_set_ready() {
        let state = AppState::new();

        // Test setting ready to false
        state.set_ready(false);
        assert!(!state.is_ready());

        // Test setting ready back to true
        state.set_ready(true);
        assert!(state.is_ready());
    }

    #[test]
    fn test_register_health_check() {
        let state = AppState::new();

        // Register a new health check
        state.register_health_check(
            "database".to_string(),
            HealthStatus::Healthy,
            Some("Database connection is healthy".to_string()),
        );

        let checks = state.get_health_checks();
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name, "database");
        assert_eq!(checks[0].status, HealthStatus::Healthy);
        assert_eq!(
            checks[0].message,
            Some("Database connection is healthy".to_string())
        );
    }

    #[test]
    fn test_register_duplicate_health_check() {
        let state = AppState::new();

        // Register a health check
        state.register_health_check("duplicate".to_string(), HealthStatus::Healthy, None);

        // Try to register the same health check again
        state.register_health_check(
            "duplicate".to_string(),
            HealthStatus::Unhealthy,
            Some("Should not be added".to_string()),
        );

        let checks = state.get_health_checks();
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].status, HealthStatus::Healthy); // Should remain unchanged
        assert!(checks[0].message.is_none()); // Should remain unchanged
    }

    #[test]
    fn test_update_health_check() {
        let state = AppState::new();

        // First register a health check
        state.register_health_check(
            "test-service".to_string(),
            HealthStatus::Healthy,
            Some("Initial message".to_string()),
        );

        // Update the health check
        state.update_health_check(
            "test-service",
            HealthStatus::Unhealthy,
            Some("Service is down".to_string()),
        );

        let checks = state.get_health_checks();
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].status, HealthStatus::Unhealthy);
        assert_eq!(checks[0].message, Some("Service is down".to_string()));
    }

    #[test]
    fn test_update_nonexistent_health_check() {
        let state = AppState::new();

        // Try to update a non-existent health check
        state.update_health_check(
            "nonexistent",
            HealthStatus::Unhealthy,
            Some("Should not work".to_string()),
        );

        // Should still have no health checks
        assert!(state.get_health_checks().is_empty());
    }

    #[test]
    fn test_unregister_health_check() {
        let state = AppState::new();

        // Register multiple health checks
        state.register_health_check("service1".to_string(), HealthStatus::Healthy, None);
        state.register_health_check("service2".to_string(), HealthStatus::Healthy, None);

        assert_eq!(state.get_health_checks().len(), 2);

        // Unregister one
        state.unregister_health_check("service1");

        let checks = state.get_health_checks();
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name, "service2");
    }

    #[test]
    fn test_unregister_nonexistent_health_check() {
        let state = AppState::new();

        // Try to unregister a non-existent health check
        state.unregister_health_check("nonexistent");

        // Should still have no health checks
        assert!(state.get_health_checks().is_empty());
    }

    #[test]
    fn test_clear_all_health_checks() {
        let state = AppState::new();

        // Register multiple health checks
        state.register_health_check("service1".to_string(), HealthStatus::Healthy, None);
        state.register_health_check("service2".to_string(), HealthStatus::Healthy, None);
        state.register_health_check("service3".to_string(), HealthStatus::Healthy, None);

        assert_eq!(state.get_health_checks().len(), 3);

        // Clear all health checks
        state.clear_all_health_checks();

        assert!(state.get_health_checks().is_empty());
    }

    #[test]
    fn test_initialize_default_health_checks() {
        let state = AppState::new();

        // Initialize default health checks
        state.initialize_default_health_checks();

        let checks = state.get_health_checks();
        assert_eq!(checks.len(), 2);

        // Check that system and memory checks are registered
        let check_names: Vec<String> = checks.iter().map(|c| c.name.clone()).collect();
        assert!(check_names.contains(&"system".to_string()));
        assert!(check_names.contains(&"memory".to_string()));

        // All should be healthy initially
        for check in checks {
            assert_eq!(check.status, HealthStatus::Healthy);
        }
    }

    #[test]
    fn test_prepare_for_shutdown() {
        let state = AppState::new();

        // Add some health checks
        state.initialize_default_health_checks();
        assert!(state.is_ready());

        // Prepare for shutdown
        state.prepare_for_shutdown();

        // Should no longer be ready
        assert!(!state.is_ready());

        // All health checks should be unhealthy
        let checks = state.get_health_checks();
        for check in checks {
            assert_eq!(check.status, HealthStatus::Unhealthy);
            assert_eq!(check.message, Some("Application shutting down".to_string()));
        }
    }

    #[tokio::test]
    async fn test_run_all_health_checks() {
        let state = AppState::new();

        // Add some health checks
        state.register_health_check(
            "test1".to_string(),
            HealthStatus::Healthy,
            Some("Original message".to_string()),
        );

        let original_time = state.get_health_checks()[0].last_checked;

        // Wait a tiny bit to ensure time difference
        time::sleep(Duration::from_millis(1)).await;

        // Run all health checks
        state.run_all_health_checks().await;

        // Check that last_checked was updated
        let updated_time = state.get_health_checks()[0].last_checked;
        assert!(updated_time > original_time);
    }

    #[tokio::test]
    async fn test_health_handler_no_checks() {
        let state = AppState::new();

        let (status, response) = health_handler(State(state)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(response.status, HealthStatus::Healthy);
        assert!(response.checks.is_empty());
        // Just verify uptime is a valid u64 value
        let _uptime = response.uptime_seconds;
    }

    #[tokio::test]
    async fn test_health_handler_with_healthy_checks() {
        let state = AppState::new();

        // Add healthy checks
        state.register_health_check(
            "service1".to_string(),
            HealthStatus::Healthy,
            Some("All good".to_string()),
        );
        state.register_health_check(
            "service2".to_string(),
            HealthStatus::Healthy,
            Some("Working fine".to_string()),
        );

        let (status, response) = health_handler(State(state)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(response.status, HealthStatus::Healthy);
        assert_eq!(response.checks.len(), 2);
    }

    #[tokio::test]
    async fn test_health_handler_with_unhealthy_checks() {
        let state = AppState::new();

        // Add mixed health checks
        state.register_health_check("service1".to_string(), HealthStatus::Healthy, None);
        state.register_health_check(
            "service2".to_string(),
            HealthStatus::Unhealthy,
            Some("Service down".to_string()),
        );

        let (status, response) = health_handler(State(state)).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.status, HealthStatus::Unhealthy);
        assert_eq!(response.checks.len(), 2);
    }

    #[tokio::test]
    async fn test_liveness_handler() {
        let state = AppState::new();

        let response = liveness_handler(State(state)).await;

        assert_eq!(response.status, "alive");
        // Just verify uptime is a valid u64 value
        let _uptime = response.uptime_seconds;
    }

    #[tokio::test]
    async fn test_readiness_handler_ready() {
        let state = AppState::new();

        // Add healthy checks
        state.register_health_check("service".to_string(), HealthStatus::Healthy, None);

        let (status, response) = readiness_handler(State(state)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(response.status, HealthStatus::Healthy);
        assert!(response.ready);
    }

    #[tokio::test]
    async fn test_readiness_handler_not_ready_due_to_flag() {
        let state = AppState::new();

        // Set not ready
        state.set_ready(false);

        let (status, response) = readiness_handler(State(state)).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.status, HealthStatus::Unhealthy);
        assert!(!response.ready);
    }

    #[tokio::test]
    async fn test_readiness_handler_not_ready_due_to_unhealthy_checks() {
        let state = AppState::new();

        // Add unhealthy check
        state.register_health_check(
            "failing-service".to_string(),
            HealthStatus::Unhealthy,
            Some("Service is down".to_string()),
        );

        let (status, response) = readiness_handler(State(state)).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.status, HealthStatus::Unhealthy);
        assert!(!response.ready);
    }

    #[tokio::test]
    async fn test_readiness_handler_no_checks() {
        let state = AppState::new();

        let (status, response) = readiness_handler(State(state)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(response.status, HealthStatus::Healthy);
        assert!(response.ready);
        assert!(response.checks.is_empty());
    }

    #[test]
    fn test_execute_health_check_system() {
        let state = AppState::new();

        // Register system health check
        state.register_health_check("system".to_string(), HealthStatus::Unknown, None);

        // Execute the system health check
        super::execute_health_check(&state, "system");

        let checks = state.get_health_checks();
        assert_eq!(checks[0].status, HealthStatus::Healthy);
        assert!(checks[0]
            .message
            .as_ref()
            .unwrap()
            .contains("system check passed"));
    }

    #[test]
    fn test_execute_health_check_memory() {
        let state = AppState::new();

        // Register memory health check
        state.register_health_check("memory".to_string(), HealthStatus::Unknown, None);

        // Execute the memory health check
        super::execute_health_check(&state, "memory");

        let checks = state.get_health_checks();
        assert_eq!(checks[0].status, HealthStatus::Healthy);
        assert!(checks[0]
            .message
            .as_ref()
            .unwrap()
            .contains("memory check passed"));
    }

    #[test]
    fn test_execute_health_check_default() {
        let state = AppState::new();

        // Register a custom health check
        state.register_health_check("custom-service".to_string(), HealthStatus::Unknown, None);

        // Execute the custom health check (should use default logic)
        super::execute_health_check(&state, "custom-service");

        let checks = state.get_health_checks();
        assert_eq!(checks[0].status, HealthStatus::Healthy);
        assert!(checks[0]
            .message
            .as_ref()
            .unwrap()
            .contains("custom-service check passed"));
    }

    #[test]
    fn test_uptime_tracking() {
        let state = AppState::new();

        let uptime1 = state.get_uptime_seconds();
        std::thread::sleep(Duration::from_millis(10)); // Small delay
        let uptime2 = state.get_uptime_seconds();

        assert!(uptime2 >= uptime1);
    }

    #[test]
    fn test_app_state_clone() {
        let state = AppState::new();

        // Add some data
        state.set_ready(false);
        state.register_health_check(
            "test".to_string(),
            HealthStatus::Healthy,
            Some("Test message".to_string()),
        );

        // Clone the state
        let cloned_state = state.clone();

        // Verify the clone has the same data
        assert_eq!(state.is_ready(), cloned_state.is_ready());
        assert_eq!(
            state.get_health_checks().len(),
            cloned_state.get_health_checks().len()
        );
        assert_eq!(
            state.get_health_checks()[0].name,
            cloned_state.get_health_checks()[0].name
        );
    }

    #[test]
    fn test_health_check_status_transitions() {
        let state = AppState::new();

        // Register a health check
        state.register_health_check(
            "transition-test".to_string(),
            HealthStatus::Healthy,
            Some("Initially healthy".to_string()),
        );

        // Test Healthy -> Unhealthy transition
        state.update_health_check(
            "transition-test",
            HealthStatus::Unhealthy,
            Some("Now unhealthy".to_string()),
        );

        let checks = state.get_health_checks();
        assert_eq!(checks[0].status, HealthStatus::Unhealthy);

        // Test Unhealthy -> Healthy transition (recovery)
        state.update_health_check(
            "transition-test",
            HealthStatus::Healthy,
            Some("Recovered".to_string()),
        );

        let checks = state.get_health_checks();
        assert_eq!(checks[0].status, HealthStatus::Healthy);

        // Test other status transitions
        state.update_health_check(
            "transition-test",
            HealthStatus::Unknown,
            Some("Status unknown".to_string()),
        );

        let checks = state.get_health_checks();
        assert_eq!(checks[0].status, HealthStatus::Unknown);
    }

    #[test]
    fn test_health_response_serialization() {
        let response = HealthResponse {
            status: HealthStatus::Healthy,
            timestamp: Utc::now(),
            uptime_seconds: 123,
            checks: vec![HealthCheck {
                name: "test".to_string(),
                status: HealthStatus::Healthy,
                last_checked: Utc::now(),
                message: Some("Test message".to_string()),
            }],
        };

        // Test that we can serialize the response
        let json = serde_json::to_string(&response);
        assert!(json.is_ok());

        let json_str = json.unwrap();
        assert!(json_str.contains("healthy"));
        assert!(json_str.contains("test"));
        assert!(json_str.contains("123"));
    }

    #[test]
    fn test_liveness_response_serialization() {
        let response = LivenessResponse {
            status: "alive",
            timestamp: Utc::now(),
            uptime_seconds: 456,
        };

        // Test that we can serialize the response
        let json = serde_json::to_string(&response);
        assert!(json.is_ok());

        let json_str = json.unwrap();
        assert!(json_str.contains("alive"));
        assert!(json_str.contains("456"));
    }

    #[test]
    fn test_readiness_response_serialization() {
        let response = ReadinessResponse {
            status: HealthStatus::Healthy,
            timestamp: Utc::now(),
            ready: true,
            checks: vec![],
        };

        // Test that we can serialize the response
        let json = serde_json::to_string(&response);
        assert!(json.is_ok());

        let json_str = json.unwrap();
        assert!(json_str.contains("healthy"));
        assert!(json_str.contains("true"));
    }

    // Integration test for the background health check task
    #[tokio::test]
    async fn test_health_check_background_task_initialization() {
        let state = AppState::new();

        // Spawn the background task for a short time
        let state_clone = state.clone();
        let handle = tokio::spawn(async move {
            // Run for a very short time
            tokio::select! {
                _ = super::health_check_background_task(state_clone) => {},
                _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            }
        });

        // Wait for the task to initialize default checks
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Cancel the background task
        handle.abort();

        // Check that default health checks were initialized
        let checks = state.get_health_checks();
        assert!(!checks.is_empty());

        let check_names: Vec<String> = checks.iter().map(|c| c.name.clone()).collect();
        assert!(check_names.contains(&"system".to_string()));
        assert!(check_names.contains(&"memory".to_string()));
    }

    #[test]
    fn test_multiple_health_check_updates() {
        let state = AppState::new();

        // Register multiple health checks
        let check_names = vec!["service1", "service2", "service3"];
        for name in &check_names {
            state.register_health_check(
                name.to_string(),
                HealthStatus::Healthy,
                Some(format!("{name} is healthy")),
            );
        }

        assert_eq!(state.get_health_checks().len(), 3);

        // Update all to unhealthy
        for name in &check_names {
            state.update_health_check(
                name,
                HealthStatus::Unhealthy,
                Some(format!("{name} is down")),
            );
        }

        let checks = state.get_health_checks();
        for check in checks {
            assert_eq!(check.status, HealthStatus::Unhealthy);
            assert!(check.message.as_ref().unwrap().contains("is down"));
        }
    }

    #[test]
    fn test_health_check_message_updates() {
        let state = AppState::new();

        // Register a health check without a message
        state.register_health_check("message-test".to_string(), HealthStatus::Healthy, None);

        assert!(state.get_health_checks()[0].message.is_none());

        // Update with a message
        state.update_health_check(
            "message-test",
            HealthStatus::Healthy,
            Some("Now with message".to_string()),
        );

        assert_eq!(
            state.get_health_checks()[0].message,
            Some("Now with message".to_string())
        );

        // Update to remove message
        state.update_health_check("message-test", HealthStatus::Healthy, None);

        assert!(state.get_health_checks()[0].message.is_none());
    }
}
