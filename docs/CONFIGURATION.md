# Configuration Guide

This guide covers all configuration options available in FerraGate and how to use them effectively.

## 📁 Configuration File Format

FerraGate uses TOML format for configuration files. The default configuration file is `gateway.toml`.

## 🔧 Complete Configuration Reference

### Basic Structure

```toml
[server]
# Server configuration

[[routes]]
# Route definitions (can have multiple)

[logging]
# Logging configuration

[server.tls]
# TLS/HTTPS configuration (optional)
```

## 🖥️ Server Configuration

The `[server]` section defines basic server settings:

```toml
[server]
host = "0.0.0.0"           # Bind address (default: "0.0.0.0")
port = 3000                # HTTP port (default: 3000)
workers = 4                # Number of worker threads (optional)
timeout_ms = 30000         # Request timeout in milliseconds (optional)
```

### Server Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `host` | String | `"0.0.0.0"` | IP address to bind the server |
| `port` | Integer | `3000` | Port for HTTP traffic |
| `workers` | Integer | CPU cores | Number of worker threads |
| `timeout_ms` | Integer | `30000` | Request timeout in milliseconds |

## 🔒 TLS/HTTPS Configuration

Enable HTTPS by adding a `[server.tls]` section:

```toml
[server.tls]
enabled = true                    # Enable HTTPS
port = 8443                      # HTTPS port (default: 8443)
cert_file = "certs/server.crt"   # Path to certificate file
key_file = "certs/server.key"    # Path to private key file
redirect_http = true             # Redirect HTTP to HTTPS (default: false)
```

### TLS Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable HTTPS support |
| `port` | Integer | `8443` | Port for HTTPS traffic |
| `cert_file` | String | Required | Path to TLS certificate file |
| `key_file` | String | Required | Path to TLS private key file |
| `redirect_http` | Boolean | `false` | Redirect HTTP requests to HTTPS |

## 🛣️ Route Configuration

Routes are defined as an array of `[[routes]]` sections:

```toml
[[routes]]
path = "/api/users"              # Path pattern to match
upstream = "http://user-service:8080"  # Upstream service URL
methods = ["GET", "POST"]        # Allowed HTTP methods (optional)
strip_path = false               # Strip matched path from upstream request
timeout_ms = 15000              # Route-specific timeout (optional)

# Custom headers to add to upstream requests
[routes.headers]
"X-Gateway" = "FerraGate"
"X-Version" = "1.0"

[[routes]]
path = "/api/orders/*"           # Wildcard matching
upstream = "http://order-service:8080"
methods = ["GET", "POST", "PUT", "DELETE"]
strip_path = true                # Remove "/api/orders" from upstream path
```

### Route Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `path` | String | Required | Path pattern to match (supports wildcards) |
| `upstream` | String | Required | Upstream service URL |
| `methods` | Array | All methods | Allowed HTTP methods |
| `strip_path` | Boolean | `false` | Remove matched path from upstream request |
| `timeout_ms` | Integer | Server default | Route-specific timeout |
| `headers` | Object | `{}` | Custom headers to add to upstream requests |

### Path Matching

FerraGate supports several path matching patterns:

```toml
# Exact match
[[routes]]
path = "/api/health"
upstream = "http://health-service:8080"

# Prefix match with wildcard
[[routes]]
path = "/api/users/*"
upstream = "http://user-service:8080"

# Multi-level wildcard
[[routes]]
path = "/static/**"
upstream = "http://static-files:8080"

# Path parameters (future feature)
[[routes]]
path = "/api/users/{id}"
upstream = "http://user-service:8080"
```

## 📝 Logging Configuration

Configure logging behavior with the `[logging]` section:

```toml
[logging]
level = "info"                   # Log level: trace, debug, info, warn, error
format = "json"                  # Log format: json, pretty, compact
file_path = "logs/gateway.log"   # Log file path (optional)
file_rotation = "daily"          # Rotation: daily, hourly, size (optional)
max_file_size = "100MB"         # Max file size for size rotation (optional)
max_files = 7                   # Number of rotated files to keep (optional)
```

### Logging Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `level` | String | `"info"` | Minimum log level |
| `format` | String | `"pretty"` | Log output format |
| `file_path` | String | None | Path to log file (logs to stdout if not set) |
| `file_rotation` | String | `"daily"` | File rotation strategy |
| `max_file_size` | String | `"100MB"` | Maximum file size before rotation |
| `max_files` | Integer | `7` | Number of rotated files to keep |

### Log Levels

- `trace`: Very detailed debugging information
- `debug`: Debugging information
- `info`: General information about operations
- `warn`: Warning messages
- `error`: Error messages only

### Log Formats

- `json`: Structured JSON format (recommended for production)
- `pretty`: Human-readable format with colors
- `compact`: Compact single-line format

