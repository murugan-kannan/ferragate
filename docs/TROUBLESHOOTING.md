# Troubleshooting Guide

This guide helps you diagnose and resolve common issues with FerraGate.

## 🔍 Common Issues

### 1. Server Won't Start

#### Port Already in Use

**Symptoms:**
```
Error: Address already in use (os error 98)
```

**Solutions:**
```bash
# Check what's using the port
lsof -i :3000
netstat -tulpn | grep :3000

# Kill the process
kill -9 <PID>

# Or use a different port
ferragate start --port 4000
```

#### Configuration File Not Found

**Symptoms:**
```
Error: No such file or directory (os error 2)
```

**Solutions:**
```bash
# Check file exists
ls -la gateway.toml

# Use absolute path
ferragate start --config /full/path/to/gateway.toml

# Generate default config
ferragate init
```

#### Permission Denied

**Symptoms:**
```
Error: Permission denied (os error 13)
```

**Solutions:**
```bash
# Check file permissions
ls -la gateway.toml

# Fix permissions
chmod 644 gateway.toml

# For privileged ports (< 1024)
sudo ferragate start
# OR use setcap
sudo setcap 'cap_net_bind_service=+ep' /usr/local/bin/ferragate
```

### 2. TLS/HTTPS Issues

#### Certificate File Not Found

**Symptoms:**
```
Error: No such file or directory: certs/server.crt
```

**Solutions:**
```bash
# Generate development certificates
ferragate gen-certs --hostname localhost

# Check certificate files exist
ls -la certs/

# Verify configuration paths
ferragate validate --config gateway.toml
```

#### Certificate Permission Denied

**Symptoms:**
```
Error: Permission denied reading certificate file
```

**Solutions:**
```bash
# Fix certificate permissions
chmod 644 certs/server.crt
chmod 600 certs/server.key

# Check ownership
chown ferragate:ferragate certs/*
```

#### Invalid Certificate

**Symptoms:**
```
Error: Invalid certificate format
```

**Solutions:**
```bash
# Check certificate validity
openssl x509 -in certs/server.crt -text -noout

# Verify certificate and key match
openssl x509 -noout -modulus -in certs/server.crt | openssl md5
openssl rsa -noout -modulus -in certs/server.key | openssl md5

# Regenerate if corrupted
ferragate gen-certs --hostname localhost --force
```

#### SSL Handshake Failed

**Symptoms:**
```
curl: (35) SSL connect error
```

**Solutions:**
```bash
# Test with self-signed certificates
curl -k https://localhost:8443/health

# Check TLS configuration
openssl s_client -connect localhost:8443 -servername localhost

# Verify certificate chain
openssl verify -CAfile ca.crt certs/server.crt
```

### 3. Proxy/Routing Issues

#### 404 Not Found

**Symptoms:**
```
HTTP/1.1 404 Not Found
{"error": "No matching route found"}
```

**Solutions:**
```bash
# Check route configuration
ferragate validate --config gateway.toml

# Test route matching
curl -v http://localhost:3000/exact/path

# Enable debug logging
RUST_LOG=debug ferragate start
```

#### 502 Bad Gateway

**Symptoms:**
```
HTTP/1.1 502 Bad Gateway
{"error": "Upstream connection failed"}
```

**Solutions:**
```bash
# Check upstream service is running
curl http://upstream-host:port/health

# Verify upstream URL in configuration
ping upstream-host
nslookup upstream-host

# Check firewall rules
telnet upstream-host port
```

#### 504 Gateway Timeout

**Symptoms:**
```
HTTP/1.1 504 Gateway Timeout
{"error": "Upstream request timeout"}
```

**Solutions:**
```toml
# Increase timeout in configuration
[server]
timeout_ms = 60000  # 60 seconds

[[routes]]
path = "/slow-endpoint"
upstream = "http://slow-service:8080"
timeout_ms = 120000  # 2 minutes per route
```

#### 405 Method Not Allowed

**Symptoms:**
```
HTTP/1.1 405 Method Not Allowed
```

**Solutions:**
```toml
# Check allowed methods in route configuration
[[routes]]
path = "/api/users"
upstream = "http://user-service:8080"
methods = ["GET", "POST", "PUT", "DELETE"]  # Add required methods
```

### 4. Configuration Issues

#### Invalid TOML Syntax

**Symptoms:**
```
Error: TOML parse error at line 15, column 5
```

**Solutions:**
```bash
# Validate TOML syntax
toml-check gateway.toml

# Use online TOML validator
# Copy config to https://www.toml-lint.com/

# Check for common issues:
# - Missing quotes around strings
# - Incorrect array syntax
# - Invalid table headers
```

