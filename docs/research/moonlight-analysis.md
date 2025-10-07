# Moonlight Protocol Technical Analysis

## Overview

Moonlight is an open-source implementation of NVIDIA's GameStream protocol, enabling low-latency game streaming between hosts and clients. This analysis covers integration with dpstream for Nintendo Switch streaming.

## Protocol Architecture

### Network Stack

```
Moonlight/GameStream Protocol Stack
┌─────────────────────────────────────────────┐
│              Application Layer              │
│  ┌─────────────────┐  ┌─────────────────┐  │
│  │  Game Control   │  │  Media Stream   │  │
│  │   Messages      │  │    Data         │  │
│  └─────────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────┘
                       │
┌─────────────────────────────────────────────┐
│             Transport Layer                 │
│  ┌─────────────────┐  ┌─────────────────┐  │
│  │    RTSP/HTTP    │  │      RTP        │  │
│  │   (Control)     │  │  (Media Data)   │  │
│  └─────────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────┘
                       │
┌─────────────────────────────────────────────┐
│              Security Layer                 │
│  ┌─────────────────┐  ┌─────────────────┐  │
│  │  TLS/DTLS       │  │   AES-128       │  │
│  │ (Handshake)     │  │ (Stream Data)   │  │
│  └─────────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────┘
                       │
┌─────────────────────────────────────────────┐
│               Network Layer                 │
│              TCP/UDP over IP                │
└─────────────────────────────────────────────┘
```

## Core Components

### 1. Service Discovery

**mDNS Broadcasting**
```
Service Type: _nvstream._tcp
Port: 47989 (default GameStream port)
TXT Records:
  - GfeVersion=3.20.4.14
  - mac=XX:XX:XX:XX:XX:XX
  - hostname=gamestream-server
  - uniqueid=XXXXXXXXXXXXXXXX
```

**Implementation in Rust**
```rust
use mdns_sd::{ServiceDaemon, ServiceInfo};

pub struct GameStreamDiscovery {
    service_daemon: ServiceDaemon,
}

impl GameStreamDiscovery {
    pub fn new() -> Result<Self> {
        let service_daemon = ServiceDaemon::new()?;
        Ok(Self { service_daemon })
    }

    pub fn advertise_service(&self, hostname: &str, port: u16) -> Result<()> {
        let service_info = ServiceInfo::new(
            "_nvstream._tcp.local.",
            &format!("{}._nvstream._tcp.local.", hostname),
            hostname,
            "127.0.0.1", // Will be replaced with actual IP
            port,
            &[
                ("GfeVersion", "3.20.4.14"),
                ("hostname", hostname),
                ("mac", "00:11:22:33:44:55"),
            ],
        )?;

        self.service_daemon.register(service_info)?;
        Ok(())
    }
}
```

### 2. Client Pairing

**Certificate Exchange Process**
```
1. Client → Server: GET /pair?uniqueid=<id>&uuid=<uuid>&devicename=<name>
2. Server → Client: 200 OK (XML response with pairing salt)
3. Client → Server: GET /pair?uniqueid=<id>&uuid=<uuid>&devicename=<name>&clientcert=<cert>
4. Server → Client: 200 OK (pairing complete)
```

**Pairing Implementation**
```rust
use openssl::rsa::Rsa;
use openssl::x509::X509;

pub struct PairingManager {
    server_keypair: Rsa<openssl::pkey::Private>,
    paired_clients: HashMap<String, ClientCert>,
}

impl PairingManager {
    pub async fn handle_pair_request(&mut self, req: PairRequest) -> Result<PairResponse> {
        match req.stage {
            PairStage::Initial => {
                let salt = generate_pairing_salt();
                Ok(PairResponse::Salt { salt })
            }
            PairStage::Challenge => {
                let client_cert = verify_client_certificate(&req.client_cert)?;
                self.paired_clients.insert(req.unique_id.clone(), client_cert);
                Ok(PairResponse::Success)
            }
        }
    }
}
```

