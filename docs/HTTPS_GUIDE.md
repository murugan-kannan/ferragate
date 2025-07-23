# HTTPS/TLS Configuration Guide

FerraGate now supports full HTTPS termination with TLS/SSL certificates. This guide covers how to configure and use HTTPS features.

## 🔒 HTTPS Features

### Core HTTPS Functionality
- ✅ **TLS/SSL Server Support**: Full HTTPS listener support
- ✅ **Certificate Management**: Support for custom certificates and auto-generated self-signed certificates
- ✅ **HTTP to HTTPS Redirect**: Automatic redirection from HTTP to HTTPS
- ✅ **Dual Protocol Support**: Run both HTTP and HTTPS simultaneously
- ✅ **HTTP/2 Support**: Automatic HTTP/2 support over HTTPS
- ✅ **Self-signed Certificate Generation**: Built-in certificate generation for development

## 🚀 Quick HTTPS Setup

### 1. Generate Self-signed Certificates (Development)
```bash
# Generate certificates for localhost
ferragate gen-certs --hostname localhost

# Generate certificates for custom domain
ferragate gen-certs --hostname example.com --output-dir /etc/ssl/certs
```

### 2. Configure HTTPS in gateway.toml
```toml
[server]
host = "0.0.0.0"
port = 3000        # HTTP port
timeout_ms = 30000

[server.tls]
enabled = true
port = 443                      # HTTPS port (use 8443 for non-privileged)
cert_file = "certs/server.crt"  # Path to certificate file
key_file = "certs/server.key"   # Path to private key file
redirect_http = true            # Redirect HTTP to HTTPS
```

### 3. Start the Server
```bash
# Start with HTTPS enabled
ferragate start --config gateway.toml

# The server will run on both:
# HTTP:  http://localhost:3000  (redirects to HTTPS)
# HTTPS: https://localhost:443
```

## 🛠️ Configuration Options

### TLS Configuration
```toml
[server.tls]
enabled = true          # Enable/disable HTTPS
port = 443             # HTTPS port (default: 443)
cert_file = "path/to/certificate.crt"
key_file = "path/to/private.key"
redirect_http = true   # Redirect HTTP to HTTPS (default: false)
```

### Certificate File Formats
- **Certificate file**: PEM format (.crt, .pem)
- **Private key file**: PEM format (.key, .pem)

## 🔧 Certificate Management

### Using Let's Encrypt Certificates
```bash
# 1. Obtain certificates with certbot
sudo certbot certonly --standalone -d yourdomain.com

# 2. Configure FerraGate
[server.tls]
enabled = true
cert_file = "/etc/letsencrypt/live/yourdomain.com/fullchain.pem"
key_file = "/etc/letsencrypt/live/yourdomain.com/privkey.pem"
```

### Using Custom Certificates
```bash
# Place your certificates in the certs directory
mkdir certs
cp your_certificate.crt certs/server.crt
cp your_private_key.key certs/server.key

# Update configuration
[server.tls]
enabled = true
cert_file = "certs/server.crt"
key_file = "certs/server.key"
```

## 🧪 Testing HTTPS

### Test HTTP Redirect
```bash
# Should return 308 redirect to HTTPS
curl -v http://localhost:3000/health
```

### Test HTTPS Endpoint
```bash
# Test with self-signed certificate (ignore SSL warnings)
curl -k https://localhost:8443/health

# Test proxy functionality
curl -k https://localhost:8443/get/anything
```

### Test POST Requests over HTTPS
```bash
curl -k -X POST \
  -H "Content-Type: application/json" \
  -d '{"test": "data"}' \
  https://localhost:8443/post/anything
```

## 🔐 Security Best Practices

### Production Deployment
1. **Use Valid Certificates**: Always use certificates from a trusted CA in production
2. **Secure Private Keys**: Restrict file permissions on private key files (600)
3. **Enable HTTP Redirect**: Always redirect HTTP to HTTPS in production
4. **Use Strong Ciphers**: Modern TLS versions are automatically enabled

### File Permissions
```bash
# Secure certificate files
chmod 644 certs/server.crt
chmod 600 certs/server.key
chown ferragate:ferragate certs/*
```

## 🐛 Troubleshooting

### Common Issues
- **Certificate not found**: Use `ferragate gen-certs` to generate self-signed certificates
- **Permission denied**: Check file permissions on certificate files
- **Port in use**: Change the HTTPS port in configuration or stop conflicting services
- **Certificate validation errors**: Use `-k` flag with curl for self-signed certificates

### Logs
HTTPS-related logs will show:
```
INFO ferragate::tls: TLS configuration loaded successfully
INFO ferragate::server: 🔒 HTTPS server running on https://0.0.0.0:8443
INFO ferragate::server: 🔀 HTTP to HTTPS redirect enabled
```

## 📊 Performance Notes

- HTTPS adds minimal overhead with modern TLS implementations
- HTTP/2 is automatically enabled over HTTPS for better performance
- Connection pooling works with both HTTP and HTTPS upstreams
- TLS handshake is optimized with session resumption
