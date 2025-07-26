use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ferragate::config::{GatewayConfig, RouteConfig, ServerConfig};
use ferragate::proxy::ProxyState;
use std::collections::HashMap;

fn create_proxy_state() -> ProxyState {
    let config = GatewayConfig {
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
                upstream: "http://backend-v1:3000".to_string(),
                methods: vec!["GET".to_string(), "POST".to_string()],
                headers: HashMap::new(),
                strip_path: true,
                preserve_host: false,
                timeout_ms: Some(5000),
            },
            RouteConfig {
                path: "/api/v2/*".to_string(),
                upstream: "http://backend-v2:3000".to_string(),
                methods: vec!["GET".to_string(), "POST".to_string()],
                headers: HashMap::new(),
                strip_path: false,
                preserve_host: true,
                timeout_ms: Some(10000),
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
            RouteConfig {
                path: "/static/*".to_string(),
                upstream: "http://static-cdn:8080".to_string(),
                methods: vec!["GET".to_string(), "HEAD".to_string()],
                headers: HashMap::new(),
                strip_path: true,
                preserve_host: false,
                timeout_ms: Some(15000),
            },
        ],
        logging: ferragate::config::LoggingConfig::default(),
    };

    ProxyState::new(config)
}

fn benchmark_route_finding(c: &mut Criterion) {
    let proxy_state = create_proxy_state();

    let test_paths = [
        "/api/v1/users",
        "/api/v1/users/123",
        "/api/v1/users/123/profile",
        "/api/v2/orders",
        "/api/v2/orders/456",
        "/health",
        "/static/css/style.css",
        "/static/js/app.js",
        "/nonexistent/path",
    ];

    c.bench_function("find_matching_route_single", |b| {
        b.iter(|| {
            black_box(proxy_state.find_matching_route("/api/v1/users/123", "GET"));
        })
    });

    let mut group = c.benchmark_group("find_matching_route_batch");
    for path_count in [1, 5, 10, 50].iter() {
        group.throughput(Throughput::Elements(*path_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(path_count),
            path_count,
            |b, &path_count| {
                b.iter(|| {
                    for i in 0..path_count {
                        let path = &test_paths[i % test_paths.len()];
                        black_box(proxy_state.find_matching_route(path, "GET"));
                    }
                })
            },
        );
    }
    group.finish();
}

fn benchmark_route_matching_patterns(c: &mut Criterion) {
    let proxy_state = create_proxy_state();

    let test_scenarios = vec![
        ("exact_match", "/health", "GET"),
        ("wildcard_short", "/api/v1/test", "GET"),
        (
            "wildcard_long",
            "/api/v1/users/123/profile/settings/advanced",
            "GET",
        ),
        ("static_file", "/static/images/logo.png", "GET"),
        ("no_match", "/unknown/endpoint", "GET"),
    ];

    let mut group = c.benchmark_group("route_matching_patterns");
    for (name, path, method) in test_scenarios {
        group.bench_function(name, |b| {
            b.iter(|| {
                black_box(proxy_state.find_matching_route(path, method));
            })
        });
    }
    group.finish();
}

fn benchmark_method_validation(c: &mut Criterion) {
    let proxy_state = create_proxy_state();

    let methods = vec!["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"];

    c.bench_function("method_validation_allowed", |b| {
        b.iter(|| {
            for method in &methods {
                black_box(proxy_state.find_matching_route("/api/v1/users", method));
            }
        })
    });

    c.bench_function("method_validation_restricted", |b| {
        b.iter(|| {
            for method in &methods {
                black_box(proxy_state.find_matching_route("/health", method));
            }
        })
    });
}

criterion_group!(
    proxy_benches,
    benchmark_route_finding,
    benchmark_route_matching_patterns,
    benchmark_method_validation
);
criterion_main!(proxy_benches);
