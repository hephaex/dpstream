# Dolphin Remote Gaming System - Technical Architecture

## Project Overview

**Dolphin Remote Gaming System** is a high-performance streaming solution that enables remote play of GameCube/Wii games from an Ubuntu 24.04 server to Nintendo Switch devices with custom firmware. Built entirely in Rust, it leverages the Moonlight/GameStream protocol for low-latency streaming over Tailscale VPN.

## Repository Information

- **Repository**: `git@github.com:hephaex/dpstream.git`
- **Maintainer**: hephaex@gmail.com
- **Language**: Rust
- **License**: MIT

## Project Structure

```
dpstream/
├── server/                     # Ubuntu Server Components
│   ├── src/
│   │   ├── main.rs            # Server entry point
│   │   ├── emulator/          # Dolphin integration
│   │   ├── streaming/         # Moonlight host
│   │   └── network/           # Network management
│   └── Cargo.toml
│
├── switch-client/              # Nintendo Switch Homebrew
│   ├── src/
│   │   ├── main.rs            # Client entry point
│   │   ├── moonlight/         # Moonlight client
│   │   ├── input/             # Controller handling
│   │   └── display/           # Display management
│   └── Cargo.toml
│
├── scripts/                    # Build and deployment scripts
│   ├── build.sh               # Main build script
│   ├── deploy.sh              # Deployment script
│   ├── setup-dev.sh           # Development environment setup
│   └── git-workflow.sh        # Git automation script
│
├── docs/                       # Documentation
│   ├── scripts/               # Script documentation
│   │   ├── build-guide.md
│   │   ├── deployment.md
│   │   └── git-workflow.md
│   └── api/                   # API documentation
│
├── .history/                   # Development history
│   ├── phase1-complete.md
│   ├── phase2-complete.md
│   ├── sprint1-summary.md
│   └── ...
│
├── .env.example               # Environment configuration template
├── .gitignore
└── README.md
```

## Network Architecture with Tailscale

### VPN Configuration

```rust
// server/src/network/vpn.rs
use std::env;
use tailscale_rs::{Client, Config};

pub struct VpnManager {
    client: Client,
    config: TailscaleConfig,
}

#[derive(Debug, Clone)]
pub struct TailscaleConfig {
    auth_key: String,
    hostname: String,
    advertise_routes: Vec<String>,
    accept_dns: bool,
}

impl TailscaleConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            auth_key: env::var("TAILSCALE_AUTH_KEY")?,
            hostname: env::var("TAILSCALE_HOSTNAME")
                .unwrap_or_else(|_| "dpstream-server".to_string()),
            advertise_routes: env::var("TAILSCALE_ROUTES")
                .unwrap_or_else(|_| "192.168.1.0/24".to_string())
                .split(',')
                .map(String::from)
                .collect(),
            accept_dns: env::var("TAILSCALE_ACCEPT_DNS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
        })
    }
}

impl VpnManager {
    pub async fn new() -> Result<Self> {
        let config = TailscaleConfig::from_env()?;
        let client = Client::new(&config.auth_key).await?;
        
        Ok(Self { client, config })
    }
    
    pub async fn connect(&mut self) -> Result<String> {
        self.client.up(tailscale_rs::UpArgs {
            hostname: Some(&self.config.hostname),
            advertise_routes: &self.config.advertise_routes,
            accept_dns: self.config.accept_dns,
            ..Default::default()
        }).await?;
        
        self.client.ip().await
    }
}
```

### Environment Configuration (.env)

```bash
# Tailscale Configuration
TAILSCALE_AUTH_KEY=tskey-auth-xxxxxxxxxxxxx
TAILSCALE_HOSTNAME=dpstream-server
TAILSCALE_ROUTES=192.168.1.0/24
TAILSCALE_ACCEPT_DNS=true

# Server Configuration
SERVER_IP=100.64.0.1
SERVER_PORT=47989
MAX_CLIENTS=4

# Dolphin Configuration
DOLPHIN_PATH=/usr/bin/dolphin-emu
ROM_PATH=/srv/games/gc-wii
SAVE_PATH=/srv/saves

# Streaming Configuration
ENCODER_TYPE=nvenc
BITRATE=15000
RESOLUTION=1080p
FPS=60
```

## Architecture

### System Components

