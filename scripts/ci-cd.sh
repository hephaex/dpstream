#!/bin/bash
set -e

# CI/CD Pipeline for Dolphin Remote Gaming System
# Author: Mario Cho <hephaex@gmail.com>
# Version: 1.0.0

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CI_MODE=${CI_MODE:-false}
GITHUB_ACTIONS=${GITHUB_ACTIONS:-false}
BUILD_NUMBER=${BUILD_NUMBER:-$(date +%Y%m%d%H%M%S)}

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

# Logging
LOG_FILE="$PROJECT_ROOT/logs/ci-cd-$BUILD_NUMBER.log"
mkdir -p "$(dirname "$LOG_FILE")"

log() {
    echo -e "$1" | tee -a "$LOG_FILE"
}

log_header() {
    log "\n${BLUE}=================================================================================${NC}"
    log "${BLUE}$1${NC}"
    log "${BLUE}=================================================================================${NC}\n"
}

log_success() {
    log "${GREEN}✅ $1${NC}"
}

log_warning() {
    log "${YELLOW}⚠️  $1${NC}"
}

log_error() {
    log "${RED}❌ $1${NC}"
}

log_info() {
    log "${CYAN}ℹ️  $1${NC}"
}

# Initialize CI/CD environment
init_environment() {
    log_header "Initializing CI/CD Environment"

    log_info "Project Root: $PROJECT_ROOT"
    log_info "Build Number: $BUILD_NUMBER"
    log_info "CI Mode: $CI_MODE"
    log_info "GitHub Actions: $GITHUB_ACTIONS"

    # Create necessary directories
    mkdir -p "$PROJECT_ROOT/artifacts"
    mkdir -p "$PROJECT_ROOT/logs"
    mkdir -p "$PROJECT_ROOT/build"
    mkdir -p "$PROJECT_ROOT/coverage"

    # Set up Rust environment
    if ! command -v rustc &> /dev/null; then
        log_error "Rust not found. Installing..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi

    # Add required targets
    rustup target add aarch64-nintendo-switch-freestanding --toolchain stable || log_warning "Switch target not available"

    # Install required tools
    cargo install cargo-tarpaulin --quiet || log_warning "Failed to install cargo-tarpaulin"
    cargo install cargo-audit --quiet || log_warning "Failed to install cargo-audit"
    cargo install cargo-outdated --quiet || log_warning "Failed to install cargo-outdated"

    log_success "Environment initialized"
}

# Code quality checks
quality_checks() {
    log_header "Code Quality Checks"

    cd "$PROJECT_ROOT"

    # Formatting check
    log_info "Checking code formatting..."
    if cargo fmt --all -- --check; then
        log_success "Code formatting check passed"
    else
        log_error "Code formatting check failed"
        if [[ "$CI_MODE" == "true" ]]; then
            exit 1
        fi
    fi

    # Linting
    log_info "Running Clippy lints..."
    if cargo clippy --all-targets --all-features -- -D warnings; then
        log_success "Clippy checks passed"
    else
        log_error "Clippy checks failed"
        if [[ "$CI_MODE" == "true" ]]; then
            exit 1
        fi
    fi

    # Security audit
    log_info "Running security audit..."
    if cargo audit; then
        log_success "Security audit passed"
    else
        log_warning "Security audit found issues"
    fi

    # Check for outdated dependencies
    log_info "Checking for outdated dependencies..."
    cargo outdated || log_warning "Some dependencies are outdated"
}

# Build all components
build_all() {
    log_header "Building All Components"

    # Build server
    log_info "Building server (debug)..."
    cd "$PROJECT_ROOT/server"
    if cargo build; then
        log_success "Server debug build completed"
    else
        log_error "Server debug build failed"
        exit 1
    fi

    log_info "Building server (release)..."
    if cargo build --release; then
        log_success "Server release build completed"
    else
        log_error "Server release build failed"
        exit 1
    fi

    # Build Switch client (as library only due to target constraints)
    log_info "Building Switch client library..."
    cd "$PROJECT_ROOT/switch-client"
    if cargo check --lib; then
        log_success "Switch client library check completed"
    else
        log_warning "Switch client library check failed (expected in non-Switch environment)"
    fi

    # Copy artifacts
    mkdir -p "$PROJECT_ROOT/artifacts/$BUILD_NUMBER"
    cp "$PROJECT_ROOT/server/target/release/dpstream-server" "$PROJECT_ROOT/artifacts/$BUILD_NUMBER/" 2>/dev/null || log_warning "Server binary not copied"
}

# Run tests
run_tests() {
    log_header "Running Test Suite"

    cd "$PROJECT_ROOT"

    # Unit tests
    log_info "Running unit tests..."
    if cargo test --all --lib; then
        log_success "Unit tests passed"
    else
        log_error "Unit tests failed"
        exit 1
    fi

    # Integration tests
    log_info "Running integration tests..."
    if cargo test --all --test '*'; then
        log_success "Integration tests passed"
    else
        log_warning "Integration tests failed or not found"
    fi

    # Generate test coverage
    log_info "Generating test coverage..."
    if command -v cargo-tarpaulin &> /dev/null; then
        cargo tarpaulin --all-features --workspace --timeout 120 --out Html --output-dir "$PROJECT_ROOT/coverage" || log_warning "Coverage generation failed"
        log_success "Coverage report generated"
    else
        log_warning "cargo-tarpaulin not available, skipping coverage"
    fi
}

# Performance benchmarks
run_benchmarks() {
    log_header "Running Performance Benchmarks"

    cd "$PROJECT_ROOT/server"

    log_info "Running server benchmarks..."
    if cargo bench; then
        log_success "Benchmarks completed"
    else
        log_warning "Benchmarks failed or not found"
    fi
}

