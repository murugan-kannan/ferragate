# Development Guide

This guide is for developers who want to understand FerraGate's codebase, contribute to the project, or extend its functionality.

## 🏗️ Project Structure

### Repository Layout

```
ferragate/
├── src/                    # Source code
│   ├── main.rs            # Binary entry point
│   ├── lib.rs             # Library entry point  
│   ├── cli.rs             # CLI argument parsing
│   ├── config.rs          # Configuration management
│   ├── server.rs          # HTTP server setup
│   ├── proxy.rs           # Core proxy logic
│   ├── health.rs          # Health check endpoints
│   ├── logging.rs         # Logging configuration
│   └── tls.rs             # TLS/SSL handling
├── tests/                 # Integration tests
│   ├── integration_tests.rs
│   └── common/
├── docs/                  # Documentation
├── examples/              # Example configurations
├── certs/                 # Development certificates
├── logs/                  # Log files
├── Cargo.toml            # Rust dependencies
├── Cargo.lock            # Dependency lock file
├── docker-compose.yml    # Docker composition
├── Dockerfile            # Container definition
└── README.md             # Project overview
```

### Core Modules

#### `main.rs` - Application Entry Point
```rust
// Initializes logging and CLI parsing
// Delegates to CLI module for command execution
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_default_logging()?;
    let cli = Cli::parse_args();
    cli.execute().await
}
```

#### `cli.rs` - Command Line Interface
```rust
// Defines CLI structure using clap
// Handles subcommands: start, validate, init, gen-certs
// Provides configuration overrides
pub enum Commands {
    Start { config: PathBuf, host: Option<String>, port: Option<u16> },
    Validate { config: PathBuf },
    Init { output: PathBuf, force: bool },
    GenCerts { hostname: String, output_dir: PathBuf },
}
```

#### `config.rs` - Configuration Management
```rust
// Configuration structures and parsing
// Validation and default values
// TOML serialization/deserialization
pub struct GatewayConfig {
    pub server: ServerConfig,
    pub routes: Vec<RouteConfig>,
    pub logging: LoggingConfig,
}
```

#### `server.rs` - HTTP Server
```rust
// Axum server setup and routing
// HTTP/HTTPS listener configuration
// Middleware setup and request routing
pub async fn create_server(config: GatewayConfig) -> Result<Router> {
    // Server creation logic
}
```

#### `proxy.rs` - Proxy Logic
```rust
// Core request proxying functionality
// Route matching and upstream forwarding
// Request/response transformation
pub async fn proxy_handler(
    State(state): State<ProxyState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Body,
) -> impl IntoResponse {
    // Proxy implementation
}
```

## 🛠️ Development Setup

### Prerequisites

1. **Install Rust**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   rustup component add clippy rustfmt
   ```

2. **Install Development Tools**
   ```bash
   # Code formatting
   rustup component add rustfmt
   
   # Linting
   rustup component add clippy
   
   # Documentation generation
   cargo install cargo-doc
   
   # Test coverage
   cargo install cargo-tarpaulin
   
   # Security audit
   cargo install cargo-audit
   ```

3. **Clone Repository**
   ```bash
   git clone https://github.com/murugan-kannan/ferragate.git
   cd ferragate
   ```

### Development Environment

#### VS Code Setup

Install recommended extensions:
- `rust-analyzer` - Rust language support
- `crates` - Cargo.toml management
- `Better TOML` - TOML syntax highlighting

Create `.vscode/settings.json`:
```json
{
    "rust-analyzer.checkOnSave.command": "clippy",
    "rust-analyzer.cargo.features": "all",
    "editor.formatOnSave": true,
    "[rust]": {
        "editor.defaultFormatter": "rust-lang.rust-analyzer"
    }
}
```

#### Development Scripts

Create `scripts/dev.sh`:
```bash
#!/bin/bash
# Development convenience script

case "$1" in
    "build")
        cargo build
        ;;
    "test")
        cargo test
        ;;
    "lint")
        cargo clippy -- -D warnings
        ;;
    "format")
        cargo fmt
        ;;
    "check")
        cargo check
        ;;
    "run")
        cargo run -- start
        ;;
    "clean")
        cargo clean
        ;;
    *)
        echo "Usage: $0 {build|test|lint|format|check|run|clean}"
        exit 1
        ;;
