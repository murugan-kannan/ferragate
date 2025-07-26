# Ferragate Makefile
# Common tasks for development and CI/CD

.PHONY: help build test check fmt clippy audit deny coverage clean install-tools docker-build docker-run release-dry setup-ci

# Default target
help: ## Show this help message
	@echo "Ferragate Development Tasks"
	@echo "=========================="
	@echo ""
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

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

setup-ci: ## Run the CI/CD setup script
	./scripts/setup-cicd.sh

# Docker tasks
docker-build: ## Build Docker image
	docker build -t ferragate:latest .

docker-run: ## Run Docker container
	docker run --rm -p 8080:8080 ferragate:latest

docker-test: ## Test Docker build
	docker build -t ferragate:test .
	docker run --rm ferragate:test --version

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

clean-all: clean ## Clean everything including caches
	rm -rf ~/.cargo/registry/cache
	rm -rf ~/.cargo/git

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

profile: ## Profile the application
	cargo build --release
	perf record --call-graph=dwarf target/release/ferragate --help
	perf report

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