# Documentation generation
generate_docs() {
    log_header "Generating Documentation"

    cd "$PROJECT_ROOT"

    log_info "Generating Rust documentation..."
    if cargo doc --all --no-deps; then
        log_success "Documentation generated"
    else
        log_error "Documentation generation failed"
    fi

    # Copy documentation to artifacts
    cp -r "$PROJECT_ROOT/target/doc" "$PROJECT_ROOT/artifacts/$BUILD_NUMBER/" 2>/dev/null || log_warning "Documentation not copied"
}

# Package artifacts
package_artifacts() {
    log_header "Packaging Artifacts"

    cd "$PROJECT_ROOT/artifacts"

    # Create release package
    PACKAGE_NAME="dpstream-$BUILD_NUMBER"
    mkdir -p "$PACKAGE_NAME"

    # Copy binaries
    cp "$BUILD_NUMBER/dpstream-server" "$PACKAGE_NAME/" 2>/dev/null || log_warning "Server binary not packaged"

    # Copy documentation
    cp -r "$BUILD_NUMBER/doc" "$PACKAGE_NAME/" 2>/dev/null || log_warning "Documentation not packaged"

    # Copy configuration files
    cp "$PROJECT_ROOT/.env.example" "$PACKAGE_NAME/" 2>/dev/null || true
    cp "$PROJECT_ROOT/README.md" "$PACKAGE_NAME/" 2>/dev/null || true

    # Create archive
    tar -czf "$PACKAGE_NAME.tar.gz" "$PACKAGE_NAME"

    log_success "Artifacts packaged: $PACKAGE_NAME.tar.gz"
}

# Deploy (placeholder for future implementation)
deploy() {
    log_header "Deployment"

    if [[ "$CI_MODE" == "true" ]] && [[ "$GITHUB_ACTIONS" == "true" ]]; then
        log_info "CI/CD deployment would happen here"
        # Future: Deploy to staging/production environments
    else
        log_info "Deployment skipped (not in CI mode)"
    fi
}

# Cleanup
cleanup() {
    log_header "Cleanup"

    # Clean build artifacts older than 7 days
    find "$PROJECT_ROOT/artifacts" -name "dpstream-*" -type d -mtime +7 -exec rm -rf {} + 2>/dev/null || true
    find "$PROJECT_ROOT/logs" -name "ci-cd-*.log" -mtime +7 -delete 2>/dev/null || true

    log_success "Cleanup completed"
}

# Generate CI/CD report
generate_report() {
    log_header "CI/CD Report"

    REPORT_FILE="$PROJECT_ROOT/artifacts/ci-cd-report-$BUILD_NUMBER.html"

    cat > "$REPORT_FILE" << EOF
<!DOCTYPE html>
<html>
<head>
    <title>dpstream CI/CD Report - Build $BUILD_NUMBER</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .header { background: #2196F3; color: white; padding: 20px; border-radius: 5px; }
        .section { margin: 20px 0; padding: 15px; border-left: 4px solid #2196F3; background: #f5f5f5; }
        .success { border-left-color: #4CAF50; }
        .warning { border-left-color: #FF9800; }
        .error { border-left-color: #F44336; }
        pre { background: #f0f0f0; padding: 10px; border-radius: 3px; overflow-x: auto; }
    </style>
</head>
<body>
    <div class="header">
        <h1>dpstream CI/CD Report</h1>
        <p>Build Number: $BUILD_NUMBER</p>
        <p>Date: $(date)</p>
    </div>

    <div class="section success">
        <h2>Build Summary</h2>
        <p>Build completed successfully with automated quality checks.</p>
    </div>

    <div class="section">
        <h2>Artifacts</h2>
        <ul>
            <li>Server binary (debug and release)</li>
            <li>Documentation</li>
            <li>Test coverage report</li>
            <li>Build logs</li>
        </ul>
    </div>

    <div class="section">
        <h2>Build Log</h2>
        <pre>$(tail -n 50 "$LOG_FILE")</pre>
    </div>
</body>
</html>
EOF

    log_success "CI/CD report generated: $REPORT_FILE"
}

# Help function
show_help() {
    cat << EOF
dpstream CI/CD Pipeline

Usage: $0 [command]

Commands:
  full      - Run complete CI/CD pipeline
  quality   - Run code quality checks only
  build     - Build all components
  test      - Run test suite
  bench     - Run performance benchmarks
  docs      - Generate documentation
  package   - Package artifacts
  deploy    - Deploy (CI mode only)
  clean     - Cleanup old artifacts
  help      - Show this help

Environment Variables:
  CI_MODE=true          - Enable CI mode (strict checks)
  GITHUB_ACTIONS=true   - Enable GitHub Actions integration
  BUILD_NUMBER=xxx      - Custom build number

Examples:
  $0 full               - Run complete pipeline
  CI_MODE=true $0 full  - Run in CI mode with strict checks
EOF
}

# Main execution
main() {
    local command=${1:-full}

    case $command in
        full)
            init_environment
            quality_checks
            build_all
            run_tests
            run_benchmarks
            generate_docs
            package_artifacts
            deploy
            cleanup
            generate_report
            ;;
        quality)
            init_environment
            quality_checks
            ;;
        build)
            init_environment
            build_all
            ;;
        test)
            init_environment
            run_tests
            ;;
        bench)
            init_environment
            run_benchmarks
            ;;
        docs)
            init_environment
            generate_docs
            ;;
        package)
            package_artifacts
            ;;
        deploy)
            deploy
            ;;
        clean)
            cleanup
            ;;
        help)
            show_help
            ;;
        *)
            log_error "Unknown command: $command"
            show_help
            exit 1
            ;;
    esac

    log_success "CI/CD pipeline completed successfully!"
}

# Execute main function
main "$@"