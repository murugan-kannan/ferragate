use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri},
    response::IntoResponse,
};
use http_body_util::BodyExt;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, instrument, warn};

use crate::config::{GatewayConfig, RouteConfig};

#[derive(Clone)]
pub struct ProxyState {
    pub config: Arc<GatewayConfig>,
    pub client: reqwest::Client,
}

impl ProxyState {
    pub fn new(config: GatewayConfig) -> Self {
        let timeout = Duration::from_millis(config.server.timeout_ms.unwrap_or(30000));

        let client = reqwest::Client::builder()
            .timeout(timeout)
            .pool_idle_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(10)
            .user_agent("FerraGate/0.1.0")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config: Arc::new(config),
            client,
        }
    }

    pub fn find_matching_route(&self, path: &str, method: &str) -> Option<&RouteConfig> {
        self.config
            .routes
            .iter()
            .find(|route| route.matches_path(path) && route.matches_method(method))
    }
}

#[instrument(skip(state, body), fields(method = %method, uri = %uri))]
pub async fn proxy_handler(
    State(state): State<ProxyState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Body,
) -> impl IntoResponse {
    let path = uri.path();
    let query = uri.query().unwrap_or("");

    debug!("Incoming request: {} {}", method, uri);

    // Find matching route
    let route = match state.find_matching_route(path, method.as_str()) {
        Some(route) => {
            debug!("Matched route: {} -> {}", route.path, route.upstream);
            route
        }
        None => {
            warn!("No matching route found for: {} {}", method, path);
            return (StatusCode::NOT_FOUND, "No matching route found").into_response();
        }
    };

    // Transform the path if needed
    let target_path = route.transform_path(path);

    // Build target URL
    let mut target_url = format!("{}{}", route.upstream, target_path);
    if !query.is_empty() {
        target_url.push('?');
        target_url.push_str(query);
    }

    debug!("Proxying to: {}", target_url);

    // Convert body to bytes
    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            error!("Failed to read request body: {}", e);
            return (StatusCode::BAD_REQUEST, "Failed to read request body").into_response();
        }
    };

    // Build the proxied request
    let reqwest_method = match method.as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "DELETE" => reqwest::Method::DELETE,
        "HEAD" => reqwest::Method::HEAD,
        "OPTIONS" => reqwest::Method::OPTIONS,
        "PATCH" => reqwest::Method::PATCH,
        _ => {
            warn!("Unsupported HTTP method: {}", method);
            return (StatusCode::METHOD_NOT_ALLOWED, "Method not allowed").into_response();
        }
    };

    let mut request_builder = state
        .client
        .request(reqwest_method, &target_url)
        .body(body_bytes);

    // Copy headers from the original request
    for (name, value) in headers.iter() {
        // Skip certain headers that shouldn't be forwarded
        let header_name = name.as_str().to_lowercase();
        if should_forward_header(&header_name) {
            if let Ok(header_value) = value.to_str() {
                request_builder = request_builder.header(name.as_str(), header_value);
            }
        }
    }

    // Add custom headers from route configuration
    for (name, value) in &route.headers {
        request_builder = request_builder.header(name, value);
    }

    // Preserve or modify the Host header
    if !route.preserve_host {
        if let Ok(target_url_parsed) = url::Url::parse(&target_url) {
            if let Some(host) = target_url_parsed.host_str() {
                let host_header = if let Some(port) = target_url_parsed.port() {
                    format!("{}:{}", host, port)
                } else {
                    host.to_string()
                };
                request_builder = request_builder.header("host", host_header);
            }
        }
    }

    // Apply route-specific timeout if configured
    if let Some(timeout_ms) = route.timeout_ms {
        request_builder = request_builder.timeout(Duration::from_millis(timeout_ms));
    }

    // Execute the request
    let response = match request_builder.send().await {
        Ok(response) => response,
        Err(e) => {
            error!("Failed to proxy request to {}: {}", target_url, e);
            return (
                StatusCode::BAD_GATEWAY,
                format!("Failed to proxy request: {}", e),
            )
                .into_response();
        }
    };

    debug!("Upstream response status: {}", response.status());

    // Build the response
    let status = StatusCode::from_u16(response.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    let mut response_headers = HeaderMap::new();

    // Copy response headers
    for (name, value) in response.headers() {
        let header_name = name.as_str().to_lowercase();
        if should_forward_response_header(&header_name) {
            if let (Ok(name), Ok(value)) = (
                HeaderName::try_from(name.as_str()),
                HeaderValue::try_from(value.as_bytes()),
            ) {
                response_headers.insert(name, value);
            }
        }
    }

    // Get response body
    let response_body = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to read response body: {}", e);
            return (StatusCode::BAD_GATEWAY, "Failed to read response body").into_response();
        }
    };

    debug!(
        "Successfully proxied request, response size: {} bytes",
        response_body.len()
    );

    // Return the response
    (status, response_headers, response_body).into_response()
}