```
┌─────────────────────────────────────────────────────────┐
│                   Ubuntu 24.04 Server                    │
│                                                          │
│  ┌──────────────────────────────────────────────────┐   │
│  │           Dolphin Emulator Core                  │   │
│  │  - GameCube/Wii Emulation                       │   │
│  │  - OpenGL/Vulkan Rendering                      │   │
│  │  - Controller Input Processing                  │   │
│  └──────────────────────────────────────────────────┘   │
│                          │                               │
│  ┌──────────────────────────────────────────────────┐   │
│  │         Rust Streaming Server                    │   │
│  │  ┌─────────────────┐  ┌────────────────────┐   │   │
│  │  │  Video Capture  │  │  Audio Processing  │   │   │
│  │  │   (GStreamer)   │  │    (PulseAudio)   │   │   │
│  │  └─────────────────┘  └────────────────────┘   │   │
│  │                                                  │   │
│  │  ┌─────────────────┐  ┌────────────────────┐   │   │
│  │  │ Moonlight Host  │  │  Input Handler    │   │   │
│  │  │   (Sunshine)    │  │   (Controller)    │   │   │
│  │  └─────────────────┘  └────────────────────┘   │   │
│  │                                                  │   │
│  │  ┌─────────────────┐  ┌────────────────────┐   │   │
│  │  │ Session Manager │  │  NVIDIA GameStream│   │   │
│  │  │   (Tokio)      │  │    Compatible     │   │   │
│  │  └─────────────────┘  └────────────────────┘   │   │
│  └──────────────────────────────────────────────────┘   │
│                                                          │
└─────────────────────────────────────────────────────────┘
                            │
                    Moonlight Protocol
                            │
┌─────────────────────────────────────────────────────────┐
│              Nintendo Switch (CFW)                       │
│                                                          │
│  ┌──────────────────────────────────────────────────┐   │
│  │         Switch Homebrew Client (Rust)            │   │
│  │  ┌─────────────────┐  ┌────────────────────┐   │   │
│  │  │ Moonlight Client│  │  libnx Bindings   │   │   │
│  │  │    (Native)     │  │  (rust-bindgen)   │   │   │
│  │  └─────────────────┘  └────────────────────┘   │   │
│  │                                                  │   │
│  │  ┌─────────────────┐  ┌────────────────────┐   │   │
│  │  │  H264 Decoder   │  │  Input Processing  │   │   │
│  │  │ (HW Accelerated)│  │  (Joy-Con/Pro)    │   │   │
│  │  └─────────────────┘  └────────────────────┘   │   │
│  │                                                  │   │
│  │  ┌─────────────────┐  ┌────────────────────┐   │   │
│  │  │ Display Manager │  │  Network Stack    │   │   │
│  │  │  (720p/1080p)   │  │  (5GHz WiFi)      │   │   │
│  │  └─────────────────┘  └────────────────────┘   │   │
│  └──────────────────────────────────────────────────┘   │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

## Tech Stack

### Core Technologies

- **Dolphin Emulator**: GameCube/Wii 에뮬레이션 엔진
- **Rust**: 시스템 프로그래밍 언어
- **Moonlight/Sunshine**: NVIDIA GameStream 호환 스트리밍 프로토콜
- **libnx**: Nintendo Switch 홈브류 개발 라이브러리
- **GStreamer**: 멀티미디어 프레임워크
- **Tokio**: 비동기 런타임

### Server Side (Ubuntu)
```toml
[dependencies]
# Moonlight/GameStream Host
sunshine-rs = "0.3"  # Rust bindings for Sunshine
nvidia-ml = "0.8"    # NVIDIA GPU management

# Async Runtime
tokio = { version = "1.40", features = ["full"] }
async-trait = "0.1"

# Network & Protocols
hyper = "1.5"
tungstenite = "0.24"

# Multimedia Processing
gstreamer = "0.23"
gstreamer-app = "0.23"
gstreamer-video = "0.23"

# System Integration
nix = "0.29"
libc = "0.2"
x11 = "2.21"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error Handling
anyhow = "1.0"
thiserror = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"
```

#### Client Side (Nintendo Switch)
```toml
[dependencies]
# Nintendo Switch Support
libnx-rs = { git = "https://github.com/aarch64-switch-rs/libnx-rs" }
nx = { git = "https://github.com/aarch64-switch-rs/nx" }

# Moonlight Client
moonlight-common-c = "0.10"

# Graphics & Display
lvgl = "0.6"  # UI framework
sdl2 = { version = "0.36", features = ["bundled"] }

# Networking
native-tls = "0.2"
curl = "0.4"

# Video Decoding
ffmpeg-next = "7.0"

# Input Handling
gilrs = "0.11"  # Gamepad library

# Core
no-std-compat = "0.4"
linked_list_allocator = "0.10"
spin = "0.9"

[profile.release]
lto = true
opt-level = "z"
codegen-units = 1
panic = "abort"

