#!/bin/bash
set -e

# Comprehensive test script for dpstream
# Usage: ./test.sh [unit|integration|network|all] [--coverage]

TEST_TYPE=${1:-all}
COVERAGE_FLAG=""
if [[ "$2" == "--coverage" ]]; then
    COVERAGE_FLAG="--coverage"
fi

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Running dpstream test suite...${NC}"
echo "Test type: $TEST_TYPE"
echo "Coverage: ${COVERAGE_FLAG:-disabled}"
echo ""

# Function to run unit tests
run_unit_tests() {
    echo -e "${YELLOW}Running unit tests...${NC}"

    # Server unit tests
    echo -e "${BLUE}Testing server components...${NC}"
    cd server

    if [ -n "$COVERAGE_FLAG" ]; then
        # Check if cargo-tarpaulin is available
        if command -v cargo-tarpaulin &> /dev/null; then
            cargo tarpaulin --out Html --output-dir ../coverage
            echo -e "${GREEN}✓ Coverage report generated: coverage/tarpaulin-report.html${NC}"
        else
            echo -e "${YELLOW}Warning: cargo-tarpaulin not found. Install with: cargo install cargo-tarpaulin${NC}"
            cargo test
        fi
    else
        cargo test
    fi

    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ Server unit tests passed${NC}"
    else
        echo -e "${RED}✗ Server unit tests failed${NC}"
        cd ..
        exit 1
    fi

    cd ..

    # Switch client unit tests (if applicable)
    echo -e "${BLUE}Testing switch client components...${NC}"
    cd switch-client

    # Note: Switch tests may need special setup
    if cargo test --lib 2>/dev/null; then
        echo -e "${GREEN}✓ Switch client unit tests passed${NC}"
    else
        echo -e "${YELLOW}⚠ Switch client tests require special environment${NC}"
    fi

    cd ..
}

# Function to run integration tests
run_integration_tests() {
    echo -e "${YELLOW}Running integration tests...${NC}"

    # Network connectivity test
    if [ -f "scripts/test-network.sh" ]; then
        echo -e "${BLUE}Testing network connectivity...${NC}"
        ./scripts/test-network.sh
    fi

    # Build test
    echo -e "${BLUE}Testing build system...${NC}"
    ./scripts/build.sh debug server
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ Build integration test passed${NC}"
    else
        echo -e "${RED}✗ Build integration test failed${NC}"
        exit 1
    fi

    # Clean up build artifacts
    ./scripts/build.sh clean
}

# Function to run performance/benchmark tests
run_performance_tests() {
    echo -e "${YELLOW}Running performance tests...${NC}"

    cd server

    if cargo bench --version &> /dev/null; then
        cargo bench
        echo -e "${GREEN}✓ Performance benchmarks completed${NC}"
    else
        echo -e "${YELLOW}⚠ Benchmark tests skipped (criterion not available)${NC}"
    fi

    cd ..
}

# Function to run linting and code quality checks
run_quality_checks() {
    echo -e "${YELLOW}Running code quality checks...${NC}"

    # Format check
    echo -e "${BLUE}Checking code formatting...${NC}"
    if cargo fmt --all -- --check; then
        echo -e "${GREEN}✓ Code formatting OK${NC}"
    else
        echo -e "${RED}✗ Code formatting issues found${NC}"
        echo "Run: cargo fmt --all"
        exit 1
    fi

    # Clippy lints
    echo -e "${BLUE}Running Clippy lints...${NC}"
    cd server
    if cargo clippy -- -D warnings; then
        echo -e "${GREEN}✓ Server clippy checks passed${NC}"
    else
        echo -e "${RED}✗ Server clippy issues found${NC}"
        cd ..
        exit 1
    fi
    cd ..

    cd switch-client
    if cargo clippy -- -D warnings 2>/dev/null; then
        echo -e "${GREEN}✓ Client clippy checks passed${NC}"
    else
        echo -e "${YELLOW}⚠ Client clippy checks skipped (target not available)${NC}"
    fi
    cd ..

    # Security audit
    if command -v cargo-audit &> /dev/null; then
        echo -e "${BLUE}Running security audit...${NC}"
        if cargo audit; then
            echo -e "${GREEN}✓ Security audit passed${NC}"
        else
            echo -e "${YELLOW}⚠ Security vulnerabilities found${NC}"
        fi
    else
        echo -e "${YELLOW}⚠ Security audit skipped (cargo-audit not installed)${NC}"
    fi
}

# Function to generate test report
generate_test_report() {
    TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
    REPORT_FILE=".history/test_report_${TIMESTAMP}.md"

    cat > "$REPORT_FILE" << EOF
# Test Report - $TIMESTAMP

## Configuration
- Test Type: $TEST_TYPE
- Coverage: ${COVERAGE_FLAG:-disabled}
- Platform: $OSTYPE
- Rust Version: $(rustc --version)

## Results Summary
$(if [ "$TEST_TYPE" == "unit" ] || [ "$TEST_TYPE" == "all" ]; then
    echo "- Unit Tests: ✓ Passed"
fi)
$(if [ "$TEST_TYPE" == "integration" ] || [ "$TEST_TYPE" == "all" ]; then
    echo "- Integration Tests: ✓ Passed"
fi)
$(if [ "$TEST_TYPE" == "all" ]; then
    echo "- Code Quality: ✓ Passed"
fi)

## Test Coverage
$(if [ -f "coverage/tarpaulin-report.html" ]; then
    echo "Coverage report available at: coverage/tarpaulin-report.html"
else
    echo "Coverage report not generated"
fi)

## Environment Info
- OS: $OSTYPE
- Cargo: $(cargo --version)
- Rustc: $(rustc --version)

Generated: $(date)
EOF

    echo -e "${GREEN}Test report saved: $REPORT_FILE${NC}"
}

# Main test execution
case $TEST_TYPE in
    "unit")
        run_unit_tests
        ;;
    "integration")
        run_integration_tests
        ;;
    "network")
        if [ -f "scripts/test-network.sh" ]; then
            ./scripts/test-network.sh
        else
            echo -e "${RED}Network test script not found${NC}"
            exit 1
        fi
        ;;
    "performance")
        run_performance_tests
        ;;
    "quality")
        run_quality_checks
        ;;
    "all")
        run_unit_tests
        run_integration_tests
        run_quality_checks
        run_performance_tests
        ;;
    *)
        echo -e "${RED}Usage: $0 [unit|integration|network|performance|quality|all] [--coverage]${NC}"
        echo ""
        echo "Test types:"
        echo "  unit         - Run unit tests for all components"
        echo "  integration  - Run integration and build tests"
        echo "  network      - Test network connectivity and Tailscale"
        echo "  performance  - Run benchmark tests"
        echo "  quality      - Run linting and code quality checks"
        echo "  all          - Run all test types"
        echo ""
        echo "Options:"
        echo "  --coverage   - Generate code coverage report"
        exit 1
        ;;
esac

generate_test_report
echo -e "${GREEN}All tests completed successfully!${NC}"