### 3. Session Management

**Application List**
```xml
<!-- GET /applist response -->
<?xml version="1.0" encoding="utf-8"?>
<root protocol="1" status_code="200">
    <app>
        <AppTitle>Dolphin - Super Smash Bros. Melee</AppTitle>
        <ID>123456789</ID>
        <IsRunning>0</IsRunning>
        <MaxControllers>4</MaxControllers>
    </app>
</root>
```

**Stream Launch**
```
POST /launch
Parameters:
  - appid: Application ID
  - mode: Resolution (1920x1080x60, 1280x720x60)
  - additionalStates: Client capabilities
  - sops: Steam Overlay Position
  - rikey: Random initialization key
  - rikeyid: Key identifier
```

### 4. Media Streaming

**Video Stream (RTP/UDP)**
```
Port: 47998 (video)
Codec: H.264 (AVC)
Profile: High Profile Level 4.2
Bitrate: Adaptive (5-50 Mbps)
Keyframe Interval: 1-2 seconds
```

**Audio Stream (RTP/UDP)**
```
Port: 47996 (audio)
Codec: Opus
Sample Rate: 48 kHz
Channels: 2 (stereo) / 6 (5.1 surround)
Bitrate: 128-512 kbps
```

**Control Stream (TCP)**
```
Port: 47999 (control)
Protocol: Custom binary protocol
Purpose: Input events, keep-alive, statistics
```

## Switch-Specific Considerations

### 1. Hardware Constraints

**Video Decoding**
```rust
// Nintendo Switch hardware decoder capabilities
pub struct SwitchVideoCapabilities {
    pub max_resolution: (u32, u32),    // 1920x1080
    pub max_framerate: u32,            // 60 FPS
    pub supported_codecs: Vec<VideoCodec>,
    pub hardware_acceleration: bool,    // True (Tegra X1 NVDEC)
}

impl SwitchVideoCapabilities {
    pub fn tegra_x1() -> Self {
        Self {
            max_resolution: (1920, 1080),
            max_framerate: 60,
            supported_codecs: vec![
                VideoCodec::H264,
                VideoCodec::H265,
                VideoCodec::VP9,
            ],
            hardware_acceleration: true,
        }
    }
}
```

**Memory Limitations**
```rust
pub struct SwitchMemoryProfile {
    pub total_ram: u64,           // ~4GB total
    pub available_app: u64,       // ~3.2GB for homebrew
    pub video_buffer_size: u64,   // ~64MB for decode buffers
    pub audio_buffer_size: u64,   // ~8MB for audio buffers
}
```

### 2. Input Handling

**Joy-Con to GameStream Mapping**
```rust
pub struct JoyConMapping {
    pub buttons: ButtonMapping,
    pub analog: AnalogMapping,
    pub motion: MotionMapping,
    pub haptic: HapticMapping,
}

impl JoyConMapping {
    pub fn to_gamestream_input(&self, joycon_state: JoyConState) -> GameStreamInput {
        GameStreamInput {
            buttons: self.map_buttons(joycon_state.buttons),
            left_stick: self.map_analog(joycon_state.left_stick),
            right_stick: self.map_analog(joycon_state.right_stick),
            triggers: self.map_triggers(joycon_state.triggers),

            // Wii-specific mappings
            pointer: self.map_gyro_to_pointer(joycon_state.gyro),
            motion: self.map_motion_controls(joycon_state.accel, joycon_state.gyro),
        }
    }
}
```

**Input Packet Format**
```rust
#[repr(C, packed)]
pub struct GameStreamInputPacket {
    pub packet_type: u16,         // 0x0C for controller input
    pub sequence: u16,            // Sequence number
    pub timestamp: u32,           // High-resolution timestamp
    pub controller_id: u8,        // 0-3 for up to 4 controllers
    pub buttons: u32,             // Button bitmask
    pub left_stick_x: i16,        // -32768 to 32767
    pub left_stick_y: i16,
    pub right_stick_x: i16,
    pub right_stick_y: i16,
    pub left_trigger: u8,         // 0-255
    pub right_trigger: u8,        // 0-255

    // Wii-specific extensions
    pub pointer_x: i16,           // IR pointer (if available)
    pub pointer_y: i16,
    pub accel_x: i16,             // Accelerometer data
    pub accel_y: i16,
    pub accel_z: i16,
    pub gyro_x: i16,              // Gyroscope data
    pub gyro_y: i16,
    pub gyro_z: i16,
}
```

