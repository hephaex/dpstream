#!/bin/bash
set -e

# Development Automation Script for dpstream
# Author: Mario Cho <hephaex@gmail.com>
# Purpose: Automate common development tasks

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_DIR="$PROJECT_ROOT/scripts"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

# Utility functions
log_info() { echo -e "${CYAN}ℹ️  $1${NC}"; }
log_success() { echo -e "${GREEN}✅ $1${NC}"; }
log_warning() { echo -e "${YELLOW}⚠️  $1${NC}"; }
log_error() { echo -e "${RED}❌ $1${NC}"; }
log_header() { echo -e "\n${BLUE}==== $1 ====${NC}"; }

# Development environment setup
setup_dev_env() {
    log_header "Setting up development environment"

    # Check for required tools
    check_tool "git" "Git version control"
    check_tool "cargo" "Rust package manager"
    check_tool "rustc" "Rust compiler"

    # Install development tools
    log_info "Installing development tools..."
    cargo install cargo-watch --quiet || log_warning "cargo-watch already installed"
    cargo install cargo-edit --quiet || log_warning "cargo-edit already installed"
    cargo install cargo-expand --quiet || log_warning "cargo-expand already installed"
    cargo install cargo-tree --quiet || log_warning "cargo-tree already installed"

    # Set up git hooks
    setup_git_hooks

    # Create development directories
    mkdir -p "$PROJECT_ROOT/logs"
    mkdir -p "$PROJECT_ROOT/temp"
    mkdir -p "$PROJECT_ROOT/.vscode"

    # Create VS Code settings
    create_vscode_settings

    log_success "Development environment setup complete"
}

# Check if a tool is installed
check_tool() {
    local tool=$1
    local description=$2

    if command -v "$tool" &> /dev/null; then
        log_success "$description found: $(command -v $tool)"
    else
        log_error "$description not found: $tool"
        return 1
    fi
}

# Set up git hooks
setup_git_hooks() {
    log_info "Setting up git hooks..."

    local hooks_dir="$PROJECT_ROOT/.git/hooks"

    # Pre-commit hook
    cat > "$hooks_dir/pre-commit" << 'EOF'
#!/bin/bash
# Pre-commit hook for dpstream

echo "Running pre-commit checks..."

# Check formatting
if ! cargo fmt --all -- --check; then
    echo "❌ Code is not formatted. Run 'cargo fmt' to fix."
    exit 1
fi

# Run clippy
if ! cargo clippy --all-targets -- -D warnings; then
    echo "❌ Clippy found issues. Fix them before committing."
    exit 1
fi

echo "✅ Pre-commit checks passed"
EOF

    chmod +x "$hooks_dir/pre-commit"
    log_success "Git hooks installed"
}

# Create VS Code settings
create_vscode_settings() {
    log_info "Creating VS Code settings..."

    cat > "$PROJECT_ROOT/.vscode/settings.json" << 'EOF'
{
    "rust-analyzer.cargo.features": "all",
    "rust-analyzer.checkOnSave.command": "clippy",
    "rust-analyzer.checkOnSave.allTargets": false,
    "editor.formatOnSave": true,
    "editor.codeActionsOnSave": {
        "source.fixAll": true
    },
    "files.watcherExclude": {
        "**/target/**": true,
        "**/.git/objects/**": true,
        "**/.git/subtree-cache/**": true,
        "**/node_modules/**": true
    }
}
EOF

    cat > "$PROJECT_ROOT/.vscode/extensions.json" << 'EOF'
{
    "recommendations": [
        "rust-lang.rust-analyzer",
        "vadimcn.vscode-lldb",
        "serayuzgur.crates",
        "tamasfe.even-better-toml"
    ]
}
EOF

    log_success "VS Code settings created"
}

# Quick development server start
dev_server() {
    log_header "Starting development server"

    cd "$PROJECT_ROOT/server"

    # Start server with auto-reload
    log_info "Starting server with auto-reload..."
    RUST_LOG=debug cargo watch -x 'run'
}

# Quick testing
quick_test() {
    log_header "Running quick tests"

    cd "$PROJECT_ROOT"

    # Run tests with output
    log_info "Running unit tests..."
    cargo test --lib -- --nocapture

    log_info "Running integration tests..."
    cargo test --test '*' -- --nocapture || log_warning "No integration tests found"
}

# Code formatting and linting
format_and_lint() {
    log_header "Formatting and linting code"

    cd "$PROJECT_ROOT"

    log_info "Formatting code..."
    cargo fmt --all

    log_info "Running Clippy..."
    cargo clippy --all-targets --all-features -- -D warnings

    log_success "Code formatted and linted"
}

