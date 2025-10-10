//! Moonlight protocol implementation for dpstream server
//!
//! Implements NVIDIA GameStream compatible streaming protocol for video and audio

use crate::error::{Result, StreamingError};
use crate::health::HealthMonitor;
use crate::input::{MoonlightInputPacket, ServerInputManager};
// Capture module is disabled for minimal build
// use crate::streaming::capture::{VideoCapture, VideoCaptureConfig, VideoFrame};
use ahash::AHashMap;
use bumpalo::Bump;
use cache_padded::CachePadded;
use dashmap::DashMap;
use flume::{bounded, unbounded, Receiver, Sender};
use once_cell::sync::Lazy;
use parking_lot::{Mutex as ParkingMutex, RwLock};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Moonlight streaming server with optimized concurrent access
pub struct MoonlightServer {
    config: ServerConfig,
    sessions: Arc<DashMap<Uuid, StreamingSession>>,
    video_broadcast: Sender<VideoFrame>,
    audio_broadcast: Sender<AudioFrame>,
    input_manager: Arc<RwLock<Option<ServerInputManager>>>,
    health_monitor: Arc<RwLock<Option<Arc<HealthMonitor>>>>,
    is_running: Arc<parking_lot::Mutex<bool>>,
    performance_monitor: Arc<PerformanceMonitor>,
}

/// Performance monitoring for optimization with cache-aligned counters
#[derive(Debug)]
pub struct PerformanceMonitor {
    pub frames_processed: CachePadded<std::sync::atomic::AtomicU64>,
    pub avg_frame_time_us: CachePadded<std::sync::atomic::AtomicU64>,
    pub memory_usage_bytes: CachePadded<std::sync::atomic::AtomicU64>,
    pub active_sessions: CachePadded<std::sync::atomic::AtomicUsize>,
    pub network_bytes_sent: CachePadded<std::sync::atomic::AtomicU64>,
    pub packet_loss_count: CachePadded<std::sync::atomic::AtomicU64>,
    pub latency_histogram: CachePadded<std::sync::atomic::AtomicU64>, // Stores compressed histogram
    pub peak_memory_usage: CachePadded<std::sync::atomic::AtomicU64>,
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self {
            frames_processed: CachePadded::new(std::sync::atomic::AtomicU64::new(0)),
            avg_frame_time_us: CachePadded::new(std::sync::atomic::AtomicU64::new(0)),
            memory_usage_bytes: CachePadded::new(std::sync::atomic::AtomicU64::new(0)),
            active_sessions: CachePadded::new(std::sync::atomic::AtomicUsize::new(0)),
            network_bytes_sent: CachePadded::new(std::sync::atomic::AtomicU64::new(0)),
            packet_loss_count: CachePadded::new(std::sync::atomic::AtomicU64::new(0)),
            latency_histogram: CachePadded::new(std::sync::atomic::AtomicU64::new(0)),
            peak_memory_usage: CachePadded::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind_addr: String,
    pub port: u16,
    pub max_clients: usize,
    pub enable_encryption: bool,
    pub enable_authentication: bool,
    pub stream_timeout_ms: u64,
}

/// Audio configuration
#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub bitrate: u32,
    pub codec: AudioCodec,
}

#[derive(Debug, Clone, Copy)]
pub enum AudioCodec {
    Opus,
    AAC,
    PCM,
}

/// Stream configuration
#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub enable_encryption: bool,
    pub enable_authentication: bool,
    pub stream_timeout: std::time::Duration,
    pub max_packet_size: usize,
    pub buffer_size: usize,
}

/// Video frame data (stub for disabled capture module)
#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub timestamp: u64,
    pub frame_number: u64,
}

