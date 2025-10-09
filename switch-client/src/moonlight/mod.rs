//! Moonlight protocol implementation for Nintendo Switch
//!
//! GameStream-compatible client for streaming from dpstream server

pub mod audio;
pub mod decoder;

use crate::error::{Result, MoonlightError, NetworkError};
use crate::input::{InputState, MoonlightInput};
use crate::display::VideoFrame;
use self::audio::{AudioPlayer, AudioFrame};
use alloc::string::String;
use alloc::vec::Vec;
use heapless::Vec as HeaplessVec;

/// Main Moonlight client
pub struct MoonlightClient {
    state: ClientState,
    server_info: Option<ServerInfo>,
    stream_config: StreamConfig,
    network: NetworkManager,
    decoder: VideoDecoder,
    audio_player: Option<AudioPlayer>,
}

impl MoonlightClient {
    /// Create a new Moonlight client
    pub fn new() -> Result<Self> {
        Ok(Self {
            state: ClientState::Disconnected,
            server_info: None,
            stream_config: StreamConfig::default(),
            network: NetworkManager::new()?,
            decoder: VideoDecoder::new()?,
            audio_player: None,
        })
    }

    /// Discover dpstream servers on the network
    pub fn discover_servers(&mut self) -> Result<Vec<ServerInfo>> {
        self.network.discover_servers()
    }

    /// Connect to a specific server
    pub fn connect(&mut self, server: &ServerInfo) -> Result<()> {
        if self.state != ClientState::Disconnected {
            return Err(MoonlightError::HandshakeFailed.into());
        }

        self.state = ClientState::Connecting;
        self.server_info = Some(server.clone());

        // Perform Moonlight handshake
        self.perform_handshake(server)?;

        // Authenticate with server
        self.authenticate(server)?;

        self.state = ClientState::Connected;
        Ok(())
    }

    /// Start streaming session
    pub fn start_stream(&mut self) -> Result<()> {
        if self.state != ClientState::Connected {
            return Err(MoonlightError::HandshakeFailed.into());
        }

        self.state = ClientState::Streaming;

        // Send stream start request
        self.network.start_stream(&self.stream_config)?;

        // Initialize decoder
        self.decoder.initialize(&self.stream_config)?;

        // Initialize audio player
        let mut audio_player = AudioPlayer::new(&self.stream_config.audio_config)?;
        audio_player.initialize()?;
        self.audio_player = Some(audio_player);

        Ok(())
    }

    /// Send input to server
    pub fn send_input(&mut self, input: &InputState) -> Result<()> {
        if self.state != ClientState::Streaming {
            return Ok(());
        }

        let moonlight_input = input.to_moonlight_input();
        self.network.send_input(&moonlight_input)
    }

    /// Receive and decode a video frame
    pub fn receive_frame(&mut self) -> Result<Option<VideoFrame>> {
        if self.state != ClientState::Streaming {
            return Ok(None);
        }

        // Check for incoming video packets
        if let Some(packet) = self.network.receive_video_packet()? {
            // Process RTP packet
            self.process_video_packet(&packet)?;

            // Try to decode a complete frame
            if let Some(frame) = self.decoder.get_decoded_frame()? {
                return Ok(Some(frame));
            }
        }

        // Also process audio packets in parallel
        if let Some(audio_frame) = self.network.receive_audio_packet()? {
            if let Some(ref mut audio_player) = self.audio_player {
                audio_player.play_frame(audio_frame)?;
            }
        }

        Ok(None)
    }

    /// Optimized RTP packet processing with fast payload type routing
    pub fn process_video_packet(&mut self, packet: &[u8]) -> Result<()> {
        if self.state != ClientState::Streaming {
            return Ok(());
        }

        // Fast path: check payload type without full parsing for routing
        match RtpPacket::get_payload_type(packet) {
            Some(96) => {
                // H264 video stream - only parse when needed
                let rtp_packet = RtpPacket::parse(packet)?;
                self.process_h264_packet(&rtp_packet)?;
            }
            Some(97) => {
                // Audio stream (Opus/AAC) - only parse when needed
                if let Some(ref mut audio_player) = self.audio_player {
                    let rtp_packet = RtpPacket::parse(packet)?;
                    audio_player.queue_audio_packet(&rtp_packet)?;
                }
            }
            _ => {
                // Unknown payload type or invalid packet, ignore silently for performance
            }
        }

        Ok(())
    }