# Clean project
clean_project() {
    log_header "Cleaning project"

    cd "$PROJECT_ROOT"

    log_info "Cleaning Cargo artifacts..."
    cargo clean

    log_info "Cleaning temporary files..."
    rm -rf "$PROJECT_ROOT/logs/*"
    rm -rf "$PROJECT_ROOT/temp/*"
    rm -rf "$PROJECT_ROOT/artifacts/*"

    log_success "Project cleaned"
}

# Update dependencies
update_deps() {
    log_header "Updating dependencies"

    cd "$PROJECT_ROOT"

    log_info "Updating Cargo dependencies..."
    cargo update

    log_info "Checking for outdated dependencies..."
    cargo install cargo-outdated --quiet || true
    cargo outdated || log_warning "cargo-outdated not available"

    log_success "Dependencies updated"
}

# Generate project statistics
project_stats() {
    log_header "Project Statistics"

    cd "$PROJECT_ROOT"

    echo "Lines of code:"
    find . -name "*.rs" -not -path "./target/*" | xargs wc -l | tail -1

    echo -e "\nFiles by type:"
    find . -type f -not -path "./target/*" -not -path "./.git/*" | sed 's/.*\.//' | sort | uniq -c | sort -rn

    echo -e "\nDependency tree:"
    cargo tree --depth 1 2>/dev/null || log_warning "cargo tree failed"

    echo -e "\nTest coverage:"
    cargo tarpaulin --skip-clean --timeout 60 2>/dev/null || log_warning "Coverage analysis failed"
}

# Monitor performance
monitor_perf() {
    log_header "Performance Monitoring"

    cd "$PROJECT_ROOT/server"

    log_info "Running benchmarks..."
    cargo bench 2>/dev/null || log_warning "No benchmarks found"

    log_info "Checking binary size..."
    if [[ -f "target/release/dpstream-server" ]]; then
        ls -lh target/release/dpstream-server
    else
        log_warning "Release binary not found. Run 'cargo build --release' first."
    fi
}

# Database operations (if applicable)
db_operations() {
    log_header "Database Operations"

    # Placeholder for future database operations
    log_info "No database operations configured yet"
}

# Release preparation
prepare_release() {
    log_header "Preparing release"

    local version=$1
    if [[ -z "$version" ]]; then
        log_error "Usage: $0 release <version>"
        return 1
    fi

    cd "$PROJECT_ROOT"

    # Update version in Cargo.toml files
    log_info "Updating version to $version..."
    find . -name "Cargo.toml" -not -path "./target/*" -exec sed -i "s/version = \".*\"/version = \"$version\"/" {} \;

    # Run full CI pipeline
    log_info "Running full CI pipeline..."
    "$SCRIPT_DIR/ci-cd.sh" full

    # Create git tag
    log_info "Creating git tag..."
    git add .
    git commit -m "Release version $version" || true
    git tag -a "v$version" -m "Release version $version"

    log_success "Release $version prepared. Push with: git push origin main --tags"
}

# Show development status
dev_status() {
    log_header "Development Status"

    cd "$PROJECT_ROOT"

    echo "Git status:"
    git status --short

    echo -e "\nRecent commits:"
    git log --oneline -5

    echo -e "\nBranch information:"
    git branch -v

    echo -e "\nCurrent workspace:"
    pwd

    echo -e "\nRust toolchain:"
    rustc --version
    cargo --version
}

# Help function
show_help() {
    cat << EOF
dpstream Development Automation

Usage: $0 <command> [arguments]

Commands:
  setup         - Set up development environment
  dev           - Start development server with auto-reload
  test          - Run quick tests
  format        - Format and lint code
  clean         - Clean project artifacts
  update        - Update dependencies
  stats         - Show project statistics
  perf          - Monitor performance
  db            - Database operations
  release <ver> - Prepare release with version
  status        - Show development status
  help          - Show this help

Examples:
  $0 setup                  # Initial development setup
  $0 dev                    # Start development server
  $0 test                   # Quick test run
  $0 release 1.0.0          # Prepare version 1.0.0 release

Development Workflow:
  1. $0 setup               # One-time setup
  2. $0 dev                 # Start coding
  3. $0 test                # Test changes
  4. $0 format              # Format code
  5. git commit             # Commit (hooks will run)
  6. $0 release X.Y.Z       # When ready for release
EOF
}

# Main function
main() {
    local command=${1:-help}

    case $command in
        setup)
            setup_dev_env
            ;;
        dev)
            dev_server
            ;;
        test)
            quick_test
            ;;
        format)
            format_and_lint
            ;;
        clean)
            clean_project
            ;;
        update)
            update_deps
            ;;
        stats)
            project_stats
            ;;
        perf)
            monitor_perf
            ;;
        db)
            db_operations
            ;;
        release)
            prepare_release "$2"
            ;;
        status)
            dev_status
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
}

# Execute main function
main "$@"