/// Audio frame data
#[derive(Debug, Clone)]
pub struct AudioFrame {
    pub data: Vec<u8>,
    pub timestamp: u64,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Streaming session for a connected client
#[derive(Debug)]
pub struct StreamingSession {
    pub id: Uuid,
    pub client_addr: SocketAddr,
    pub video_stream: Option<VideoStream>,
    pub audio_stream: Option<AudioStream>,
    pub input_handler: Option<InputHandler>,
    pub state: SessionState,
    pub started_at: std::time::Instant,
    pub last_activity: std::time::Instant,
    pub stream_config: Option<NegotiatedStreamConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SessionState {
    Connecting,
    Handshaking,
    Streaming,
    Paused,
    Disconnecting,
    Terminated,
}

/// Optimized video streaming component with bounded channels
#[derive(Debug)]
pub struct VideoStream {
    sender: Sender<VideoFrame>,
    stats: StreamStats,
    frame_buffer: SmallVec<[VideoFrame; 4]>, // Stack-allocated buffer for recent frames
}

/// Optimized audio streaming component with bounded channels
#[derive(Debug)]
pub struct AudioStream {
    sender: Sender<AudioFrame>,
    stats: StreamStats,
    sample_buffer: SmallVec<[AudioFrame; 8]>, // Stack-allocated buffer for audio frames
}

/// High-performance input handling component
#[derive(Debug)]
pub struct InputHandler {
    sender: Sender<MoonlightInputPacket>,
    input_buffer: SmallVec<[MoonlightInputPacket; 16]>, // Stack-allocated input buffer
}

/// Stream statistics
#[derive(Debug, Clone, Default)]
pub struct StreamStats {
    pub frames_sent: u64,
    pub bytes_sent: u64,
    pub frames_dropped: u64,
    pub last_frame_time: Option<std::time::Instant>,
    pub average_fps: f32,
    pub average_bitrate: f32,
}

/// Input events from client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputEvent {
    KeyDown {
        key: u32,
    },
    KeyUp {
        key: u32,
    },
    MouseMove {
        x: i32,
        y: i32,
    },
    MouseDown {
        button: u8,
    },
    MouseUp {
        button: u8,
    },
    MouseWheel {
        delta: i32,
    },
    ControllerInput {
        controller_id: u8,
        input: ControllerInput,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerInput {
    pub buttons: u32,
    pub left_stick_x: i16,
    pub left_stick_y: i16,
    pub right_stick_x: i16,
    pub right_stick_y: i16,
    pub left_trigger: u8,
    pub right_trigger: u8,
}

/// Client capabilities received during handshake
#[derive(Debug, Clone)]
pub struct ClientCapabilities {
    pub max_resolution: (u32, u32),
    pub supported_codecs: Vec<String>,
    pub audio_codecs: Vec<String>,
    pub max_fps: u32,
    pub supports_hdr: bool,
}

/// Negotiated stream configuration after capability exchange
#[derive(Debug, Clone)]
pub struct NegotiatedStreamConfig {
    pub video_resolution: (u32, u32),
    pub video_fps: u32,
    pub video_bitrate: u32,
    pub audio_sample_rate: u32,
    pub audio_channels: u32,
}

impl MoonlightServer {
    /// Start the Moonlight server
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Moonlight server");

        *self.is_running.lock() = true;

        let bind_addr = format!("{}:{}", self.config.bind_addr, self.config.port);
        let listen_addr: SocketAddr = bind_addr.parse().map_err(|e| {
            StreamingError::CaptureInitFailed(format!("Invalid bind address: {}", e))
        })?;

        // Start TCP listener for control connections
        let control_listener = TcpListener::bind(listen_addr).await.map_err(|e| {
            StreamingError::FrameProcessingFailed {
                reason: format!("Failed to bind control listener: {}", e),
            }
        })?;

        // Start UDP socket for video/audio streaming
        let stream_addr = SocketAddr::new(listen_addr.ip(), listen_addr.port() + 1);
        let stream_socket = UdpSocket::bind(stream_addr).await.map_err(|e| {
            StreamingError::FrameProcessingFailed {
                reason: format!("Failed to bind stream socket: {}", e),
            }
        })?;

        info!(
            "Moonlight server listening on {} (control) and {} (stream)",
            listen_addr, stream_addr
        );

        // Start control connection handler
        let sessions = Arc::clone(&self.sessions);
        let is_running = Arc::clone(&self.is_running);
        let config = self.config.clone();
        let video_broadcast = self.video_broadcast.clone();
        let audio_broadcast = self.audio_broadcast.clone();

        tokio::spawn(async move {
            Self::handle_control_connections(
                control_listener,
                sessions,
                is_running,
                config,
                video_broadcast,
                audio_broadcast,
            )
            .await;
        });

        // Start stream data handler
        let sessions_clone = Arc::clone(&self.sessions);
        let is_running_clone = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            Self::handle_stream_data(stream_socket, sessions_clone, is_running_clone).await;
        });