#### Missing Required Fields

**Symptoms:**
```
Error: Missing required field 'upstream' in route configuration
```

**Solutions:**
```toml
# Ensure all required fields are present
[[routes]]
path = "/api/users"        # Required
upstream = "http://backend:8080"  # Required
methods = ["GET"]          # Optional but recommended
```

#### Invalid URL Format

**Symptoms:**
```
Error: Invalid upstream URL format
```

**Solutions:**
```toml
# Use proper URL format
upstream = "http://backend:8080"    # ✅ Correct
upstream = "https://api.example.com"  # ✅ Correct
upstream = "backend:8080"           # ❌ Missing scheme
upstream = "http:/backend:8080"     # ❌ Missing slash
```

### 5. Performance Issues

#### High Memory Usage

**Symptoms:**
- Gateway consuming excessive memory
- Out of memory errors

**Diagnosis:**
```bash
# Monitor memory usage
top -p $(pgrep ferragate)
htop

# Check for memory leaks
valgrind --tool=memcheck --leak-check=full ferragate start
```

**Solutions:**
```toml
# Reduce connection pool size
[server]
max_connections = 100

# Set appropriate timeouts
timeout_ms = 30000

# Enable log rotation
[logging]
file_rotation = "daily"
max_files = 7
```

#### High CPU Usage

**Symptoms:**
- Gateway using 100% CPU
- Slow response times

**Diagnosis:**
```bash
# Profile CPU usage
cargo flamegraph --bin ferragate

# Check system load
uptime
iostat 1
```

**Solutions:**
```bash
# Reduce log level
RUST_LOG=warn ferragate start

# Scale horizontally
# Run multiple instances behind load balancer

# Optimize configuration
# Remove unnecessary routes
# Reduce timeout values
```

#### Slow Response Times

**Symptoms:**
- High latency responses
- Timeouts under load

**Diagnosis:**
```bash
# Test response times
curl -w "@curl-format.txt" http://localhost:3000/api/test

# Load testing
ab -n 1000 -c 10 http://localhost:3000/api/test
wrk -t12 -c400 -d30s http://localhost:3000/api/test
```

**Solutions:**
```toml
# Tune connection settings
[server]
workers = 8                # Match CPU cores
timeout_ms = 15000        # Reduce if upstream is fast

# Optimize upstream timeouts
[[routes]]
timeout_ms = 5000         # Per-route optimization
```

## 🔧 Debugging Tools

### 1. Logging

#### Enable Debug Logging
```bash
# Full debug logging
RUST_LOG=debug ferragate start

# Module-specific logging
RUST_LOG=ferragate::proxy=debug ferragate start

# Multiple modules
RUST_LOG=ferragate::proxy=debug,ferragate::config=info ferragate start
```

#### Log Analysis
```bash
# Follow logs in real-time
tail -f logs/gateway.log

# Search for errors
grep "ERROR" logs/gateway.log

# Analyze JSON logs with jq
cat logs/gateway.log | jq 'select(.level == "ERROR")'

# Count error types
cat logs/gateway.log | jq -r '.msg' | sort | uniq -c
```

### 2. Health Checks

#### Basic Health Check
```bash
curl http://localhost:3000/health
```

#### Detailed Health Check
```bash
curl -H "X-Detailed: true" http://localhost:3000/health
```

#### Readiness Check
```bash
curl http://localhost:3000/ready
```

### 3. Configuration Validation

```bash
# Validate configuration file
ferragate validate --config gateway.toml

# Dry run (parse config without starting)
ferragate start --config gateway.toml --dry-run
```

### 4. Network Debugging

#### Test Connectivity
```bash
# Test upstream connectivity
curl -v http://upstream-host:port/

# Test DNS resolution
nslookup upstream-host
dig upstream-host

# Test network path
traceroute upstream-host
mtr upstream-host
```

#### Network Monitoring
```bash
# Monitor network connections
netstat -tuln | grep ferragate
ss -tuln | grep :3000

# Monitor network traffic
tcpdump -i any port 3000
```

### 5. System Monitoring

#### Process Monitoring
```bash
# Process information
ps aux | grep ferragate
pstree -p $(pgrep ferragate)

# Resource usage
top -p $(pgrep ferragate)
htop
```

#### File Descriptors
```bash
# Check open files
lsof -p $(pgrep ferragate)

# File descriptor limits
ulimit -n
cat /proc/$(pgrep ferragate)/limits
```

## 📊 Performance Analysis

### 1. Load Testing

