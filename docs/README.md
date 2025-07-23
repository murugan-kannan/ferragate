# FerraGate Documentation

Welcome to the comprehensive documentation for FerraGate, the high-performance API Gateway built in Rust.

## 📚 Documentation Index

### 🚀 Getting Started
- **[Getting Started Guide](GETTING_STARTED.md)** - Quick start guide to get FerraGate running in minutes
- **[Installation](../README.md#installation)** - Installation options and requirements

### 🔧 Configuration and Setup  
- **[Configuration Guide](CONFIGURATION.md)** - Complete reference for all configuration options
- **[HTTPS/TLS Guide](HTTPS_GUIDE.md)** - Setting up HTTPS with certificates
- **[HTTPS Implementation](HTTPS_IMPLEMENTATION.md)** - Technical details of HTTPS support

### 📖 Reference Documentation
- **[API Reference](API_REFERENCE.md)** - Complete CLI and HTTP API reference
- **[Architecture](ARCHITECTURE.md)** - System architecture and design principles

### 🚀 Deployment
- **[Deployment Guide](DEPLOYMENT.md)** - Production deployment strategies (Docker, Kubernetes, systemd)

### 👨‍💻 Development
- **[Development Guide](DEVELOPMENT.md)** - For developers working on FerraGate
- **[Contributing Guide](CONTRIBUTING.md)** - How to contribute to the project

### 🔍 Support
- **[Troubleshooting Guide](TROUBLESHOOTING.md)** - Common issues and solutions

## 🎯 Quick Navigation

### For New Users
1. Start with [Getting Started](GETTING_STARTED.md)
2. Learn about [Configuration](CONFIGURATION.md)
3. Set up [HTTPS](HTTPS_GUIDE.md)

### For Production Deployment
1. Review [Architecture](ARCHITECTURE.md)
2. Follow [Deployment Guide](DEPLOYMENT.md)
3. Set up monitoring and logging

### For Developers
1. Read [Development Guide](DEVELOPMENT.md)
2. Check [Contributing Guidelines](CONTRIBUTING.md)
3. Explore the codebase

### For Troubleshooting
1. Check [Troubleshooting Guide](TROUBLESHOOTING.md)
2. Review configuration with [Configuration Guide](CONFIGURATION.md)
3. Use [API Reference](API_REFERENCE.md) for debugging

## 📋 Document Overview

| Document | Purpose | Audience |
|----------|---------|----------|
| [Getting Started](GETTING_STARTED.md) | Quick setup and first gateway | New users |
| [Configuration](CONFIGURATION.md) | Complete config reference | All users |
| [HTTPS Guide](HTTPS_GUIDE.md) | TLS/SSL setup | System administrators |
| [API Reference](API_REFERENCE.md) | CLI and HTTP API docs | Developers, DevOps |
| [Architecture](ARCHITECTURE.md) | System design and components | Developers, Architects |
| [Deployment](DEPLOYMENT.md) | Production deployment | DevOps, System administrators |
| [Development](DEVELOPMENT.md) | Codebase and development setup | Contributors |
| [Contributing](CONTRIBUTING.md) | Contribution guidelines | Contributors |
| [Troubleshooting](TROUBLESHOOTING.md) | Problem diagnosis and fixes | All users |

## 🚀 What is FerraGate?

FerraGate is a modern, high-performance API Gateway built in Rust that provides:

- ⚡ **High Performance** - Handle millions of requests per second
- 🔒 **TLS Termination** - Full HTTPS support with certificate management
- 🛣️ **Smart Routing** - Flexible path matching and request forwarding
- 📊 **Observability** - Comprehensive logging and monitoring
- 🐳 **Cloud Native** - Docker and Kubernetes ready
- 🔧 **Easy Configuration** - Simple TOML-based configuration

## 🎯 Common Use Cases

### API Gateway
Route requests from clients to backend microservices:
```toml
[[routes]]
path = "/api/users/*"
upstream = "http://user-service:8080"

[[routes]]
path = "/api/orders/*"
upstream = "http://order-service:8080"
```

### TLS Termination
Handle SSL/TLS encryption for backend services:
```toml
[server.tls]
enabled = true
port = 8443
cert_file = "certs/server.crt"
key_file = "certs/server.key"
redirect_http = true
```

### Load Balancing (Future)
Distribute requests across multiple backend instances:
```toml
[[routes]]
path = "/api/*"
upstream = ["http://backend1:8080", "http://backend2:8080"]
load_balancing = "round_robin"
```

## 🏗️ Architecture Overview

FerraGate follows a modular, high-performance architecture:

```
Client Request → HTTP/HTTPS Listener → Router → Proxy Handler → Upstream Service
                                        ↓
                              Middleware Stack (Auth, Rate Limiting, etc.)
```

Key components:
- **HTTP Server** - Async Axum-based server with HTTP/2 support
- **Router** - Fast path matching and request routing
- **Proxy Engine** - Efficient request forwarding with connection pooling
- **TLS Handler** - Modern TLS implementation with certificate management
- **Configuration** - TOML-based configuration with validation

## 🚀 Quick Start Example

1. **Install FerraGate**
   ```bash
   cargo install ferragate
   ```

2. **Create Configuration**
   ```bash
   ferragate init
   ```

3. **Start Gateway**
   ```bash
   ferragate start
   ```

4. **Test**
   ```bash
   curl http://localhost:3000/health
   ```

## 📈 Performance Characteristics

- **Throughput**: 100K+ requests/second on modern hardware
- **Latency**: Sub-millisecond overhead for proxied requests
- **Memory**: Low memory footprint with efficient connection pooling
- **Concurrency**: Async/await architecture with work-stealing scheduler

## 🔒 Security Features

- **TLS 1.2+** support with modern cipher suites
- **Certificate management** with automatic generation
- **Input validation** and sanitization
- **Secure defaults** for all configuration options
- **Regular security audits** of dependencies

## 🌍 Community and Support

- 📖 **Documentation**: Comprehensive guides and references
- 💬 **Discord**: [FerraGate Community](https://discord.gg/zECWRRgW)
- 🐛 **Issues**: [GitHub Issues](https://github.com/murugan-kannan/ferragate/issues)
- 💡 **Discussions**: [GitHub Discussions](https://github.com/murugan-kannan/ferragate/discussions)
- 📧 **Email**: contact@ferragate.dev

## 🛣️ Roadmap

### Current (v0.1.x)
- ✅ Basic HTTP/HTTPS proxy
- ✅ TLS termination
- ✅ Configuration management
- ✅ Health checks
- ✅ Structured logging

### Near Term (v0.2.x)
- 🔄 Rate limiting
- 🔄 Authentication (JWT, API keys)
- 🔄 Load balancing
- 🔄 Metrics and monitoring
- 🔄 WebSocket support

### Future (v1.0+)
- 🔮 Plugin system (WASM)
- 🔮 Multi-tenancy
- 🔮 Circuit breaker
- 🔮 Caching layer
- 🔮 Admin API

## 📜 License

FerraGate is open source software licensed under the [MIT License](../LICENSE).

## 🤝 Contributing

We welcome contributions! See our [Contributing Guide](CONTRIBUTING.md) for details on:

- Code contributions
- Documentation improvements
- Bug reports
- Feature requests
- Community support

## 📞 Getting Help

If you need help:

1. **Check the documentation** - Most questions are answered here
2. **Search GitHub issues** - Your question might already be answered
3. **Join our Discord** - Get help from the community
4. **Create an issue** - For bugs or feature requests
5. **Contact us** - For commercial support or private questions

---

**Ready to get started?** Head to the [Getting Started Guide](GETTING_STARTED.md) to build your first API gateway with FerraGate! 🚀