    /// Process H264 RTP packet
    fn process_h264_packet(&mut self, rtp_packet: &RtpPacket) -> Result<()> {
        // Handle H264 fragmentation (FU-A packets)
        if rtp_packet.payload.len() > 0 {
            let nal_header = rtp_packet.payload[0];
            let nal_type = nal_header & 0x1F;

            match nal_type {
                28 => {
                    // FU-A (fragmented unit)
                    self.handle_fu_a_packet(rtp_packet)?;
                }
                1..=23 => {
                    // Single NAL unit
                    self.decoder.queue_nal_unit(&rtp_packet.payload)?;
                }
                _ => {
                    // Other NAL unit types (SPS, PPS, etc.)
                    self.decoder.queue_nal_unit(&rtp_packet.payload)?;
                }
            }
        }

        Ok(())
    }

    /// Handle fragmented H264 packets (FU-A)
    fn handle_fu_a_packet(&mut self, rtp_packet: &RtpPacket) -> Result<()> {
        if rtp_packet.payload.len() < 2 {
            return Ok(());
        }

        let fu_header = rtp_packet.payload[1];
        let start_bit = (fu_header & 0x80) != 0;
        let end_bit = (fu_header & 0x40) != 0;

        if start_bit {
            // Start of fragmented NAL unit
            let nal_type = fu_header & 0x1F;
            let reconstructed_nal_header = (rtp_packet.payload[0] & 0xE0) | nal_type;

            self.decoder.start_fragmented_unit(reconstructed_nal_header)?;
        }

        // Add fragment data
        self.decoder.add_fragment(&rtp_packet.payload[2..])?;

        if end_bit {
            // End of fragmented NAL unit
            self.decoder.complete_fragmented_unit()?;
        }

        Ok(())
    }

    /// Receive and play an audio frame
    pub fn receive_audio_frame(&mut self) -> Result<()> {
        if self.state != ClientState::Streaming {
            return Ok(());
        }

        // Check for incoming audio packets
        if let Some(audio_frame) = self.network.receive_audio_packet()? {
            // Queue frame for playback
            if let Some(ref mut audio_player) = self.audio_player {
                audio_player.queue_frame(audio_frame)?;
            }
        }

        Ok(())
    }

    /// Get audio playback statistics
    pub fn get_audio_stats(&self) -> Option<audio::AudioStats> {
        self.audio_player.as_ref().map(|player| player.get_stats())
    }

    /// Disconnect from server
    pub fn disconnect(&mut self) -> Result<()> {
        match self.state {
            ClientState::Streaming => {
                self.network.stop_stream()?;
                self.decoder.cleanup()?;
                if let Some(mut audio_player) = self.audio_player.take() {
                    audio_player.shutdown()?;
                }
            }
            ClientState::Connected => {
                self.network.disconnect()?;
            }
            _ => {}
        }

        self.state = ClientState::Disconnected;
        self.server_info = None;

        Ok(())
    }

    /// Get current connection state
    pub fn get_state(&self) -> ClientState {
        self.state
    }

    /// Get current server info
    pub fn get_server_info(&self) -> Option<&ServerInfo> {
        self.server_info.as_ref()
    }

    /// Perform Moonlight handshake protocol
    fn perform_handshake(&mut self, server: &ServerInfo) -> Result<()> {
        // Step 1: Send initial handshake
        let handshake = HandshakeRequest {
            version: [7, 1, 408, 0], // Moonlight protocol version
            gfe_version: server.version_quad,
            client_version: "dpstream-switch-1.0.0",
        };

        self.network.send_handshake(&handshake)?;

        // Step 2: Receive handshake response
        let response = self.network.receive_handshake_response()?;

        if !response.success {
            return Err(MoonlightError::HandshakeFailed.into());
        }

        Ok(())
    }

