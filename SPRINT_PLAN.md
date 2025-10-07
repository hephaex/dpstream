# SPRINT_PLAN.md - Dolphin Remote Gaming System

## Project Information
**Repository**: `git@github.com:hephaex/dpstream.git`  
**Maintainer**: hephaex@gmail.com  
**Language**: Rust  
**Duration**: 16 weeks (4 months)  
**Team**: Backend Dev (2), Frontend Dev (1), DevOps (1)

## Development Workflow

Each sprint and phase completion includes:
1. Code review and testing
2. Documentation in `.history/` folder (markdown format)
3. Git commit and push: `git add . && git commit -m "message" && git push`
4. Script archival in `scripts/` folder
5. Script documentation in `docs/scripts/` folder

### Git Workflow Commands
```bash
# Sprint completion
./scripts/git-workflow.sh sprint-complete "Sprint-X" "Summary" "Tasks" "Next"

# Phase completion  
./scripts/git-workflow.sh phase-complete "Phase-X" "Summary" "Sprints" "Next"

# Daily backup
./scripts/git-workflow.sh backup "Work description"
```

---

## Phase 1: Foundation (Week 1-4)

### Sprint 1: Project Setup & Research (Week 1-2)

#### Goals
- Configure project repository and development environment
- Setup Tailscale VPN infrastructure
- Analyze Dolphin emulator and Moonlight protocol
- Establish git workflow and documentation structure

#### Tasks

**Week 1**
- [ ] **SETUP-001**: Repository initialization
  ```bash
  git clone git@github.com:hephaex/dpstream.git
  ./scripts/setup-dev.sh
  cp .env.example .env
  # Configure Tailscale auth key in .env
  ```
  
- [ ] **SETUP-002**: Tailscale VPN setup
  - Install Tailscale on server and development machines
  - Configure network routes
  - Test connectivity between nodes
  
- [ ] **SETUP-003**: Development environment
  - Install Rust toolchain
  - Setup devkitPro for Switch
  - Configure VSCode/IDE with Rust analyzer
  
- [ ] **RESEARCH-001**: Technical analysis
  - Dolphin emulator architecture study
  - Moonlight-Switch source code review
  - Document findings in `docs/research/`

**Week 2**
- [ ] **PROTOTYPE-001**: Proof of concept
  - Basic Sunshine to Moonlight streaming test
  - Measure baseline latency over Tailscale
  - Document results in `.history/poc_results.md`
  
- [ ] **DESIGN-001**: System architecture
  - Create architecture diagrams
  - Define API specifications
  - Document in `docs/architecture/`

#### Sprint Completion
```bash
# Generate sprint documentation
echo "# Sprint 1 Summary
- Repository setup complete
- Tailscale VPN configured
- Development environment ready
- POC streaming validated" > .history/sprint1_$(date +%Y%m%d).md

# Commit and push
./scripts/git-workflow.sh sprint-complete \
  "Sprint-1" \
  "Project foundation and research complete" \
  "Setup repo, Tailscale VPN, dev environment, POC streaming" \
  "Begin core module development"
```

### Sprint 2: Core Module Development (Week 3-4)

#### Goals
- Implement Dolphin process management in Rust
- Build Switch homebrew framework
- Setup Tailscale networking module

#### Tasks

**Week 3**
- [ ] **SERVER-001**: Dolphin process manager (`server/src/emulator/`)
  ```rust
  // server/src/emulator/process.rs
  pub struct DolphinManager {
      process: Child,
      window_id: u64,
      tailscale_ip: String,
  }
  ```
  
- [ ] **SERVER-002**: Tailscale integration (`server/src/network/vpn.rs`)
  - Implement Tailscale client wrapper
  - Auto-discovery of game servers
  - Connection management
  
- [ ] **DOC-001**: Create initial documentation
  ```bash
  mkdir -p docs/scripts
  echo "# Build Instructions" > docs/scripts/build-guide.md
  echo "# Deployment Guide" > docs/scripts/deployment.md
  ```

**Week 4**
- [ ] **CLIENT-001**: Switch homebrew base (`switch-client/`)
  - Setup Rust no_std environment
  - Implement custom allocator
  - Basic libnx bindings
  
- [ ] **CLIENT-002**: Network initialization
  - Tailscale node discovery
  - Moonlight protocol handshake
  - Connection state management
  
- [ ] **SCRIPT-001**: Build automation
  ```bash
  # Create build script
  cat > scripts/build.sh << 'EOF'
  #!/bin/bash
  cargo build --release
  # Additional build steps
  EOF
  chmod +x scripts/build.sh
  ```