[target.aarch64-nintendo-switch-freestanding]
rustflags = ["-C", "target-cpu=cortex-a57"]
```

## Module Structure

```
dolphin-remote-gaming/
├── server/                     # Ubuntu Server Components
│   ├── src/
│   │   ├── main.rs            # Server entry point
│   │   ├── emulator/          # Dolphin integration
│   │   │   ├── mod.rs
│   │   │   ├── process.rs     # Process management
│   │   │   └── config.rs      # Emulator configuration
│   │   │
│   │   ├── streaming/         # Moonlight host
│   │   │   ├── mod.rs
│   │   │   ├── sunshine.rs    # Sunshine integration
│   │   │   ├── capture.rs     # Screen capture
│   │   │   └── encoder.rs     # H264/H265 encoding
│   │   │
│   │   └── network/           # Network management
│   │       ├── mod.rs
│   │       ├── discovery.rs   # mDNS/UPnP
│   │       └── pairing.rs     # Client pairing
│   │
│   └── Cargo.toml
│
├── switch-client/              # Nintendo Switch Homebrew
│   ├── src/
│   │   ├── main.rs            # Client entry point
│   │   ├── moonlight/         # Moonlight client
│   │   │   ├── mod.rs
│   │   │   ├── stream.rs      # Stream handling
│   │   │   └── decoder.rs     # HW video decoding
│   │   │
│   │   ├── input/             # Controller handling
│   │   │   ├── mod.rs
│   │   │   ├── joycon.rs      # Joy-Con support
│   │   │   ├── pro.rs         # Pro Controller
│   │   │   └── touch.rs       # Touch input
│   │   │
│   │   ├── display/           # Display management
│   │   │   ├── mod.rs
│   │   │   ├── docked.rs      # Docked mode (1080p)
│   │   │   └── handheld.rs    # Handheld mode (720p)
│   │   │
│   │   └── sys/               # System bindings
│   │       ├── mod.rs
│   │       ├── libnx.rs       # libnx FFI
│   │       └── memory.rs      # Memory management
│   │
│   ├── Cargo.toml
│   ├── Makefile               # NRO build configuration
│   └── icon.jpg               # Homebrew icon
│
├── tools/                      # Development tools
│   ├── forwarder-gen/         # NSP forwarder generator
│   └── debug-bridge/          # Remote debugging
│
└── docs/                       # Documentation
```

## Key Features

### 1. Low Latency Streaming
- Moonlight/GameStream 프로토콜 활용
- Hardware accelerated H264/H265 디코딩 (Switch Tegra X1)
- 적응형 비트레이트 스트리밍
- 60 FPS 게임플레이 지원 (720p handheld / 1080p docked)

### 2. Nintendo Switch Integration
- Custom Firmware (Atmosphere) 지원
- Joy-Con 및 Pro Controller 네이티브 지원
- 터치스크린 마우스 에뮬레이션
- Gyro/가속도계 입력 지원
- HD Rumble 피드백

### 3. Network Optimization
- 5GHz WiFi 우선 지원
- mDNS 자동 호스트 발견
- NAT traversal (UPnP)
- 로컬 네트워크 최적화

### 4. Performance Features
- Tegra X1 GPU 하드웨어 디코딩
- Overclocking 지원 (sys-clk 연동)
- 메모리 최적화 (Full RAM access mode)
- 배터리 효율 모드

## Implementation Details

### Server-Side: Sunshine Integration

```rust
// server/src/streaming/sunshine.rs
use sunshine_rs::{Config, StreamServer};
use tokio::sync::mpsc;

pub struct DolphinStreamHost {
    server: StreamServer,
    dolphin_window_id: u32,
    config: HostConfig,
}

impl DolphinStreamHost {
    pub async fn new(config: HostConfig) -> Result<Self> {
        let sunshine_config = Config {
            encoder: "nvenc",  // Use NVIDIA hardware encoding
            bitrate: config.bitrate,
            resolution: config.resolution,
            fps: config.fps,
            ..Default::default()
        };
        
        let server = StreamServer::new(sunshine_config)?;
        
        Ok(Self {
            server,
            dolphin_window_id: 0,
            config,
        })
    }
    
    pub async fn start_stream(&mut self, client_id: &str) -> Result<()> {
        // Capture Dolphin window
        self.dolphin_window_id = self.find_dolphin_window()?;
        
        // Start Sunshine streaming
        self.server.add_client(client_id).await?;
        self.server.start_capture(self.dolphin_window_id).await?;
        
        Ok(())
    }
}
```

### Client-Side: Switch Homebrew

```rust
// switch-client/src/main.rs
#![no_std]
#![no_main]