esac
```

## 🧪 Testing Strategy

### Test Hierarchy

1. **Unit Tests** - Test individual functions and modules
2. **Integration Tests** - Test component interactions
3. **End-to-End Tests** - Test complete user workflows
4. **Performance Tests** - Test under load

### Unit Testing

```rust
// src/config.rs
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_route_config_validation() {
        let route = RouteConfig {
            path: "/api/*".to_string(),
            upstream: "http://backend:8080".to_string(),
            methods: vec!["GET".to_string()],
            ..Default::default()
        };
        
        assert!(route.is_valid());
    }
    
    #[test]
    fn test_invalid_upstream_url() {
        let route = RouteConfig {
            path: "/api/*".to_string(),
            upstream: "invalid-url".to_string(),
            ..Default::default()
        };
        
        assert!(!route.is_valid());
    }
}
```

### Integration Testing

```rust
// tests/integration_tests.rs
use ferragate::*;
use axum_test::TestServer;
use std::time::Duration;

#[tokio::test]
async fn test_gateway_proxy_flow() {
    // Setup mock backend
    let backend = setup_mock_backend().await;
    
    // Configure gateway
    let config = GatewayConfig {
        server: ServerConfig::default(),
        routes: vec![RouteConfig {
            path: "/api/*".to_string(),
            upstream: backend.uri(),
            ..Default::default()
        }],
        logging: LoggingConfig::default(),
    };
    
    // Create test server
    let app = create_app(config).await;
    let server = TestServer::new(app).unwrap();
    
    // Test request
    let response = server
        .get("/api/users")
        .await;
    
    assert_eq!(response.status_code(), 200);
}
```

### Testing Commands

```bash
# Run all tests
cargo test

# Run tests with coverage
cargo tarpaulin --out Html

# Run specific test
cargo test test_name

# Run integration tests only
cargo test --test integration_tests

# Run with debug output
cargo test -- --nocapture

# Test documentation examples
cargo test --doc
```

## 🔧 Build and Debugging

### Build Configurations

#### Development Build
```bash
# Fast compilation, includes debug symbols
cargo build

# With specific features
cargo build --features "tls-rustls"
```

#### Release Build
```bash
# Optimized build
cargo build --release

# With link-time optimization
RUSTFLAGS="-C lto=fat" cargo build --release
```

#### Cross Compilation
```bash
# Install target
rustup target add x86_64-unknown-linux-musl

# Build for target
cargo build --target x86_64-unknown-linux-musl --release
```

### Debugging

#### Logging Configuration

```rust
// Enable debug logging
RUST_LOG=debug cargo run

// Module-specific logging
RUST_LOG=ferragate::proxy=debug cargo run

// Multiple modules
RUST_LOG=ferragate::proxy=debug,ferragate::config=info cargo run
```

#### Using Debugger

```bash
# Install rust-gdb
rustup component add rust-src

# Debug with gdb
rust-gdb target/debug/ferragate

# Debug with lldb (macOS)
rust-lldb target/debug/ferragate
```

#### Memory Profiling

```bash
# Install valgrind (Linux)
sudo apt install valgrind

# Run with valgrind
valgrind --tool=memcheck --leak-check=full target/debug/ferragate start

# Use heaptrack for better analysis
heaptrack target/debug/ferragate start
```

### Performance Profiling

#### CPU Profiling
```bash
# Install profiling tools
cargo install flamegraph

# Generate flame graph
cargo flamegraph --bin ferragate -- start

# With specific duration
timeout 30s cargo flamegraph --bin ferragate -- start
```

#### Benchmarking
```rust
// benches/proxy_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ferragate::proxy::*;

fn bench_route_matching(c: &mut Criterion) {
    let routes = create_test_routes();
    
    c.bench_function("route_matching", |b| {
        b.iter(|| {
            find_matching_route(black_box(&routes), black_box("/api/users/123"))
        })
    });
}

criterion_group!(benches, bench_route_matching);
criterion_main!(benches);
```

## 🔌 Adding New Features

### Feature Development Workflow

1. **Design Phase**
   - Create GitHub issue with RFC template
   - Discuss architecture and API design
   - Get community feedback

2. **Implementation Phase**
   - Create feature branch
   - Implement core functionality
   - Add comprehensive tests
   - Update documentation

3. **Integration Phase**
   - Integration testing
   - Performance testing
   - Security review
   - Documentation review

### Example: Adding Rate Limiting

#### 1. Define Configuration

```rust
// src/config.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub requests_per_minute: u32,
    pub burst_size: u32,
    pub storage: RateLimitStorage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimitStorage {
    Memory,
    Redis { url: String },
}
```

#### 2. Implement Middleware

```rust
// src/middleware/rate_limit.rs
use tower::{Layer, Service};
use std::task::{Context, Poll};

