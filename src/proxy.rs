use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri},
    response::IntoResponse,
};
use bytes::Bytes;
use http_body_util::BodyExt;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, instrument, warn};

use crate::config::{GatewayConfig, RouteConfig};
use crate::constants::*;

/// State shared across all proxy handlers
///
/// Contains the gateway configuration and HTTP client for making upstream requests.
#[derive(Clone)]
pub struct ProxyState {
    /// Gateway configuration (wrapped in Arc for shared ownership)
    pub config: Arc<GatewayConfig>,
    /// HTTP client for upstream requests
    pub client: reqwest::Client,
}

impl ProxyState {
    /// Create a new ProxyState with the given configuration
    ///
    /// Sets up an HTTP client with appropriate timeouts and connection pooling.
    pub fn new(config: GatewayConfig) -> Self {
        let timeout = Duration::from_millis(config.server.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS));

        let client = reqwest::Client::builder()
            .timeout(timeout)
            .pool_idle_timeout(Duration::from_secs(CLIENT_POOL_IDLE_TIMEOUT_SECS))
            .pool_max_idle_per_host(CLIENT_POOL_MAX_IDLE_PER_HOST)
            .user_agent(CLIENT_USER_AGENT)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config: Arc::new(config),
            client,
        }
    }

    /// Find the first route that matches the given path and method
    ///
    /// Routes are evaluated in the order they appear in the configuration.
    /// Returns None if no matching route is found.
    pub fn find_matching_route(&self, path: &str, method: &str) -> Option<&RouteConfig> {
        self.config
            .routes
            .iter()
            .find(|route| route.matches_path(path) && route.matches_method(method))
    }
}

/// Main proxy handler for incoming requests
///
/// This function:
/// 1. Finds a matching route for the request
/// 2. Transforms the request for upstream forwarding
/// 3. Executes the upstream request
/// 4. Returns the upstream response to the client
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

    debug!("Processing request: {} {}", method, uri);

    // Find matching route
    let route = match find_route_for_request(&state, path, method.as_str()) {
        Some(route) => route,
        None => {
            warn!("No matching route found for: {} {}", method, path);
            return (StatusCode::NOT_FOUND, MSG_ROUTE_NOT_FOUND).into_response();
        }
    };

    debug!("Matched route: {} -> {}", route.path, route.upstream);

    // Build target URL
    let target_url = build_target_url(route, path, query);
    debug!("Proxying to: {}", target_url);

    // Read request body
    let body_bytes = match read_request_body(body).await {
        Ok(bytes) => bytes,
        Err(err_resp) => return err_resp,
    };

    // Create and configure upstream request
    let request_builder =
        match create_upstream_request(&state, route, &method, &target_url, &headers, body_bytes)
            .await
        {
            Ok(builder) => builder,
            Err(err_resp) => return err_resp,
        };

    // Execute upstream request
    let response = match execute_upstream_request(request_builder, &target_url).await {
        Ok(response) => response,
        Err(err_resp) => return err_resp,
    };

    // Process and return upstream response
    process_upstream_response(response).await
}

/// Find a matching route for the given request
fn find_route_for_request<'a>(
    state: &'a ProxyState,
    path: &str,
    method: &str,
) -> Option<&'a RouteConfig> {
    state.find_matching_route(path, method)
}

/// Build the target URL for upstream forwarding
fn build_target_url(route: &RouteConfig, path: &str, query: &str) -> String {
    let target_path = route.transform_path(path);
    let mut target_url = format!("{}{}", route.upstream, target_path);

    if !query.is_empty() {
        target_url.push('?');
        target_url.push_str(query);
    }

    target_url
}

/// Read the request body from the incoming request
async fn read_request_body(body: Body) -> Result<Bytes, axum::response::Response> {
    match body.collect().await {
        Ok(collected) => Ok(collected.to_bytes()),
        Err(e) => {
            error!("Failed to read request body: {}", e);
            Err((StatusCode::BAD_REQUEST, MSG_INVALID_REQUEST_BODY).into_response())
        }
    }
}

/// Create and configure the upstream request
async fn create_upstream_request(
    state: &ProxyState,
    route: &RouteConfig,
    method: &Method,
    target_url: &str,
    headers: &HeaderMap,
    body_bytes: Bytes,
) -> Result<reqwest::RequestBuilder, axum::response::Response> {
    // Convert HTTP method
    let reqwest_method = match convert_http_method(method) {
        Ok(reqwest_method) => reqwest_method,
        Err(err_resp) => return Err(err_resp),
    };

    // Create base request
    let mut request_builder = state
        .client
        .request(reqwest_method, target_url)
        .body(body_bytes);

    // Add headers from original request
    request_builder = add_forwarded_headers(request_builder, headers);

    // Add custom headers from route configuration
    request_builder = add_route_headers(request_builder, route);

    // Handle Host header
    request_builder = handle_host_header(request_builder, route, target_url);

    // Apply timeout (route-specific or server default)
    let server_default_timeout = state.config.server.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    let effective_timeout_ms = route.effective_timeout(server_default_timeout);
    request_builder = request_builder.timeout(Duration::from_millis(effective_timeout_ms));

    Ok(request_builder)
}

