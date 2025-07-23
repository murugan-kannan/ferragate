# Deployment Guide

This guide covers various deployment strategies for FerraGate in production environments.

## 🎯 Deployment Overview

FerraGate can be deployed in multiple ways depending on your infrastructure and requirements:

- **Docker containers** (recommended)
- **Kubernetes** (for orchestration)
- **Systemd service** (bare metal/VM)
- **Binary distribution** (simple deployment)

## 🐳 Docker Deployment

### Single Container Deployment

#### Basic Dockerfile

```dockerfile
FROM rust:1.80 as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/ferragate /usr/local/bin/ferragate
COPY gateway.toml ./

EXPOSE 3000 8443
CMD ["ferragate", "start"]
```

#### Build and Run

```bash
# Build image
docker build -t ferragate:latest .

# Run container
docker run -d \
  --name ferragate \
  -p 3000:3000 \
  -p 8443:8443 \
  -v $(pwd)/gateway.toml:/app/gateway.toml \
  -v $(pwd)/certs:/app/certs \
  -v $(pwd)/logs:/app/logs \
  ferragate:latest
```

### Docker Compose Deployment

#### Complete docker-compose.yml

```yaml
version: '3.8'

services:
  ferragate:
    build: .
    restart: unless-stopped
    ports:
      - "3000:3000"
      - "8443:8443"
    volumes:
      - ./config/gateway.toml:/app/gateway.toml:ro
      - ./certs:/app/certs:ro
      - ./logs:/app/logs
    environment:
      - RUST_LOG=info
      - FERRAGATE_CONFIG=/app/gateway.toml
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 10s
    depends_on:
      - user-service
      - order-service
    networks:
      - ferragate-network

  user-service:
    image: nginx:alpine
    restart: unless-stopped
    ports:
      - "8081:80"
    volumes:
      - ./examples/user-service:/usr/share/nginx/html:ro
    networks:
      - ferragate-network

  order-service:
    image: nginx:alpine
    restart: unless-stopped
    ports:
      - "8082:80"
    volumes:
      - ./examples/order-service:/usr/share/nginx/html:ro
    networks:
      - ferragate-network

networks:
  ferragate-network:
    driver: bridge

volumes:
  logs:
    driver: local
```

#### Start Services

```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f ferragate

# Scale gateway
docker-compose up -d --scale ferragate=3

# Stop services
docker-compose down
```

### Production Docker Setup

#### Multi-stage Production Dockerfile

```dockerfile
# Build stage
FROM rust:1.80-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# Copy source and build
COPY src ./src
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -s /bin/false ferragate

WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/ferragate /usr/local/bin/ferragate

# Create necessary directories
RUN mkdir -p /app/config /app/certs /app/logs \
    && chown -R ferragate:ferragate /app

# Switch to non-root user
USER ferragate

EXPOSE 3000 8443

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:3000/health || exit 1

CMD ["ferragate", "start", "--config", "/app/config/gateway.toml"]
```

## ☸️ Kubernetes Deployment

### Basic Kubernetes Manifests

#### Namespace

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: ferragate
```

#### ConfigMap

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: ferragate-config
  namespace: ferragate
data:
  gateway.toml: |
    [server]
    host = "0.0.0.0"
    port = 3000
    timeout_ms = 30000

    [server.tls]
    enabled = true
    port = 8443
    cert_file = "/app/certs/tls.crt"
    key_file = "/app/certs/tls.key"
    redirect_http = true

    [[routes]]
    path = "/api/v1/users/*"
    upstream = "http://user-service:8080"
    strip_path = true

    [[routes]]
    path = "/api/v1/orders/*"
    upstream = "http://order-service:8080"
    strip_path = true

    [logging]
    level = "info"
    format = "json"
```

#### Secret for TLS

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: ferragate-tls
  namespace: ferragate
type: kubernetes.io/tls
data:
  tls.crt: <base64-encoded-certificate>
  tls.key: <base64-encoded-private-key>