extern crate libnx_rs;
use libnx_rs::{libnx, console, hid};
use moonlight_client::MoonlightStream;

#[no_mangle]
pub extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    // Initialize Switch services
    unsafe {
        libnx::consoleInit(core::ptr::null_mut());
        libnx::hidInitialize();
        libnx::socketInitializeDefault();
    }
    
    // Print to console
    println!("Dolphin Remote Gaming Client v1.0");
    println!("CFW Mode: Atmosphere");
    
    // Main loop
    let mut app = match DolphinClient::new() {
        Ok(app) => app,
        Err(e) => {
            println!("Failed to initialize: {:?}", e);
            return -1;
        }
    };
    
    app.run();
    
    // Cleanup
    unsafe {
        libnx::socketExit();
        libnx::hidExit();
        libnx::consoleExit(core::ptr::null_mut());
    }
    
    0
}

pub struct DolphinClient {
    moonlight: MoonlightStream,
    input_handler: InputHandler,
    display: DisplayManager,
}

impl DolphinClient {
    pub fn new() -> Result<Self> {
        let moonlight = MoonlightStream::new()?;
        let input_handler = InputHandler::new()?;
        let display = DisplayManager::new()?;
        
        Ok(Self {
            moonlight,
            input_handler,
            display,
        })
    }
    
    pub fn run(&mut self) {
        loop {
            // Update input
            self.input_handler.update();
            
            // Check for home button
            if self.input_handler.is_home_pressed() {
                break;
            }
            
            // Handle streaming
            if let Some(frame) = self.moonlight.get_frame() {
                self.display.render_frame(frame);
            }
            
            // Send input to server
            let input_state = self.input_handler.get_state();
            self.moonlight.send_input(input_state);
        }
    }
}
```

### Hardware Video Decoding on Switch

```rust
// switch-client/src/moonlight/decoder.rs
use libnx_rs::nvidia::{NvDecoder, NvFrame};

pub struct HardwareDecoder {
    decoder: NvDecoder,
    surface_pool: Vec<NvFrame>,
}

impl HardwareDecoder {
    pub fn new() -> Result<Self> {
        // Initialize NVIDIA decoder on Tegra X1
        let decoder = unsafe {
            NvDecoder::new_h264()?
        };
        
        // Pre-allocate decode surfaces
        let surface_pool = (0..4)
            .map(|_| NvFrame::new(1920, 1080))
            .collect::<Result<Vec<_>>>()?;
        
        Ok(Self {
            decoder,
            surface_pool,
        })
    }
    
    pub fn decode_frame(&mut self, data: &[u8]) -> Result<NvFrame> {
        // Hardware accelerated H264 decoding
        let surface = self.surface_pool.pop()
            .ok_or_else(|| anyhow!("No available decode surface"))?;
        
        let decoded = unsafe {
            self.decoder.decode_to_surface(data, surface)?
        };
        
        Ok(decoded)
    }
}
```

### Joy-Con Input Handling

```rust
// switch-client/src/input/joycon.rs
use libnx_rs::hid::{Controller, ControllerID, ControllerState};

pub struct JoyConHandler {
    controllers: Vec<Controller>,
    previous_state: ControllerState,
}

impl JoyConHandler {
    pub fn new() -> Result<Self> {
        let mut controllers = Vec::new();
        
        // Initialize all connected controllers
        for id in &[ControllerID::Player1, ControllerID::Handheld] {
            if let Ok(controller) = Controller::new(*id) {
                controllers.push(controller);
            }
        }
        
        Ok(Self {
            controllers,
            previous_state: Default::default(),
        })
    }
    
    pub fn update(&mut self) -> InputState {
        let mut state = InputState::default();
        
        for controller in &mut self.controllers {
            let current = controller.get_state();
            
            // Map Switch buttons to GameCube/Wii inputs
            state.a = current.buttons.contains(Buttons::A);
            state.b = current.buttons.contains(Buttons::B);
            state.x = current.buttons.contains(Buttons::X);
            state.y = current.buttons.contains(Buttons::Y);
            
            // Analog sticks
            state.left_stick_x = current.left_stick.x;
            state.left_stick_y = current.left_stick.y;
            state.right_stick_x = current.right_stick.x;
            state.right_stick_y = current.right_stick.y;
            
            // Gyro for Wii pointer emulation
            if let Some(gyro) = current.gyro {
                state.pointer_x = gyro.x;
                state.pointer_y = gyro.y;
            }
            
            // HD Rumble feedback
            if state != self.previous_state {
                controller.set_rumble(0.5, 0.5, 100);
            }
        }
        
        self.previous_state = state.clone();
        state
    }
}
```

## Performance Considerations

### Network Optimization
- **STUN/TURN 서버 구성**: NAT 통과를 위한 릴레이 서버
- **적응형 비트레이트**: 네트워크 상태에 따른 품질 조정
- **Jitter Buffer**: 패킷 손실 보정
- **FEC (Forward Error Correction)**: 오류 복구 메커니즘

### Resource Management
- **CPU 사용률 최적화**: 병렬 처리 및 SIMD 활용
- **메모리 풀링**: 빈번한 할당/해제 최소화
- **GPU 가속**: NVENC/VAAPI 하드웨어 인코딩
- **디스크 I/O 최적화**: 게임 데이터 캐싱

## Security

### Authentication & Authorization
```rust
pub struct AuthenticationMiddleware {
    jwt_secret: String,
    session_store: Arc<SessionStore>,
}

