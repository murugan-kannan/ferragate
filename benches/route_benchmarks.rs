use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ferragate::config::{GatewayConfig, RouteConfig, ServerConfig};
use std::collections::HashMap;

fn create_test_config() -> GatewayConfig {
    GatewayConfig {
        server: ServerConfig {
            host: "0.0.0.0".to_string(),
            port: 8080,
            workers: Some(4),
            timeout_ms: Some(30000),
            tls: None,
        },
        routes: vec![
            RouteConfig {
                path: "/api/v1/*".to_string(),
                upstream: "http://backend:3000".to_string(),
                methods: vec!["GET".to_string(), "POST".to_string()],
                headers: HashMap::new(),
                strip_path: true,
                preserve_host: false,
                timeout_ms: Some(5000),
            },
            RouteConfig {
                path: "/health".to_string(),
                upstream: "http://health-service:8080".to_string(),
                methods: vec!["GET".to_string()],
                headers: HashMap::new(),
                strip_path: false,
                preserve_host: false,
                timeout_ms: None,
            },
        ],
        logging: ferragate::config::LoggingConfig::default(),
    }
}

fn benchmark_route_matching(c: &mut Criterion) {
    let config = create_test_config();

    c.bench_function("route_matching_exact", |b| {
        b.iter(|| {
            for route in &config.routes {
                black_box(route.matches_path("/health"));
            }
        })
    });

    c.bench_function("route_matching_wildcard", |b| {
        b.iter(|| {
            for route in &config.routes {
                black_box(route.matches_path("/api/v1/users/123"));
            }
        })
    });

    c.bench_function("route_matching_method", |b| {
        b.iter(|| {
            for route in &config.routes {
                black_box(route.matches_method("GET"));
            }
        })
    });
}

fn benchmark_path_transformation(c: &mut Criterion) {
    let route = RouteConfig {
        path: "/api/v1/*".to_string(),
        upstream: "http://backend:3000".to_string(),
        methods: vec![],
        headers: HashMap::new(),
        strip_path: true,
        preserve_host: false,
        timeout_ms: None,
    };

    c.bench_function("path_transformation", |b| {
        b.iter(|| {
            black_box(route.transform_path("/api/v1/users/123/profile"));
        })
    });
}

fn benchmark_config_validation(c: &mut Criterion) {
    let config = create_test_config();

    c.bench_function("config_validation", |b| {
        b.iter(|| {
            black_box(config.validate().is_ok());
        })
    });
}

criterion_group!(
    route_benches,
    benchmark_route_matching,
    benchmark_path_transformation,
    benchmark_config_validation
);
criterion_main!(route_benches);
