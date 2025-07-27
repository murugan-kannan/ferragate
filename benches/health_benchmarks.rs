use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use ferragate::health::{AppState, HealthStatus};

fn benchmark_health_operations(c: &mut Criterion) {
    let app_state = AppState::new();

    c.bench_function("app_state_creation", |b| {
        b.iter(|| {
            black_box(AppState::new());
        })
    });

    c.bench_function("get_uptime_seconds", |b| {
        b.iter(|| {
            black_box(app_state.get_uptime_seconds());
        })
    });

    c.bench_function("is_ready_check", |b| {
        b.iter(|| {
            black_box(app_state.is_ready());
        })
    });
}

fn benchmark_health_check_operations(c: &mut Criterion) {
    let app_state = AppState::new();

    // Pre-register some health checks
    app_state.register_health_check(
        "database".to_string(),
        HealthStatus::Healthy,
        Some("DB connected".to_string()),
    );
    app_state.register_health_check(
        "cache".to_string(),
        HealthStatus::Healthy,
        Some("Cache operational".to_string()),
    );
    app_state.register_health_check(
        "external_api".to_string(),
        HealthStatus::Healthy,
        Some("API responding".to_string()),
    );

    c.bench_function("get_health_checks", |b| {
        b.iter(|| {
            black_box(app_state.get_health_checks());
        })
    });

    c.bench_function("update_health_check", |b| {
        b.iter(|| {
            app_state.update_health_check(
                "database",
                HealthStatus::Healthy,
                Some("Still connected".to_string()),
            );
        })
    });

    c.bench_function("register_new_health_check", |b| {
        let mut counter = 0;
        b.iter(|| {
            counter += 1;
            app_state.register_health_check(
                format!("service_{counter}"),
                HealthStatus::Healthy,
                Some("New service".to_string()),
            );
        })
    });
}

fn benchmark_health_check_batch_operations(c: &mut Criterion) {
    let app_state = AppState::new();

    // Initialize with multiple health checks
    for i in 0..10 {
        app_state.register_health_check(
            format!("service_{i}"),
            HealthStatus::Healthy,
            Some(format!("Service {i} operational")),
        );
    }

    c.bench_function("batch_health_check_updates", |b| {
        b.iter(|| {
            for i in 0..10 {
                app_state.update_health_check(
                    &format!("service_{i}"),
                    if i % 2 == 0 {
                        HealthStatus::Healthy
                    } else {
                        HealthStatus::Unhealthy
                    },
                    Some(format!("Service {i} status update")),
                );
            }
        })
    });

    c.bench_function("health_state_with_many_checks", |b| {
        b.iter(|| {
            let checks = app_state.get_health_checks();
            black_box(checks.len());
        })
    });
}

criterion_group!(
    health_benches,
    benchmark_health_operations,
    benchmark_health_check_operations,
    benchmark_health_check_batch_operations
);
criterion_main!(health_benches);