### 3. Network Optimization

**Tailscale Integration**
```rust
pub struct TailscaleGameStream {
    pub server_ip: std::net::IpAddr,
    pub direct_connection: bool,
    pub latency_ms: u16,
    pub bandwidth_mbps: u32,
}

impl TailscaleGameStream {
    pub async fn optimize_for_connection(&self) -> StreamConfig {
        StreamConfig {
            resolution: if self.bandwidth_mbps > 25 {
                Resolution::FHD_60  // 1920x1080@60
            } else {
                Resolution::HD_60   // 1280x720@60
            },
            bitrate: self.calculate_optimal_bitrate(),
            buffer_size: self.calculate_buffer_size(),
            fec_enabled: self.latency_ms > 50,  // Enable FEC for high latency
        }
    }
}
```

## Performance Optimizations

### 1. Latency Reduction

**Frame Pacing**
```rust
pub struct FramePacer {
    target_frametime: Duration,
    last_frame_time: Instant,
    frame_debt: Duration,
}

impl FramePacer {
    pub fn wait_for_next_frame(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame_time);

        if elapsed < self.target_frametime {
            let sleep_time = self.target_frametime - elapsed;
            std::thread::sleep(sleep_time);
        }

        self.last_frame_time = Instant::now();
    }
}
```

**Predictive Input**
```rust
pub struct InputPredictor {
    history: VecDeque<InputEvent>,
    prediction_window: Duration,
}

impl InputPredictor {
    pub fn predict_next_input(&self, network_latency: Duration) -> Option<InputEvent> {
        // Use input history to predict future input
        // Compensate for network latency
        None  // Placeholder
    }
}
```

### 2. Quality Adaptation

**Dynamic Bitrate Control**
```rust
pub struct BitrateController {
    current_bitrate: u32,
    target_latency: Duration,
    packet_loss_rate: f32,
    bandwidth_estimate: u32,
}

impl BitrateController {
    pub fn update(&mut self, stats: NetworkStats) -> u32 {
        if stats.latency > self.target_latency * 2 {
            // High latency: reduce bitrate
            self.current_bitrate = (self.current_bitrate * 80) / 100;
        } else if stats.packet_loss_rate < 0.01 {
            // Low packet loss: can increase bitrate
            self.current_bitrate = std::cmp::min(
                self.current_bitrate + 1_000_000,  // +1 Mbps
                self.bandwidth_estimate * 80 / 100  // Don't exceed 80% of available
            );
        }

        self.current_bitrate
    }
}
```

## Error Handling and Recovery

### 1. Connection Recovery
```rust
pub enum StreamError {
    NetworkTimeout,
    DecodingError,
    AuthenticationFailure,
    ServerDisconnected,
    InsufficientBandwidth,
}

pub struct StreamRecovery {
    retry_count: u32,
    backoff_duration: Duration,
}

impl StreamRecovery {
    pub async fn attempt_recovery(&mut self, error: StreamError) -> Result<()> {
        match error {
            StreamError::NetworkTimeout => {
                self.reconnect_with_backoff().await
            }
            StreamError::DecodingError => {
                self.request_keyframe().await
            }
            StreamError::InsufficientBandwidth => {
                self.reduce_quality().await
            }
            _ => Err(anyhow!("Unrecoverable error"))
        }
    }
}
```