    /// Authenticate with server
    fn authenticate(&mut self, server: &ServerInfo) -> Result<()> {
        // For dpstream, we use a simple token-based authentication
        let auth_request = AuthRequest {
            client_id: "dpstream-switch",
            auth_token: server.auth_token.clone(),
        };

        self.network.send_auth_request(&auth_request)?;

        let auth_response = self.network.receive_auth_response()?;

        if !auth_response.success {
            return Err(MoonlightError::AuthenticationFailed.into());
        }

        Ok(())
    }
}

/// Client connection state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClientState {
    Disconnected,
    Connecting,
    Connected,
    Streaming,
    Error,
}

/// Server information discovered via mDNS
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub address: String,
    pub port: u16,
    pub mac_address: String,
    pub version_quad: [u32; 4],
    pub auth_token: String,
    pub supported_codecs: HeaplessVec<VideoCodec, 4>,
}

/// Streaming configuration
#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub width: u16,
    pub height: u16,
    pub fps: u8,
    pub bitrate: u32, // kbps
    pub codec: VideoCodec,
    pub audio_config: AudioConfig,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            fps: 60,
            bitrate: 15000, // 15 Mbps
            codec: VideoCodec::H264,
            audio_config: AudioConfig::default(),
        }
    }
}

/// Video codec types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VideoCodec {
    H264,
    H265,
}

/// Audio configuration
#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub codec: AudioCodec,
    pub sample_rate: u32,
    pub channels: u8,
    pub bitrate: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            codec: AudioCodec::Opus,
            sample_rate: 48000,
            channels: 2,
            bitrate: 128,
        }
    }
}

/// Audio codec types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioCodec {
    Opus,
    AAC,
    PCM,
}

/// Network manager for Moonlight protocol
pub struct NetworkManager {
    // In real implementation, this would contain:
    // - TCP/UDP sockets
    // - TLS context
    // - Packet buffers
}

impl NetworkManager {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }

    pub fn discover_servers(&mut self) -> Result<Vec<ServerInfo>> {
        // Mock implementation - would use mDNS discovery
        let server = ServerInfo {
            name: "dpstream-server".to_string(),
            address: "100.64.0.1".to_string(),
            port: 47989,
            mac_address: "00:11:22:33:44:55".to_string(),
            version_quad: [7, 1, 408, 0],
            auth_token: "dpstream-auth-token".to_string(),
            supported_codecs: {
                let mut codecs = HeaplessVec::new();
                codecs.push(VideoCodec::H264).ok();
                codecs.push(VideoCodec::H265).ok();
                codecs
            },
        };

        let mut servers = Vec::new();
        servers.push(server);
        Ok(servers)
    }

    pub fn send_handshake(&mut self, _handshake: &HandshakeRequest<'_>) -> Result<()> {
        // Mock implementation
        Ok(())
    }

    pub fn receive_handshake_response(&mut self) -> Result<HandshakeResponse> {
        // Mock implementation
        Ok(HandshakeResponse { success: true })
    }

    pub fn send_auth_request(&mut self, _auth: &AuthRequest) -> Result<()> {
        // Mock implementation
        Ok(())
    }

    pub fn receive_auth_response(&mut self) -> Result<AuthResponse> {
        // Mock implementation
        Ok(AuthResponse { success: true })
    }

    pub fn start_stream(&mut self, _config: &StreamConfig) -> Result<()> {
        // Mock implementation
        Ok(())
    }

    pub fn stop_stream(&mut self) -> Result<()> {
        // Mock implementation
        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<()> {
        // Mock implementation
        Ok(())
    }

    pub fn send_input(&mut self, _input: &MoonlightInput) -> Result<()> {
        // Mock implementation
        Ok(())
    }

    pub fn receive_video_packet(&mut self) -> Result<Option<VideoPacket>> {
        // Mock implementation
        Ok(None)
    }

    pub fn receive_audio_packet(&mut self) -> Result<Option<AudioFrame>> {
        // Mock implementation
        Ok(None)
    }
}

/// Hardware video decoder
pub struct VideoDecoder {
    initialized: bool,
    nal_buffer: Vec<u8>,
    fragment_buffer: Vec<u8>,
    is_fragmenting: bool,
    decoded_frames: HeaplessVec<VideoFrame, 4>,
}

