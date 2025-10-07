# Script Documentation - dpstream

Repository: `git@github.com:hephaex/dpstream.git`  
Maintainer: hephaex@gmail.com

## Overview

This directory contains documentation for all automation scripts used in the dpstream project. All scripts are written in Bash and designed to work on Ubuntu 24.04 and macOS.

## Script Index

1. **[setup-dev.sh](#setup-devsh)** - Development environment setup
2. **[build.sh](#buildsh)** - Main build script
3. **[git-workflow.sh](#git-workflowsh)** - Git automation workflow
4. **[deploy.sh](#deploysh)** - Deployment script
5. **[test.sh](#testsh)** - Testing automation

---

## setup-dev.sh

**Purpose**: Sets up complete development environment for dpstream

### Features
- Installs system dependencies
- Configures Rust with Switch target
- Installs devkitPro for Switch development
- Sets up Tailscale VPN
- Configures Dolphin and Sunshine
- Creates project structure

### Usage
```bash
./scripts/setup-dev.sh
```

### Requirements
- Ubuntu 24.04 or macOS
- sudo access
- Internet connection

### What it installs
- Rust toolchain (latest stable)
- aarch64-nintendo-switch-freestanding target
- devkitPro with Switch development tools
- GStreamer development libraries
- Tailscale VPN client
- Dolphin emulator
- Sunshine streaming host

### Environment Variables Created
The script creates a `.env` file with all necessary configuration variables.

---

## build.sh

**Purpose**: Builds server and Switch client components

### Features
- Supports debug and release builds
- Can build server, client, or both
- Generates build documentation
- Automatic git commit of build history

### Usage
```bash
# Build everything in release mode
./scripts/build.sh release all

# Build server only in debug mode
./scripts/build.sh debug server

# Build Switch client only
./scripts/build.sh release switch

# Clean build artifacts
./scripts/build.sh clean
```

### Build Types
- **debug**: Includes debug symbols, no optimization
- **release**: Optimized build, no debug symbols

### Target Platforms
- **server**: Ubuntu server component
- **switch**: Nintendo Switch homebrew client
- **all**: Both components
- **clean**: Remove all build artifacts

### Output
Build artifacts are placed in `build/` directory:
- `dpstream-server` - Server executable
- `dpstream.nro` - Switch homebrew file
- Build logs in `.history/`

---

## git-workflow.sh

**Purpose**: Automates git operations and history tracking

### Features
- Sprint completion workflow
- Phase completion workflow
- Automatic history documentation
- Daily backup commits
- Git tag creation for phases

### Usage

#### Complete a Sprint
```bash
./scripts/git-workflow.sh sprint-complete \
    "Sprint-1" \
    "Basic streaming implementation complete" \
    "- Server setup\n- Client framework\n- Network protocol" \
    "Begin Sprint-2 with input handling"
```

#### Complete a Phase
```bash
./scripts/git-workflow.sh phase-complete \
    "Phase-1" \
    "Foundation phase complete" \
    "Sprint-1: Setup\nSprint-2: Core modules" \
    "Phase-2: Streaming pipeline"
```

#### Daily Backup
```bash
./scripts/git-workflow.sh backup "Work in progress on decoder"
```

### History Documentation
All operations create markdown documentation in `.history/`:
- Sprint summaries
- Phase summaries
- Build reports
- Commit details

### Git Tags
Phase completions automatically create annotated git tags:
- Format: `phase-{name}-{date}`
- Example: `phase-1-20240115`

---

## deploy.sh

**Purpose**: Deploys dpstream to production server

### Features
- Tailscale VPN connection
- Secure file transfer
- Service management
- Rollback capability

### Usage
```bash
# Deploy to production
./scripts/deploy.sh production

# Deploy to staging
./scripts/deploy.sh staging

# Rollback to previous version
./scripts/deploy.sh rollback
```

### Deployment Process
1. Builds release version
2. Connects via Tailscale
3. Stops existing service
4. Backs up current version
5. Deploys new version
6. Starts service
7. Verifies deployment

---

## test.sh

**Purpose**: Runs automated tests

### Features
- Unit tests
- Integration tests
- Performance benchmarks
- Switch client testing via nxlink

### Usage
```bash
# Run all tests
./scripts/test.sh all

# Run server tests only
./scripts/test.sh server

# Run client tests only
./scripts/test.sh client

# Run benchmarks
./scripts/test.sh bench
```

---

## Common Environment Variables

All scripts respect the following environment variables from `.env`:

### Tailscale Configuration
- `TAILSCALE_AUTH_KEY`: Authentication key for Tailscale
- `TAILSCALE_HOSTNAME`: Hostname for this machine
- `TAILSCALE_ROUTES`: Routes to advertise

### Build Configuration
- `BUILD_TYPE`: debug or release
- `TARGET_PLATFORM`: server, switch, or all
- `OUTPUT_DIR`: Build output directory

### Development
- `DEBUG_MODE`: Enable debug features
- `LOG_LEVEL`: Logging verbosity
- `DEV_MODE`: Development mode features

---

## Script Development Guidelines

When creating new scripts:

1. **Header Template**
```bash
#!/bin/bash
# Script Purpose
# Repository: git@github.com:hephaex/dpstream.git
# Maintainer: hephaex@gmail.com
```

2. **Error Handling**
```bash
set -e  # Exit on error
set -u  # Error on undefined variables
```

3. **Color Output**
```bash
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'
```

4. **Logging**
- All scripts should log to `.history/`
- Use consistent timestamp format: `%Y%m%d_%H%M%S`

5. **Git Integration**
- Commit important operations
- Create meaningful commit messages
- Tag major milestones

---

## Troubleshooting

### Common Issues

#### Permission Denied
```bash
chmod +x scripts/*.sh
```

#### Missing Dependencies
```bash
./scripts/setup-dev.sh
```

#### Build Failures
Check `.history/build_*.log` for detailed error messages

#### Git Issues
```bash
git remote set-url origin git@github.com:hephaex/dpstream.git
```

---

## Contributing

When modifying scripts:
1. Test on both Ubuntu and macOS
2. Update this documentation
3. Commit with descriptive message
4. Create history entry in `.history/`

---

## Support

For issues or questions:
- Email: hephaex@gmail.com
- Repository: https://github.com/hephaex/dpstream

---

*Last updated: Script documentation v1.0*