#### Sprint Completion
```bash
# Document sprint results
cat > .history/sprint2_$(date +%Y%m%d).md << EOF
# Sprint 2 Summary
- Dolphin process manager implemented
- Tailscale VPN module complete
- Switch client framework initialized
- Build automation scripts created

## Code Metrics
- Lines of Rust: $(find . -name "*.rs" | xargs wc -l | tail -1)
- Test coverage: $(cargo tarpaulin --print-summary)
EOF

# Commit and push
./scripts/git-workflow.sh sprint-complete \
  "Sprint-2" \
  "Core modules implemented" \
  "Dolphin manager, Tailscale integration, Switch framework" \
  "Begin streaming pipeline"
```

---

## Phase 1 Completion
```bash
# Phase 1 complete - Foundation established
./scripts/git-workflow.sh phase-complete \
  "Phase-1-Foundation" \
  "Project foundation complete with core modules" \
  "Sprint-1: Setup and research\nSprint-2: Core module development" \
  "Phase-2: Streaming Pipeline"
```

---

## Phase 2: Streaming Pipeline (Week 5-8)

### Sprint 3: Media Processing (Week 5-6)

#### Goals
- Implement hardware accelerated video decoding
- Configure Sunshine streaming with Tailscale
- Optimize network performance

#### Tasks

**Week 5**
- [ ] **CLIENT-003**: Tegra X1 hardware decoder (`switch-client/src/decoder/`)
  ```rust
  // switch-client/src/decoder/nvdec.rs
  pub struct TegraDecoder {
      nvdec: NvDecoder,
      surface_pool: Vec<Surface>,
      tailscale_buffer: RingBuffer,
  }
  ```
  
- [ ] **NETWORK-001**: Tailscale optimization
  - Configure MTU for optimal performance
  - Implement packet priority queuing
  - Setup bandwidth monitoring
  
- [ ] **SCRIPT-002**: Network testing script
  ```bash
  # scripts/test-network.sh
  #!/bin/bash
  tailscale ping $SERVER_IP
  iperf3 -c $SERVER_IP -t 30
  ```

**Week 6**
- [ ] **SERVER-003**: NVENC encoder optimization
  - Low latency preset configuration
  - Adaptive bitrate based on Tailscale metrics
  - Frame pacing optimization
  
- [ ] **SERVER-004**: Streaming pipeline
  - GStreamer pipeline with Tailscale sink
  - Audio/video synchronization
  - Buffer management
  
- [ ] **DOC-002**: Performance documentation
  ```bash
  echo "# Performance Tuning Guide" > docs/performance.md
  echo "# Network Optimization" > docs/network-setup.md
  ```

#### Sprint Completion
```bash
# Performance metrics collection
cat > .history/sprint3_$(date +%Y%m%d).md << EOF
# Sprint 3 Summary
- Hardware decoder implemented
- Tailscale networking optimized
- Streaming pipeline complete

## Performance Metrics
- Decode latency: $(./scripts/measure-latency.sh)
- Network throughput: $(tailscale status)
- Memory usage: $(free -h)
EOF

./scripts/git-workflow.sh sprint-complete \
  "Sprint-3" \
  "Media processing pipeline complete" \
  "HW decoder, NVENC optimization, Tailscale tuning" \
  "Begin input system implementation"
```

### Sprint 4: Input System (Week 7-8)

#### Goals
- Implement Joy-Con/Pro Controller support
- Add gyro and touch input
- Integrate with Tailscale for low-latency input

#### Tasks

**Week 7**
- [ ] **INPUT-001**: Controller mapping (`switch-client/src/input/`)
  ```rust
  // switch-client/src/input/controller.rs
  pub struct SwitchInput {
      joycon: JoyConPair,
      pro_controller: Option<ProController>,
      input_queue: VecDeque<InputEvent>,
      tailscale_socket: UdpSocket,
  }
  ```
  
- [ ] **INPUT-002**: Gyro motion control
  - 6-axis sensor reading
  - Wii Remote pointer emulation
  - Calibration system
  
- [ ] **SCRIPT-003**: Input testing utilities
  ```bash
  # scripts/test-input.sh
  #!/bin/bash
  cargo test --package switch-client --lib input
  ```

**Week 8**
- [ ] **INPUT-003**: HD Rumble implementation
  - Vibration pattern conversion
  - Frequency/amplitude mapping
  - Network packet optimization
  
- [ ] **INPUT-004**: Touch input handler
  - Touchscreen mouse emulation
  - Gesture recognition
  - Coordinate transformation
  
- [ ] **TEST-001**: End-to-end testing
  ```bash
  # scripts/e2e-test.sh
  ./scripts/build.sh release all
  ./scripts/deploy.sh test
  cargo test --all
  ```

#### Sprint Completion
```bash
cat > .history/sprint4_$(date +%Y%m%d).md << EOF
# Sprint 4 Summary
- Complete input system implemented
- All controller types supported
- Gyro and touch input working

## Test Results
- Input latency: < 5ms
- Packet loss: 0%
- All tests passing
EOF

./scripts/git-workflow.sh sprint-complete \
  "Sprint-4" \
  "Input system complete" \
  "Joy-Con, Pro Controller, Gyro, Touch, HD Rumble" \
  "Begin performance optimization"
```