impl AuthenticationMiddleware {
    pub async fn verify_token(&self, token: &str) -> Result<Claims> {
        // JWT token verification
        let claims = decode_jwt(token, &self.jwt_secret)?;
        Ok(claims)
    }
}
```

### Encryption
- WebRTC DTLS-SRTP 암호화
- WSS (WebSocket Secure) 연결
- 저장 데이터 AES-256 암호화

## Deployment

### System Requirements

**Server (Ubuntu 24.04)**:
- CPU: 8+ cores (Ryzen 5 3600 or better)
- RAM: 16GB minimum
- GPU: NVIDIA GTX 1060 or better (for NVENC)
- Network: 100Mbps+ upload, 5GHz WiFi router
- Storage: 500GB+ for game ROMs

**Client (Nintendo Switch)**:
- Custom Firmware (Atmosphere 1.7.0+)
- Homebrew Menu access
- 5GHz WiFi connection
- SD Card: 2GB+ free space
- Sys-clk for overclocking (recommended)

### Switch Client Installation

```bash
# Build the homebrew app
cd switch-client
export DEVKITPRO=/opt/devkitpro
export DEVKITARM=$DEVKITPRO/devkitARM
export DEVKITPPC=$DEVKITPRO/devkitPPC

# Build NRO file
make

# Directory structure on SD card
/switch/
├── dolphin-remote/
│   ├── dolphin-remote.nro
│   ├── config.toml
│   └── icon.jpg
```

### Server Setup

```bash
# Install dependencies
sudo apt update
sudo apt install -y \
    build-essential \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    nvidia-cuda-toolkit \
    dolphin-emu

# Build and install Sunshine
git clone https://github.com/LizardByte/Sunshine.git
cd Sunshine
mkdir build && cd build
cmake ..
make -j$(nproc)
sudo make install

# Build Rust server
cd server
cargo build --release

# Configure systemd service
sudo cp dolphin-remote.service /etc/systemd/system/
sudo systemctl enable dolphin-remote
sudo systemctl start dolphin-remote
```

### NSP Forwarder Creation

```toml
# forwarder_config.toml
[forwarder]
title_id = "0100000000001337"
title_name = "Dolphin Remote"
author = "DolphinTeam"
version = "1.0.0"
icon = "icon.jpg"

[launch_params]
server_ip = "192.168.1.100"
auto_connect = true
game_id = "GALE01"  # Super Smash Bros. Melee
```

## Testing Strategy

### Unit Tests
- Module-level testing for each component
- Mock Dolphin process for testing
- WebRTC connection simulation

### Integration Tests
- End-to-end streaming pipeline
- Multi-client session handling
- Performance benchmarks

### Load Testing
- Concurrent user simulation
- Network condition emulation
- Resource usage monitoring

## Future Enhancements

1. **AI-Powered Upscaling**: DLSS/FSR integration for improved visual quality
2. **Cloud Save Sync**: Automatic save state synchronization
3. **Mobile Native Apps**: iOS/Android native clients
4. **Multi-GPU Support**: Distributed rendering for multiple sessions
5. **VR Streaming**: Support for VR game streaming
6. **Replay System**: Game session recording and replay

## Contributing

프로젝트에 기여하려면:
1. Repository fork
2. Feature branch 생성
3. 변경사항 commit
4. Branch에 push
5. Pull request 생성

## License

MIT License - 자유롭게 사용 및 수정 가능

## References

- [Dolphin Emulator Documentation](https://dolphin-emu.org/docs/)
- [WebRTC.rs Documentation](https://webrtc.rs/)
- [GStreamer Rust Bindings](https://gstreamer.freedesktop.org/bindings/rust.html)
- [Tokio Async Runtime](https://tokio.rs/)