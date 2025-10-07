#!/bin/bash
set -e

BUILD_TYPE=${1:-debug}
TARGET=${2:-server}

echo "Building Dolphin Remote Gaming System..."
echo "Build type: $BUILD_TYPE"
echo "Target: $TARGET"

# Function to build server
build_server() {
    echo "Building server..."
    cd server

    if [ "$BUILD_TYPE" = "release" ]; then
        cargo build --release
        echo "Server built: target/release/dpstream-server"
    else
        cargo build
        echo "Server built: target/debug/dpstream-server"
    fi

    cd ..
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

# Build based on target
case $TARGET in
    "server")
        build_server
        ;;
    "client")
        build_client
        ;;
    "all")
        build_server
        build_client
        ;;
    "test")
        run_tests
        ;;
    *)
        echo "Usage: $0 [debug|release] [server|client|all|test]"
        exit 1
        ;;
esac

echo "Build completed successfully!"