```

#### Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ferragate
  namespace: ferragate
  labels:
    app: ferragate
spec:
  replicas: 3
  selector:
    matchLabels:
      app: ferragate
  template:
    metadata:
      labels:
        app: ferragate
    spec:
      containers:
      - name: ferragate
        image: ferragate:latest
        ports:
        - containerPort: 3000
          name: http
        - containerPort: 8443
          name: https
        env:
        - name: RUST_LOG
          value: "info"
        - name: FERRAGATE_CONFIG
          value: "/app/config/gateway.toml"
        volumeMounts:
        - name: config
          mountPath: /app/config
          readOnly: true
        - name: tls-certs
          mountPath: /app/certs
          readOnly: true
        livenessProbe:
          httpGet:
            path: /health
            port: 3000
          initialDelaySeconds: 10
          periodSeconds: 30
        readinessProbe:
          httpGet:
            path: /ready
            port: 3000
          initialDelaySeconds: 5
          periodSeconds: 10
        resources:
          requests:
            memory: "128Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "500m"
      volumes:
      - name: config
        configMap:
          name: ferragate-config
      - name: tls-certs
        secret:
          secretName: ferragate-tls
```

#### Service

```yaml
apiVersion: v1
kind: Service
metadata:
  name: ferragate-service
  namespace: ferragate
spec:
  selector:
    app: ferragate
  ports:
  - name: http
    port: 80
    targetPort: 3000
  - name: https
    port: 443
    targetPort: 8443
  type: LoadBalancer
```

#### Ingress (Alternative to LoadBalancer)

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: ferragate-ingress
  namespace: ferragate
  annotations:
    kubernetes.io/ingress.class: nginx
    cert-manager.io/cluster-issuer: letsencrypt-prod
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
spec:
  tls:
  - hosts:
    - api.example.com
    secretName: ferragate-tls-secret
  rules:
  - host: api.example.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: ferragate-service
            port:
              number: 80
```

### Deploy to Kubernetes

```bash
# Apply all manifests
kubectl apply -f k8s/

# Check deployment status
kubectl get pods -n ferragate

# View logs
kubectl logs -f deployment/ferragate -n ferragate

# Scale deployment
kubectl scale deployment ferragate --replicas=5 -n ferragate

# Port forward for testing
kubectl port-forward service/ferragate-service 3000:80 -n ferragate
```

### Helm Chart (Advanced)

#### Chart.yaml

```yaml
apiVersion: v2
name: ferragate
description: A Helm chart for FerraGate API Gateway
type: application
version: 0.1.0
appVersion: "0.1.0"
```

#### values.yaml

```yaml
replicaCount: 3

image:
  repository: ferragate
  pullPolicy: IfNotPresent
  tag: "latest"

nameOverride: ""
fullnameOverride: ""

service:
  type: LoadBalancer
  httpPort: 80
  httpsPort: 443

ingress:
  enabled: false
  className: ""
  annotations: {}
  hosts:
    - host: api.example.com
      paths:
        - path: /
          pathType: Prefix
  tls: []

resources:
  limits:
    cpu: 500m
    memory: 512Mi
  requests:
    cpu: 100m
    memory: 128Mi

autoscaling:
  enabled: false
  minReplicas: 2
  maxReplicas: 10
  targetCPUUtilizationPercentage: 80

config:
  server:
    host: "0.0.0.0"
    port: 3000
    timeout_ms: 30000
  routes: []
  logging:
    level: "info"
    format: "json"

tls:
  enabled: true
  port: 8443
  secretName: "ferragate-tls"
```

## 🖥️ Systemd Service Deployment

### Create Service User

```bash
# Create ferragate user
sudo useradd -r -s /bin/false ferragate

# Create directories
sudo mkdir -p /etc/ferragate /var/log/ferragate /opt/ferragate
sudo chown ferragate:ferragate /var/log/ferragate
```

### Install Binary

```bash
# Build or download binary
cargo build --release
sudo cp target/release/ferragate /usr/local/bin/
sudo chmod +x /usr/local/bin/ferragate

# Create configuration
sudo cp gateway.toml /etc/ferragate/
sudo chown root:ferragate /etc/ferragate/gateway.toml
sudo chmod 640 /etc/ferragate/gateway.toml
```

### Systemd Service File

Create `/etc/systemd/system/ferragate.service`:

```ini
[Unit]
Description=FerraGate API Gateway
After=network.target
Wants=network.target