## 📋 Complete Configuration Examples

### Basic HTTP Gateway

```toml
[server]
host = "0.0.0.0"
port = 3000

[[routes]]
path = "/api/users/*"
upstream = "http://user-service:8080"
methods = ["GET", "POST", "PUT", "DELETE"]

[[routes]]
path = "/api/orders/*"
upstream = "http://order-service:8080"

[[routes]]
path = "/health"
upstream = "http://health-service:8080"
methods = ["GET"]

[logging]
level = "info"
format = "pretty"
```

### Production HTTPS Gateway

```toml
[server]
host = "0.0.0.0"
port = 3000
timeout_ms = 30000

[server.tls]
enabled = true
port = 8443
cert_file = "/etc/ssl/certs/gateway.crt"
key_file = "/etc/ssl/private/gateway.key"
redirect_http = true

[[routes]]
path = "/api/v1/users/*"
upstream = "http://user-service:8080"
methods = ["GET", "POST", "PUT", "DELETE"]
strip_path = true
timeout_ms = 15000

[routes.headers]
"X-Gateway" = "FerraGate"
"X-API-Version" = "v1"

[[routes]]
path = "/api/v1/orders/*"
upstream = "http://order-service:8080"
strip_path = true

[routes.headers]
"X-Gateway" = "FerraGate"
"X-API-Version" = "v1"

[[routes]]
path = "/static/**"
upstream = "http://cdn-service:8080"
methods = ["GET"]

[logging]
level = "info"
format = "json"
file_path = "/var/log/ferragate/gateway.log"
file_rotation = "daily"
max_files = 30
```

### Development Configuration

```toml
[server]
host = "127.0.0.1"
port = 3000

[server.tls]
enabled = true
port = 8443
cert_file = "certs/server.crt"
key_file = "certs/server.key"

[[routes]]
path = "/api/*"
upstream = "http://localhost:8080"
strip_path = true

[[routes]]
path = "/frontend/*"
upstream = "http://localhost:3001"
strip_path = true

[logging]
level = "debug"
format = "pretty"
```

## 🔧 Configuration Validation

Validate your configuration file before starting the server:

```bash
# Validate configuration
ferragate validate --config gateway.toml

# Generate example configuration
ferragate init --output example.toml

# Override configuration with CLI flags
ferragate start --config gateway.toml --port 4000 --host 127.0.0.1
```

## 🌍 Environment Variables

Some configuration options can be overridden with environment variables:

```bash
# Set log level
export RUST_LOG=debug

# Override server port
export FERRAGATE_PORT=4000

# Override server host
export FERRAGATE_HOST=127.0.0.1

# Set custom config file
export FERRAGATE_CONFIG=custom.toml
```

## 🔍 Troubleshooting Configuration

### Common Issues

1. **Invalid TOML syntax**
   ```bash
   ferragate validate --config gateway.toml
   ```

2. **Certificate file not found**
   - Check file paths in `cert_file` and `key_file`
   - Ensure files are readable by the process

3. **Port already in use**
   - Change `port` or `tls.port` values
   - Check for conflicting services

4. **Upstream connection refused**
   - Verify upstream URLs are correct
   - Ensure upstream services are running

### Debugging Tips

1. **Enable debug logging**
   ```toml
   [logging]
   level = "debug"
   ```

2. **Use pretty format for development**
   ```toml
   [logging]
   format = "pretty"
   ```

3. **Test individual routes**
   ```bash
   curl -v http://localhost:3000/api/users
   ```

4. **Check certificate validity**
   ```bash
   openssl x509 -in certs/server.crt -text -noout
   ```

## 📚 Best Practices

### Production Deployment

1. **Use separate configuration files**
   - `gateway-dev.toml` for development
   - `gateway-prod.toml` for production

2. **Enable HTTPS in production**
   ```toml
   [server.tls]
   enabled = true
   redirect_http = true
   ```

3. **Configure appropriate timeouts**
   ```toml
   [server]
   timeout_ms = 30000  # 30 seconds
   ```

4. **Use JSON logging for production**
   ```toml
   [logging]
   format = "json"
   file_path = "/var/log/ferragate/gateway.log"
   ```

5. **Set up log rotation**
   ```toml
   [logging]
   file_rotation = "daily"
   max_files = 30
   ```

### Security Considerations

1. **Protect private keys**
   - Use proper file permissions (600)
   - Store keys securely

2. **Use strong TLS configuration**
   - Keep certificates up to date
   - Use proper certificate chains

3. **Validate upstream URLs**
   - Use HTTPS for upstream services when possible
   - Validate SSL certificates

4. **Monitor and log appropriately**
   - Don't log sensitive data
   - Set up log monitoring and alerting
