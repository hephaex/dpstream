# dpstream - Dolphin Remote Gaming System

**Stream GameCube/Wii games from Ubuntu servers to Nintendo Switch devices over Tailscale VPN**

![Version](https://img.shields.io/badge/version-1.0.0--alpha-orange)
![License](https://img.shields.io/badge/license-MIT-blue)
![Rust](https://img.shields.io/badge/rust-1.70+-orange)

## Overview

dpstream is a high-performance remote gaming solution that enables streaming of GameCube and Wii games from Ubuntu 24.04 servers to Nintendo Switch devices with custom firmware. Built entirely in Rust, it leverages the proven Moonlight/GameStream protocol for low-latency streaming over secure Tailscale VPN connections.

### Key Features

- 🎮 **Full GameCube/Wii Support** via Dolphin Emulator integration
- 🌐 **Secure VPN Streaming** using Tailscale for zero-configuration networking
- 📱 **Native Switch Client** optimized for Tegra X1 hardware acceleration
- 🎯 **Low Latency** targeting <30ms over good network connections
- 🎨 **High Quality** up to 1080p60 docked, 720p60 handheld
- 🎮 **Full Controller Support** Joy-Con, Pro Controller, Gyro, HD Rumble
- 🔒 **Security First** encrypted streaming with device authentication

## Quick Start

### Prerequisites

**Server Requirements:**
- Ubuntu 24.04 LTS
- 8+ core CPU (AMD Ryzen 5 3600 or better)
- 16GB+ RAM
- NVIDIA GPU with NVENC (GTX 1060 or better)
- Tailscale account

**Client Requirements:**
- Nintendo Switch with Atmosphere CFW 1.7.0+
- Homebrew Menu access
- 5GHz WiFi connection
- 2GB+ SD card space

### Installation

1. **Clone Repository**
   ```bash
   git clone git@github.com:hephaex/dpstream.git
   cd dpstream
   ```

2. **Setup Development Environment**
   ```bash
   ./scripts/setup-dev.sh
   ```

3. **Configure Environment**
   ```bash
   cp .env.example .env
   # Edit .env with your Tailscale auth key and paths
   ```

4. **Build Server**
   ```bash
   ./scripts/build.sh release server
   ```

5. **Build Switch Client** (requires devkitPro)
   ```bash
   ./scripts/build.sh release client
   ```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Ubuntu 24.04 Server                    │
│  ┌──────────────────────────────────────────────────┐   │
│  │           Dolphin Emulator Core                  │   │
│  │  - GameCube/Wii Emulation                       │   │
│  │  - OpenGL/Vulkan Rendering                      │   │
│  └──────────────────────────────────────────────────┘   │
│                          │                               │
│  ┌──────────────────────────────────────────────────┐   │
│  │         Rust Streaming Server                    │   │
│  │  • Tailscale VPN Integration                     │   │
│  │  • NVIDIA GameStream Host                        │   │
│  │  • Hardware H264/H265 Encoding                   │   │
│  │  • Session & Client Management                   │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
                            │
                    Moonlight Protocol over Tailscale
                            │
┌─────────────────────────────────────────────────────────┐
│              Nintendo Switch (CFW)                       │
│  ┌──────────────────────────────────────────────────┐   │
│  │         Switch Homebrew Client (Rust)            │   │
│  │  • Tailscale Network Discovery                   │   │
│  │  • Hardware H264 Decoding (Tegra X1)            │   │
│  │  • Native Input Processing                       │   │
│  │  • 720p/1080p Display Management                 │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

## Usage

### Server Side

1. **Start the server**
   ```bash
   ./target/release/dpstream-server
   ```

2. **Server will automatically:**
   - Connect to Tailscale network
   - Advertise gaming service
   - Wait for client connections

### Switch Client

1. **Copy NRO to SD card**
   ```
   /switch/dpstream/dpstream-client.nro
   ```

2. **Launch from Homebrew Menu**
   - Server discovery happens automatically
   - Select server and game
   - Start streaming!

### Controls

| Switch Input | GameCube/Wii Function |
|--------------|----------------------|
| Left Stick | GC Left Stick / Nunchuk |
| Right Stick | GC C-Stick / Camera |
| A/B/X/Y | GC A/B/X/Y |
| L/R | GC L/R Triggers |
| ZL/ZR | GC Z / Wii Z |
| Gyro | Wii Remote Pointer |
| Touch | Mouse Emulation |

## Development

### Project Structure

```
dpstream/
├── server/                 # Ubuntu server (Rust)
│   ├── src/
│   │   ├── main.rs        # Server entry point
│   │   ├── emulator/      # Dolphin integration
│   │   ├── streaming/     # Moonlight host
│   │   └── network/       # Tailscale networking
│   └── Cargo.toml
│
├── switch-client/         # Nintendo Switch client (Rust)
│   ├── src/
│   │   ├── main.rs       # Client entry point
│   │   ├── moonlight/    # Streaming protocol
│   │   ├── input/        # Controller handling
│   │   └── display/      # Video rendering
│   └── Cargo.toml
│
├── scripts/              # Build and deployment
├── docs/                 # Documentation
└── .history/            # Development logs
```

### Building from Source

```bash
# Development build
./scripts/build.sh debug all

# Release build
./scripts/build.sh release all

# Run tests
./scripts/build.sh test
```

### Sprint Development

This project follows an agile sprint methodology:

- **Sprint 1 (Week 1-2)**: Project setup and research ✅
- **Sprint 2 (Week 3-4)**: Core module development
- **Sprint 3 (Week 5-6)**: Media processing pipeline
- **Sprint 4 (Week 7-8)**: Input system implementation
- **Sprint 5 (Week 9-10)**: Performance optimization
- **Sprint 6 (Week 11-12)**: User experience
- **Sprint 7 (Week 13-14)**: Testing and debugging
- **Sprint 8 (Week 15-16)**: Polish and release

See `SPRINT_PLAN.md` for detailed roadmap.

## Performance

### Target Specifications

| Mode | Resolution | FPS | Latency | Bitrate |
|------|------------|-----|---------|---------|
| Handheld | 1280x720 | 60 | <30ms | 10 Mbps |
| Docked | 1920x1080 | 60 | <25ms | 20 Mbps |

### Optimizations

- **Hardware Encoding**: NVENC on server, NVDEC on Switch
- **Network**: Tailscale direct connections, 5GHz WiFi
- **Overclocking**: sys-clk integration for maximum performance
- **Memory**: Custom allocators, DMA optimizations

## Security

- **VPN-Only**: All traffic over encrypted Tailscale network
- **Authentication**: Device-based authentication via Tailscale identity
- **No Open Ports**: Zero configuration networking
- **CFW Safe**: No Nintendo service interaction

## Troubleshooting

### Common Issues

| Problem | Solution |
|---------|----------|
| Black screen | Check NVDEC initialization, verify H264 codec |
| High latency | Use 5GHz WiFi, enable performance mode |
| No controller | Recalibrate controllers in system settings |
| Connection fails | Check Tailscale connectivity: `tailscale ping server` |

### Debug Mode

Enable debug overlay with: `L + R + Plus`
- Shows FPS, latency, bitrate
- Network statistics
- Performance metrics

## Contributing

1. Fork the repository
2. Create feature branch: `git checkout -b feature/amazing-feature`
3. Commit changes: `git commit -m 'Add amazing feature'`
4. Push to branch: `git push origin feature/amazing-feature`
5. Open Pull Request

### Development Workflow

```bash
# Daily development cycle
./scripts/git-workflow.sh backup "Daily progress on feature X"

# Sprint completion
./scripts/git-workflow.sh sprint-complete "Sprint-N" "Summary" "Tasks" "Next"

# Phase completion
./scripts/git-workflow.sh phase-complete "Phase-N" "Summary" "Sprints" "Next"
```

## Roadmap

### Version 1.0 (Current)
- [x] Basic server/client architecture
- [x] Tailscale VPN integration
- [ ] Full Dolphin integration
- [ ] Switch homebrew client
- [ ] Performance optimization

### Version 1.1 (Q2 2024)
- [ ] Android/iOS native apps
- [ ] Multi-server support
- [ ] Save state sync

### Version 2.0 (Q4 2024)
- [ ] Additional emulators (Citra, PPSSPP)
- [ ] Cloud gaming features
- [ ] AI upscaling

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Disclaimer

This project is for educational and development purposes. Users are responsible for:
- Owning legal copies of games
- Complying with local laws regarding emulation
- Using only with authorized custom firmware

Not affiliated with Nintendo, NVIDIA, or the Dolphin team.

## Contact

- **Maintainer**: hephaex@gmail.com
- **Repository**: https://github.com/hephaex/dpstream
- **Issues**: https://github.com/hephaex/dpstream/issues

---

Built with ❤️ using Rust and powered by the amazing work of the Dolphin, Moonlight, and Tailscale teams.