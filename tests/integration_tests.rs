use ferragate::config::GatewayConfig;

#[tokio::test]
async fn test_config_validation() {
    // Test valid config
    let config = GatewayConfig::default_config();
    assert!(config.validate().is_ok());

    // Test invalid upstream URL
    let mut invalid_config = GatewayConfig::default_config();
    invalid_config.routes[0].upstream = "invalid-url".to_string();
    assert!(invalid_config.validate().is_err());

    // Test invalid HTTP method
    let mut invalid_config = GatewayConfig::default_config();
    invalid_config.routes[0].methods = vec!["INVALID".to_string()];
    assert!(invalid_config.validate().is_err());
}

#[tokio::test]
async fn test_route_path_matching() {
    use ferragate::config::RouteConfig;
    use std::collections::HashMap;

    let route = RouteConfig {
        path: "/get/*".to_string(),
        upstream: "https://httpbin.org".to_string(),
        methods: vec!["GET".to_string()],
        headers: HashMap::new(),
        strip_path: true,
        preserve_host: false,
        timeout_ms: None,
    };

    // Test path matching
    assert!(route.matches_path("/get/anything"));
    assert!(route.matches_path("/get/"));
    assert!(!route.matches_path("/post/anything"));
    assert!(!route.matches_path("/health"));

    // Test method matching
    assert!(route.matches_method("GET"));
    assert!(route.matches_method("get")); // case insensitive
    assert!(!route.matches_method("POST"));
}

#[tokio::test]
async fn test_path_transformation() {
    use ferragate::config::RouteConfig;
    use std::collections::HashMap;

    let route = RouteConfig {
        path: "/status/*".to_string(),
        upstream: "https://httpbin.org".to_string(),
        methods: vec![],
        headers: HashMap::new(),
        strip_path: true,
        preserve_host: false,
        timeout_ms: None,
    };

    // Test path transformation with strip_path = true
    assert_eq!(route.transform_path("/status/200"), "/200");
    assert_eq!(route.transform_path("/status/"), "/");
    assert_eq!(route.transform_path("/status"), "/");

    // Test path transformation with strip_path = false
    let route_no_strip = RouteConfig {
        strip_path: false,
        ..route
    };
    assert_eq!(route_no_strip.transform_path("/status/200"), "/status/200");
}

#[tokio::test]
async fn test_httpbin_get_endpoint() {
    use ferragate::config::RouteConfig;
    use std::collections::HashMap;

    let route = RouteConfig {
        path: "/get/*".to_string(),
        upstream: "https://httpbin.org".to_string(),
        methods: vec!["GET".to_string()],
        headers: HashMap::new(),
        strip_path: true,
        preserve_host: false,
        timeout_ms: Some(30000),
    };

    // Test that the route configuration is valid for httpbin
    assert!(route.matches_path("/get/anything"));
    assert!(route.matches_method("GET"));
    assert_eq!(route.transform_path("/get/anything"), "/anything");

    // Verify upstream URL is valid
    let url = url::Url::parse(&route.upstream);
    assert!(url.is_ok());
    assert_eq!(url.unwrap().scheme(), "https");
}

#[tokio::test]
async fn test_httpbin_post_endpoint() {
    use ferragate::config::RouteConfig;
    use std::collections::HashMap;

    let route = RouteConfig {
        path: "/post/*".to_string(),
        upstream: "https://httpbin.org".to_string(),
        methods: vec!["POST".to_string()],
        headers: HashMap::new(),
        strip_path: true,
        preserve_host: false,
        timeout_ms: Some(30000),
    };

    // Test that the route configuration is valid for httpbin POST
    assert!(route.matches_path("/post/anything"));
    assert!(route.matches_method("POST"));
    assert_eq!(route.transform_path("/post/data"), "/data");

    // Verify only POST method is allowed
    assert!(!route.matches_method("GET"));
    assert!(!route.matches_method("PUT"));
}

#[tokio::test]
async fn test_httpbin_status_endpoint() {
    use ferragate::config::RouteConfig;
    use std::collections::HashMap;

    let route = RouteConfig {
        path: "/status/*".to_string(),
        upstream: "https://httpbin.org".to_string(),
        methods: vec!["GET".to_string()],
        headers: HashMap::new(),
        strip_path: true,
        preserve_host: false,
        timeout_ms: Some(30000),
    };

    // Test status code endpoints
    assert!(route.matches_path("/status/200"));
    assert!(route.matches_path("/status/404"));
    assert!(route.matches_path("/status/500"));

    // Test path transformation for status codes
    assert_eq!(route.transform_path("/status/200"), "/200");
    assert_eq!(route.transform_path("/status/404"), "/404");
    assert_eq!(route.transform_path("/status/500"), "/500");
}

#[tokio::test]
async fn test_httpbin_json_endpoint() {
    use ferragate::config::RouteConfig;
    use std::collections::HashMap;

    let route = RouteConfig {
        path: "/json/*".to_string(),
        upstream: "https://httpbin.org".to_string(),
        methods: vec!["GET".to_string()],
        headers: HashMap::new(),
        strip_path: true,
        preserve_host: false,
        timeout_ms: Some(30000),
    };

    // Test JSON endpoint
    assert!(route.matches_path("/json/"));
    assert!(route.matches_path("/json/data"));
    assert_eq!(route.transform_path("/json/"), "/");
    assert_eq!(route.transform_path("/json/data"), "/data");
}

#[tokio::test]
async fn test_multiple_httpbin_routes() {
    let config = GatewayConfig::default_config();

    // Verify we have multiple routes configured
    assert!(config.routes.len() >= 4);

    // Find and test each route type
    let get_route = config.routes.iter().find(|r| r.path == "/get/*").unwrap();
    let post_route = config.routes.iter().find(|r| r.path == "/post/*").unwrap();
    let status_route = config.routes.iter().find(|r| r.path == "/status/*").unwrap();
    let json_route = config.routes.iter().find(|r| r.path == "/json/*").unwrap();

    // All should point to httpbin.org
    assert_eq!(get_route.upstream, "https://httpbin.org");
    assert_eq!(post_route.upstream, "https://httpbin.org");
    assert_eq!(status_route.upstream, "https://httpbin.org");
    assert_eq!(json_route.upstream, "https://httpbin.org");

    // All should have strip_path enabled for proper httpbin routing
    assert!(get_route.strip_path);
    assert!(post_route.strip_path);
    assert!(status_route.strip_path);
    assert!(json_route.strip_path);
}

#[tokio::test]
async fn test_httpbin_route_isolation() {
    let config = GatewayConfig::default_config();

    let get_route = config.routes.iter().find(|r| r.path == "/get/*").unwrap();
    let post_route = config.routes.iter().find(|r| r.path == "/post/*").unwrap();

    // GET route should not match POST paths and vice versa
    assert!(get_route.matches_path("/get/anything"));
    assert!(!get_route.matches_path("/post/anything"));

    assert!(post_route.matches_path("/post/anything"));
    assert!(!post_route.matches_path("/get/anything"));

    // Method restrictions should be enforced
    assert!(get_route.matches_method("GET"));
    assert!(!get_route.matches_method("POST"));

    assert!(post_route.matches_method("POST"));
    assert!(!post_route.matches_method("GET"));
}