#[derive(Clone)]
pub struct RateLimitLayer {
    config: RateLimitConfig,
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, service: S) -> Self::Service {
        RateLimitService {
            inner: service,
            config: self.config.clone(),
        }
    }
}

pub struct RateLimitService<S> {
    inner: S,
    config: RateLimitConfig,
}

impl<S> Service<Request<Body>> for RateLimitService<S>
where
    S: Service<Request<Body>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        // Rate limiting logic here
        self.inner.call(req)
    }
}
```

#### 3. Integrate with Server

```rust
// src/server.rs
use crate::middleware::rate_limit::RateLimitLayer;

pub async fn create_app(config: GatewayConfig) -> Router {
    let mut app = Router::new();
    
    // Add rate limiting if enabled
    if config.rate_limit.enabled {
        app = app.layer(RateLimitLayer::new(config.rate_limit));
    }
    
    app
}
```

#### 4. Add Tests

```rust
// tests/rate_limit_tests.rs
#[tokio::test]
async fn test_rate_limiting() {
    let config = create_rate_limited_config();
    let server = TestServer::new(create_app(config)).unwrap();
    
    // Make requests within limit
    for _ in 0..10 {
        let response = server.get("/api/test").await;
        assert_eq!(response.status_code(), 200);
    }
    
    // Exceed rate limit
    let response = server.get("/api/test").await;
    assert_eq!(response.status_code(), 429);
}
```

## 📊 Monitoring and Observability

### Logging

#### Structured Logging

```rust
use tracing::{info, warn, error, instrument};
use serde_json::json;

#[instrument(skip(state), fields(method = %method, uri = %uri))]
pub async fn proxy_handler(
    State(state): State<ProxyState>,
    method: Method,
    uri: Uri,
) -> impl IntoResponse {
    let start_time = std::time::Instant::now();
    
    info!("Processing request");
    
    // ... proxy logic ...
    
    let duration = start_time.elapsed();
    info!(
        duration_ms = duration.as_millis(),
        status = response.status().as_u16(),
        "Request completed"
    );
}
```

#### Log Levels

- `TRACE` - Very detailed debugging
- `DEBUG` - Debugging information  
- `INFO` - General information
- `WARN` - Warning conditions
- `ERROR` - Error conditions

### Metrics (Future)

#### Prometheus Integration

```rust
// src/metrics.rs
use prometheus::{Counter, Histogram, Gauge, Registry};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref REQUEST_COUNTER: Counter = Counter::new(
        "ferragate_requests_total",
        "Total number of requests"
    ).unwrap();
    
    pub static ref REQUEST_DURATION: Histogram = Histogram::new(
        "ferragate_request_duration_seconds",
        "Request duration in seconds"
    ).unwrap();
    
    pub static ref ACTIVE_CONNECTIONS: Gauge = Gauge::new(
        "ferragate_active_connections",
        "Number of active connections"
    ).unwrap();
}

pub fn register_metrics(registry: &Registry) -> Result<()> {
    registry.register(Box::new(REQUEST_COUNTER.clone()))?;
    registry.register(Box::new(REQUEST_DURATION.clone()))?;
    registry.register(Box::new(ACTIVE_CONNECTIONS.clone()))?;
    Ok(())
}
```

## 🔐 Security Considerations

### Secure Coding Practices

1. **Input Validation**
   ```rust
   fn validate_upstream_url(url: &str) -> Result<Url> {
       let parsed = Url::parse(url)
           .context("Invalid URL format")?;
       
       // Only allow HTTP/HTTPS
       match parsed.scheme() {
           "http" | "https" => Ok(parsed),
           _ => Err(anyhow!("Only HTTP/HTTPS schemes allowed")),
       }
   }
   ```

2. **Memory Safety**
   ```rust
   // Use bounds checking
   if index < vec.len() {
       vec[index]
   } else {
       return Err(anyhow!("Index out of bounds"));
   }
   
   // Prefer iterators over indexing
   for item in vec.iter() {
       process(item);
   }
   ```

3. **Error Handling**
   ```rust
   // Don't expose internal errors
   match internal_operation() {
       Ok(result) => Ok(result),
       Err(e) => {
           error!("Internal error: {}", e);
           Err(anyhow!("Operation failed"))
       }
   }
   ```

### Security Auditing

```bash
# Audit dependencies
cargo audit