impl VideoDecoder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            initialized: false,
            nal_buffer: Vec::new(),
            fragment_buffer: Vec::new(),
            is_fragmenting: false,
            decoded_frames: HeaplessVec::new(),
        })
    }

    pub fn initialize(&mut self, _config: &StreamConfig) -> Result<()> {
        // In real implementation: initialize Tegra X1 hardware decoder
        // nvdecCreateDecoder() etc.
        self.initialized = true;
        Ok(())
    }

    /// Queue a complete NAL unit for decoding
    pub fn queue_nal_unit(&mut self, nal_data: &[u8]) -> Result<()> {
        if !self.initialized {
            return Err(MoonlightError::DecodingError.into());
        }

        // Add NAL unit to buffer
        self.nal_buffer.extend_from_slice(nal_data);

        // Try to decode if we have enough data
        self.try_decode_frame()?;

        Ok(())
    }

    /// Start a fragmented NAL unit
    pub fn start_fragmented_unit(&mut self, nal_header: u8) -> Result<()> {
        self.fragment_buffer.clear();
        self.fragment_buffer.push(nal_header);
        self.is_fragmenting = true;
        Ok(())
    }

    /// Add fragment data to current fragmented unit
    pub fn add_fragment(&mut self, data: &[u8]) -> Result<()> {
        if self.is_fragmenting {
            self.fragment_buffer.extend_from_slice(data);
        }
        Ok(())
    }

    /// Complete a fragmented NAL unit and queue for decoding
    pub fn complete_fragmented_unit(&mut self) -> Result<()> {
        if self.is_fragmenting {
            self.queue_nal_unit(&self.fragment_buffer)?;
            self.fragment_buffer.clear();
            self.is_fragmenting = false;
        }
        Ok(())
    }

    /// Try to decode a complete frame from buffered NAL units
    fn try_decode_frame(&mut self) -> Result<()> {
        // In real implementation: feed NAL units to hardware decoder
        // and check for completed frames

        // Mock: create a fake frame every few NAL units
        if self.nal_buffer.len() > 1000 {
            let frame = VideoFrame {
                width: 1280,
                height: 720,
                format: crate::display::PixelFormat::YUV420,
                planes: [
                    crate::display::Plane {
                        data: Vec::new(), // Would contain actual pixel data
                        stride: 1280,
                    },
                    crate::display::Plane {
                        data: Vec::new(),
                        stride: 640,
                    },
                    crate::display::Plane {
                        data: Vec::new(),
                        stride: 640,
                    },
                ],
                timestamp: 0,
            };

            if self.decoded_frames.push(frame).is_err() {
                // Buffer full, drop oldest frame
                self.decoded_frames.pop_at(0);
                self.decoded_frames.push(frame).ok();
            }

            self.nal_buffer.clear();
        }

        Ok(())
    }

    /// Get a decoded frame if available
    pub fn get_decoded_frame(&mut self) -> Result<Option<VideoFrame>> {
        if self.decoded_frames.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.decoded_frames.pop_at(0).unwrap()))
        }
    }

    pub fn cleanup(&mut self) -> Result<()> {
        self.initialized = false;
        self.nal_buffer.clear();
        self.fragment_buffer.clear();
        self.decoded_frames.clear();
        Ok(())
    }
}

/// Protocol message types
#[derive(Debug)]
pub struct HandshakeRequest<'a> {
    pub version: [u32; 4],
    pub gfe_version: [u32; 4],
    pub client_version: &'a str,
}

#[derive(Debug)]
pub struct HandshakeResponse {
    pub success: bool,
}

#[derive(Debug)]
pub struct AuthRequest {
    pub client_id: &'static str,
    pub auth_token: String,
}

#[derive(Debug)]
pub struct AuthResponse {
    pub success: bool,
}

/// Video packet from network
pub struct VideoPacket {
    pub sequence: u32,
    pub timestamp: u64,
    pub data: HeaplessVec<u8, 65536>, // 64KB max packet
}

/// RTP packet structure for video/audio streaming
#[derive(Debug)]
pub struct RtpPacket<'a> {
    pub version: u8,
    pub padding: bool,
    pub extension: bool,
    pub cc: u8,
    pub marker: bool,
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub payload: &'a [u8],
}