---

## Phase 2 Completion
```bash
./scripts/git-workflow.sh phase-complete \
  "Phase-2-Streaming" \
  "Complete streaming pipeline with input system" \
  "Sprint-3: Media processing\nSprint-4: Input system" \
  "Phase-3: Advanced Features"
```

---

## Phase 3: Advanced Features (Week 9-12)

### Sprint 5: Performance Optimization (Week 9-10)

#### Goals
- Optimize Switch hardware utilization
- Minimize network latency over Tailscale
- Implement battery efficiency features

#### Tasks

**Week 9**
- [ ] **PERF-001**: Switch overclock support (`switch-client/src/sys/`)
  ```rust
  // switch-client/src/sys/overclock.rs
  pub struct SysClkManager {
      cpu_freq: u32,  // Up to 1785 MHz
      gpu_freq: u32,  // Up to 921 MHz
      mem_freq: u32,  // Up to 1600 MHz
      tailscale_priority: QosPriority,
  }
  ```
  
- [ ] **PERF-002**: Memory optimization
  - Custom allocator implementation
  - DMA buffer management
  - Cache-line alignment
  
- [ ] **SCRIPT-004**: Performance benchmarking
  ```bash
  # scripts/benchmark.sh
  #!/bin/bash
  cargo bench --all
  echo "Results saved to .history/bench_$(date +%Y%m%d).md"
  ```

**Week 10**
- [ ] **PERF-003**: Tailscale network optimization
  - DERP relay configuration
  - Direct connection establishment
  - Packet aggregation
  
- [ ] **PERF-004**: Power management
  - Dynamic frequency scaling
  - Screen brightness control
  - WiFi power saving modes
  
- [ ] **DOC-003**: Optimization guide
  ```bash
  cat > docs/optimization.md << EOF
  # Performance Optimization Guide
  - Tailscale tuning parameters
  - Overclock settings
  - Battery optimization
  EOF
  ```

#### Sprint Completion
```bash
cat > .history/sprint5_$(date +%Y%m%d).md << EOF
# Sprint 5 Summary
- Performance optimizations complete
- Tailscale latency minimized
- Battery life improved

## Benchmark Results
- FPS: Stable 60
- Latency: < 20ms over Tailscale
- Battery: 3.5 hours playtime
EOF

./scripts/git-workflow.sh sprint-complete \
  "Sprint-5" \
  "Performance optimization complete" \
  "Overclock, memory opt, Tailscale tuning, power management" \
  "Begin UI/UX implementation"
```

### Sprint 6: User Experience (Week 11-12)

#### Goals
- Create native Switch UI
- Implement game library management
- Add quality of life features

#### Tasks

**Week 11**
- [ ] **UI-001**: Switch native UI (`switch-client/src/ui/`)
  ```rust
  // switch-client/src/ui/menu.rs
  pub struct MainMenu {
      server_list: Vec<TailscaleNode>,
      game_library: GameLibrary,
      settings: Settings,
  }
  ```
  
- [ ] **UI-002**: Server discovery
  - Automatic Tailscale node discovery
  - Server status display
  - Connection management UI
  
- [ ] **SCRIPT-005**: UI asset builder
  ```bash
  # scripts/build-assets.sh
  #!/bin/bash
  # Convert images to Switch format
  # Generate font atlases
  ```

**Week 12**
- [ ] **FEATURE-001**: Save state sync
  - Cloud save over Tailscale
  - Automatic backup
  - Cross-device synchronization
  
- [ ] **FEATURE-002**: NSP Forwarder
  - Direct game launch
  - Custom icons per game
  - Auto-connect to server
  
- [ ] **DOC-004**: User manual
  ```bash
  cat > docs/user-guide.md << EOF
  # dpstream User Guide
  - Installation
  - Tailscale setup
  - Game library management
  - Troubleshooting
  EOF
  ```

#### Sprint Completion
```bash
cat > .history/sprint6_$(date +%Y%m%d).md << EOF
# Sprint 6 Summary
- UI implementation complete
- Game library functional
- All features integrated

## Feature Checklist
- [x] Native UI
- [x] Server discovery
- [x] Save sync
- [x] NSP forwarders
EOF

./scripts/git-workflow.sh sprint-complete \
  "Sprint-6" \
  "User experience features complete" \
  "Native UI, game library, save sync, NSP forwarders" \
  "Begin testing phase"
```

---

## Phase 3 Completion
```bash
./scripts/git-workflow.sh phase-complete \
  "Phase-3-Advanced" \
  "Advanced features and optimizations complete" \
  "Sprint-5: Performance\nSprint-6: User Experience" \
  "Phase-4: Testing and Release"
```

---