### 2. Quality Degradation
```rust
pub struct QualityFallback {
    levels: Vec<StreamConfig>,
    current_level: usize,
}

impl QualityFallback {
    pub fn degrade_quality(&mut self) -> Option<&StreamConfig> {
        if self.current_level + 1 < self.levels.len() {
            self.current_level += 1;
            Some(&self.levels[self.current_level])
        } else {
            None  // Already at lowest quality
        }
    }
}
```

## Integration Architecture

### 1. Server Side (Host)
```rust
pub struct GameStreamServer {
    pub discovery: GameStreamDiscovery,
    pub pairing: PairingManager,
    pub sessions: SessionManager,
    pub encoder: VideoEncoder,
    pub audio: AudioCapture,
}

impl GameStreamServer {
    pub async fn start(&mut self, bind_addr: SocketAddr) -> Result<()> {
        // Start mDNS advertising
        self.discovery.advertise_service("dpstream-server", bind_addr.port())?;

        // Listen for client connections
        let listener = TcpListener::bind(bind_addr).await?;

        loop {
            let (stream, addr) = listener.accept().await?;
            self.handle_client(stream, addr).await?;
        }
    }
}
```

### 2. Client Side (Switch)
```rust
pub struct GameStreamClient {
    pub discovery: ServiceDiscovery,
    pub session: Option<StreamSession>,
    pub decoder: HardwareDecoder,
    pub input: InputManager,
    pub audio: AudioRenderer,
}

impl GameStreamClient {
    pub async fn connect_to_server(&mut self, server_addr: SocketAddr) -> Result<()> {
        // Establish control connection
        let control = ControlChannel::connect(server_addr).await?;

        // Negotiate stream parameters
        let config = self.negotiate_stream_config(&control).await?;

        // Start media streams
        let session = StreamSession::new(control, config).await?;
        self.session = Some(session);

        Ok(())
    }

    pub fn process_frame(&mut self) -> Result<()> {
        if let Some(session) = &mut self.session {
            // Receive video frame
            if let Some(frame) = session.receive_video_frame()? {
                self.decoder.decode_frame(frame)?;
            }

            // Receive audio data
            if let Some(audio) = session.receive_audio_data()? {
                self.audio.play_audio(audio)?;
            }

            // Send input updates
            let input_state = self.input.get_current_state();
            session.send_input(input_state)?;
        }

        Ok(())
    }
}
```

## Testing and Validation

### 1. Latency Measurement
```rust
pub struct LatencyTester {
    sent_timestamps: HashMap<u64, Instant>,
}

impl LatencyTester {
    pub fn send_test_input(&mut self, sequence: u64) {
        self.sent_timestamps.insert(sequence, Instant::now());
    }

    pub fn receive_video_frame(&mut self, sequence: u64) -> Duration {
        if let Some(sent_time) = self.sent_timestamps.remove(&sequence) {
            Instant::now().duration_since(sent_time)
        } else {
            Duration::from_millis(0)
        }
    }
}
```

### 2. Quality Metrics
```rust
pub struct StreamQuality {
    pub frame_drops: u32,
    pub decode_errors: u32,
    pub average_bitrate: u32,
    pub peak_latency: Duration,
    pub jitter: Duration,
}

impl StreamQuality {
    pub fn calculate_score(&self) -> f32 {
        // Composite quality score 0.0-1.0
        let drop_penalty = (self.frame_drops as f32) * 0.01;
        let error_penalty = (self.decode_errors as f32) * 0.05;
        let latency_penalty = (self.peak_latency.as_millis() as f32) / 1000.0;

        (1.0 - drop_penalty - error_penalty - latency_penalty).max(0.0)
    }
}
```

## References

- [Moonlight Documentation](https://github.com/moonlight-stream/moonlight-docs/wiki)
- [GameStream Protocol Reverse Engineering](https://github.com/limelight-stream/limelight-common)
- [NVIDIA GameStream Technical Details](https://docs.nvidia.com/gameworks/content/gameworkslibrary/coresdk/nvapi/_g_p_u.html)
- [Nintendo Switch Hardware Documentation](https://switchbrew.org/wiki/Hardware)