fn should_forward_header(header_name: &str) -> bool {
    match header_name {
        // Don't forward connection-specific headers
        "connection"
        | "upgrade"
        | "proxy-authenticate"
        | "proxy-authorization"
        | "te"
        | "trailers"
        | "transfer-encoding" => false,
        // Don't forward hop-by-hop headers
        "keep-alive" | "proxy-connection" => false,
        // Forward everything else
        _ => true,
    }
}

fn should_forward_response_header(header_name: &str) -> bool {
    match header_name {
        // Don't forward connection-specific headers
        "connection"
        | "upgrade"
        | "proxy-authenticate"
        | "proxy-authorization"
        | "te"
        | "trailers"
        | "transfer-encoding" => false,
        // Don't forward hop-by-hop headers
        "keep-alive" | "proxy-connection" => false,
        // Forward everything else
        _ => true,
    }
}

pub async fn handle_not_found() -> impl IntoResponse {
    debug!("404 Not Found response");
    (StatusCode::NOT_FOUND, "Route not found")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GatewayConfig, RouteConfig, ServerConfig};
    use std::collections::HashMap;

    // Helper function to create a test configuration
    fn create_test_config() -> GatewayConfig {
        GatewayConfig {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                workers: None,
                timeout_ms: Some(5000),
                tls: None,
            },
            routes: vec![
                RouteConfig {
                    path: "/api/v1/*".to_string(),
                    upstream: "http://backend1:3000".to_string(),
                    methods: vec!["GET".to_string(), "POST".to_string()],
                    headers: HashMap::new(),
                    strip_path: true,
                    preserve_host: false,
                    timeout_ms: Some(30000),
                },
                RouteConfig {
                    path: "/users/*".to_string(),
                    upstream: "http://user-service:8000".to_string(),
                    methods: vec![],
                    headers: HashMap::new(),
                    strip_path: false,
                    preserve_host: true,
                    timeout_ms: None,
                },
                RouteConfig {
                    path: "/health".to_string(),
                    upstream: "http://health-service:9000".to_string(),
                    methods: vec!["GET".to_string()],
                    headers: HashMap::new(),
                    strip_path: false,
                    preserve_host: false,
                    timeout_ms: None,
                },
            ],
            logging: crate::config::LoggingConfig::default(),
        }
    }

    #[test]
    fn test_proxy_state_creation() {
        let config = create_test_config();
        let proxy_state = ProxyState::new(config.clone());

        // Verify the proxy state was created correctly
        assert_eq!(proxy_state.config.server.port, 8080);
        assert_eq!(proxy_state.config.routes.len(), 3);

        // The client should be created
        // We can't easily test the client configuration directly,
        // but we can verify it exists
    }

    #[test]
    fn test_find_matching_route_exact_match() {
        let config = create_test_config();
        let proxy_state = ProxyState::new(config);

        // Test exact path match
        let route = proxy_state.find_matching_route("/health", "GET");
        assert!(route.is_some());
        assert_eq!(route.unwrap().path, "/health");
    }

    #[test]
    fn test_find_matching_route_wildcard_match() {
        let config = create_test_config();
        let proxy_state = ProxyState::new(config);

        // Test wildcard path match
        let route = proxy_state.find_matching_route("/api/v1/test", "GET");
        assert!(route.is_some());
        assert_eq!(route.unwrap().path, "/api/v1/*");
    }

    #[test]
    fn test_find_matching_route_method_filtering() {
        let config = create_test_config();
        let proxy_state = ProxyState::new(config);

        // Test method filtering for exact route
        let route_get = proxy_state.find_matching_route("/health", "GET");
        assert!(route_get.is_some());

        let route_post = proxy_state.find_matching_route("/health", "POST");
        assert!(route_post.is_none()); // POST not allowed for /health
    }

    #[test]
    fn test_find_matching_route_empty_methods_allows_all() {
        let config = create_test_config();
        let proxy_state = ProxyState::new(config);

        // /users/* has empty methods list, should allow all methods
        let route_get = proxy_state.find_matching_route("/users/123", "GET");
        assert!(route_get.is_some());

        let route_post = proxy_state.find_matching_route("/users/123", "POST");
        assert!(route_post.is_some());

        let route_delete = proxy_state.find_matching_route("/users/123", "DELETE");
        assert!(route_delete.is_some());
    }

    #[test]
    fn test_find_matching_route_no_match() {
        let config = create_test_config();
        let proxy_state = ProxyState::new(config);

        // Test path that doesn't match any route
        let route = proxy_state.find_matching_route("/nonexistent", "GET");
        assert!(route.is_none());
    }

    #[test]
    fn test_find_matching_route_case_sensitivity() {
        let config = create_test_config();
        let proxy_state = ProxyState::new(config);

        // Test case insensitivity for methods (this should work based on the config implementation)
        let route_lower = proxy_state.find_matching_route("/health", "get");
        assert!(
            route_lower.is_some(),
            "Method matching should be case insensitive"
        );

        let route_upper = proxy_state.find_matching_route("/health", "GET");
        assert!(route_upper.is_some());
    }

    #[test]
    fn test_header_filtering() {
        // Test headers that should NOT be forwarded
        assert!(!should_forward_header("connection"));
        assert!(!should_forward_header("upgrade"));
        assert!(!should_forward_header("proxy-authenticate"));
        assert!(!should_forward_header("proxy-authorization"));
        assert!(!should_forward_header("te"));
        assert!(!should_forward_header("trailers"));
        assert!(!should_forward_header("transfer-encoding"));
        assert!(!should_forward_header("keep-alive"));
        assert!(!should_forward_header("proxy-connection"));

        // Test headers that SHOULD be forwarded
        assert!(should_forward_header("authorization"));
        assert!(should_forward_header("content-type"));
        assert!(should_forward_header("user-agent"));
        assert!(should_forward_header("accept"));
        assert!(should_forward_header("cache-control"));
        assert!(should_forward_header("custom-header"));
    }

    #[test]
    fn test_response_header_filtering() {
        // Test headers that should NOT be forwarded
        assert!(!should_forward_response_header("connection"));
        assert!(!should_forward_response_header("upgrade"));
        assert!(!should_forward_response_header("proxy-authenticate"));
        assert!(!should_forward_response_header("proxy-authorization"));
        assert!(!should_forward_response_header("te"));
        assert!(!should_forward_response_header("trailers"));
        assert!(!should_forward_response_header("transfer-encoding"));
        assert!(!should_forward_response_header("keep-alive"));
        assert!(!should_forward_response_header("proxy-connection"));

        // Test headers that SHOULD be forwarded
        assert!(should_forward_response_header("content-type"));
        assert!(should_forward_response_header("cache-control"));
        assert!(should_forward_response_header("etag"));
        assert!(should_forward_response_header("last-modified"));
        assert!(should_forward_response_header("custom-response-header"));
    }

    #[test]
    fn test_header_filtering_case_insensitivity() {
        // Header names are case sensitive in this implementation
        assert!(!should_forward_header("connection"));
        assert!(should_forward_header("CONNECTION")); // Different case, should be forwarded
        assert!(!should_forward_header("upgrade"));
        assert!(should_forward_header("UPGRADE")); // Different case, should be forwarded

        assert!(should_forward_header("content-type"));
        assert!(should_forward_header("CONTENT-TYPE"));
        assert!(should_forward_header("User-Agent"));
        assert!(should_forward_header("user-agent"));
    }

    #[tokio::test]
    async fn test_handle_not_found() {
        let response = handle_not_found().await.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_proxy_state_with_different_timeout() {
        let mut config = create_test_config();
        config.server.timeout_ms = Some(60000);

        let proxy_state = ProxyState::new(config);

        // The client should be created with the specified timeout
        // We can't easily test the internal timeout value,
        // but we can verify the state was created
        assert_eq!(proxy_state.config.server.timeout_ms, Some(60000));
    }

    #[test]
    fn test_proxy_state_with_no_timeout() {
        let mut config = create_test_config();
        config.server.timeout_ms = None;

        let proxy_state = ProxyState::new(config);

        // Should use default timeout of 30000ms
        assert!(proxy_state.config.server.timeout_ms.is_none());
    }

    #[test]
    fn test_route_priority_first_match() {
        let mut config = create_test_config();

        // Add a more specific route that could conflict
        config.routes.insert(
            0,
            RouteConfig {
                path: "/api/v1/users/*".to_string(),
                upstream: "http://specific-service:4000".to_string(),
                methods: vec!["GET".to_string()],
                headers: HashMap::new(),
                strip_path: false,
                preserve_host: false,
                timeout_ms: None,
            },
        );

        let proxy_state = ProxyState::new(config);

        // Should match the first (more specific) route
        let route = proxy_state.find_matching_route("/api/v1/users/123", "GET");
        assert!(route.is_some());
        assert_eq!(route.unwrap().upstream, "http://specific-service:4000");
    }

    #[test]
    fn test_empty_routes_configuration() {
        let config = GatewayConfig {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                workers: None,
                timeout_ms: Some(5000),
                tls: None,
            },
            routes: vec![],
            logging: crate::config::LoggingConfig::default(),
        };

        let proxy_state = ProxyState::new(config);

        // No routes should match
        let route = proxy_state.find_matching_route("/any/path", "GET");
        assert!(route.is_none());
    }

    #[test]
    fn test_route_matching_with_special_characters() {
        let config = create_test_config();
        let proxy_state = ProxyState::new(config);

        // Test paths with special characters
        let route = proxy_state.find_matching_route("/api/v1/users%20test", "GET");
        assert!(route.is_some());

        let route = proxy_state.find_matching_route("/api/v1/test?query=value", "GET");
        assert!(route.is_some());
    }

    #[test]
    fn test_clone_proxy_state() {
        let config = create_test_config();
        let proxy_state = ProxyState::new(config);

        // ProxyState should be cloneable
        let cloned_state = proxy_state.clone();

        assert_eq!(
            proxy_state.config.server.port,
            cloned_state.config.server.port
        );
        assert_eq!(
            proxy_state.config.routes.len(),
            cloned_state.config.routes.len()
        );
    }
}