## Phase 4: Testing & Release (Week 13-16)

### Sprint 7: Testing & Debugging (Week 13-14)

#### Goals
- Comprehensive testing suite
- Bug fixes and stability improvements
- Performance validation over Tailscale

#### Tasks

**Week 13**
- [ ] **TEST-001**: Unit test suite
  ```bash
  # scripts/run-tests.sh
  #!/bin/bash
  cargo test --all
  cargo tarpaulin --out Html
  mv tarpaulin-report.html .history/coverage_$(date +%Y%m%d).html
  ```
  
- [ ] **TEST-002**: Integration testing
  - Tailscale connectivity tests
  - End-to-end streaming validation
  - Controller input verification
  
- [ ] **TEST-003**: Network resilience
  ```rust
  // tests/network_resilience.rs
  #[test]
  fn test_tailscale_reconnection() {
      // Simulate network drops
      // Verify automatic recovery
  }
  ```

**Week 14**
- [ ] **DEBUG-001**: Remote debugging setup
  - nxlink configuration
  - Tailscale debug tunnel
  - Log aggregation system
  
- [ ] **DEBUG-002**: Performance profiling
  ```bash
  # scripts/profile.sh
  #!/bin/bash
  cargo flamegraph --bin dpstream-server
  perf record -g ./target/release/dpstream-server
  ```
  
- [ ] **DOC-005**: Testing documentation
  ```bash
  cat > docs/testing.md << EOF
  # Testing Guide
  - Unit test structure
  - Integration test setup
  - Performance benchmarks
  - Debug procedures
  EOF
  ```

#### Sprint Completion
```bash
cat > .history/sprint7_$(date +%Y%m%d).md << EOF
# Sprint 7 Summary
- Test coverage: 85%
- All integration tests passing
- Performance targets met

## Test Report
$(cargo test --all 2>&1)

## Coverage Report
$(cargo tarpaulin --print-summary)
EOF

./scripts/git-workflow.sh sprint-complete \
  "Sprint-7" \
  "Testing and debugging complete" \
  "Unit tests, integration tests, performance profiling" \
  "Begin final polish and release preparation"
```

### Sprint 8: Polish & Release (Week 15-16)

#### Goals
- Final UI polish
- Release packaging
- Community distribution

#### Tasks

**Week 15**
- [ ] **POLISH-001**: UI refinements
  - Splash screen animation
  - Loading indicators
  - Error message improvements
  
- [ ] **POLISH-002**: Setup wizard
  ```rust
  // switch-client/src/ui/wizard.rs
  pub struct SetupWizard {
      steps: Vec<WizardStep>,
      tailscale_config: TailscaleSetup,
      controller_mapping: ControllerSetup,
  }
  ```
  
- [ ] **SCRIPT-006**: Release builder
  ```bash
  # scripts/build-release.sh
  #!/bin/bash
  VERSION=$(cat Cargo.toml | grep version | head -1 | awk '{print $3}' | tr -d '"')
  ./scripts/build.sh release all
  mkdir -p releases/v$VERSION
  cp build/* releases/v$VERSION/
  tar -czf dpstream-v$VERSION.tar.gz releases/v$VERSION
  ```

**Week 16**
- [ ] **RELEASE-001**: Package creation
  - Build optimized NRO
  - Create NSP forwarder templates
  - Generate SHA256 checksums
  
- [ ] **RELEASE-002**: Documentation finalization
  ```bash
  # Generate complete documentation
  cat > README.md << EOF
  # dpstream - Dolphin Remote Gaming System
  
  Repository: git@github.com:hephaex/dpstream.git
  Maintainer: hephaex@gmail.com
  
  ## Features
  - GameCube/Wii streaming over Tailscale VPN
  - Hardware accelerated decoding on Switch
  - Full controller support with HD Rumble
  
  ## Quick Start
  1. Install Tailscale: tailscale up
  2. Configure .env file
  3. Run: ./dpstream.nro
  EOF
  ```
  
- [ ] **RELEASE-003**: Distribution
  - GitHub release creation
  - Homebrew App Store submission
  - Community announcement

#### Sprint Completion
```bash
cat > .history/sprint8_$(date +%Y%m%d).md << EOF
# Sprint 8 Summary
- Release v1.0.0 complete
- All documentation finalized
- Distribution packages ready

## Release Checklist
- [x] Code freeze
- [x] Final testing
- [x] Documentation
- [x] Package creation
- [x] GitHub release
- [x] Community announcement

## Release Notes
Version 1.0.0
- Initial release
- Full GameCube/Wii support
- Tailscale VPN integration
- 60 FPS streaming
EOF

./scripts/git-workflow.sh sprint-complete \
  "Sprint-8" \
  "Release preparation complete" \
  "UI polish, packaging, documentation, distribution" \
  "Version 1.0.0 released!"

# Tag the release
git tag -a v1.0.0 -m "Release version 1.0.0"
git push origin v1.0.0
```