impl<'a> RtpPacket<'a> {
    /// Optimized RTP packet parsing with bounds checking and fast path for common cases
    #[inline(always)]
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        // Fast path: check minimum length for RTP header
        if data.len() < 12 {
            return Err(MoonlightError::InvalidPacket.into());
        }

        // Optimized field extraction using unsafe for performance (bounds already checked)
        let header_word1 = unsafe {
            u32::from_be_bytes([data[0], data[1], data[2], data[3]])
        };
        let header_word2 = unsafe {
            u32::from_be_bytes([data[4], data[5], data[6], data[7]])
        };
        let header_word3 = unsafe {
            u32::from_be_bytes([data[8], data[9], data[10], data[11]])
        };

        // Extract fields from packed header words
        let version = (header_word1 >> 30) as u8;
        let padding = (header_word1 & 0x20000000) != 0;
        let extension = (header_word1 & 0x10000000) != 0;
        let cc = ((header_word1 >> 24) & 0x0F) as u8;
        let marker = (header_word1 & 0x00800000) != 0;
        let payload_type = ((header_word1 >> 16) & 0x7F) as u8;
        let sequence_number = (header_word1 & 0xFFFF) as u16;
        let timestamp = header_word2;
        let ssrc = header_word3;

        // Fast path: most packets don't have CSRC identifiers
        let header_size = if cc == 0 {
            12
        } else {
            let extended_header_size = 12 + (cc as usize * 4);
            if data.len() < extended_header_size {
                return Err(MoonlightError::InvalidPacket.into());
            }
            extended_header_size
        };

        let payload = &data[header_size..];

        Ok(RtpPacket {
            version,
            padding,
            extension,
            cc,
            marker,
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
            payload,
        })
    }

    /// Fast validation without full parsing for filtering
    #[inline(always)]
    pub fn is_valid_header(data: &[u8]) -> bool {
        data.len() >= 12 && (data[0] >> 6) == 2 // Check RTP version
    }

    /// Extract payload type quickly for packet routing
    #[inline(always)]
    pub fn get_payload_type(data: &[u8]) -> Option<u8> {
        if data.len() >= 2 {
            Some(data[1] & 0x7F)
        } else {
            None
        }
    }
}

/// Audio player for decoded audio frames with hardware acceleration
pub struct AudioPlayer {
    is_initialized: bool,
    audio_buffer: HeaplessVec<AudioFrame, 8>,
    playback_state: PlaybackState,
    sample_rate: u32,
    channels: u8,
    volume: f32,
    decoder: Option<AudioDecoder>,
    // Buffer for audio samples ready for playback
    pcm_buffer: HeaplessVec<i16, 4096>,
    last_play_time: u64,
    underrun_count: u32,
}

impl AudioPlayer {
    pub fn new(config: &AudioConfig) -> Result<Self> {
        let decoder = match config.codec {
            AudioCodec::Opus => Some(AudioDecoder::new_opus(config.sample_rate, config.channels)?),
            AudioCodec::AAC => Some(AudioDecoder::new_aac(config.sample_rate, config.channels)?),
            AudioCodec::PCM => None, // No decoding needed for PCM
        };

        Ok(Self {
            is_initialized: false,
            audio_buffer: HeaplessVec::new(),
            playback_state: PlaybackState::Stopped,
            sample_rate: config.sample_rate,
            channels: config.channels,
            volume: 1.0,
            decoder,
            pcm_buffer: HeaplessVec::new(),
            last_play_time: 0,
            underrun_count: 0,
        })
    }

    pub fn initialize(&mut self) -> Result<()> {
        if self.is_initialized {
            return Ok(());
        }

        // Initialize Switch audio subsystem
        unsafe {
            // In real implementation: audoutInitialize()
            // Configure audio output parameters
        }

        self.is_initialized = true;
        self.playback_state = PlaybackState::Ready;

        Ok(())
    }