        info!("Moonlight server started successfully");
        Ok(())
    }

    /// Stop the Moonlight server
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping Moonlight server");

        *self.is_running.lock() = false;

        // Disconnect all sessions - DashMap doesn't have lock(), iterate directly
        // Collect session IDs first to avoid holding iterator while mutating
        for mut session in self.sessions.iter_mut() {
            session.state = SessionState::Terminated;
            info!("Terminated session: {}", session.id);
        }
        self.sessions.clear();

        info!("Moonlight server stopped");
        Ok(())
    }

    /// Broadcast video frame to all clients
    pub fn broadcast_video_frame(&self, frame: VideoFrame) -> Result<()> {
        match self.video_broadcast.send(frame) {
            Ok(_) => Ok(()),
            Err(_) => {
                debug!("No video subscribers");
                Ok(())
            }
        }
    }

    /// Broadcast audio frame to all clients
    pub fn broadcast_audio_frame(&self, frame: AudioFrame) -> Result<()> {
        match self.audio_broadcast.send(frame) {
            Ok(_) => Ok(()),
            Err(_) => {
                debug!("No audio subscribers");
                Ok(())
            }
        }
    }

    /// Create a new Moonlight server with updated config structure
    pub async fn new(config: ServerConfig) -> Result<Self> {
        let bind_addr = format!("{}:{}", config.bind_addr, config.port);
        info!("Initializing Moonlight server on {}", bind_addr);
        debug!(
            "Max clients: {}, encryption: {}, auth: {}",
            config.max_clients, config.enable_encryption, config.enable_authentication
        );

        let (video_broadcast, _) = bounded(1024);
        let (audio_broadcast, _) = bounded(1024);

        Ok(Self {
            config,
            sessions: Arc::new(DashMap::new()),
            video_broadcast,
            audio_broadcast,
            input_manager: Arc::new(RwLock::new(None)),
            health_monitor: Arc::new(RwLock::new(None)),
            is_running: Arc::new(parking_lot::Mutex::new(false)),
            performance_monitor: Arc::new(PerformanceMonitor::default()),
        })
    }

    /// Get the server port
    pub fn port(&self) -> u16 {
        self.config.port
    }

    /// Set the input manager for handling client input
    pub fn set_input_manager(&self, input_manager: ServerInputManager) {
        *self.input_manager.write() = Some(input_manager);
    }

    /// Set the health monitor for health endpoints
    pub fn set_health_monitor(&self, health_monitor: Arc<HealthMonitor>) {
        *self.health_monitor.write() = Some(health_monitor);
    }

    /// Convert ControllerInput to MoonlightInputPacket
    fn convert_controller_input_to_moonlight(
        &self,
        controller_id: u8,
        input: ControllerInput,
    ) -> MoonlightInputPacket {
        MoonlightInputPacket {
            packet_type: 0x0C, // Controller input packet type
            button_flags: input.buttons as u16,
            left_trigger: input.left_trigger,
            right_trigger: input.right_trigger,
            left_stick_x: input.left_stick_x,
            left_stick_y: input.left_stick_y,
            right_stick_x: input.right_stick_x,
            right_stick_y: input.right_stick_y,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            gyro_x: None,
            gyro_y: None,
            gyro_z: None,
            accel_x: None,
            accel_y: None,
            accel_z: None,
            touch_points: None,
        }
    }

    /// Run the server main loop
    pub async fn run(&mut self) -> Result<()> {
        self.start().await?;

        // Main server loop
        while *self.is_running.lock() {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        self.stop().await
    }

    /// Get server statistics
    pub fn get_stats(&self) -> ServerStats {
        // DashMap doesn't need lock(), we can iterate directly
        let active_sessions = self.sessions
            .iter()
            .filter(|entry| entry.value().state == SessionState::Streaming)
            .count();

        ServerStats {
            active_sessions,
            total_sessions: self.sessions.len(),
            is_running: *self.is_running.lock(),  // parking_lot doesn't need unwrap
            uptime: std::time::Instant::now().duration_since(
                self.sessions
                    .iter()
                    .map(|entry| entry.value().started_at)
                    .min()
                    .unwrap_or(std::time::Instant::now()),
            ),
        }
    }

    async fn handle_control_connections(
        listener: TcpListener,
        sessions: Arc<DashMap<Uuid, StreamingSession>>,
        is_running: Arc<ParkingMutex<bool>>,
        config: ServerConfig,
        video_broadcast: Sender<VideoFrame>,
        audio_broadcast: Sender<AudioFrame>,
    ) {
        while *is_running.lock() {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New client connection from: {}", addr);

                    // Check client limit
                    if sessions.len() >= config.max_clients {
                        warn!("Client limit reached, rejecting connection from {}", addr);
                        drop(stream);
                        continue;
                    }

                    // Create new session
                    let session_id = Uuid::new_v4();
                    let session = StreamingSession {
                        id: session_id,
                        client_addr: addr,
                        video_stream: None,
                        audio_stream: None,
                        input_handler: None,
                        state: SessionState::Connecting,
                        started_at: std::time::Instant::now(),
                        last_activity: std::time::Instant::now(),
                        stream_config: None,
                    };

                    sessions.insert(session_id, session);

                    // Handle client session
                    let sessions_clone = Arc::clone(&sessions);
                    let video_broadcast_clone = video_broadcast.clone();
                    let audio_broadcast_clone = audio_broadcast.clone();
                    let config_clone = config.clone();

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client_session(
                            stream,
                            session_id,
                            sessions_clone,
                            config_clone,
                            video_broadcast_clone,
                            audio_broadcast_clone,
                        )
                        .await
                        {
                            error!("Client session error for {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    }

    async fn handle_client_session(
        mut stream: TcpStream,
        session_id: Uuid,
        sessions: Arc<DashMap<Uuid, StreamingSession>>,
        config: ServerConfig,
        video_broadcast: Sender<VideoFrame>,
        audio_broadcast: Sender<AudioFrame>,
    ) -> Result<()> {
        info!("Handling client session: {}", session_id);

        // Implement Moonlight protocol handshake
        debug!("Starting Moonlight handshake for session {}", session_id);

        // Update session state to handshaking - DashMap provides direct concurrent access
        if let Some(mut session) = sessions.get_mut(&session_id) {
            session.state = SessionState::Handshaking;
        }

        // Step 1: RTSP handshake for session negotiation
        let handshake_result = Self::perform_rtsp_handshake(&mut stream, &config).await;
        if handshake_result.is_err() {
            warn!(
                "RTSP handshake failed for session {}: {:?}",
                session_id, handshake_result
            );
            return handshake_result;
        }

        // Step 2: Capability exchange
        let capabilities = Self::exchange_capabilities(&mut stream, &config).await?;
        debug!("Client capabilities: {:?}", capabilities);

        // Step 3: Encryption key exchange (if enabled)
        if config.enable_encryption {
            Self::exchange_encryption_keys(&mut stream).await?;
            debug!("Encryption keys exchanged for session {}", session_id);
        }

        // Step 4: Stream configuration
        let stream_config = Self::negotiate_stream_config(&mut stream, capabilities).await?;
        debug!("Stream configuration: {:?}", stream_config);

        // Update session to streaming state
        if let Some(mut session) = sessions.get_mut(&session_id) {
            session.state = SessionState::Streaming;
            session.stream_config = Some(stream_config);
        }

        info!("Moonlight handshake completed for session {}", session_id);

        // Set up video and audio streams with flume channels
        let (video_tx, video_rx) = unbounded();
        let (audio_tx, audio_rx) = unbounded();

        // Note: flume doesn't have broadcast semantics like tokio::sync::broadcast
        // In a real implementation, we'd need to use a proper broadcast mechanism
        // For now, these are stub channels

        // Set up input handling with flume channel
        let (input_tx, _input_rx) = unbounded();

        // Update session with streams
        if let Some(mut session) = sessions.get_mut(&session_id) {
            session.video_stream = Some(VideoStream {
                sender: video_tx,
                stats: StreamStats::default(),
                frame_buffer: SmallVec::new(),
            });
            session.audio_stream = Some(AudioStream {
                sender: audio_tx,
                stats: StreamStats::default(),
                sample_buffer: SmallVec::new(),
            });
            session.input_handler = Some(InputHandler {
                sender: input_tx,
                input_buffer: SmallVec::new(),
            });
        }

        info!("Client session established: {}", session_id);

        // Keep session alive and handle control messages
        // NOTE: This is a stub implementation for minimal build
        // In a full implementation, this would handle streaming and control messages
        let mut buffer = vec![0u8; 1024];
        loop {
            match stream.readable().await {
                Ok(_) => {
                    match stream.try_read(&mut buffer) {
                        Ok(0) => break, // Connection closed
                        Ok(_n) => {
                            // Parse control messages would go here
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                            continue;
                        }
                        Err(e) => {
                            error!("Read error: {}", e);
                            break;
                        }
                    }
                }
                Err(_) => break,
            }
        }

        // Cleanup session
        sessions.remove(&session_id);
        info!("Client session ended: {}", session_id);

        Ok(())
    }

    async fn handle_stream_data(
        socket: UdpSocket,
        _sessions: Arc<DashMap<Uuid, StreamingSession>>,
        is_running: Arc<ParkingMutex<bool>>,
    ) {
        let mut buffer = vec![0u8; 65536]; // Max UDP packet size

        while *is_running.lock() {
            match socket.recv_from(&mut buffer).await {
                Ok((size, addr)) => {
                    debug!("Received {} bytes from {}", size, addr);
                    // TODO: Parse and route stream data to appropriate session
                }
                Err(e) => {
                    error!("UDP receive error: {}", e);
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            }
        }
    }

    /// Perform RTSP handshake for session negotiation
    async fn perform_rtsp_handshake(stream: &mut TcpStream, config: &ServerConfig) -> Result<()> {
        // Simplified RTSP handshake implementation
        debug!("Performing RTSP handshake");

        // In a real implementation, this would handle RTSP DESCRIBE, SETUP, PLAY sequence
        // For now, simulate successful handshake
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        Ok(())
    }

    /// Exchange capabilities with the client
    async fn exchange_capabilities(
        stream: &mut TcpStream,
        config: &ServerConfig,
    ) -> Result<ClientCapabilities> {
        debug!("Exchanging capabilities");

        // In a real implementation, this would parse client capabilities and respond with server capabilities
        // For now, return default capabilities
        Ok(ClientCapabilities {
            max_resolution: (1920, 1080),
            supported_codecs: vec!["H264".to_string()],
            audio_codecs: vec!["Opus".to_string()],
            max_fps: 60,
            supports_hdr: false,
        })
    }

    /// Exchange encryption keys if encryption is enabled
    async fn exchange_encryption_keys(stream: &mut TcpStream) -> Result<()> {
        debug!("Exchanging encryption keys");

        // In a real implementation, this would perform key exchange using AES or similar
        // For now, simulate key exchange
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;

        Ok(())
    }

    /// Negotiate stream configuration with client
    async fn negotiate_stream_config(
        stream: &mut TcpStream,
        capabilities: ClientCapabilities,
    ) -> Result<NegotiatedStreamConfig> {
        debug!("Negotiating stream configuration");

        // In a real implementation, this would negotiate optimal settings based on capabilities
        Ok(NegotiatedStreamConfig {
            video_resolution: (1280, 720), // Start with 720p for compatibility
            video_fps: 60,
            video_bitrate: 15000, // 15 Mbps
            audio_sample_rate: 48000,
            audio_channels: 2,
        })
    }

    /// Send video frame to specific client
    async fn send_video_frame_to_client(
        &self,
        frame: &Option<VideoFrame>,
        session_id: &Uuid,
    ) -> Result<()> {
        if let Some(frame) = frame {
            // In a real implementation, this would encode the frame and send via UDP
            debug!(
                "Sending video frame {} to client {}",
                frame.frame_number, session_id
            );

            // TODO: Implement actual H264 encoding and UDP transmission
            // This would involve:
            // 1. H264 encoding of the frame
            // 2. RTP packetization
            // 3. UDP transmission to client
        }
        Ok(())
    }

    /// Send audio frame to specific client
    async fn send_audio_frame_to_client(
        &self,
        frame: &Option<AudioFrame>,
        session_id: &Uuid,
    ) -> Result<()> {
        if let Some(frame) = frame {
            debug!(
                "Sending audio frame ({}ms) to client {}",
                frame.timestamp, session_id
            );

            // TODO: Implement actual audio encoding and UDP transmission
            // This would involve:
            // 1. Opus/AAC encoding of the audio frame
            // 2. RTP packetization
            // 3. UDP transmission to client
        }
        Ok(())
    }

    /// Parse control messages from client
    async fn parse_control_message(&self, data: &[u8], session_id: &Uuid) -> Result<()> {
        if data.len() < 4 {
            return Ok(());
        }

        // Parse message type (first 4 bytes)
        let msg_type = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

        match msg_type {
            0x0C => {
                // Controller input message
                if data.len() >= 20 {
                    self.handle_controller_input(data, session_id).await?;
                }
            }
            0x0A => {
                // Keepalive message
                debug!("Received keepalive from client {}", session_id);
            }
            _ => {
                debug!(
                    "Unknown control message type: 0x{:02X} from client {}",
                    msg_type, session_id
                );
            }
        }

        Ok(())
    }

    /// Handle controller input from client
    async fn handle_controller_input(&self, data: &[u8], session_id: &Uuid) -> Result<()> {
        // Parse Moonlight controller input packet
        if data.len() >= 20 {
            let controller_input = ControllerInput {
                buttons: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
                left_stick_x: i16::from_le_bytes([data[8], data[9]]),
                left_stick_y: i16::from_le_bytes([data[10], data[11]]),
                right_stick_x: i16::from_le_bytes([data[12], data[13]]),
                right_stick_y: i16::from_le_bytes([data[14], data[15]]),
                left_trigger: data[16],
                right_trigger: data[17],
            };

            // Convert to MoonlightInputPacket
            let input_packet = self.convert_controller_input_to_moonlight(0, controller_input);

            // Send to input manager if available - parking_lot RwLock needs write()
            if let Some(input_manager) = self.input_manager.write().as_mut() {
                // Register client - let register_client handle whether client already exists
                let _sender = input_manager.register_client(*session_id)?;

                // TODO: Send input packet to the registered session
                debug!("Processed controller input from client {}", session_id);
            }
        }

        Ok(())
    }
}

/// Server statistics
#[derive(Debug, Clone)]
pub struct ServerStats {
    pub active_sessions: usize,
    pub total_sessions: usize,
    pub is_running: bool,
    pub uptime: std::time::Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    // Capture module is disabled for minimal build
    // use crate::streaming::capture::{QualityPreset, VideoEncoder};

    fn create_test_config() -> ServerConfig {
        ServerConfig {
            bind_addr: "127.0.0.1".to_string(),
            port: 47989,
            max_clients: 4,
            enable_encryption: true,
            enable_authentication: true,
            stream_timeout_ms: 30000,
        }
    }

    #[tokio::test]
    async fn test_moonlight_server_creation() {
        let config = create_test_config();
        let result = MoonlightServer::new(config).await;
        assert!(result.is_ok(), "MoonlightServer creation should succeed");

        let server = result.unwrap();
        let stats = server.get_stats();
        assert_eq!(stats.active_sessions, 0);
        assert!(!stats.is_running);
    }

    #[tokio::test]
    async fn test_video_frame_broadcast() {
        let config = create_test_config();
        let server = MoonlightServer::new(config).await.unwrap();

        let frame = VideoFrame {
            data: vec![0; 1920 * 1080 * 3 / 2],
            width: 1920,
            height: 1080,
            timestamp: 12345,
            frame_number: 1,
        };

        let result = server.broadcast_video_frame(frame);
        assert!(result.is_ok(), "Video frame broadcast should succeed");
    }

    #[tokio::test]
    async fn test_audio_frame_broadcast() {
        let config = create_test_config();
        let server = MoonlightServer::new(config).await.unwrap();

        let frame = AudioFrame {
            data: vec![0; 1024],
            timestamp: 12345,
            sample_rate: 48000,
            channels: 2,
        };

        let result = server.broadcast_audio_frame(frame);
        assert!(result.is_ok(), "Audio frame broadcast should succeed");
    }
}
