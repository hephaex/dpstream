#!/bin/bash
set -e

# Build script for Dolphin Remote Gaming System
# Usage: ./build.sh [debug|release] [server|client|all|test|clean] [--features feature1,feature2]

BUILD_TYPE=${1:-debug}
TARGET=${2:-server}
FEATURES=${3:-}

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Building Dolphin Remote Gaming System...${NC}"
echo "Build type: $BUILD_TYPE"
echo "Target: $TARGET"
echo "Features: ${FEATURES:-default}"
echo ""

# Parse features argument
FEATURES_ARG=""
if [[ "$FEATURES" == --features* ]]; then
    FEATURES_ARG="$FEATURES"
fi

# Build timestamp
BUILD_TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
BUILD_INFO_FILE=".build_info"

# Function to build server
build_server() {
    echo -e "${YELLOW}Building server...${NC}"
    cd server

    # Check for required system dependencies (only on Linux/full build)
    if [[ "$OSTYPE" == "linux-gnu"* ]] && [[ "$FEATURES_ARG" == *"full"* ]]; then
        check_system_deps
    fi

    # Build command
    BUILD_CMD="cargo build"
    if [ "$BUILD_TYPE" = "release" ]; then
        BUILD_CMD="$BUILD_CMD --release"
    fi

    # Add features if specified
    if [ -n "$FEATURES_ARG" ]; then
        BUILD_CMD="$BUILD_CMD $FEATURES_ARG"
    fi

    echo "Executing: $BUILD_CMD"
    if $BUILD_CMD; then
        if [ "$BUILD_TYPE" = "release" ]; then
            echo -e "${GREEN}✓ Server built: target/release/dpstream-server${NC}"
            # Check binary size
            if [ -f "target/release/dpstream-server" ]; then
                SIZE=$(ls -lh target/release/dpstream-server | awk '{print $5}')
                echo "  Binary size: $SIZE"
            fi
        else
            echo -e "${GREEN}✓ Server built: target/debug/dpstream-server${NC}"
        fi
    else
        echo -e "${RED}✗ Server build failed${NC}"
        exit 1
    fi

    cd ..
}

# Function to check system dependencies
check_system_deps() {
    echo -e "${BLUE}Checking system dependencies...${NC}"

    DEPS_MISSING=false

    # Check for GStreamer
    if ! pkg-config --exists gstreamer-1.0; then
        echo -e "${RED}Missing: GStreamer development libraries${NC}"
        DEPS_MISSING=true
    fi

    # Check for X11
    if ! pkg-config --exists x11; then
        echo -e "${RED}Missing: X11 development libraries${NC}"
        DEPS_MISSING=true
    fi

    if [ "$DEPS_MISSING" = true ]; then
        echo -e "${YELLOW}Install missing dependencies:${NC}"
        echo "sudo apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libx11-dev"
        exit 1
    fi

    echo -e "${GREEN}✓ System dependencies OK${NC}"
}

# Function to build client (if devkitPro is available)
build_client() {
    echo "Building Switch client..."

    if [ ! -d "/opt/devkitpro" ]; then
        echo "Error: devkitPro not found. Please install devkitPro for Switch development."
        exit 1
    fi

    export DEVKITPRO=/opt/devkitpro
    export DEVKITARM=$DEVKITPRO/devkitARM
    export DEVKITPPC=$DEVKITPRO/devkitPPC

    cd switch-client

    # Build using Make if Makefile exists
    if [ -f "Makefile" ]; then
        make clean || true
        make
        echo "Switch client built: dpstream-client.nro"
    else
        echo "Building with Cargo (experimental)..."
        if [ "$BUILD_TYPE" = "release" ]; then
            cargo build --release --target aarch64-nintendo-switch-freestanding 2>/dev/null || echo "Switch target not available"
        else
            cargo build --target aarch64-nintendo-switch-freestanding 2>/dev/null || echo "Switch target not available"
        fi
    fi

    cd ..
}

# Function to run tests
run_tests() {
    echo "Running tests..."

    # Test server
    cd server
    cargo test
    cd ..

    # Note: Switch client tests would require special setup
    echo "Tests completed"
}

# Function to clean build artifacts
clean_build() {
    echo -e "${YELLOW}Cleaning build artifacts...${NC}"

    # Clean server
    if [ -d "server/target" ]; then
        cd server && cargo clean && cd ..
        echo -e "${GREEN}✓ Server cleaned${NC}"
    fi

    # Clean client
    if [ -d "switch-client/target" ]; then
        cd switch-client && cargo clean && cd ..
        echo -e "${GREEN}✓ Client cleaned${NC}"
    fi

    # Clean additional artifacts
    find . -name "*.nro" -delete 2>/dev/null || true
    find . -name "*.nacp" -delete 2>/dev/null || true
    find . -name "*.elf" -delete 2>/dev/null || true
    rm -f "$BUILD_INFO_FILE" 2>/dev/null || true

    echo -e "${GREEN}✓ Clean completed${NC}"
}

# Function to save build info
save_build_info() {
    cat > "$BUILD_INFO_FILE" << EOF
Build Information - $BUILD_TIMESTAMP

Configuration:
- Build Type: $BUILD_TYPE
- Target: $TARGET
- Features: ${FEATURES:-default}
- Platform: $OSTYPE
- Rust Version: $(rustc --version)
- Cargo Version: $(cargo --version)

Build Results:
$(find . -name "dpstream-*" -type f -executable 2>/dev/null | head -10)

Generated: $(date)
EOF
}

# Build based on target
case $TARGET in
    "server")
        build_server
        save_build_info
        ;;
    "client")
        build_client
        save_build_info
        ;;
    "all")
        build_server
        build_client
        save_build_info
        ;;
    "test")
        run_tests
        ;;
    "clean")
        clean_build
        ;;
    *)
        echo -e "${RED}Usage: $0 [debug|release] [server|client|all|test|clean] [--features feature1,feature2]${NC}"
        echo ""
        echo "Examples:"
        echo "  ./build.sh debug server"
        echo "  ./build.sh release all --features full"
        echo "  ./build.sh debug server --features streaming,crypto"
        echo "  ./build.sh clean"
        exit 1
        ;;
esac

echo -e "${GREEN}Build completed successfully!${NC}"
if [ -f "$BUILD_INFO_FILE" ]; then
    echo -e "${BLUE}Build info saved to: $BUILD_INFO_FILE${NC}"
fi