    pub fn queue_audio_packet(&mut self, rtp_packet: &RtpPacket) -> Result<()> {
        if !self.is_initialized {
            self.initialize()?;
        }

        // Extract audio data from RTP packet
        let audio_data = rtp_packet.payload;
        let timestamp = rtp_packet.timestamp;

        // Decode audio based on codec
        let pcm_samples = if let Some(ref mut decoder) = self.decoder {
            decoder.decode(audio_data)?
        } else {
            // Assume PCM data if no decoder
            self.convert_raw_to_pcm(audio_data)?
        };

        // Queue PCM samples for playback
        self.queue_pcm_samples(&pcm_samples, timestamp as u64)?;

        Ok(())
    }

    pub fn play_frame(&mut self, frame: AudioFrame) -> Result<()> {
        if !self.is_initialized {
            self.initialize()?;
        }

        // Add frame to buffer for processing
        if self.audio_buffer.push(frame).is_err() {
            // Buffer full, drop oldest frame
            let _ = self.audio_buffer.pop();
            self.audio_buffer.push(frame).map_err(|_|
                crate::error::ClientError::AudioBufferOverflow
            )?;
        }

        // Process audio frames in buffer
        self.process_audio_buffer()?;

        Ok(())
    }

    fn process_audio_buffer(&mut self) -> Result<()> {
        while let Some(frame) = self.audio_buffer.pop() {
            // Decode frame if needed
            let pcm_data = if let Some(ref mut decoder) = self.decoder {
                decoder.decode(&frame.data)?
            } else {
                self.convert_raw_to_pcm(&frame.data)?
            };

            // Queue for immediate playback
            self.queue_pcm_samples(&pcm_data, frame.timestamp)?;
        }

        Ok(())
    }

    fn queue_pcm_samples(&mut self, samples: &[i16], timestamp: u64) -> Result<()> {
        // Apply volume adjustment
        let adjusted_samples: HeaplessVec<i16, 1024> = samples.iter()
            .map(|&sample| ((sample as f32) * self.volume) as i16)
            .take(1024)
            .collect();

        // Add to PCM buffer
        for &sample in adjusted_samples.iter() {
            if self.pcm_buffer.push(sample).is_err() {
                // Buffer full, play current buffer
                self.flush_audio_buffer()?;
                self.pcm_buffer.push(sample).map_err(|_|
                    crate::error::ClientError::AudioBufferOverflow
                )?;
            }
        }

        // Check if we have enough samples for playback
        if self.pcm_buffer.len() >= 480 { // 10ms worth at 48kHz
            self.flush_audio_buffer()?;
        }

        self.last_play_time = timestamp;
        Ok(())
    }

    fn flush_audio_buffer(&mut self) -> Result<()> {
        if self.pcm_buffer.is_empty() {
            return Ok(());
        }

        let samples = self.pcm_buffer.as_slice();

        // Play audio samples through Switch audio system
        unsafe {
            // In real implementation: audoutPlayBuffer()
            // This would send PCM data to the Switch audio subsystem
            self.play_pcm_samples(samples)?;
        }

        self.pcm_buffer.clear();
        self.playback_state = PlaybackState::Playing;

        Ok(())
    }

    unsafe fn play_pcm_samples(&mut self, samples: &[i16]) -> Result<()> {
        // Mock implementation - in reality this would interface with libnx audio
        // audoutPlayBuffer() would be called here with proper buffer management

        if samples.len() == 0 {
            self.underrun_count += 1;
            return Err(crate::error::ClientError::AudioUnderrun.into());
        }

        Ok(())
    }

    fn convert_raw_to_pcm(&self, data: &[u8]) -> Result<HeaplessVec<i16, 1024>> {
        let mut pcm_samples = HeaplessVec::new();

        // Convert bytes to 16-bit PCM samples
        for chunk in data.chunks_exact(2) {
            if chunk.len() == 2 {
                let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                if pcm_samples.push(sample).is_err() {
                    break; // Buffer full
                }
            }
        }

        Ok(pcm_samples)
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    pub fn get_playback_state(&self) -> PlaybackState {
        self.playback_state
    }

    pub fn get_buffer_level(&self) -> f32 {
        self.pcm_buffer.len() as f32 / self.pcm_buffer.capacity() as f32
    }

    pub fn cleanup(&mut self) -> Result<()> {
        self.playback_state = PlaybackState::Stopped;
        self.pcm_buffer.clear();
        self.audio_buffer.clear();

        if self.is_initialized {
            unsafe {
                // In real implementation: audoutExit()
            }
            self.is_initialized = false;
        }

        Ok(())
    }
}

/// Audio frame from stream
#[derive(Debug, Clone)]
pub struct AudioFrame {
    pub data: HeaplessVec<u8, 1024>,
    pub timestamp: u64,
    pub codec: AudioCodec,
    pub sample_rate: u32,
    pub channels: u8,
    pub duration_ms: u16,
}

/// Audio playback state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Ready,
    Playing,
    Paused,
    Buffering,
    Error,
}

