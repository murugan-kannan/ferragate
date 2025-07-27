# Ferragate Makefile
# Common tasks for development and CI/CD

.PHONY: help build test check fmt clippy audit deny coverage clean install-tools docker-build docker-run release-dry dev dev-setup health-check config-check logs

# Default target
help: ## Show this help message
	@echo "Ferragate Development Tasks"
	@echo "=========================="
	@echo ""
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

# Quick start for new developers
quick-start: dev-setup build test ## Complete setup and verification for new developers
	@echo ""
	@echo "🎉 Quick start complete!"
	@echo "Your Ferragate development environment is ready!"
	@echo "Run 'make dev' to start the development server"

# Development tasks
build: ## Build the project
	cargo build

build-release: ## Build in release mode
	cargo build --release

test: ## Run all tests
	cargo test --all-features --workspace

test-integration: ## Run integration tests
	cargo test --test integration_tests --all-features

check: ## Run cargo check
	cargo check --all-targets --all-features

fmt: ## Format code
	cargo fmt --all

fmt-check: ## Check code formatting
	cargo fmt --all -- --check

clippy: ## Run clippy lints
	cargo clippy --all-targets --all-features -- -D warnings

# Quality and security checks
audit: ## Run security audit
	cargo audit

deny: ## Run cargo deny checks
	cargo deny check

coverage: ## Generate code coverage report
	cargo llvm-cov --all-features --workspace --html
	@echo "Coverage report generated in target/llvm-cov/html/index.html"

coverage-lcov: ## Generate LCOV coverage report
	cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info

# Documentation
docs: ## Build documentation
	cargo doc --all-features --no-deps --document-private-items

docs-open: ## Build and open documentation
	cargo doc --all-features --no-deps --document-private-items --open

# CI/CD simulation
ci-check: fmt-check clippy test audit deny coverage ## Run all CI checks locally
	@echo "All CI checks passed! ✅"

# Installation and setup
install-tools: ## Install required development tools
	cargo install cargo-audit cargo-deny cargo-llvm-cov cargo-deadlinks

install-targets: ## Install cross-compilation targets
	rustup target add x86_64-unknown-linux-gnu
	rustup target add x86_64-unknown-linux-musl
	rustup target add x86_64-apple-darwin
	rustup target add aarch64-apple-darwin
	rustup target add x86_64-pc-windows-msvc

# Development environment
dev-setup: install-tools install-targets ## Complete development environment setup
	@echo "Development environment setup complete! ✅"
	@echo "Run 'make help' to see available commands"

# Local development server
dev: ## Run in development mode with hot reload
	cargo run

dev-watch: ## Run with file watching for development
	cargo watch -c -x 'run'

# Health and status checks
health-check: ## Quick health check of the application
	@echo "Running health checks..."
	@if cargo build --quiet; then echo "✅ Build: OK"; else echo "❌ Build: FAILED"; fi
	@if cargo test --quiet; then echo "✅ Tests: OK"; else echo "❌ Tests: FAILED"; fi
	@if cargo clippy --quiet -- -D warnings; then echo "✅ Clippy: OK"; else echo "❌ Clippy: FAILED"; fi

# Release tasks
release-dry: ## Perform a dry run release (check what would be published)
	cargo publish --dry-run

release-check: ## Check if ready for release
	@echo "Checking release readiness..."
	@cargo check --release
	@cargo test --release
	@cargo clippy --release -- -D warnings
	@cargo audit
	@cargo deny check
	@echo "Release checks passed! ✅"

# Docker tasks
docker-build: ## Build Docker image
	docker build -t ferragate:latest .

docker-run: ## Run Docker container
	docker run --rm -p 8080:8080 ferragate:latest

docker-test: ## Test Docker build
	docker build -t ferragate:test .
	docker run --rm ferragate:test --version

docker-clean: ## Clean Docker images
	docker rmi ferragate:latest ferragate:test 2>/dev/null || true
	docker image prune -f

# Logging and debugging
logs: ## View application logs
	@if [ -d "logs" ]; then tail -f logs/ferragate.* 2>/dev/null || echo "No log files found"; else echo "Logs directory not found"; fi

logs-clean: ## Clean old log files
	@if [ -d "logs" ]; then find logs -name "*.log" -mtime +7 -delete && echo "Old log files cleaned"; else echo "Logs directory not found"; fi

