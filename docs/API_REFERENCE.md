# API Reference

This document provides a comprehensive reference for FerraGate's HTTP API endpoints and command-line interface.

## 🌐 HTTP API Endpoints

FerraGate exposes several built-in endpoints for monitoring, health checks, and management.

### Health Check Endpoints

#### `GET /health`
Basic health check endpoint that returns the status of the gateway.

**Response:**
```json
{
  "status": "healthy",
  "timestamp": "2024-01-15T10:30:00Z",
  "version": "0.1.0",
  "uptime_seconds": 3600
}
```

**Status Codes:**
- `200 OK`: Gateway is healthy
- `503 Service Unavailable`: Gateway is unhealthy

**Example:**
```bash
curl http://localhost:3000/health
```

#### `GET /ready`
Readiness probe endpoint for Kubernetes deployments.

**Response:**
```json
{
  "status": "ready",
  "checks": {
    "config": "ok",
    "upstreams": "ok"
  }
}
```

**Status Codes:**
- `200 OK`: Gateway is ready to serve traffic
- `503 Service Unavailable`: Gateway is not ready

**Example:**
```bash
curl http://localhost:3000/ready
```

### Management Endpoints (Future)

#### `GET /metrics`
Prometheus-compatible metrics endpoint.

**Response:**
```
# HELP ferragate_requests_total Total number of requests
# TYPE ferragate_requests_total counter
ferragate_requests_total{method="GET",status="200"} 1234

# HELP ferragate_request_duration_seconds Request duration
# TYPE ferragate_request_duration_seconds histogram
ferragate_request_duration_seconds_bucket{le="0.1"} 100
ferragate_request_duration_seconds_bucket{le="0.5"} 200
```

#### `GET /config`
Current configuration endpoint (admin only).

#### `POST /reload`
Reload configuration without restart (admin only).

## 📝 HTTP Headers

### Request Headers

FerraGate forwards most client headers to upstream services. Some headers are handled specially:

#### Standard Headers
- `Host`: Preserved and forwarded
- `User-Agent`: Preserved and forwarded
- `Authorization`: Preserved and forwarded
- `Content-Type`: Preserved and forwarded
- `Content-Length`: Preserved and forwarded

#### Gateway Headers
FerraGate may add these headers to upstream requests:

- `X-Forwarded-For`: Client IP address
- `X-Forwarded-Proto`: Original protocol (http/https)
- `X-Forwarded-Host`: Original host header
- `X-Gateway`: Always set to "FerraGate"
- `X-Request-ID`: Unique request identifier

### Response Headers

FerraGate may add these headers to client responses:

- `X-Gateway`: Always set to "FerraGate"
- `X-Response-Time`: Request processing time in milliseconds
- `Server`: Set to "FerraGate/0.1.0"

### Custom Headers

You can add custom headers in the configuration:

```toml
[[routes]]
path = "/api/*"
upstream = "http://backend:8080"

[routes.headers]
"X-API-Version" = "v1"
"X-Environment" = "production"
```

## 🖥️ Command Line Interface

### Main Command

```bash
ferragate [SUBCOMMAND] [OPTIONS]
```

### Subcommands

#### `start`
Start the gateway server.

```bash
ferragate start [OPTIONS]
```

**Options:**
- `-c, --config <FILE>`: Configuration file path (default: `gateway.toml`)
- `-p, --port <PORT>`: Override server port
- `--host <HOST>`: Override server host

**Examples:**
```bash
# Start with default configuration
ferragate start

# Start with custom configuration
ferragate start --config production.toml

# Override port
ferragate start --port 4000

# Override host and port
ferragate start --host 127.0.0.1 --port 4000
```

#### `validate`
Validate configuration file without starting the server.

```bash
ferragate validate [OPTIONS]
```

**Options:**
- `-c, --config <FILE>`: Configuration file path (default: `gateway.toml`)

**Examples:**
```bash
# Validate default configuration
ferragate validate

# Validate specific configuration
ferragate validate --config production.toml
```

**Exit Codes:**
- `0`: Configuration is valid
- `1`: Configuration is invalid

#### `init`
Generate example configuration file.

```bash
ferragate init [OPTIONS]
```

**Options:**
- `-o, --output <FILE>`: Output file path (default: `gateway.toml`)
- `--force`: Overwrite existing file

**Examples:**
```bash
# Generate default configuration
ferragate init

# Generate configuration to specific file
ferragate init --output example.toml

# Overwrite existing file
ferragate init --force
```

#### `gen-certs`
Generate self-signed TLS certificates for development.

```bash
ferragate gen-certs [OPTIONS]
```

**Options:**
- `--hostname <HOST>`: Hostname for certificate (default: `localhost`)
- `--output-dir <DIR>`: Output directory (default: `certs/`)
- `--key-size <SIZE>`: Private key size in bits (default: `2048`)
- `--days <DAYS>`: Certificate validity in days (default: `365`)
- `--force`: Overwrite existing certificates

**Examples:**
```bash
# Generate certificates for localhost
ferragate gen-certs

# Generate certificates for custom domain
ferragate gen-certs --hostname api.example.com

# Generate certificates in custom directory
ferragate gen-certs --output-dir /etc/ssl/certs

# Generate with custom validity period
ferragate gen-certs --days 730 --key-size 4096
```