#### Apache Bench (ab)
```bash
# Basic load test
ab -n 1000 -c 10 http://localhost:3000/api/test

# With keep-alive
ab -n 1000 -c 10 -k http://localhost:3000/api/test

# POST requests
ab -n 100 -c 5 -p data.json -T application/json http://localhost:3000/api/users
```

#### wrk
```bash
# Basic test
wrk -t12 -c400 -d30s http://localhost:3000/api/test

# With custom script
wrk -t12 -c400 -d30s -s script.lua http://localhost:3000/
```

#### hey
```bash
# Install hey
go install github.com/rakyll/hey@latest

# Run test
hey -n 1000 -c 50 http://localhost:3000/api/test
```

### 2. Profiling

#### CPU Profiling
```bash
# Install flamegraph
cargo install flamegraph

# Generate flame graph
cargo flamegraph --bin ferragate -- start

# With specific duration
timeout 30s cargo flamegraph --bin ferragate -- start
```

#### Memory Profiling
```bash
# Use heaptrack (Linux)
heaptrack ferragate start

# Use Instruments (macOS)
instruments -t "Allocations" ferragate start
```

### 3. Metrics Collection

#### System Metrics
```bash
# CPU usage over time
sar -u 1 60

# Memory usage
free -h
vmstat 1

# Disk I/O
iostat -x 1

# Network traffic
iftop
nethogs
```

#### Application Metrics
```bash
# HTTP request metrics
curl http://localhost:3000/metrics

# Health status
curl http://localhost:3000/health | jq .
```

## 🛠️ Common Fixes

### 1. Reset Configuration
```bash
# Backup existing config
cp gateway.toml gateway.toml.backup

# Generate fresh config
ferragate init --force

# Restore custom routes
# Edit gateway.toml with your routes
```

### 2. Reset TLS Certificates
```bash
# Remove old certificates
rm -rf certs/

# Generate new certificates
ferragate gen-certs --hostname localhost

# For production, use proper CA certificates
# certbot, acme.sh, or manual certificate installation
```

### 3. Clear Logs
```bash
# Clear log files
rm -f logs/*.log

# Restart with fresh logs
ferragate start
```

### 4. Restart Services
```bash
# Restart gateway
pkill ferragate
ferragate start

# For systemd service
sudo systemctl restart ferragate

# For Docker
docker-compose restart ferragate
```

## 🆘 Getting Help

### 1. Check Documentation
- [Configuration Guide](CONFIGURATION.md)
- [API Reference](API_REFERENCE.md)
- [Getting Started](GETTING_STARTED.md)

### 2. Search Issues
- [GitHub Issues](https://github.com/murugan-kannan/ferragate/issues)
- Check closed issues for solutions

### 3. Community Support
- [Discord Server](https://discord.gg/zECWRRgW)
- [GitHub Discussions](https://github.com/murugan-kannan/ferragate/discussions)

### 4. Report Bugs

When reporting bugs, include:
- FerraGate version (`ferragate --version`)
- Operating system and version
- Rust version (`rustc --version`)
- Configuration file (sanitized)
- Full error message
- Steps to reproduce
- Expected vs actual behavior

### 5. Get Professional Support

For production deployments needing dedicated support:
- Email: support@ferragate.dev
- Commercial support packages available

## 📋 Troubleshooting Checklist

Before asking for help, verify:

- [ ] Configuration file is valid (`ferragate validate`)
- [ ] All required files exist (certificates, config)
- [ ] Correct file permissions
- [ ] Network connectivity to upstream services
- [ ] No port conflicts
- [ ] Sufficient system resources
- [ ] Latest FerraGate version
- [ ] Checked logs for error messages
- [ ] Tried with debug logging enabled
- [ ] Tested with minimal configuration

## 🔧 Advanced Debugging

### 1. Strace/DTrace

#### Linux (strace)
```bash
# Trace system calls
strace -p $(pgrep ferragate) -e trace=network

# Trace file operations
strace -p $(pgrep ferragate) -e trace=file
```

#### macOS (dtruss)
```bash
# Trace system calls
sudo dtruss -p $(pgrep ferragate)
```

### 2. Network Packet Analysis

```bash
# Capture HTTP traffic
tcpdump -i any -A -s 0 port 3000

# Capture HTTPS traffic (encrypted)
tcpdump -i any -s 0 port 8443

# Use Wireshark for detailed analysis
wireshark
```

### 3. Core Dumps

```bash
# Enable core dumps
ulimit -c unlimited

# Analyze core dump
gdb ferragate core
(gdb) bt full
```

This troubleshooting guide should help you resolve most issues you might encounter with FerraGate. If you're still having problems, don't hesitate to reach out to the community!
