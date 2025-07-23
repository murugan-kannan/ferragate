# Getting Started with FerraGate

This guide will help you get up and running with FerraGate in just a few minutes.

## 🚀 Quick Start

### Prerequisites

- **Rust 1.80+** (for building from source)
- **Docker** (for containerized deployment)
- **A backend service** to proxy to (we'll use a simple example)

### Option 1: Using Docker (Recommended)

1. **Clone the repository**
   ```bash
   git clone https://github.com/murugan-kannan/ferragate
   cd ferragate
   ```

2. **Start with Docker Compose**
   ```bash
   docker-compose up -d
   ```

3. **Test the gateway**
   ```bash
   curl http://localhost:3000/health
   ```

### Option 2: Building from Source

1. **Install Rust** (if not already installed)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **Clone and build**
   ```bash
   git clone https://github.com/murugan-kannan/ferragate
   cd ferragate
   cargo build --release
   ```

3. **Run FerraGate**
   ```bash
   ./target/release/ferragate start
   ```

## 🎯 Your First Gateway

Let's create a simple API gateway that forwards requests to a backend service.

### Step 1: Set Up a Test Backend

First, let's create a simple backend service to test with:

```bash
# Start a simple HTTP server (Python example)
python3 -m http.server 8080 --directory /tmp &
echo "Backend server started on port 8080"
```

Or using Node.js:
```bash
npx serve -l 8080 &
```

### Step 2: Create Gateway Configuration

Create a `gateway.toml` file:

```bash
ferragate init
```

This creates a basic configuration. Let's customize it:

```toml
[server]
host = "0.0.0.0"
port = 3000

[[routes]]
path = "/api/*"
upstream = "http://localhost:8080"
methods = ["GET", "POST"]

[[routes]]
path = "/health"
upstream = "http://localhost:8080"
methods = ["GET"]

[logging]
level = "info"
format = "pretty"
```

### Step 3: Start the Gateway

```bash
ferragate start --config gateway.toml
```

You should see output like:
```
2024-01-15T10:30:00.123Z  INFO ferragate: Starting FerraGate v0.1.0
2024-01-15T10:30:00.124Z  INFO ferragate: Server listening on http://0.0.0.0:3000
2024-01-15T10:30:00.124Z  INFO ferragate: Configured 2 routes
```

### Step 4: Test Your Gateway

```bash
# Test health endpoint
curl http://localhost:3000/health

# Test API proxy
curl http://localhost:3000/api/
```

🎉 **Congratulations!** You've successfully set up your first API gateway with FerraGate.

## 🔒 Adding HTTPS

Let's add HTTPS support to secure your gateway.

### Step 1: Generate Development Certificates

```bash
ferragate gen-certs --hostname localhost
```

This creates:
- `certs/server.crt` - TLS certificate
- `certs/server.key` - Private key

### Step 2: Update Configuration

Add TLS configuration to your `gateway.toml`:

```toml
[server]
host = "0.0.0.0"
port = 3000

[server.tls]
enabled = true
port = 8443
cert_file = "certs/server.crt"
key_file = "certs/server.key"
redirect_http = true

[[routes]]
path = "/api/*"
upstream = "http://localhost:8080"

[[routes]]
path = "/health"
upstream = "http://localhost:8080"

[logging]
level = "info"
format = "pretty"
```

### Step 3: Restart the Gateway

```bash
ferragate start --config gateway.toml
```

### Step 4: Test HTTPS

```bash
# Test HTTPS (note the -k flag to ignore self-signed certificate)
curl -k https://localhost:8443/health

# Test HTTP redirect
curl -i http://localhost:3000/health
```

## 🏗️ Real-World Example

Let's create a more realistic setup with multiple services.

### Backend Services Setup

For this example, we'll simulate microservices:

```bash
# User service (port 8081)
mkdir -p /tmp/user-service
echo '{"service": "users", "status": "running"}' > /tmp/user-service/status.json
python3 -m http.server 8081 --directory /tmp/user-service &

# Order service (port 8082)
mkdir -p /tmp/order-service
echo '{"service": "orders", "status": "running"}' > /tmp/order-service/status.json
python3 -m http.server 8082 --directory /tmp/order-service &
```

### Gateway Configuration

Create `production.toml`:

```toml
[server]
host = "0.0.0.0"
port = 3000
timeout_ms = 30000

[server.tls]
enabled = true
port = 8443
cert_file = "certs/server.crt"
key_file = "certs/server.key"
redirect_http = true

# User service routes
[[routes]]
path = "/api/v1/users/*"
upstream = "http://localhost:8081"
methods = ["GET", "POST", "PUT", "DELETE"]
strip_path = true
timeout_ms = 15000

[routes.headers]
"X-Service" = "users"
"X-Gateway" = "FerraGate"

# Order service routes
[[routes]]
path = "/api/v1/orders/*"
upstream = "http://localhost:8082"
methods = ["GET", "POST", "PUT", "DELETE"]
strip_path = true

[routes.headers]
"X-Service" = "orders"
"X-Gateway" = "FerraGate"

# Health check for gateway
[[routes]]
path = "/health"
upstream = "http://localhost:8081"
methods = ["GET"]

[logging]
level = "info"
format = "json"
file_path = "logs/gateway.log"
file_rotation = "daily"
max_files = 7
```

### Start the Production Gateway

```bash
ferragate start --config production.toml
```

### Test the Setup

```bash
# Test user service
curl -k https://localhost:8443/api/v1/users/status.json

# Test order service
curl -k https://localhost:8443/api/v1/orders/status.json

# Test health
curl -k https://localhost:8443/health
```

## 🐳 Docker Deployment

### Using Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  ferragate:
    build: .
    ports:
      - "3000:3000"
      - "8443:8443"
    volumes:
      - ./gateway.toml:/app/gateway.toml
      - ./certs:/app/certs
      - ./logs:/app/logs
    environment:
      - RUST_LOG=info
    depends_on:
      - user-service
      - order-service

  user-service:
    image: nginx:alpine
    ports:
      - "8081:80"
    volumes:
      - ./examples/user-service:/usr/share/nginx/html

  order-service:
    image: nginx:alpine
    ports:
      - "8082:80"
    volumes:
      - ./examples/order-service:/usr/share/nginx/html
```

### Start with Docker Compose

```bash
docker-compose up -d
```

## 🔧 Configuration Validation

Always validate your configuration before deploying:

```bash
# Validate configuration
ferragate validate --config production.toml

# Test configuration with dry run
ferragate start --config production.toml --dry-run
```

## 📊 Monitoring Your Gateway

### Health Checks

FerraGate provides built-in health endpoints:

```bash
# Basic health check
curl http://localhost:3000/health

# Detailed health check
curl -H "X-Detailed: true" http://localhost:3000/health

# Readiness check (for Kubernetes)
curl http://localhost:3000/ready
```

### Logging

Configure structured logging for production:

```toml
[logging]
level = "info"
format = "json"
file_path = "logs/gateway.log"
file_rotation = "daily"
```

View logs:
```bash
# Follow logs
tail -f logs/gateway.log

# View JSON logs with jq
tail -f logs/gateway.log | jq .
```

## 🚨 Troubleshooting

### Common Issues

1. **Port already in use**
   ```bash
   # Check what's using the port
   lsof -i :3000
   
   # Use a different port
   ferragate start --port 4000
   ```

2. **Certificate errors**
   ```bash
   # Regenerate certificates
   ferragate gen-certs --hostname localhost --force
   
   # Check certificate validity
   openssl x509 -in certs/server.crt -text -noout
   ```

3. **Upstream connection refused**
   ```bash
   # Check if upstream service is running
   curl http://localhost:8080/health
   
   # Check configuration
   ferragate validate
   ```

4. **Configuration errors**
   ```bash
   # Validate configuration
   ferragate validate --config gateway.toml
   
   # Check syntax
   toml-check gateway.toml
   ```

### Debug Mode

Enable debug logging for troubleshooting:

```bash
RUST_LOG=debug ferragate start
```

## 📚 Next Steps

Now that you have FerraGate running, explore these advanced topics:

1. **[Configuration Guide](CONFIGURATION.md)** - Deep dive into all configuration options
2. **[HTTPS Guide](HTTPS_GUIDE.md)** - Production HTTPS setup
3. **[API Reference](API_REFERENCE.md)** - Complete API documentation
4. **[Architecture](ARCHITECTURE.md)** - Understanding FerraGate's design

### Production Deployment

For production deployment, consider:

1. **Security**: Use proper TLS certificates from a CA
2. **Monitoring**: Set up log aggregation and metrics
3. **High Availability**: Run multiple instances behind a load balancer
4. **Performance**: Tune timeout and connection settings

### Community and Support

- 📖 [Documentation](https://ferragate.dev/docs)
- 💬 [Discord Community](https://discord.gg/zECWRRgW)
- 🐛 [GitHub Issues](https://github.com/murugan-kannan/ferragate/issues)
- 📧 [Contact](mailto:contact@ferragate.dev)

## 🎯 Quick Reference

### Essential Commands
```bash
# Initialize configuration
ferragate init

# Start gateway
ferragate start

# Generate certificates
ferragate gen-certs

# Validate configuration
ferragate validate

# Help
ferragate --help
```

### Essential Configuration
```toml
[server]
host = "0.0.0.0"
port = 3000

[[routes]]
path = "/api/*"
upstream = "http://backend:8080"

[logging]
level = "info"
```

### Essential Test Commands
```bash
# Health check
curl http://localhost:3000/health

# Test route
curl http://localhost:3000/api/endpoint

# HTTPS test
curl -k https://localhost:8443/health
```

Happy gatewaying with FerraGate! 🚀