---

## Phase 4 Completion
```bash
./scripts/git-workflow.sh phase-complete \
  "Phase-4-Release" \
  "Testing and release complete - v1.0.0 shipped!" \
  "Sprint-7: Testing\nSprint-8: Release" \
  "Post-release: Maintenance and v2.0 planning"
```

---

## Risk Management

### Technical Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Dolphin integration complexity | High | Early prototype, fallback to process isolation |
| Network latency | High | Multiple codec options, regional servers |
| Hardware encoding support | Medium | Software fallback, quality presets |
| Browser compatibility | Medium | Progressive enhancement, polyfills |

### Dependencies

```mermaid
graph TD
    A[Project Setup] --> B[Core Modules]
    B --> C[Streaming Pipeline]
    C --> D[WebRTC Integration]
    D --> E[Input System]
    E --> F[Client Development]
    F --> G[Testing]
    G --> H[Production Deployment]
```

---

## Technical Implementation Details

### Repository Structure
```
git@github.com:hephaex/dpstream.git
├── .env                    # Tailscale and server configuration
├── .history/              # Sprint/phase documentation (*.md)
│   ├── sprint1_*.md
│   ├── sprint2_*.md
│   └── phase1_*.md
├── scripts/               # All automation scripts
│   ├── setup-dev.sh       # Development environment setup
│   ├── build.sh          # Build automation
│   ├── git-workflow.sh   # Git commit automation
│   ├── test.sh           # Test runner
│   └── deploy.sh         # Deployment via Tailscale
├── docs/scripts/         # Script documentation
│   ├── build-guide.md
│   ├── deployment.md
│   └── git-workflow.md
├── server/               # Rust server implementation
└── switch-client/        # Rust Switch client
```

### Tailscale VPN Configuration

#### Environment Variables (.env)
```bash
# Tailscale Configuration
TAILSCALE_AUTH_KEY=tskey-auth-xxxxxxxxxx
TAILSCALE_HOSTNAME=dpstream-server
TAILSCALE_ROUTES=192.168.1.0/24
TAILSCALE_ACCEPT_DNS=true

# Server binding to Tailscale IP
SERVER_IP=100.x.x.x  # Tailscale IP
SERVER_PORT=47989
```

#### Network Architecture
```
Developer Machine          Tailscale Network         Game Server
  (Switch/PC)      <----->  (100.64.0.0/10)  <---->  (Ubuntu)
     |                           |                       |
  Moonlight                  DERP Relay              Sunshine
   Client                   (if needed)               Host
```

### Development Scripts

#### Automated Git Workflow
Every sprint and phase completion triggers:
```bash
# 1. Create history documentation
cat > .history/sprint_$(date +%Y%m%d).md << EOF
# Sprint Summary
- Tasks completed
- Metrics
- Next steps
EOF

# 2. Commit all changes
git add .
git commit -m "Complete Sprint: Description"

# 3. Push to repository
git push origin main

# 4. Tag for phases
git tag -a phase-1-$(date +%Y%m%d) -m "Phase complete"
```

### Rust Implementation Standards

#### Server Code Structure
```rust
// server/src/main.rs
use tailscale_rs::Client;
use sunshine_rs::StreamHost;

#[tokio::main]
async fn main() -> Result<()> {
    // Load config from .env
    let config = Config::from_env()?;
    
    // Connect to Tailscale
    let tailscale = Client::new(&config.tailscale_auth_key).await?;
    let ip = tailscale.ip().await?;
    
    // Start streaming server
    let host = StreamHost::new(ip, config.port)?;
    host.serve().await
}
```

#### Client Code Structure
```rust
// switch-client/src/main.rs
#![no_std]
#![no_main]

use moonlight::Client;
use tailscale_discovery::discover_servers;

#[no_mangle]
pub extern "C" fn main() -> i32 {
    // Discover servers via Tailscale
    let servers = discover_servers();
    
    // Connect to first available
    let client = Client::connect(&servers[0]);
    
    // Main loop
    loop {
        client.process_frame();
        client.send_input();
    }
}
```

---

## Technical Stack Details

### Nintendo Switch Homebrew Development

#### Build Environment
```bash
# Required tools
- devkitA64 (aarch64-none-elf toolchain)
- libnx (Switch system libraries)
- switch-tools (elf2nro, nacptool)
- cargo-nx (Rust build wrapper)

# Rust target
rustup target add aarch64-nintendo-switch-freestanding

# Build configuration
[target.aarch64-nintendo-switch-freestanding]
linker = "aarch64-none-elf-gcc"
ar = "aarch64-none-elf-ar"
rustflags = [
    "-C", "target-cpu=cortex-a57",
    "-C", "target-feature=+neon",
    "-C", "link-arg=-specs=switch.specs",
]
```

