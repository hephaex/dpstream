# dpstream Development Guide

## Overview

This guide covers development practices, optimization improvements, and quality standards for the Dolphin Remote Gaming System.

## Project Structure (Optimized)

```
dpstream/
├── server/                     # Ubuntu Server (Rust)
│   ├── src/
│   │   ├── main.rs            # Enhanced with structured logging
│   │   ├── error.rs           # Centralized error handling
│   │   ├── emulator/          # Dolphin integration
│   │   ├── streaming/         # Moonlight host with optimizations
│   │   └── network/           # Enhanced networking with mDNS
│   └── Cargo.toml             # Optimized dependencies with features
│
├── switch-client/              # Nintendo Switch (Rust no-std)
│   ├── src/                   # Optimized for embedded environment
│   └── Cargo.toml             # no-std compatible dependencies
│
├── scripts/                    # Enhanced automation
│   ├── build.sh               # Multi-feature build system
│   ├── test.sh                # Comprehensive testing suite
│   ├── setup-dev.sh           # Environment setup
│   └── git-workflow.sh        # Git automation
│
├── docs/                       # Enhanced documentation
│   ├── architecture/          # System design
│   ├── research/              # Technical analysis
│   └── scripts/               # Developer guides
│
└── .history/                   # Development logs and reports
```

## Recent Optimizations

### 1. Dependency Management

**Server Dependencies (Enhanced)**
```toml
[dependencies]
# Core async runtime
tokio = { version = "1.40", features = ["full"] }
async-trait = "0.1"

# Enhanced networking
hyper = "1.5"
mdns-sd = "0.11"                # Service discovery
uuid = { version = "1.0", features = ["v4"] }
socket2 = "0.5"                 # Low-level networking
tokio-util = { version = "0.7", features = ["codec", "net"] }
hostname = "0.4"                # System hostname

# Security & encryption
rustls = "0.23"
rustls-pemfile = "2.0"
ring = "0.17"                   # Cryptographic operations

# Structured logging
tracing = "0.1"
tracing-subscriber = "0.3"

# Error handling
anyhow = "1.0"
thiserror = "1.0"
```

**Feature Flags (Optimized)**
```toml
[features]
default = ["crypto"]
full = ["gstreamer", "gstreamer-app", "gstreamer-video", "nix", "x11", "crypto"]
crypto = []                      # Cryptographic features
streaming = ["gstreamer", "gstreamer-app", "gstreamer-video"]
system = ["nix", "x11"]         # System integration
discovery = ["mdns-sd"]         # Service discovery
```

### 2. Build System Enhancements

**Enhanced Build Script**
- Color-coded output for better visibility
- Feature-based building with `--features` support
- System dependency checking
- Build artifact tracking
- Comprehensive error reporting

```bash
# Examples
./build.sh debug server                    # Basic server build
./build.sh release all --features full     # Full feature build
./build.sh release server --features streaming,crypto
./build.sh clean                          # Clean artifacts
```

**New Testing Framework**
```bash
./test.sh unit          # Unit tests
./test.sh integration   # Integration tests
./test.sh network       # Network connectivity
./test.sh quality       # Code quality checks
./test.sh all --coverage # Complete test suite with coverage
```

### 3. Error Handling System

**Centralized Error Types**
```rust
#[derive(Error, Debug)]
pub enum DpstreamError {
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),

    #[error("Emulator error: {0}")]
    Emulator(#[from] EmulatorError),

    #[error("Streaming error: {0}")]
    Streaming(#[from] StreamingError),

    #[error("VPN error: {0}")]
    Vpn(#[from] VpnError),

    // ... additional error types
}
```

**Error Severity and Recovery**
- Automatic error severity classification
- Recovery suggestions for common issues
- User-friendly error messages
- Structured error reporting with correlation IDs

### 4. Enhanced Logging

**Multi-output Logging**
- Console output with colors and structure
- JSON file logging for analysis
- Configurable log levels via environment
- Session correlation IDs
- Performance metrics logging

```rust
// Enhanced logging initialization
fn init_logging() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer().compact())           // Console
        .with(fmt::layer().json().with_writer(file)) // File
        .with(EnvFilter::new(&log_level))
        .try_init()?;
}
```

### 5. Network Optimizations

**Service Discovery**
- Real mDNS implementation with mdns-sd
- GameStream protocol compatibility
- Automatic capability advertising
- Client discovery with timeout handling
- Graceful service cleanup

**VPN Integration**
- Enhanced Tailscale configuration
- Better error handling and recovery
- Network status monitoring
- Connection quality metrics

### 6. Switch Client Optimizations

**no-std Compatibility**
```toml
[dependencies]
# Embedded-friendly collections
heapless = "0.8"
nb = "1.1"

# no-std networking
smoltcp = { version = "0.11", default-features = false }

# Compact serialization
postcard = "1.0"
serde-json-core = "0.5"

# no-std crypto
aes = "0.8"
chacha20poly1305 = "0.10"
```

