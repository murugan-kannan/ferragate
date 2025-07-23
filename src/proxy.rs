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
        let timeout = Duration::from_millis(
            config.server.timeout_ms.unwrap_or(30000)
        );

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
        self.config.routes.iter().find(|route| {
            route.matches_path(path) && route.matches_method(method)
        })
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
            return (
                StatusCode::NOT_FOUND,
                "No matching route found",
            ).into_response();
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
            return (
                StatusCode::BAD_REQUEST,
                "Failed to read request body",
            ).into_response();
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
            return (
                StatusCode::METHOD_NOT_ALLOWED,
                "Method not allowed",
            ).into_response();
        }
    };

    let mut request_builder = state.client
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
            ).into_response();
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
                HeaderValue::try_from(value.as_bytes())
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
            return (
                StatusCode::BAD_GATEWAY,
                "Failed to read response body",
            ).into_response();
        }
    };

    debug!("Successfully proxied request, response size: {} bytes", response_body.len());

    // Return the response
    (status, response_headers, response_body).into_response()
}

fn should_forward_header(header_name: &str) -> bool {
    match header_name {
        // Don't forward connection-specific headers
        "connection" | "upgrade" | "proxy-authenticate" | "proxy-authorization" |
        "te" | "trailers" | "transfer-encoding" => false,
        // Don't forward hop-by-hop headers
        "keep-alive" | "proxy-connection" => false,
        // Forward everything else
        _ => true,
    }
}

fn should_forward_response_header(header_name: &str) -> bool {
    match header_name {
        // Don't forward connection-specific headers
        "connection" | "upgrade" | "proxy-authenticate" | "proxy-authorization" |
        "te" | "trailers" | "transfer-encoding" => false,
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

    #[test]
    fn test_header_filtering() {
        assert!(!should_forward_header("connection"));
        assert!(!should_forward_header("upgrade"));
        assert!(should_forward_header("authorization"));
        assert!(should_forward_header("content-type"));
        assert!(should_forward_header("user-agent"));
    }

    #[test]
    fn test_response_header_filtering() {
        assert!(!should_forward_response_header("connection"));
        assert!(!should_forward_response_header("transfer-encoding"));
        assert!(should_forward_response_header("content-type"));
        assert!(should_forward_response_header("cache-control"));
    }
}
