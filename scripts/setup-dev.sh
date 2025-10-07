#!/bin/bash
set -e

echo "Setting up Dolphin Remote Gaming System development environment..."

# Check if running on supported OS
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    echo "Detected Linux - setting up Ubuntu development environment"
elif [[ "$OSTYPE" == "darwin"* ]]; then
    echo "Detected macOS - setting up development environment"
else
    echo "Warning: Untested OS detected. Some dependencies may need manual installation."
fi

# Check for Rust installation
if ! command -v rustc &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source ~/.cargo/env
else
    echo "Rust already installed: $(rustc --version)"
fi

# Check for required Rust components
echo "Installing required Rust components..."
rustup component add clippy
rustup component add rustfmt

# Install additional targets for Switch development (if devkitPro is available)
if [ -d "/opt/devkitpro" ]; then
    echo "devkitPro detected - setting up Switch development"
    export DEVKITPRO=/opt/devkitpro
    export DEVKITARM=$DEVKITPRO/devkitARM
    export DEVKITPPC=$DEVKITPRO/devkitPPC

    # Add aarch64-nintendo-switch target if available
    rustup target add aarch64-nintendo-switch-freestanding 2>/dev/null || echo "Switch Rust target not available"
else
    echo "devkitPro not found - Switch development will be limited"
fi

# Install system dependencies based on OS
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    echo "Installing Linux dependencies..."

    # Check if we can use apt (Ubuntu/Debian)
    if command -v apt-get &> /dev/null; then
        sudo apt-get update
        sudo apt-get install -y \
            build-essential \
            pkg-config \
            libssl-dev \
            libgstreamer1.0-dev \
            libgstreamer-plugins-base1.0-dev \
            libx11-dev \
            libasound2-dev
    fi

elif [[ "$OSTYPE" == "darwin"* ]]; then
    echo "Installing macOS dependencies..."

    # Check if Homebrew is available
    if command -v brew &> /dev/null; then
        brew install pkg-config openssl gstreamer
    else
        echo "Homebrew not found - please install dependencies manually"
    fi
fi

# Create .env file if it doesn't exist
if [ ! -f ".env" ]; then
    echo "Creating .env file from template..."
    cp .env.example .env
    echo "Please edit .env file with your Tailscale configuration"
fi

# Check if Tailscale is installed
if ! command -v tailscale &> /dev/null; then
    echo "Warning: Tailscale not installed"
    echo "Please install Tailscale from: https://tailscale.com/download"
else
    echo "Tailscale detected: $(tailscale version)"
fi

# Create necessary directories
mkdir -p logs
mkdir -p releases

echo "Development environment setup complete!"
echo ""
echo "Next steps:"
echo "1. Edit .env file with your configuration"
echo "2. Install Tailscale if not already installed"
echo "3. Run 'cargo build' in server/ directory to test setup"
echo "4. For Switch development, ensure devkitPro is properly installed"