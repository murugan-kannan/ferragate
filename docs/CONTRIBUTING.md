# Contributing to FerraGate

We're excited that you're interested in contributing to FerraGate! This guide will help you get started with contributing to the project.

## 🎯 Ways to Contribute

There are many ways to contribute to FerraGate:

- 🐛 **Report bugs** - Help us identify and fix issues
- 💡 **Suggest features** - Share ideas for new functionality
- 📖 **Improve documentation** - Help make our docs better
- 🛠️ **Submit code** - Fix bugs or implement new features
- 🧪 **Write tests** - Improve our test coverage
- 🎨 **Improve UX** - Help with CLI design and usability
- 📢 **Spread the word** - Blog, tweet, or talk about FerraGate

## 🚀 Getting Started

### Prerequisites

- **Rust 1.80+** - Install from [rustup.rs](https://rustup.rs/)
- **Git** - For version control
- **Docker** (optional) - For testing containerized deployments

### Setting Up Development Environment

1. **Fork the repository**
   ```bash
   # Click "Fork" on GitHub, then clone your fork
   git clone https://github.com/YOUR_USERNAME/ferragate.git
   cd ferragate
   ```

2. **Install dependencies**
   ```bash
   # Rust dependencies are managed by Cargo
   cargo check
   ```

3. **Run tests**
   ```bash
   cargo test
   ```

4. **Start development server**
   ```bash
   cargo run -- start
   ```

5. **Format code**
   ```bash
   cargo fmt
   ```

6. **Run linter**
   ```bash
   cargo clippy
   ```

## 🏗️ Development Workflow

### 1. Create a Branch

```bash
# Create a new branch for your feature/fix
git checkout -b feature/your-feature-name

# Or for bug fixes
git checkout -b fix/issue-description
```

### 2. Make Changes

- Follow the [coding standards](#coding-standards)
- Write tests for new functionality
- Update documentation if needed
- Ensure all tests pass

### 3. Commit Changes

We use [Conventional Commits](https://www.conventionalcommits.org/) for commit messages:

```bash
# Feature
git commit -m "feat: add rate limiting middleware"

# Bug fix
git commit -m "fix: resolve memory leak in proxy handler"

# Documentation
git commit -m "docs: update configuration guide"

# Tests
git commit -m "test: add integration tests for TLS"

# Refactor
git commit -m "refactor: improve error handling in config module"
```

### 4. Push and Create PR

```bash
# Push your branch
git push origin feature/your-feature-name

# Create a Pull Request on GitHub
```

## 📋 Pull Request Guidelines

### Before Submitting

- [ ] Tests pass (`cargo test`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] Linter passes (`cargo clippy`)
- [ ] Documentation is updated
- [ ] Changelog is updated (for significant changes)

### PR Title and Description

Use a clear, descriptive title:
- ✅ "feat: add JWT authentication middleware"
- ✅ "fix: resolve connection pool exhaustion"
- ❌ "update code"
- ❌ "bug fix"

Include in the description:
- What changed and why
- How to test the changes
- Any breaking changes
- Related issues (use "Fixes #123")

### Example PR Description

```markdown
## Summary
Add JWT authentication middleware to support OAuth2 bearer tokens.

## Changes
- Add `jwt` module with token validation
- Update `proxy.rs` to call auth middleware
- Add configuration options for JWT settings
- Include comprehensive tests

## Testing
- Unit tests for JWT validation
- Integration tests with sample tokens
- Manual testing with real JWT tokens

## Breaking Changes
None - this is an optional feature.

Fixes #45
```

## 🧪 Testing Guidelines

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run integration tests
cargo test --test integration_tests
```

### Writing Tests

#### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_matching() {
        let route = RouteConfig {
            path: "/api/users/*".to_string(),
            upstream: "http://backend:8080".to_string(),
            methods: vec!["GET".to_string()],
            ..Default::default()
        };

        assert!(route.matches_path("/api/users/123"));
        assert!(!route.matches_path("/api/orders/123"));
    }

    #[tokio::test]
    async fn test_proxy_handler() {
        // Async test example
        let result = proxy_handler(mock_request()).await;
        assert!(result.is_ok());
    }
}
```

#### Integration Tests

```rust
// tests/integration_tests.rs
use ferragate::*;
use axum_test::TestServer;

#[tokio::test]
async fn test_gateway_end_to_end() {
    let config = GatewayConfig::default();
    let app = create_app(config).await;
    let server = TestServer::new(app).unwrap();

    let response = server
        .get("/health")
        .await;

    assert_eq!(response.status_code(), 200);
}
```

### Test Coverage

Aim for high test coverage:
- Unit tests for all public functions
- Integration tests for key user flows
- Edge case testing
- Error condition testing

## 💻 Coding Standards

### Rust Style Guide

Follow the official [Rust Style Guide](https://doc.rust-lang.org/1.0.0/style/). Key points:

#### Formatting

```rust
// Use cargo fmt for automatic formatting
cargo fmt

// Configuration in rustfmt.toml
max_width = 100
tab_spaces = 4
newline_style = "Unix"
```

#### Naming Conventions

```rust
// Types: PascalCase
struct GatewayConfig { }
enum HttpMethod { }

// Functions and variables: snake_case
fn create_proxy_handler() { }
let route_config = RouteConfig::new();

// Constants: SCREAMING_SNAKE_CASE
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

// Modules: snake_case
mod config_parser { }
```

#### Error Handling

```rust
// Use anyhow::Result for functions that can fail
use anyhow::{Result, Context};

fn load_config(path: &str) -> Result<GatewayConfig> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path))?;
    
    toml::from_str(&contents)
        .with_context(|| "Failed to parse configuration")
}

// Use proper error types for library code
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid route pattern: {0}")]
    InvalidRoutePattern(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

#### Documentation

```rust
/// Creates a new proxy handler with the given configuration.
///
/// # Arguments
///
/// * `config` - The gateway configuration
/// * `client` - HTTP client for upstream requests
///
/// # Returns
///
/// Returns a configured proxy handler that can process requests.
///
/// # Examples
///
/// ```rust
/// let config = GatewayConfig::default();
/// let client = reqwest::Client::new();
/// let handler = create_proxy_handler(config, client);
/// ```
pub fn create_proxy_handler(
    config: GatewayConfig,
    client: reqwest::Client,
) -> ProxyHandler {
    // Implementation
}
```

### Code Organization

#### Module Structure

```
src/
├── main.rs          # Binary entry point
├── lib.rs           # Library entry point
├── cli.rs           # CLI argument parsing
├── config.rs        # Configuration management
├── server.rs        # HTTP server setup
├── proxy.rs         # Proxy logic
├── health.rs        # Health check endpoints
├── logging.rs       # Logging configuration
├── tls.rs           # TLS/SSL handling
└── middleware/      # Future middleware modules
    ├── mod.rs
    ├── auth.rs
    ├── rate_limit.rs
    └── metrics.rs
```

#### Import Organization

```rust
// Standard library imports first
use std::collections::HashMap;
use std::time::Duration;

// External crate imports
use anyhow::Result;
use axum::{Router, extract::State};
use serde::{Deserialize, Serialize};
use tokio::time::timeout;
use tracing::{info, warn, error};

// Internal imports last
use crate::config::GatewayConfig;
use crate::proxy::ProxyState;
```

## 📚 Documentation

### Code Documentation

- Use `///` for public API documentation
- Include examples for complex functions
- Document error conditions
- Explain non-obvious behavior

### User Documentation

When adding features, update relevant documentation:

- `README.md` - For major features
- `docs/CONFIGURATION.md` - For new config options
- `docs/API_REFERENCE.md` - For new CLI commands
- `docs/GETTING_STARTED.md` - For user-facing changes

### Documentation Style

- Use clear, concise language
- Include examples
- Explain the "why" not just the "what"
- Use consistent terminology

## 🏷️ Issue Guidelines

### Reporting Bugs

Use the bug report template and include:

- FerraGate version
- Operating system
- Rust version
- Configuration file (sanitized)
- Steps to reproduce
- Expected vs actual behavior
- Relevant logs

### Feature Requests

Use the feature request template and include:

- Clear description of the problem
- Proposed solution
- Alternative solutions considered
- Use cases and examples
- Implementation ideas (if any)

## 🔄 Release Process

### Versioning

We follow [Semantic Versioning](https://semver.org/):

- `MAJOR.MINOR.PATCH`
- Major: Breaking changes
- Minor: New features (backward compatible)
- Patch: Bug fixes (backward compatible)

### Changelog

Update `CHANGELOG.md` for significant changes:

```markdown
## [0.2.0] - 2024-02-01

### Added
- JWT authentication middleware
- Rate limiting support
- Prometheus metrics endpoint

### Changed
- Improved error handling in proxy module
- Updated configuration schema

### Fixed
- Memory leak in connection pool
- TLS certificate reload issue

### Removed
- Deprecated legacy configuration format
```

## 🎯 Areas Needing Help

We especially welcome contributions in these areas:

### High Priority
- [ ] Rate limiting middleware
- [ ] Authentication (JWT, OAuth2, API keys)
- [ ] Metrics and observability
- [ ] Load balancing algorithms
- [ ] WebSocket support

### Medium Priority
- [ ] Plugin system (WASM)
- [ ] Configuration hot-reload
- [ ] Circuit breaker pattern
- [ ] Request/response transformation
- [ ] Caching layer

### Documentation
- [ ] More configuration examples
- [ ] Performance tuning guide
- [ ] Security best practices
- [ ] Migration guides
- [ ] Video tutorials

## 🤝 Community

### Communication

- **GitHub Issues** - Bug reports and feature requests
- **GitHub Discussions** - General questions and ideas
- **Discord** - Real-time chat and support
- **Email** - Security issues and private communication

### Code of Conduct

We are committed to providing a welcoming and inclusive environment. Please read our [Code of Conduct](CODE_OF_CONDUCT.md) before participating.

### Getting Help

If you need help:

1. Check existing documentation
2. Search GitHub issues
3. Ask in GitHub Discussions
4. Join our Discord server
5. Email us for complex questions

## 🏆 Recognition

We value all contributions! Contributors will be:

- Added to the contributors list
- Mentioned in release notes
- Invited to the contributors Discord channel
- Eligible for contributor swag (coming soon!)

## 📝 Legal

By contributing to FerraGate, you agree that your contributions will be licensed under the MIT License.

For significant contributions, you may be asked to sign a Contributor License Agreement (CLA).

## 🎉 Thank You!

Thank you for contributing to FerraGate! Every contribution, no matter how small, makes a difference. Together, we're building something amazing! 🚀

---

**Questions?** Don't hesitate to reach out:
- 📧 Email: contributors@ferragate.dev
- 💬 Discord: [FerraGate Community](https://discord.gg/zECWRRgW)
- 🐛 Issues: [GitHub Issues](https://github.com/murugan-kannan/ferragate/issues)