[Service]
Type=simple
User=ferragate
Group=ferragate
ExecStart=/usr/local/bin/ferragate start --config /etc/ferragate/gateway.toml
ExecReload=/bin/kill -HUP $MAINPID
Restart=always
RestartSec=5

# Security settings
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=/var/log/ferragate

# Environment
Environment=RUST_LOG=info

# Limits
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

### Manage Service

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service
sudo systemctl enable ferragate

# Start service
sudo systemctl start ferragate

# Check status
sudo systemctl status ferragate

# View logs
sudo journalctl -u ferragate -f

# Restart service
sudo systemctl restart ferragate
```

## 🔧 Production Configuration

### Environment-Specific Configurations

#### Development

```toml
[server]
host = "127.0.0.1"
port = 3000

[server.tls]
enabled = true
port = 8443
cert_file = "certs/server.crt"
key_file = "certs/server.key"

[logging]
level = "debug"
format = "pretty"
```

#### Staging

```toml
[server]
host = "0.0.0.0"
port = 3000
timeout_ms = 30000

[server.tls]
enabled = true
port = 8443
cert_file = "/etc/ssl/certs/staging.crt"
key_file = "/etc/ssl/private/staging.key"
redirect_http = true

[logging]
level = "info"
format = "json"
file_path = "/var/log/ferragate/gateway.log"
file_rotation = "daily"
max_files = 7
```

#### Production

```toml
[server]
host = "0.0.0.0"
port = 3000
timeout_ms = 30000
workers = 8

[server.tls]
enabled = true
port = 8443
cert_file = "/etc/ssl/certs/production.crt"
key_file = "/etc/ssl/private/production.key"
redirect_http = true

[logging]
level = "warn"
format = "json"
file_path = "/var/log/ferragate/gateway.log"
file_rotation = "daily"
max_files = 30
```

## 🔐 Security Considerations

### TLS/SSL Configuration

#### Certificate Management

```bash
# Production certificates (Let's Encrypt)
certbot certonly --webroot -w /var/www/html -d api.example.com