# Check for security advisories
cargo audit --db advisory-db

# Generate security report
cargo audit --format json > security-report.json
```

## 📦 Dependency Management

### Adding Dependencies

```bash
# Add runtime dependency
cargo add tokio --features full

# Add development dependency
cargo add --dev axum-test

# Add optional dependency
cargo add serde --optional --features derive
```

### Dependency Policies

1. **Minimal Dependencies** - Only add what's necessary
2. **Well-maintained** - Choose actively maintained crates
3. **Security** - Regular audit and updates
4. **Licensing** - Compatible with MIT license

### Updating Dependencies

```bash
# Update all dependencies
cargo update

# Update specific dependency
cargo update -p tokio

# Check for outdated dependencies
cargo outdated
```

## 🚀 Performance Optimization

### Profiling Tools

1. **perf** (Linux)
   ```bash
   sudo perf record target/release/ferragate start
   sudo perf report
   ```

2. **Instruments** (macOS)
   ```bash
   instruments -t "Time Profiler" target/release/ferragate start
   ```

3. **cargo-flamegraph**
   ```bash
   cargo flamegraph --bin ferragate
   ```

### Optimization Techniques

#### Async Optimization

```rust
// Use join! for concurrent operations
let (result1, result2) = tokio::join!(
    async_operation1(),
    async_operation2()
);

// Use buffered streams for batch processing
use futures::stream::StreamExt;
let results: Vec<_> = stream
    .buffer_unordered(10)
    .collect()
    .await;
```

#### Memory Optimization

```rust
// Use string interning for repeated strings
use string_cache::DefaultAtom as Atom;

// Pre-allocate collections when size is known
let mut vec = Vec::with_capacity(expected_size);

// Use Cow for borrowed/owned data
use std::borrow::Cow;
fn process_data(data: Cow<str>) -> String {
    match data {
        Cow::Borrowed(s) => s.to_uppercase(),
        Cow::Owned(s) => s.to_uppercase(),
    }
}
```

## 📚 Documentation

### Code Documentation

```rust
/// Creates a new proxy handler for the given configuration.
///
/// This function sets up the proxy state and HTTP client with appropriate
/// timeouts and connection pooling settings.
///
/// # Arguments
///
/// * `config` - The gateway configuration containing routes and settings
///
/// # Returns
///
/// Returns a `ProxyState` that can be used with the proxy handler.
///
/// # Examples
///
/// ```rust
/// use ferragate::config::GatewayConfig;
/// use ferragate::proxy::ProxyState;
///
/// let config = GatewayConfig::default();
/// let state = ProxyState::new(config);
/// ```
///
/// # Panics
///
/// Panics if the HTTP client cannot be created due to invalid TLS configuration.
pub fn new(config: GatewayConfig) -> Self {
    // Implementation
}
```

### API Documentation

Generate documentation:
```bash
# Generate docs
cargo doc --no-deps

# Generate and open docs
cargo doc --open

# Include private items
cargo doc --document-private-items
```

## 🔧 IDE Configuration

### Rust Analyzer Settings

```json
{
    "rust-analyzer.cargo.loadOutDirsFromCheck": true,
    "rust-analyzer.procMacro.enable": true,
    "rust-analyzer.cargo.runBuildScripts": true,
    "rust-analyzer.checkOnSave.command": "clippy",
    "rust-analyzer.checkOnSave.extraArgs": ["--", "-W", "clippy::all"]
}
```

### Debugging Configuration

```json
{
    "version": "0.2.0",
    "configurations": [
        {
            "type": "lldb",
            "request": "launch",
            "name": "Debug ferragate",
            "cargo": {
                "args": ["build", "--bin=ferragate"],
                "filter": {
                    "name": "ferragate",
                    "kind": "bin"
                }
            },
            "args": ["start", "--config", "gateway.toml"],
            "cwd": "${workspaceFolder}"
        }
    ]
}
```

This comprehensive development guide should help you get up to speed with FerraGate's codebase and development practices!