/// Convert Axum HTTP method to reqwest method
fn convert_http_method(method: &Method) -> Result<reqwest::Method, axum::response::Response> {
    match method.as_str() {
        "GET" => Ok(reqwest::Method::GET),
        "POST" => Ok(reqwest::Method::POST),
        "PUT" => Ok(reqwest::Method::PUT),
        "DELETE" => Ok(reqwest::Method::DELETE),
        "HEAD" => Ok(reqwest::Method::HEAD),
        "OPTIONS" => Ok(reqwest::Method::OPTIONS),
        "PATCH" => Ok(reqwest::Method::PATCH),
        _ => {
            warn!("Unsupported HTTP method: {}", method);
            Err((StatusCode::METHOD_NOT_ALLOWED, "Method not allowed").into_response())
        }
    }
}

/// Add appropriate headers from the original request to the upstream request
fn add_forwarded_headers(
    mut request_builder: reqwest::RequestBuilder,
    headers: &HeaderMap,
) -> reqwest::RequestBuilder {
    for (name, value) in headers.iter() {
        let header_name = name.as_str().to_lowercase();
        if should_forward_header(&header_name) {
            if let Ok(header_value) = value.to_str() {
                request_builder = request_builder.header(name.as_str(), header_value);
            }
        }
    }
    request_builder
}

/// Add custom headers from route configuration
fn add_route_headers(
    mut request_builder: reqwest::RequestBuilder,
    route: &RouteConfig,
) -> reqwest::RequestBuilder {
    for (name, value) in &route.headers {
        request_builder = request_builder.header(name, value);
    }
    request_builder
}

/// Handle the Host header based on route configuration
fn handle_host_header(
    mut request_builder: reqwest::RequestBuilder,
    route: &RouteConfig,
    target_url: &str,
) -> reqwest::RequestBuilder {
    if !route.preserve_host {
        if let Ok(target_url_parsed) = url::Url::parse(target_url) {
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
    request_builder
}

/// Execute the upstream request
async fn execute_upstream_request(
    request_builder: reqwest::RequestBuilder,
    target_url: &str,
) -> Result<reqwest::Response, axum::response::Response> {
    match request_builder.send().await {
        Ok(response) => {
            debug!("Upstream response status: {}", response.status());
            Ok(response)
        }
        Err(e) => {
            error!("Failed to proxy request to {}: {}", target_url, e);
            Err((
                StatusCode::BAD_GATEWAY,
                format!("Failed to proxy request: {}", e),
            )
                .into_response())
        }
    }
}

/// Process the upstream response and prepare it for the client
async fn process_upstream_response(response: reqwest::Response) -> axum::response::Response {
    // Convert status code
    let status = StatusCode::from_u16(response.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    // Process response headers
    let mut response_headers = HeaderMap::new();
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

    // Read response body
    let response_body = match response.bytes().await {
        Ok(bytes) => {
            debug!(
                "Successfully proxied request, response size: {} bytes",
                bytes.len()
            );
            bytes
        }
        Err(e) => {
            error!("Failed to read response body: {}", e);
            return (StatusCode::BAD_GATEWAY, "Failed to read response body").into_response();
        }
    };

    (status, response_headers, response_body).into_response()
}

fn should_forward_header(header_name: &str) -> bool {
    // Check if header is in the filtered list
    if FILTERED_HEADERS.contains(&header_name) {
        return false;
    }

    match header_name {
        // Don't forward hop-by-hop headers
        "keep-alive" | "proxy-connection" => false,
        // Forward everything else
        _ => true,
    }
}

fn should_forward_response_header(header_name: &str) -> bool {
    // Check if header is in the filtered list
    if FILTERED_HEADERS.contains(&header_name) {
        return false;
    }

    match header_name {
        // Don't forward hop-by-hop headers
        "keep-alive" | "proxy-connection" => false,
        // Forward everything else
        _ => true,
    }
}

pub async fn handle_not_found() -> impl IntoResponse {
    debug!("404 Not Found response");
    (StatusCode::NOT_FOUND, MSG_ROUTE_NOT_FOUND)
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
