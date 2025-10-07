# Build Guide - Dolphin Remote Gaming System

This guide covers building the dpstream project from source on different platforms.

## Prerequisites

### Common Requirements

- **Rust 1.70+**: Install from [rustup.rs](https://rustup.rs/)
- **Git**: Version control
- **Tailscale**: VPN networking (install from [tailscale.com](https://tailscale.com/download))

### Platform-Specific Requirements

#### Ubuntu 24.04 (Server)

```bash
sudo apt update
sudo apt install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    libx11-dev \
    libasound2-dev \
    dolphin-emu
```

#### macOS (Development)

```bash
# Install Homebrew if not already installed
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install dependencies
brew install pkg-config openssl gstreamer
```

#### Nintendo Switch Development

```bash
# Install devkitPro
# Download and run installer from: https://github.com/devkitPro/installer/releases

# Set environment variables (add to ~/.bashrc or ~/.zshrc)
export DEVKITPRO=/opt/devkitpro
export DEVKITARM=$DEVKITPRO/devkitARM
export DEVKITPPC=$DEVKITPRO/devkitPPC
export PATH=$DEVKITPRO/tools/bin:$PATH

# Install Switch development packages
sudo dkp-pacman -S switch-dev
```

## Building the Project

### Automated Setup

```bash
# Clone repository
git clone git@github.com:hephaex/dpstream.git
cd dpstream

# Run setup script
./scripts/setup-dev.sh

# Configure environment
cp .env.example .env
# Edit .env with your configuration
```

### Manual Build Steps

#### Server (Ubuntu)

```bash
cd server

# Development build
cargo build

# Release build (optimized)
cargo build --release

# With all features (requires system libraries)
cargo build --features full --release

# Run tests
cargo test
```

#### Switch Client

```bash
cd switch-client

# Ensure devkitPro environment is set
export DEVKITPRO=/opt/devkitpro
export DEVKITARM=$DEVKITPRO/devkitARM

# Build NRO file (if Makefile exists)
make

# Or build with Cargo (experimental)
cargo build --target aarch64-nintendo-switch-freestanding --release
```

### Using Build Scripts

The project includes automated build scripts:

```bash
# Build server only (debug)
./scripts/build.sh debug server

# Build server only (release)
./scripts/build.sh release server

# Build Switch client
./scripts/build.sh release client

# Build everything
./scripts/build.sh release all

# Run tests
./scripts/build.sh test
```

## Build Configurations

### Server Features

The server supports different feature configurations:

```toml
[features]
default = []                    # Minimal build (development)
full = [                       # Full build (production)
    "gstreamer",
    "gstreamer-app",
    "gstreamer-video",
    "nix",
    "x11"
]
```

Build with specific features:
```bash
# Minimal build (no system dependencies)
cargo build

# Full build (all features)
cargo build --features full

# Custom feature set
cargo build --features "gstreamer,x11"
```

### Switch Client Configurations

```toml
[profile.release]
lto = true              # Link-time optimization
opt-level = "z"         # Size optimization
codegen-units = 1       # Single compilation unit
panic = "abort"         # Abort on panic (no unwinding)

[target.aarch64-nintendo-switch-freestanding]
rustflags = ["-C", "target-cpu=cortex-a57"]  # Tegra X1 optimization
```

## Cross-Compilation

### Building Server on macOS for Ubuntu

```bash
# Add Linux target
rustup target add x86_64-unknown-linux-gnu

# Install cross-compilation toolchain
brew install FiloSottile/musl-cross/musl-cross

# Build for Linux
cargo build --target x86_64-unknown-linux-gnu --release
```

### Building Switch Client on Ubuntu

```bash
# Install ARM64 toolchain
sudo apt install gcc-aarch64-linux-gnu

# Add Switch Rust target (if available)
rustup target add aarch64-nintendo-switch-freestanding

# Build Switch client
cargo build --target aarch64-nintendo-switch-freestanding --release
```

## Development Workflow

### Daily Development

```bash
# Pull latest changes
git pull origin main

# Check code quality
cargo fmt --check         # Format check
cargo clippy -- -D warnings  # Lint check

# Build and test
cargo build
cargo test

# Commit changes
git add .
git commit -m "feat: add new feature"
./scripts/git-workflow.sh backup "Daily progress"
```

### Testing

#### Unit Tests
```bash
# Run all tests
cargo test

# Run specific test module
cargo test network::vpn

# Run with output
cargo test -- --nocapture
```

#### Integration Tests
```bash
# End-to-end testing
./scripts/test-integration.sh

# Network testing
./scripts/test-network.sh
```

#### Performance Benchmarks
```bash
# Run benchmarks
cargo bench

# Specific benchmark
cargo bench --bench streaming_performance
```

### Code Quality

#### Formatting
```bash
# Format code
cargo fmt

# Check formatting without applying
cargo fmt -- --check
```

#### Linting
```bash
# Run Clippy lints
cargo clippy

# Strict linting (fail on warnings)
cargo clippy -- -D warnings

# Fix automatically fixable issues
cargo clippy --fix
```

#### Security Audit
```bash
# Install cargo-audit
cargo install cargo-audit

# Run security audit
cargo audit
```

## Troubleshooting

### Common Build Issues

#### Missing System Libraries (Linux)

```bash
# Error: could not find system library 'gstreamer-1.0'
sudo apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev

# Error: could not find system library 'x11'
sudo apt install libx11-dev

# Error: linker 'cc' not found
sudo apt install build-essential
```

#### devkitPro Issues (Switch)

```bash
# Error: devkitPro not found
export DEVKITPRO=/opt/devkitpro
export PATH=$DEVKITPRO/tools/bin:$PATH

# Error: switch-dev package not found
sudo dkp-pacman -S switch-dev

# Error: aarch64-none-elf-gcc not found
export DEVKITARM=$DEVKITPRO/devkitARM
export PATH=$DEVKITARM/bin:$PATH
```

#### Rust Target Issues

```bash
# Error: target 'aarch64-nintendo-switch-freestanding' not found
# This target may not be available in stable Rust yet
# Use nightly or wait for official support

# Workaround: use generic ARM64 target
rustup target add aarch64-unknown-none
```

### Performance Issues

#### Slow Compilation
```bash
# Use parallel compilation
export CARGO_BUILD_JOBS=8

# Use faster linker (Linux)
sudo apt install lld
export RUSTFLAGS="-C link-arg=-fuse-ld=lld"

# Use faster linker (macOS)
brew install llvm
export RUSTFLAGS="-C link-arg=-fuse-ld=/usr/local/opt/llvm/bin/ld64.lld"
```

#### Large Binary Size
```bash
# Strip debug symbols
cargo build --release
strip target/release/dpstream-server

# Optimize for size
export RUSTFLAGS="-C opt-level=z -C lto=fat -C codegen-units=1"
cargo build --release
```

## Continuous Integration

### GitHub Actions Configuration

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache dependencies
        uses: actions/cache@v3
        with:
          path: ~/.cargo
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Install system dependencies
        run: |
          sudo apt update
          sudo apt install -y libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev

      - name: Format check
        run: cargo fmt -- --check

      - name: Lint check
        run: cargo clippy -- -D warnings

      - name: Build
        run: cargo build --verbose

      - name: Test
        run: cargo test --verbose
```

## Deployment

### Server Deployment

```bash
# Build optimized release
cargo build --release --features full

# Create systemd service
sudo cp scripts/dpstream.service /etc/systemd/system/
sudo systemctl enable dpstream
sudo systemctl start dpstream
```

### Switch Client Installation

```bash
# Copy to SD card
mkdir -p /path/to/sd/switch/dpstream/
cp switch-client/dpstream-client.nro /path/to/sd/switch/dpstream/
cp switch-client/icon.jpg /path/to/sd/switch/dpstream/
```

## References

- [Rust Book](https://doc.rust-lang.org/book/)
- [Cargo Book](https://doc.rust-lang.org/cargo/)
- [devkitPro Documentation](https://devkitpro.org/wiki/Getting_Started)
- [Switch Homebrew Development](https://switchbrew.org/wiki/Homebrew_Development)