#### Memory Layout
```
Application Memory Map (Full RAM Access):
┌─────────────────────────┐ 0x0000000000000000
│      Reserved           │
├─────────────────────────┤ 0x0000000008000000
│   Application Heap      │ (~3.2GB available)
├─────────────────────────┤ 0x00000000D0000000
│   Stack & TLS           │
├─────────────────────────┤ 0x00000000E0000000
│   GPU Memory            │
└─────────────────────────┘ 0x0000000100000000
```

### Moonlight Protocol Extensions

#### Custom Control Messages
```rust
pub enum DolphinControlMessage {
    // Wii-specific inputs
    WiiPointer { x: f32, y: f32 },
    WiiMotion { pitch: f32, roll: f32, yaw: f32 },
    NunchukStick { x: f32, y: f32 },
    
    // GameCube adapter emulation
    GCAdapter { port: u8, state: GCControllerState },
    
    // Save state management
    SaveStateRequest { slot: u8 },
    LoadStateRequest { slot: u8 },
    
    // Performance tuning
    SetQuality { bitrate: u32, resolution: Resolution },
}
```

---

## Success Metrics

### Performance KPIs
- **Latency**: < 30ms over Tailscale VPN
- **Frame rate**: Stable 60 FPS
- **Video quality**: 720p handheld / 1080p docked
- **Battery life**: 3+ hours continuous play

### Technical Metrics
- **Tailscale overhead**: < 5ms additional latency
- **Memory usage**: < 256MB (client)
- **Network bandwidth**: 5-20 Mbps adaptive
- **Test coverage**: > 80%

### Delivery Metrics
- **Sprint velocity**: 40 story points/sprint
- **Git commits**: Daily minimum
- **Documentation**: 100% coverage
- **Script automation**: All tasks scripted

---

## Post-Release Roadmap (v2.0)

### Additional Emulator Support
- [ ] Citra (3DS) integration
- [ ] PPSSPP (PSP) support
- [ ] RetroArch cores

### Network Enhancements
- [ ] WireGuard alternative to Tailscale
- [ ] P2P multiplayer relay
- [ ] Cloud save sync service

### Client Expansion
- [ ] Android app (via Tailscale)
- [ ] Steam Deck optimization
- [ ] Web client (WebAssembly)

---

## Risk Management

| Risk | Mitigation | Owner |
|------|------------|-------|
| Tailscale latency | DERP relay optimization, direct connections | DevOps |
| Hardware decoder issues | Software fallback, multiple codec support | Backend |
| Memory constraints | Aggressive optimization, custom allocator | Backend |
| Network drops | Auto-reconnection, state persistence | Backend |

---

## Communication Plan

### Daily Standups
```bash
# Auto-generate standup report
./scripts/daily-standup.sh > .history/standup_$(date +%Y%m%d).md
git add . && git commit -m "Daily: $(date +%Y-%m-%d)" && git push
```

### Sprint Reviews
- Bi-weekly demos
- Performance metrics review
- Stakeholder feedback

### Documentation Updates
- Every merge to main
- Script changes documented immediately
- API changes versioned

---

## Quality Assurance

### Code Review Process
1. Feature branch creation
2. Pull request to main
3. Automated tests via GitHub Actions
4. Manual review by maintainer (hephaex@gmail.com)
5. Merge with squash commit

### Testing Strategy
```bash
# Pre-commit hooks
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all

# Integration tests
./scripts/test.sh integration

# Performance benchmarks
cargo bench
```

---

## Deployment Pipeline

### Continuous Integration
```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Test
        run: |
          ./scripts/build.sh release all
          ./scripts/test.sh all
```

### Release Process
1. Version bump in Cargo.toml
2. Update CHANGELOG.md
3. Run `./scripts/build-release.sh`
4. Create GitHub release
5. Deploy via Tailscale: `./scripts/deploy.sh production`

---

## License and Legal

- **License**: MIT
- **Copyright**: 2024 hephaex@gmail.com
- **Repository**: git@github.com:hephaex/dpstream.git
- **Dependencies**: All FOSS compatible

---

## Contact Information

**Project Maintainer**: hephaex@gmail.com  
**Repository**: https://github.com/hephaex/dpstream  
**Documentation**: https://github.com/hephaex/dpstream/wiki  
**Issues**: https://github.com/hephaex/dpstream/issues  

---

*Last Updated: Sprint Plan v1.0 - Ready for implementation*

---

## Testing Strategy

### Hardware Testing Matrix

