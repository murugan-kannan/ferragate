#!/bin/bash

# Ferragate CI/CD Setup Script
# This script helps set up the CI/CD pipeline for Ferragate

set -e

echo "🚀 Setting up Ferragate CI/CD Pipeline"
echo "======================================"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

# Check if we're in a git repository
if [ ! -d ".git" ]; then
    print_error "This doesn't appear to be a git repository"
    exit 1
fi

# Check if GitHub CLI is installed
if command -v gh &> /dev/null; then
    GH_AVAILABLE=true
    print_status "GitHub CLI is available"
else
    GH_AVAILABLE=false
    print_warning "GitHub CLI not found. You'll need to set up secrets manually"
fi

# Check if the repository has a remote origin
if git remote get-url origin &> /dev/null; then
    REPO_URL=$(git remote get-url origin)
    print_status "Repository remote: $REPO_URL"
else
    print_warning "No remote origin found. Make sure to push to GitHub"
fi

echo ""
echo "📋 Pre-flight Checks"
echo "===================="

# Check for required files
required_files=(
    "Cargo.toml"
    "src/main.rs"
    "src/lib.rs"
    "deny.toml"
    "Dockerfile"
)

for file in "${required_files[@]}"; do
    if [ -f "$file" ]; then
        print_status "$file exists"
    else
        print_error "$file is missing"
        exit 1
    fi
done

# Check Rust installation
if command -v cargo &> /dev/null; then
    RUST_VERSION=$(rustc --version)
    print_status "Rust is installed: $RUST_VERSION"
else
    print_error "Rust is not installed. Please install Rust first"
    exit 1
fi

# Check Docker installation
if command -v docker &> /dev/null; then
    print_status "Docker is available"
else
    print_warning "Docker not found. Docker builds will not work locally"
fi

echo ""
echo "🔧 Setting up required tools"
echo "============================"

# Install cargo tools if not present
install_cargo_tool() {
    local tool=$1
    local package=${2:-$tool}
    
    if cargo install --list | grep -q "^$package "; then
        print_status "$tool is already installed"
    else
        print_info "Installing $tool..."
        cargo install "$package"
        print_status "$tool installed"
    fi
}

# Install required cargo tools
install_cargo_tool "cargo-audit"
install_cargo_tool "cargo-deny"
install_cargo_tool "cargo-llvm-cov"

echo ""
echo "📝 GitHub Repository Setup"
echo "=========================="

if [ "$GH_AVAILABLE" = true ]; then
    print_info "Checking repository settings..."
    
    # Check if already logged in to GitHub CLI
    if gh auth status &> /dev/null; then
        print_status "Authenticated with GitHub CLI"
        
        # Enable GitHub Pages if not already enabled
        print_info "Checking GitHub Pages settings..."
        if gh api "repos/:owner/:repo/pages" &> /dev/null; then
            print_status "GitHub Pages is already configured"
        else
            print_info "GitHub Pages will be configured automatically by the workflow"
        fi
        
        # Check for branch protection (optional)
        print_info "Repository is ready for CI/CD"
    else
        print_warning "Please authenticate with GitHub CLI: gh auth login"
    fi
else
    print_info "Please ensure your repository is pushed to GitHub"
fi

echo ""
echo "🔐 Required Secrets Setup"
echo "========================="

print_info "The following secrets need to be configured in your GitHub repository:"
echo ""
echo "Required for releases:"
echo "  - CARGO_REGISTRY_TOKEN (for crates.io publishing)"
echo ""
echo "Optional for Docker Hub:"
echo "  - DOCKERHUB_USERNAME"
echo "  - DOCKERHUB_TOKEN"

if [ "$GH_AVAILABLE" = true ] && gh auth status &> /dev/null; then
    echo ""
    read -p "Would you like to set up CARGO_REGISTRY_TOKEN now? (y/N): " setup_cargo_token
    
    if [[ $setup_cargo_token =~ ^[Yy]$ ]]; then
        echo ""
        echo "To get a crates.io token:"
        echo "1. Go to https://crates.io/me"
        echo "2. Create a new API token"
        echo "3. Copy the token"
        echo ""
        read -s -p "Enter your crates.io token: " cargo_token
        echo ""
        
        if [ -n "$cargo_token" ]; then
            gh secret set CARGO_REGISTRY_TOKEN --body "$cargo_token"
            print_status "CARGO_REGISTRY_TOKEN has been set"
        else
            print_warning "No token provided, skipping"
        fi
    fi
fi

echo ""
echo "🧪 Testing the Setup"
echo "===================="

print_info "Running basic checks..."

# Test cargo check
if cargo check --quiet; then
    print_status "cargo check passed"
else
    print_error "cargo check failed"
    exit 1
fi

# Test cargo fmt check
if cargo fmt -- --check --quiet &> /dev/null; then
    print_status "Code formatting is correct"
else
    print_warning "Code needs formatting. Run: cargo fmt"
fi

# Test clippy
if cargo clippy --quiet -- -D warnings &> /dev/null; then
    print_status "Clippy checks passed"
else
    print_warning "Clippy found issues. Run: cargo clippy"
fi

# Test cargo deny
if command -v cargo-deny &> /dev/null; then
    if cargo deny check --quiet &> /dev/null; then
        print_status "Cargo deny checks passed"
    else
        print_warning "Cargo deny found issues. Run: cargo deny check"
    fi
fi

echo ""
echo "🎉 Setup Complete!"
echo "=================="

print_status "CI/CD pipeline has been configured"
print_info "Next steps:"
echo "  1. Push your changes to GitHub"
echo "  2. Configure the required secrets in your repository settings"
echo "  3. Create a pull request to test the CI pipeline"
echo "  4. Create a release tag (e.g., v0.1.0) to test the release pipeline"
echo ""
echo "For detailed documentation, see: docs/CI_CD.md"
echo ""
print_info "Happy coding! 🦀"
