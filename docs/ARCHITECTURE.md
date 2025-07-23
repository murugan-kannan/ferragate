# FerraGate Architecture

This document provides a comprehensive overview of FerraGate's architecture, design principles, and component interactions.

## 🏗️ System Architecture

FerraGate is built as a high-performance, async API Gateway using Rust's modern ecosystem. The architecture follows a modular design with clear separation of concerns.

```
┌─────────────────────────────────────────────────────────────┐
│                        Client Requests                      │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│                  HTTP/HTTPS Listener                        │
│                   (Axum Server)                             │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│                 Request Router                              │
│              (Route Matching)                               │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│                Proxy Handler                                │
│              (Request Processing)                           │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│               Upstream Services                             │
│              (Backend APIs)                                 │
└─────────────────────────────────────────────────────────────┘
```

## 🧩 Core Components

### 1. CLI Module (`cli.rs`)
- **Purpose**: Command-line interface and application entry point
- **Responsibilities**:
  - Argument parsing using `clap`
  - Subcommand routing (start, validate, init, gen-certs)
  - Configuration loading and validation
  - Certificate generation

### 2. Configuration Module (`config.rs`)
- **Purpose**: Configuration management and parsing
- **Key Structures**:
  - `GatewayConfig`: Root configuration structure
  - `ServerConfig`: Server-specific settings
  - `RouteConfig`: Route definitions and matching
  - `TlsConfig`: HTTPS/TLS configuration
  - `LoggingConfig`: Logging configuration

### 3. Server Module (`server.rs`)
- **Purpose**: HTTP/HTTPS server setup and management
- **Responsibilities**:
  - Axum application setup
  - Route registration
  - Middleware configuration
  - Server lifecycle management
  - Dual HTTP/HTTPS support

### 4. Proxy Module (`proxy.rs`)
- **Purpose**: Core proxying logic and request handling
- **Key Components**:
  - `ProxyState`: Shared application state
  - `proxy_handler`: Main request handler
  - Route matching algorithm
  - Upstream request forwarding
  - Response processing

### 5. TLS Module (`tls.rs`)
- **Purpose**: TLS/SSL certificate management
- **Responsibilities**:
  - Certificate loading and validation
  - Self-signed certificate generation
  - TLS configuration setup
  - Certificate file management

### 6. Health Module (`health.rs`)
- **Purpose**: Health checking and monitoring endpoints
- **Features**:
  - Health status reporting
  - System metrics
  - Readiness checks

### 7. Logging Module (`logging.rs`)
- **Purpose**: Centralized logging configuration
- **Features**:
  - Structured logging with `tracing`
  - File rotation
  - JSON output support
  - Environment-based configuration

## 🔄 Request Flow

### 1. Request Reception
1. Client sends HTTP/HTTPS request
2. Axum server receives and parses the request
3. Request is passed to the router

### 2. Route Matching
1. Extract path and method from request
2. Iterate through configured routes
3. Find first matching route using:
   - Path pattern matching
   - HTTP method validation
   - Optional header matching

### 3. Request Processing
1. Apply route-specific transformations:
   - Header modifications
   - Path stripping
   - Authentication (future)
   - Rate limiting (future)

### 4. Upstream Forwarding
1. Construct upstream URL
2. Forward request using `reqwest` client
3. Handle connection pooling and timeouts
4. Stream response back to client

### 5. Response Processing
1. Apply response transformations
2. Add gateway-specific headers
3. Log request/response metrics
4. Return response to client

## 🏛️ Design Principles

### 1. Performance First
- **Async/Await**: Full async architecture using Tokio
- **Zero-Copy**: Minimal data copying where possible
- **Connection Pooling**: Efficient upstream connection management
- **Memory Efficiency**: Careful memory management and allocation

### 2. Modularity
- **Separation of Concerns**: Each module has a single responsibility
- **Loose Coupling**: Modules communicate through well-defined interfaces
- **Extensibility**: Easy to add new features and middleware

### 3. Configuration-Driven
- **TOML Configuration**: Human-readable configuration format
- **Runtime Validation**: Configuration validation at startup
- **Hot Reloading**: Support for configuration updates (future)

### 4. Observability
- **Structured Logging**: JSON-formatted logs with context
- **Tracing**: Request tracing across components
- **Metrics**: Performance and health metrics
- **Health Checks**: Built-in health endpoints

## 🔧 Technology Stack

### Core Framework
- **Axum**: Modern, async web framework
- **Tokio**: Async runtime
- **Hyper**: HTTP implementation
- **Tower**: Middleware and service abstractions

### Configuration & CLI
- **Clap**: Command-line argument parsing
- **TOML**: Configuration file format
- **Serde**: Serialization/deserialization

### TLS/Security
- **Rustls**: Pure Rust TLS implementation
- **rcgen**: Certificate generation
- **tokio-rustls**: Async TLS support

### HTTP Client
- **Reqwest**: HTTP client for upstream requests
- **HTTP**: HTTP types and utilities

### Logging & Observability
- **Tracing**: Structured logging and tracing
- **Tracing-subscriber**: Log formatting and output
- **Tracing-appender**: File logging with rotation

## 🚀 Performance Characteristics

### Concurrency Model
- **Async/Await**: Non-blocking I/O operations
- **Thread Pool**: Tokio's work-stealing scheduler
- **Connection Pooling**: Efficient upstream connection reuse

### Memory Management
- **Stack Allocation**: Prefer stack over heap allocation
- **Zero-Copy Streaming**: Stream large responses without buffering
- **Efficient Data Structures**: Use of `Arc` for shared state

### Scalability Features
- **Horizontal Scaling**: Stateless design for easy scaling
- **Resource Limits**: Configurable timeouts and limits
- **Graceful Degradation**: Error handling and circuit breaking

## 🔮 Future Architecture Enhancements

### Plugin System
- **WASM Runtime**: WebAssembly plugin support
- **Dynamic Loading**: Runtime plugin loading
- **Plugin API**: Standardized plugin interface

### Multi-Tenancy
- **Tenant Isolation**: Separate configurations per tenant
- **Resource Quotas**: Per-tenant resource limits
- **Billing Integration**: Usage tracking and billing

### Advanced Features
- **Rate Limiting**: Distributed rate limiting
- **Circuit Breaker**: Upstream failure protection
- **Load Balancing**: Multiple upstream instances
- **Authentication**: JWT, OAuth2, API keys
- **Caching**: Response caching layer

## 📊 Monitoring and Metrics

### Health Endpoints
- `/health`: Basic health check
- `/metrics`: Prometheus-compatible metrics (future)
- `/ready`: Readiness probe for Kubernetes

### Key Metrics (Future)
- Request rate and latency
- Upstream response times
- Error rates by route
- Connection pool statistics
- Memory and CPU usage

## 🔐 Security Considerations

### TLS/SSL
- Modern TLS versions (1.2+)
- Strong cipher suites
- Certificate validation
- HSTS support (future)

### Request Security
- Header sanitization
- Request size limits
- Timeout protection
- Input validation

### Future Security Features
- Rate limiting and DDoS protection
- Web Application Firewall (WAF)
- OAuth2/JWT authentication
- API key management
- Audit logging