**Memory Optimization**
- Custom allocators for Switch environment
- NEON optimizations for Tegra X1
- Size-optimized release builds
- Debug symbol stripping

## Development Workflow

### 1. Daily Development
```bash
# Start development session
./scripts/setup-dev.sh

# Build and test
./scripts/build.sh debug server
./scripts/test.sh unit

# Code quality check
./scripts/test.sh quality
```

### 2. Feature Development
```bash
# Create feature branch
git checkout -b feature/new-feature

# Development cycle
./scripts/build.sh debug server --features streaming
./scripts/test.sh integration

# Quality gate
./scripts/test.sh quality
```

### 3. Release Preparation
```bash
# Full build and test
./scripts/build.sh release all --features full
./scripts/test.sh all --coverage

# Performance validation
./scripts/test.sh performance
```

## Code Quality Standards

### 1. Formatting
```bash
cargo fmt --all
```

### 2. Linting
```bash
cargo clippy -- -D warnings
```

### 3. Testing
- Minimum 80% test coverage
- All public APIs must have tests
- Integration tests for network components
- Performance benchmarks for critical paths

### 4. Documentation
- All public APIs documented with rustdoc
- Architecture decisions recorded
- Performance characteristics documented
- Error conditions and recovery documented

## Performance Targets

### Server Performance
- **Startup Time**: <5 seconds
- **Memory Usage**: <512MB baseline
- **CPU Usage**: <50% single core
- **Network Latency**: <5ms internal processing

### Switch Client Performance
- **Binary Size**: <2MB NRO file
- **Memory Usage**: <256MB total
- **Decode Latency**: <10ms per frame
- **Input Latency**: <2ms processing

### Network Performance
- **Discovery Time**: <3 seconds
- **Connection Setup**: <1 second
- **Stream Latency**: <30ms end-to-end
- **Throughput**: 10-25 Mbps adaptive

## Debugging and Troubleshooting

### 1. Logging Configuration
```bash
# Environment variables
export RUST_LOG=debug              # Debug level logging
export RUST_LOG=dpstream=trace     # Trace level for dpstream
```

### 2. Common Issues

**Build Issues**
```bash
# Clean and rebuild
./scripts/build.sh clean
./scripts/build.sh debug server

# Check dependencies
./scripts/test.sh quality
```

**Network Issues**
```bash
# Test connectivity
./scripts/test-network.sh

# Check Tailscale
tailscale status
```

**Performance Issues**
```bash
# Run benchmarks
./scripts/test.sh performance

# Profile server
cargo bench
```

### 3. Debug Tools

**Server Debugging**
```bash
# Start with debug logging
RUST_LOG=debug cargo run

# Network tracing
RUST_LOG=dpstream::network=trace cargo run
```

**Client Debugging**
- nxlink for remote debugging
- Console output via Homebrew Menu
- File logging on SD card

## Contributing Guidelines

### 1. Code Standards
- Follow Rust naming conventions
- Use `cargo fmt` and `cargo clippy`
- Write comprehensive tests
- Document public APIs

### 2. Commit Standards
```bash
# Use conventional commits
git commit -m "feat: add service discovery"
git commit -m "fix: resolve memory leak in decoder"
git commit -m "docs: update build instructions"
```

### 3. Pull Request Process
1. Create feature branch
2. Implement changes with tests
3. Ensure all quality checks pass
4. Update documentation
5. Submit pull request with description

## Optimization Roadmap

### Phase 1 Complete ✅
- [x] Repository structure optimization
- [x] Dependency management with features
- [x] Enhanced build system
- [x] Centralized error handling
- [x] Structured logging
- [x] Network optimizations

### Phase 2 (Sprint 2)
- [ ] Core Dolphin integration
- [ ] Basic streaming pipeline
- [ ] Switch client framework
- [ ] Performance profiling

### Phase 3 (Sprint 3+)
- [ ] Hardware acceleration
- [ ] Advanced networking features
- [ ] Performance optimizations
- [ ] Security enhancements

## Resources

### Documentation
- [Rust Book](https://doc.rust-lang.org/book/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [no-std Book](https://docs.rust-embedded.org/book/)

### Tools
- [Cargo](https://doc.rust-lang.org/cargo/)
- [Clippy](https://github.com/rust-lang/rust-clippy)
- [Rustfmt](https://github.com/rust-lang/rustfmt)

### Debugging
- [Tracing](https://tracing.rs/)
- [Console](https://github.com/tokio-rs/console)
- [Flamegraph](https://github.com/flamegraph-rs/flamegraph)

---

This development guide ensures consistent quality and efficient development practices for the dpstream project.