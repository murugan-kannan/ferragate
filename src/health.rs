use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use tracing::{info, warn, debug, instrument};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    pub last_checked: DateTime<Utc>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl AppState {
    pub fn new() -> Self {
        Self {
            startup_time: SystemTime::now(),
            ready: Arc::new(RwLock::new(true)),
            health_checks: Arc::new(RwLock::new(vec![])), // Start with no health checks
        }
    }

    pub fn get_uptime_seconds(&self) -> u64 {
        self.startup_time
            .elapsed()
            .unwrap_or_default()
            .as_secs()
    }

    pub fn is_ready(&self) -> bool {
        *self.ready.read().unwrap()
    }

    pub fn set_ready(&self, ready: bool) {
        *self.ready.write().unwrap() = ready;
    }

    pub fn get_health_checks(&self) -> Vec<HealthCheck> {
        self.health_checks.read().unwrap().clone()
    }

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
                    warn!("Health check '{}' changed from healthy to unhealthy: {:?}", name, message);
                }
                (HealthStatus::Unhealthy, HealthStatus::Healthy) => {
                    info!("Health check '{}' recovered from unhealthy to healthy: {:?}", name, message);
                }
                _ => {
                    debug!("Health check '{}' status updated: {:?} - {:?}", name, status, message);
                }
            }
        } else {
            warn!("Attempted to update non-existent health check: {}", name);
        }
    }

    /// Register a new health check
    pub fn register_health_check(&self, name: String, status: HealthStatus, message: Option<String>) {
        let mut checks = self.health_checks.write().unwrap();
        // Check if health check already exists
        if !checks.iter().any(|c| c.name == name) {
            info!("Registering new health check: {} with status: {:?}", name, status);
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
    pub fn unregister_health_check(&self, name: &str) {
        let mut checks = self.health_checks.write().unwrap();
        let initial_count = checks.len();
        checks.retain(|c| c.name != name);
        if checks.len() < initial_count {
            info!("Unregistered health check: {}", name);
        } else {
            warn!("Attempted to unregister non-existent health check: {}", name);
        }
    }

    /// Run all registered health checks
    /// This method can be extended to actually execute health check functions
    /// For now, it just refreshes the timestamp of existing checks
    pub async fn run_all_health_checks(&self) {
        let mut checks = self.health_checks.write().unwrap();
        for check in checks.iter_mut() {
            check.last_checked = Utc::now();
            // Future: Here you could call actual health check functions
            // based on the check name or type
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
        checks.iter().all(|check| matches!(check.status, HealthStatus::Healthy))
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
        HealthStatus::Unhealthy => warn!("Health check failed - some systems unhealthy"),
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
pub async fn readiness_handler(State(state): State<AppState>) -> (StatusCode, Json<ReadinessResponse>) {
    debug!("Readiness endpoint accessed");
    let checks = state.get_health_checks();
    
    // If no health checks are registered, rely only on the ready flag
    let checks_healthy = if checks.is_empty() {
        true
    } else {
        checks.iter().all(|check| matches!(check.status, HealthStatus::Healthy))
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
        HealthStatus::Unhealthy => warn!("Readiness check failed - application not ready"),
        HealthStatus::Unknown => warn!("Readiness check returned unknown status"),
    }

    (status_code, Json(response))
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
            state.run_all_health_checks().await;
            debug!("Background health checks completed");
        } else {
            // No health checks registered, just log that we're still alive
            debug!("Application health check - no external dependencies to check");
        }
    }
}

// Health check trait for extensibility
pub trait HealthChecker: Send + Sync {
    async fn check(&self) -> HealthCheck;
}

// Example implementations for future use:
// Uncomment and implement these when you have the actual services

/*
pub struct DatabaseHealthChecker {
    // Add your database connection pool here
}

impl HealthChecker for DatabaseHealthChecker {
    async fn check(&self) -> HealthCheck {
        // Implement actual database health check
        HealthCheck {
            name: "database".to_string(),
            status: HealthStatus::Healthy,
            last_checked: Utc::now(),
            message: Some("Database connection successful".to_string()),
        }
    }
}

pub struct RedisHealthChecker {
    // Add your Redis connection here
}

impl HealthChecker for RedisHealthChecker {
    async fn check(&self) -> HealthCheck {
        // Implement actual Redis health check
        HealthCheck {
            name: "redis".to_string(),
            status: HealthStatus::Healthy,
            last_checked: Utc::now(),
            message: Some("Redis cache operational".to_string()),
        }
    }
}

pub struct ExternalApiHealthChecker {
    pub api_url: String,
}

impl HealthChecker for ExternalApiHealthChecker {
    async fn check(&self) -> HealthCheck {
        // Implement actual external API health check
        HealthCheck {
            name: "external_api".to_string(),
            status: HealthStatus::Healthy,
            last_checked: Utc::now(),
            message: Some("External API responding".to_string()),
        }
    }
}
*/
