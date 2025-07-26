# CI/CD Documentation

This document describes the continuous integration and deployment setup for Ferragate.

## Overview

The CI/CD pipeline is built using GitHub Actions and consists of several workflows:

- **CI**: Continuous integration for pull requests and main branch
- **Release**: Automated releases with multi-platform binaries and Docker images
- **Nightly**: Nightly builds and security scans

## Workflows

### CI Workflow (`.github/workflows/ci.yml`)

Triggered on:
- Push to `main` or `develop` branches
- Pull requests to `main` or `develop` branches

Jobs:
- **Check**: Basic cargo check
- **Test**: Run tests on multiple platforms (Ubuntu, Windows, macOS) and Rust versions
- **Format**: Check code formatting with rustfmt
- **Clippy**: Lint code with clippy
- **Security Audit**: Run cargo-audit for security vulnerabilities
- **Cargo Deny**: Check licenses and banned dependencies
- **Coverage**: Generate code coverage reports
- **Documentation**: Build and verify documentation
- **Docker Build**: Test Docker image builds
- **Benchmark**: Run performance benchmarks (main branch only)

### Release Workflow (`.github/workflows/release.yml`)

Triggered on:
- Git tags matching `v*.*.*`
- Manual workflow dispatch

Jobs:
- **Create Release**: Create GitHub release
- **Build and Upload**: Build binaries for multiple platforms:
  - Linux (x86_64-gnu, x86_64-musl)
  - macOS (x86_64, aarch64)
  - Windows (x86_64)
- **Docker Release**: Build and push multi-arch Docker images
- **Publish Crate**: Publish to crates.io
- **Deploy Docs**: Deploy documentation to GitHub Pages

### Nightly Workflow (`.github/workflows/nightly.yml`)

Triggered on:
- Daily schedule (2 AM UTC)
- Manual workflow dispatch

Jobs:
- **Nightly Test**: Test with nightly Rust compiler
- **Security Scan**: Run Trivy security scanner
- **Dependency Review**: Check for outdated dependencies
- **Performance Baseline**: Track performance over time

## Required Secrets

The following secrets need to be configured in your GitHub repository:

### For Releases
- `CARGO_REGISTRY_TOKEN`: Token for publishing to crates.io
- `DOCKERHUB_USERNAME`: Docker Hub username (optional)
- `DOCKERHUB_TOKEN`: Docker Hub access token (optional)

## Setting Up Secrets

1. Go to your GitHub repository
2. Navigate to Settings → Secrets and variables → Actions
3. Click "New repository secret"
4. Add each required secret

### Getting a Crates.io Token

1. Go to [crates.io](https://crates.io)
2. Log in with your GitHub account
3. Go to Account Settings → API Tokens
4. Create a new token with publish permissions
5. Add it as `CARGO_REGISTRY_TOKEN` secret

## Features

### Multi-Platform Builds
- Automated builds for Linux, macOS, and Windows
- Both GNU and musl targets for Linux
- ARM64 support for Apple Silicon

### Security
- Automated security audits with cargo-audit
- License and dependency checking with cargo-deny
- Container security scanning with Trivy
- CodeQL analysis for code security

### Code Quality
- Format checking with rustfmt
- Linting with clippy
- Code coverage reporting
- Documentation building and link checking

### Performance Monitoring
- Automated benchmarking
- Performance regression detection
- Historical performance tracking

## Customization

### Modifying Platforms

To add or remove build platforms, edit the `matrix` in `.github/workflows/release.yml`:

```yaml
strategy:
  matrix:
    include:
      - target: x86_64-unknown-linux-gnu
        os: ubuntu-latest
        # ... other configurations
```

### Adding New Checks

To add new CI checks, create a new job in `.github/workflows/ci.yml`:

```yaml
new-check:
  name: New Check
  runs-on: ubuntu-latest
  steps:
    - name: Checkout sources
      uses: actions/checkout@v4
    # ... your check steps
```

### Environment Configuration

If you need deployment capabilities in the future, you can add a deployment workflow that:

1. Deploys to staging and production environments
2. Integrates with cloud providers (AWS, GCP, Azure)
3. Supports Kubernetes or other container orchestration platforms
4. Includes proper health checks and rollback mechanisms

## Troubleshooting

### Common Issues

1. **Build Failures**: Check that all dependencies are properly specified in `Cargo.toml`
2. **Test Failures**: Ensure tests can run in a clean environment without external dependencies
3. **Security Audit Failures**: Review flagged dependencies and update if necessary
4. **Format Failures**: Run `cargo fmt` locally before pushing

### Debugging Workflows

1. Check the Actions tab in your GitHub repository
2. Look at individual job logs for detailed error messages
3. Use `act` tool to run GitHub Actions locally for testing

### Performance Issues

If CI is slow:
1. Review caching configuration
2. Consider parallelizing more jobs
3. Optimize test suite for faster execution

## Maintenance

### Regular Tasks

1. **Update Dependencies**: Dependabot will create PRs for dependency updates
2. **Review Security Advisories**: Monitor and address security issues promptly
3. **Performance Monitoring**: Review benchmark results regularly
4. **Documentation**: Keep CI/CD documentation up to date

### Monitoring

- GitHub Actions usage and billing
- Build performance metrics
- Security scan results
- Dependency health

## Contributing

When contributing to the CI/CD setup:

1. Test changes in a fork first
2. Document any new requirements or secrets
3. Update this documentation for significant changes
4. Consider the impact on build times and costs