# Configuration validation
config-check: ## Validate configuration files
	@echo "Validating configuration files..."
	@if [ -f "gateway.toml" ]; then echo "✅ gateway.toml found"; else echo "❌ gateway.toml missing"; fi
	@if [ -f "gateway-https.toml" ]; then echo "✅ gateway-https.toml found"; else echo "❌ gateway-https.toml missing"; fi
	@if [ -f "deny.toml" ]; then echo "✅ deny.toml found"; else echo "❌ deny.toml missing"; fi

# Security and compliance
security-check: audit deny ## Run security checks
	@echo "Security checks completed ✅"

# TLS/SSL certificate management
cert-check: ## Check TLS certificates
	@if [ -f "certs/server.crt" ]; then \
		echo "Certificate expires on:"; \
		openssl x509 -in certs/server.crt -noout -enddate; \
	else \
		echo "❌ Certificate not found at certs/server.crt"; \
	fi

cert-info: ## Display certificate information
	@if [ -f "certs/server.crt" ]; then \
		openssl x509 -in certs/server.crt -text -noout; \
	else \
		echo "❌ Certificate not found at certs/server.crt"; \
	fi

# Benchmarking
bench: ## Run benchmarks
	cargo bench --all-features

bench-baseline: ## Set performance baseline
	cargo bench --all-features -- --save-baseline main

bench-compare: ## Compare against baseline
	cargo bench --all-features -- --baseline main

# Cleaning
clean: ## Clean build artifacts
	cargo clean
	rm -rf target/criterion
	rm -rf target/llvm-cov
	rm -f lcov.info

clean-logs: ## Clean log files
	@if [ -d "logs" ]; then rm -f logs/* && echo "Log files cleaned"; else echo "No logs directory found"; fi

clean-all: clean clean-logs ## Clean everything including caches
	rm -rf ~/.cargo/registry/cache
	rm -rf ~/.cargo/git
	@echo "All artifacts and caches cleaned ✅"

# Platform-specific builds
build-linux: ## Build for Linux (GNU)
	cargo build --release --target x86_64-unknown-linux-gnu

build-linux-musl: ## Build for Linux (musl)
	cargo build --release --target x86_64-unknown-linux-musl

build-macos: ## Build for macOS
	cargo build --release --target x86_64-apple-darwin

build-macos-arm: ## Build for macOS ARM64
	cargo build --release --target aarch64-apple-darwin

build-windows: ## Build for Windows
	cargo build --release --target x86_64-pc-windows-msvc

build-all: build-linux build-linux-musl build-macos build-macos-arm build-windows ## Build for all platforms

# Utility tasks
watch: ## Watch for changes and rebuild
	cargo watch -c -x check -x test

watch-tests: ## Watch for changes and run tests
	cargo watch -c -x test

watch-run: ## Watch for changes and restart application
	cargo watch -c -x 'run'

profile: ## Profile the application
	cargo build --release
	@echo "Profiling requires 'perf' on Linux. Run manually: perf record --call-graph=dwarf target/release/ferragate"

# Performance and load testing
load-test: ## Run basic load test (requires 'ab' - Apache Bench)
	@if command -v ab >/dev/null 2>&1; then \
		echo "Running load test on http://localhost:8080/health..."; \
		ab -n 1000 -c 10 http://localhost:8080/health; \
	else \
		echo "❌ Apache Bench (ab) not found. Install with: brew install httpie"; \
	fi

stress-test: ## Run stress test (requires wrk)
	@if command -v wrk >/dev/null 2>&1; then \
		echo "Running stress test on http://localhost:8080/health..."; \
		wrk -t12 -c400 -d30s http://localhost:8080/health; \
	else \
		echo "❌ wrk not found. Install with: brew install wrk"; \
	fi

# Version management
version-patch: ## Bump patch version
	@current=$$(cargo pkgid | cut -d'#' -f2); \
	echo "Current version: $$current"; \
	echo "This would create a patch release. Use 'git tag v\$$new_version' to tag."

version-minor: ## Bump minor version
	@current=$$(cargo pkgid | cut -d'#' -f2); \
	echo "Current version: $$current"; \
	echo "This would create a minor release. Update Cargo.toml manually."

version-major: ## Bump major version
	@current=$$(cargo pkgid | cut -d'#' -f2); \
	echo "Current version: $$current"; \
	echo "This would create a major release. Update Cargo.toml manually."