/// Audio decoder for various codecs
pub struct AudioDecoder {
    codec: AudioCodec,
    sample_rate: u32,
    channels: u8,
    initialized: bool,
    // Decoder state would be stored here
    opus_state: Option<u64>, // Mock pointer to opus decoder
    aac_state: Option<u64>,  // Mock pointer to AAC decoder
}

impl AudioDecoder {
    pub fn new_opus(sample_rate: u32, channels: u8) -> Result<Self> {
        let mut decoder = Self {
            codec: AudioCodec::Opus,
            sample_rate,
            channels,
            initialized: false,
            opus_state: None,
            aac_state: None,
        };

        decoder.initialize_opus()?;
        Ok(decoder)
    }

    pub fn new_aac(sample_rate: u32, channels: u8) -> Result<Self> {
        let mut decoder = Self {
            codec: AudioCodec::AAC,
            sample_rate,
            channels,
            initialized: false,
            opus_state: None,
            aac_state: None,
        };

        decoder.initialize_aac()?;
        Ok(decoder)
    }

    fn initialize_opus(&mut self) -> Result<()> {
        // In real implementation: opus_decoder_create()
        self.opus_state = Some(0x12345678); // Mock state
        self.initialized = true;
        Ok(())
    }

    fn initialize_aac(&mut self) -> Result<()> {
        // In real implementation: AAC decoder initialization
        self.aac_state = Some(0x87654321); // Mock state
        self.initialized = true;
        Ok(())
    }

    pub fn decode(&mut self, encoded_data: &[u8]) -> Result<HeaplessVec<i16, 1024>> {
        if !self.initialized {
            return Err(crate::error::ClientError::AudioDecoderNotInitialized.into());
        }

        match self.codec {
            AudioCodec::Opus => self.decode_opus(encoded_data),
            AudioCodec::AAC => self.decode_aac(encoded_data),
            AudioCodec::PCM => self.decode_pcm(encoded_data),
        }
    }

    fn decode_opus(&mut self, data: &[u8]) -> Result<HeaplessVec<i16, 1024>> {
        let mut samples = HeaplessVec::new();

        // Mock Opus decoding - in real implementation:
        // opus_decode() would be called here

        // For demonstration, convert input bytes to samples
        for chunk in data.chunks_exact(2) {
            if chunk.len() == 2 {
                let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                if samples.push(sample).is_err() {
                    break;
                }
            }
        }

        Ok(samples)
    }

    fn decode_aac(&mut self, data: &[u8]) -> Result<HeaplessVec<i16, 1024>> {
        let mut samples = HeaplessVec::new();

        // Mock AAC decoding - in real implementation:
        // AAC decoder library would be used

        for chunk in data.chunks_exact(2) {
            if chunk.len() == 2 {
                let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                if samples.push(sample).is_err() {
                    break;
                }
            }
        }

        Ok(samples)
    }

    fn decode_pcm(&mut self, data: &[u8]) -> Result<HeaplessVec<i16, 1024>> {
        let mut samples = HeaplessVec::new();

        for chunk in data.chunks_exact(2) {
            if chunk.len() == 2 {
                let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                if samples.push(sample).is_err() {
                    break;
                }
            }
        }

        Ok(samples)
    }

    pub fn cleanup(&mut self) -> Result<()> {
        if self.initialized {
            match self.codec {
                AudioCodec::Opus => {
                    // opus_decoder_destroy()
                    self.opus_state = None;
                }
                AudioCodec::AAC => {
                    // AAC decoder cleanup
                    self.aac_state = None;
                }
                AudioCodec::PCM => {}
            }
            self.initialized = false;
        }
        Ok(())
    }
}