# Copy certificates
sudo cp /etc/letsencrypt/live/api.example.com/fullchain.pem /etc/ferragate/certs/
sudo cp /etc/letsencrypt/live/api.example.com/privkey.pem /etc/ferragate/certs/
sudo chown ferragate:ferragate /etc/ferragate/certs/*
sudo chmod 600 /etc/ferragate/certs/*
```

#### Certificate Renewal

```bash
# Automatic renewal with systemd timer
sudo cat > /etc/systemd/system/ferragate-cert-renewal.service << EOF
[Unit]
Description=Renew FerraGate TLS certificates
Requires=network.target

[Service]
Type=oneshot
ExecStart=/usr/bin/certbot renew --quiet
ExecStartPost=/bin/systemctl reload ferragate
EOF

sudo cat > /etc/systemd/system/ferragate-cert-renewal.timer << EOF
[Unit]
Description=Run ferragate-cert-renewal twice daily
Requires=ferragate-cert-renewal.service

[Timer]
OnCalendar=*-*-* 00,12:00:00
RandomizedDelaySec=3600
Persistent=true

[Install]
WantedBy=timers.target
EOF

sudo systemctl enable ferragate-cert-renewal.timer
sudo systemctl start ferragate-cert-renewal.timer
```

### Firewall Configuration

```bash
# UFW (Ubuntu)
sudo ufw allow 3000/tcp
sudo ufw allow 8443/tcp
sudo ufw enable

# iptables
sudo iptables -A INPUT -p tcp --dport 3000 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 8443 -j ACCEPT
sudo iptables-save > /etc/iptables/rules.v4
```

### File Permissions

```bash
# Configuration files
sudo chmod 640 /etc/ferragate/gateway.toml
sudo chown root:ferragate /etc/ferragate/gateway.toml

# Certificate files
sudo chmod 600 /etc/ferragate/certs/*
sudo chown ferragate:ferragate /etc/ferragate/certs/*

# Log directory
sudo chmod 755 /var/log/ferragate
sudo chown ferragate:ferragate /var/log/ferragate
```

## 📊 Monitoring and Observability

### Health Checks

```bash
# Simple health check script
#!/bin/bash
# /usr/local/bin/ferragate-health-check.sh

HEALTH_URL="http://localhost:3000/health"
RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" $HEALTH_URL)

if [ $RESPONSE -eq 200 ]; then
    echo "FerraGate is healthy"
    exit 0
else
    echo "FerraGate is unhealthy (HTTP $RESPONSE)"
    exit 1
fi
```

### Prometheus Metrics (Future)

```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'ferragate'
    static_configs:
      - targets: ['localhost:3000']
    metrics_path: '/metrics'
```

### Log Management

#### Logrotate Configuration

```bash
# /etc/logrotate.d/ferragate
/var/log/ferragate/*.log {
    daily
    missingok
    rotate 30
    compress
    delaycompress
    notifempty
    create 644 ferragate ferragate
    postrotate
        systemctl reload ferragate
    endscript
}
```

#### Centralized Logging

```yaml
# filebeat.yml (for ELK stack)
filebeat.inputs:
- type: log
  enabled: true
  paths:
    - /var/log/ferragate/*.log
  json.keys_under_root: true
  json.add_error_key: true

output.elasticsearch:
  hosts: ["elasticsearch:9200"]
  index: "ferragate-%{+yyyy.MM.dd}"
```

## 🔄 High Availability Setup

### Load Balancer Configuration

#### HAProxy

```
# /etc/haproxy/haproxy.cfg
global
    daemon

defaults
    mode http
    timeout connect 5000ms
    timeout client 50000ms
    timeout server 50000ms

frontend ferragate_frontend
    bind *:80
    bind *:443 ssl crt /etc/ssl/certs/ferragate.pem
    redirect scheme https if !{ ssl_fc }
    default_backend ferragate_backend

backend ferragate_backend
    balance roundrobin
    option httpchk GET /health
    server ferragate1 10.0.1.10:3000 check
    server ferragate2 10.0.1.11:3000 check
    server ferragate3 10.0.1.12:3000 check
```

#### Nginx

```nginx
upstream ferragate_backend {
    least_conn;
    server 10.0.1.10:3000 max_fails=3 fail_timeout=30s;
    server 10.0.1.11:3000 max_fails=3 fail_timeout=30s;
    server 10.0.1.12:3000 max_fails=3 fail_timeout=30s;
}

server {
    listen 80;
    listen 443 ssl http2;
    
    ssl_certificate /etc/ssl/certs/ferragate.crt;
    ssl_certificate_key /etc/ssl/private/ferragate.key;
    
    location / {
        proxy_pass http://ferragate_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
    
    location /health {
        access_log off;
        proxy_pass http://ferragate_backend;
    }
}
```

## 🚀 Performance Optimization

### System Tuning

```bash
# Increase file descriptor limits
echo "* soft nofile 65536" >> /etc/security/limits.conf
echo "* hard nofile 65536" >> /etc/security/limits.conf

# Kernel parameters
echo "net.core.somaxconn = 65536" >> /etc/sysctl.conf
echo "net.ipv4.tcp_max_syn_backlog = 65536" >> /etc/sysctl.conf
echo "net.core.netdev_max_backlog = 5000" >> /etc/sysctl.conf
sysctl -p
```

### Container Resource Limits

```yaml
# Kubernetes resource limits
resources:
  requests:
    memory: "256Mi"
    cpu: "200m"
  limits:
    memory: "1Gi"
    cpu: "1000m"
```

### JVM Tuning (if using containerized deployment)

```dockerfile
# Dockerfile optimizations
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1
# Add memory profiling in development
# ENV RUST_LOG=ferragate=debug,tower_http=debug
```

## 📋 Deployment Checklist

### Pre-deployment

- [ ] Configuration validated
- [ ] TLS certificates installed
- [ ] Firewall rules configured
- [ ] Monitoring setup
- [ ] Backup procedures in place
- [ ] Load testing completed

### During deployment

- [ ] Rolling deployment strategy
- [ ] Health checks passing
- [ ] Traffic gradually shifted
- [ ] Metrics monitored
- [ ] Logs checked for errors

### Post-deployment

- [ ] End-to-end testing
- [ ] Performance metrics verified
- [ ] Error rates monitored
- [ ] Security scan completed
- [ ] Documentation updated

This comprehensive deployment guide should help you deploy FerraGate successfully in any environment!