### Global Options

**Common options available for all subcommands:**
- `-h, --help`: Show help information
- `-V, --version`: Show version information
- `-v, --verbose`: Enable verbose output
- `-q, --quiet`: Suppress non-error output

### Environment Variables

The CLI respects these environment variables:

- `FERRAGATE_CONFIG`: Default configuration file path
- `FERRAGATE_HOST`: Default server host
- `FERRAGATE_PORT`: Default server port
- `RUST_LOG`: Log level (trace, debug, info, warn, error)

**Examples:**
```bash
# Set default configuration
export FERRAGATE_CONFIG=/etc/ferragate/gateway.toml

# Set log level
export RUST_LOG=debug

# Start with environment variables
ferragate start
```

## 🔌 Proxy Behavior

### Request Processing

1. **Route Matching**: Incoming requests are matched against configured routes in order
2. **Path Processing**: Paths may be stripped or modified based on `strip_path` setting
3. **Header Processing**: Custom headers are added, standard headers are forwarded
4. **Upstream Request**: Request is forwarded to the matched upstream service
5. **Response Processing**: Upstream response is processed and returned to client

### Path Handling

#### Without `strip_path` (default)
```toml
[[routes]]
path = "/api/users"
upstream = "http://backend:8080"
strip_path = false
```

Client request: `GET /api/users/123`
Upstream request: `GET /api/users/123`

#### With `strip_path`
```toml
[[routes]]
path = "/api/users"
upstream = "http://backend:8080"
strip_path = true
```

Client request: `GET /api/users/123`
Upstream request: `GET /123`

### Error Handling

FerraGate handles various error scenarios:

#### Upstream Errors
- **Connection Refused**: Returns `502 Bad Gateway`
- **Timeout**: Returns `504 Gateway Timeout`
- **DNS Resolution**: Returns `502 Bad Gateway`

#### Client Errors
- **No Route Match**: Returns `404 Not Found`
- **Method Not Allowed**: Returns `405 Method Not Allowed`
- **Request Too Large**: Returns `413 Payload Too Large`

#### Gateway Errors
- **Configuration Error**: Returns `500 Internal Server Error`
- **Resource Exhaustion**: Returns `503 Service Unavailable`

### Timeout Behavior

Timeouts can be configured at multiple levels:

1. **Global Timeout**: Default for all requests
2. **Route Timeout**: Override for specific routes
3. **Upstream Timeout**: Connection and read timeouts

```toml
[server]
timeout_ms = 30000  # 30 seconds global default

[[routes]]
path = "/api/slow"
upstream = "http://slow-service:8080"
timeout_ms = 60000  # 60 seconds for this route
```

## 📊 Response Codes

FerraGate returns standard HTTP status codes:

### Success Codes
- `200 OK`: Successful request
- `201 Created`: Resource created
- `204 No Content`: Successful request with no body

### Client Error Codes
- `400 Bad Request`: Invalid request format
- `401 Unauthorized`: Authentication required
- `403 Forbidden`: Access denied
- `404 Not Found`: Route not found
- `405 Method Not Allowed`: HTTP method not allowed for route
- `413 Payload Too Large`: Request body too large
- `429 Too Many Requests`: Rate limit exceeded (future)

### Server Error Codes
- `500 Internal Server Error`: Gateway internal error
- `502 Bad Gateway`: Upstream connection error
- `503 Service Unavailable`: Gateway overloaded
- `504 Gateway Timeout`: Upstream timeout

## 🔍 Debugging and Troubleshooting

### Debug Headers

Enable debug headers to troubleshoot routing issues:

```bash
# Add debug header to see route matching
curl -H "X-Debug: true" http://localhost:3000/api/users
```

Response will include additional debug information:
```json
{
  "debug": {
    "matched_route": "/api/users/*",
    "upstream": "http://user-service:8080",
    "processing_time_ms": 45
  }
}
```

### Verbose Logging

Enable verbose logging for detailed request information:

```bash
RUST_LOG=debug ferragate start
```

### Health Check Details

The health endpoint provides detailed information when accessed with appropriate headers:

```bash
curl -H "Accept: application/json" -H "X-Detailed: true" http://localhost:3000/health
```

Response:
```json
{
  "status": "healthy",
  "timestamp": "2024-01-15T10:30:00Z",
  "version": "0.1.0",
  "uptime_seconds": 3600,
  "details": {
    "routes_configured": 5,
    "active_connections": 42,
    "memory_usage_mb": 128,
    "upstream_status": {
      "http://user-service:8080": "healthy",
      "http://order-service:8080": "healthy"
    }
  }
}
```

## 📚 Examples

### Basic Proxy Setup

```bash
# 1. Create configuration
ferragate init

# 2. Edit gateway.toml
# 3. Start gateway
ferragate start

# 4. Test endpoint
curl http://localhost:3000/health
```

### HTTPS Setup

```bash
# 1. Generate certificates
ferragate gen-certs --hostname localhost

# 2. Update configuration to enable TLS
# 3. Start gateway
ferragate start

# 4. Test HTTPS
curl -k https://localhost:8443/health
```

### Production Deployment

```bash
# 1. Create production configuration
ferragate init --output production.toml

# 2. Validate configuration
ferragate validate --config production.toml

# 3. Start with production config
ferragate start --config production.toml
```