| Test Case | Handheld | Docked | Tabletop | WiFi 2.4 | WiFi 5 |
|-----------|----------|---------|----------|----------|---------|
| 720p30 Stream | ✓ | ✓ | ✓ | ✓ | ✓ |
| 720p60 Stream | ✓ | ✓ | ✓ | △ | ✓ |
| 1080p30 Stream | - | ✓ | - | △ | ✓ |
| 1080p60 Stream | - | ✓ | - | ✗ | ✓ |
| Gyro Controls | ✓ | ✓ | ✓ | ✓ | ✓ |
| Touch Input | ✓ | ✗ | ✓ | ✓ | ✓ |
| HD Rumble | ✓ | ✓ | ✓ | ✓ | ✓ |

Legend: ✓ Full Support, △ Limited, ✗ Not Supported, - N/A

### Performance Benchmarks

```rust
// Benchmark suite for Switch client
pub struct BenchmarkResults {
    pub decode_time_avg: Duration,      // Target: < 5ms
    pub render_time_avg: Duration,      // Target: < 8ms
    pub input_latency: Duration,        // Target: < 2ms
    pub network_jitter: Duration,       // Target: < 10ms
    pub memory_usage: usize,           // Target: < 256MB
    pub battery_drain: f32,            // mA/hour
}

#[cfg(test)]
mod benchmarks {
    use super::*;
    
    fn bench_h264_decode() {
        // Test 1080p H264 frame decode performance
    }
    
    fn bench_input_processing() {
        // Test controller input to network packet time
    }
    
    fn bench_memory_allocation() {
        // Test custom allocator performance
    }
}
```

---

## Network Protocol Specification

### Discovery Protocol
```
1. Client broadcasts mDNS query for "_nvstream._tcp"
2. Server responds with:
   - Hostname
   - IP address
   - Port number
   - Supported codecs
   - Max resolution/framerate

3. Client initiates pairing if unpaired
4. Client requests app list
5. Client starts stream with selected app
```

### Stream Negotiation
```rust
pub struct StreamConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,
    pub codec: VideoCodec,
    pub audio_config: AudioConfig,
    pub controller_count: u8,
}

impl StreamConfig {
    pub fn for_handheld() -> Self {
        Self {
            width: 1280,
            height: 720,
            fps: 60,
            bitrate: 10_000_000,  // 10 Mbps
            codec: VideoCodec::H264,
            audio_config: AudioConfig::Stereo48khz,
            controller_count: 1,
        }
    }
    
    pub fn for_docked() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 60,
            bitrate: 20_000_000,  // 20 Mbps
            codec: VideoCodec::H264,
            audio_config: AudioConfig::Surround51,
            controller_count: 4,
        }
    }
}
```

---

## Security Considerations

### Authentication
- PIN-based pairing (first connection)
- Certificate validation
- Session tokens with expiration

### Network Security
```rust
pub struct SecurityConfig {
    pub encryption: bool,          // Always true
    pub cert_validation: bool,     // Verify server cert
    pub pin_length: u8,            // 4-8 digits
    pub session_timeout: Duration, // Default: 30 minutes
}
```

### CFW Safety
- No telemetry or analytics
- No Nintendo services interaction
- Offline mode support
- No persistent logs

---

## Troubleshooting Guide

### Common Issues

| Issue | Symptoms | Solution |
|-------|----------|----------|
| Black screen | Stream starts but no video | Check NVDEC initialization, verify codec support |
| Audio delay | Sound out of sync | Adjust audio buffer size, check network latency |
| Controller drift | Input without touching | Recalibrate in system settings |
| Connection drops | Frequent disconnects | Check WiFi signal, reduce bitrate |
| High latency | Noticeable input delay | Enable performance mode, check server load |
| Crashes on startup | App won't launch | Ensure full RAM access mode |

### Debug Commands

```rust
// Debug overlay shortcuts
pub const DEBUG_SHORTCUTS: &[(Buttons, Action)] = &[
    (L | R | PLUS, ShowStats),        // FPS, latency, bitrate
    (L | R | MINUS, ToggleDebugLog),  // Console output
    (L | R | X, ForceReconnect),      // Restart stream
    (L | R | Y, DumpMemory),          // Memory snapshot
];
```

---

## Community & Support

### Release Channels
- **Stable**: Tested releases for general use
- **Beta**: Pre-release testing (via Discord)
- **Nightly**: Automated builds (use at own risk)

### Support Resources
- GitHub Issues: Bug reports and feature requests
- Discord Server: Real-time help and discussion
- GBAtemp Thread: Community support
- Wiki: Detailed guides and FAQ

### Contributing
```bash
# Fork and clone
git clone https://github.com/yourusername/dolphin-remote-switch
cd dolphin-remote-switch

# Build server
cd server
cargo build --release

# Build client (requires devkitPro)
cd ../switch-client
make

# Run tests
cargo test --all
```

---

## Legal Disclaimer

This software is provided for educational and development purposes only. Users are responsible for:
- Complying with local laws regarding emulation
- Owning legal copies of games
- Using only with authorized custom firmware
- Not distributing copyrighted content

The project is not affiliated with Nintendo, NVIDIA, or the Dolphin team.

---

## Future Roadmap

### Version 2.0 (Post-Release)
- [ ] **Multi-Console Support**
  - Citra (3DS)
  - Yuzu/Ryujinx (Switch)
  - PPSSPP (PSP)

- [ ] **Advanced Features**
  - Netplay support
  - Voice chat integration
  - Screenshot/video capture
  - Achievement system

- [ ] **Platform Expansion**
  - Android client
  - PS Vita port
  - Steam Deck optimization

### Research Areas
- AI upscaling (Real-ESRGAN)
- Frame interpolation
- Predictive input
- P2P multiplayer relay

---

## Appendix

### A. Build Scripts

```makefile
# switch-client/Makefile
include $(DEVKITPRO)/libnx/switch_rules

APP_TITLE := Dolphin Remote
APP_AUTHOR := DolphinTeam
APP_VERSION := 1.0.0
ICON := icon.jpg

TARGET := dolphin-remote
BUILD := build
SOURCES := src
INCLUDES := include

ARCH := -march=armv8-a+crc+crypto -mtune=cortex-a57 -mtp=soft -fPIE
CFLAGS := -Wall -O3 -ffunction-sections $(ARCH)
LDFLAGS := -specs=$(DEVKITPRO)/libnx/switch.specs -g $(ARCH)

.PHONY: clean all

all: $(TARGET).nro

$(TARGET).nro: $(TARGET).elf
	@elf2nro $< $@ --nacp=$(TARGET).nacp --icon=$(ICON)

clean:
	@rm -rf $(BUILD) $(TARGET).elf $(TARGET).nro $(TARGET).nacp
```

### B. Configuration Files

```toml
# config/server.toml
[dolphin]
executable = "/usr/bin/dolphin-emu"
rom_path = "/srv/games/gc-wii"
save_path = "/srv/saves"
config_path = "/etc/dolphin"

[streaming]
host = "0.0.0.0"
port = 47989
max_clients = 4
encoder = "nvenc"
preset = "llhq"  # Low Latency High Quality

[network]
upnp_enabled = true
mdns_enabled = true
ipv6_enabled = false

# config/client.toml
[connection]
auto_discover = true
preferred_hosts = ["192.168.1.100"]
reconnect_attempts = 3

[performance]
enable_overclock = true
cpu_boost = 1785  # MHz
gpu_boost = 921   # MHz
prefer_5ghz = true

[display]
handheld_resolution = "720p"
docked_resolution = "1080p"
target_fps = 60
vsync = true
```

### C. API Documentation

```rust
/// Main client API for Switch homebrew
pub mod dolphin_remote {
    /// Initialize the client library
    pub fn init() -> Result<Client>;
    
    /// Discover servers on local network
    pub fn discover_hosts(timeout: Duration) -> Result<Vec<Host>>;
    
    /// Connect to a specific host
    pub fn connect(host: &Host, config: StreamConfig) -> Result<Session>;
    
    /// Start streaming a game
    pub fn start_game(session: &Session, game_id: &str) -> Result<()>;
    
    /// Send controller input
    pub fn send_input(session: &Session, input: InputState) -> Result<()>;
    
    /// Get current performance metrics
    pub fn get_metrics(session: &Session) -> Metrics;
    
    /// Disconnect and cleanup
    pub fn disconnect(session: Session) -> Result<()>;
}
```

---

## Post-Launch Roadmap

### Version 1.1 (Month 5-6)
- [ ] Android/iOS native apps
- [ ] Controller profile marketplace
- [ ] Tournament mode

### Version 1.2 (Month 7-8)
- [ ] VR streaming support
- [ ] AI upscaling (DLSS/FSR)
- [ ] Multi-GPU support

### Version 2.0 (Month 9-12)
- [ ] Additional emulator support (PCSX2, RPCS3)
- [ ] Cloud gaming platform
- [ ] Blockchain-based achievements

---

## Communication Plan

### Daily Standup
- Time: 10:00 AM UTC
- Duration: 15 minutes
- Platform: Discord

### Sprint Review
- Frequency: Bi-weekly
- Duration: 2 hours
- Deliverables: Demo, metrics review

### Stakeholder Updates
- Frequency: Monthly
- Format: Progress report
- Metrics: KPI dashboard

---

## Definition of Done

A task is considered "Done" when:
- [ ] Code is written and follows style guide
- [ ] Unit tests pass with > 80% coverage
- [ ] Code review completed by 2+ developers
- [ ] Integration tests pass
- [ ] Documentation updated
- [ ] Performance benchmarks meet targets
- [ ] Security scan shows no critical issues
- [ ] Merged to main branch
- [ ] Deployed to staging environment

---

## Notes

- Priorities may shift based on user feedback
- Performance targets are for optimal network conditions
- Additional resources may be needed for scaling
- Regular security audits will be conducted
- Open source contributions are welcome after v1.0