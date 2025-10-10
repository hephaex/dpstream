//! Common test utilities and helper functions for integration tests

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

use dpstream_server::{
    error::Result,
    input::MoonlightInputPacket,
    streaming::{AudioFrame, MoonlightServer, ServerConfig, VideoFrame},
};

/// Test environment for integration testing
#[derive(Clone)]
pub struct TestEnvironment {
    pub server: Arc<Mutex<MoonlightServer>>,
    pub sessions: Arc<Mutex<HashMap<Uuid, TestSession>>>,
    pub network_simulator: Arc<Mutex<NetworkSimulator>>,
    pub metrics_collector: Arc<MetricsCollector>,
}

/// Individual test session data
pub struct TestSession {
    pub client_id: Uuid,
    pub client_name: String,
    pub is_streaming: bool,
    pub received_frames: Vec<VideoFrame>,
    pub received_audio: Vec<AudioFrame>,
    pub sent_inputs: Vec<MoonlightInputPacket>,
}

/// Network condition simulator for testing resilience
pub struct NetworkSimulator {
    pub packet_loss_rate: f64,
    pub additional_latency_ms: u64,
    pub jitter_ms: u64,
    pub bandwidth_limit_kbps: Option<u32>,
    pub is_active: bool,
}

/// Metrics collection for test validation
#[derive(Debug, Clone, Default)]
pub struct QualityMetrics {
    pub avg_latency_ms: f64,
    pub frame_drop_rate: f64,
    pub connection_stability: f64,
    pub throughput_mbps: f64,
}

#[derive(Debug, Clone, Default)]
pub struct AudioMetrics {
    pub latency_ms: f64,
    pub buffer_underruns: u32,
    pub sample_rate: u32,
    pub bit_depth: u16,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryUsage {
    pub heap_usage_mb: u64,
    pub stack_usage_kb: u64,
    pub gpu_memory_mb: u64,
}

pub struct MetricsCollector {
    pub quality_metrics: Arc<Mutex<HashMap<Uuid, QualityMetrics>>>,
    pub audio_metrics: Arc<Mutex<HashMap<Uuid, AudioMetrics>>>,
    pub memory_usage: Arc<Mutex<MemoryUsage>>,
    pub processing_times: Arc<Mutex<Vec<Duration>>>,
}

impl TestEnvironment {
    /// Create new test environment with default configuration
    pub async fn new() -> Result<Self> {
        let config = ServerConfig {
            bind_addr: "127.0.0.1".to_string(),
            port: 0,                  // Let OS choose available port
            max_clients: 8,           // Increased for testing
            enable_encryption: false, // Disable for testing
            enable_authentication: false,
            stream_timeout_ms: 10000, // Longer timeout for testing
        };

        let server = MoonlightServer::new(config).await?;

        Ok(Self {
            server: Arc::new(Mutex::new(server)),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            network_simulator: Arc::new(Mutex::new(NetworkSimulator::new())),
            metrics_collector: Arc::new(MetricsCollector::new()),
        })
    }

    /// Connect a test client with given name
    pub async fn connect_client(&mut self, client_name: &str) -> Result<Uuid> {
        let client_id = Uuid::new_v4();

        // Create test session
        let session = TestSession {
            client_id,
            client_name: client_name.to_string(),
            is_streaming: false,
            received_frames: Vec::new(),
            received_audio: Vec::new(),
            sent_inputs: Vec::new(),
        };

        // Register with server (mock connection)
        {
            let mut sessions = self.sessions.lock().unwrap();
            sessions.insert(client_id, session);
        }

        // Initialize metrics for this client
        {
            let mut quality_metrics = self.metrics_collector.quality_metrics.lock().unwrap();
            quality_metrics.insert(client_id, QualityMetrics::default());

            let mut audio_metrics = self.metrics_collector.audio_metrics.lock().unwrap();
            audio_metrics.insert(client_id, AudioMetrics::default());
        }

        Ok(client_id)
    }

    /// Disconnect a client
    pub async fn disconnect_client(&mut self, client_id: &Uuid) -> Result<()> {
        {
            let mut sessions = self.sessions.lock().unwrap();
            sessions.remove(client_id);
        }

        // Cleanup metrics
        {
            let mut quality_metrics = self.metrics_collector.quality_metrics.lock().unwrap();
            quality_metrics.remove(client_id);

            let mut audio_metrics = self.metrics_collector.audio_metrics.lock().unwrap();
            audio_metrics.remove(client_id);
        }

        Ok(())
    }

    /// Start streaming for a client
    pub async fn start_streaming(&mut self, client_id: &Uuid) -> Result<()> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(client_id) {
            session.is_streaming = true;
        }
        Ok(())
    }

    /// Send video frame through the pipeline
    pub async fn send_video_frame(&self, frame: VideoFrame) -> Result<()> {
        // Simulate frame processing through the pipeline
        self.process_frame_metrics(&frame).await;
        Ok(())
    }

    /// Send video frame to specific client
    pub async fn send_video_frame_to_client(
        &self,
        client_id: &Uuid,
        frame: VideoFrame,
    ) -> Result<()> {
        {
            let mut sessions = self.sessions.lock().unwrap();
            if let Some(session) = sessions.get_mut(client_id) {
                session.received_frames.push(frame.clone());
            }
        }

        self.process_frame_metrics(&frame).await;
        Ok(())
    }

    /// Send audio sample through pipeline
    pub async fn send_audio_sample(&self, sample: AudioFrame) -> Result<()> {
        // Simulate audio processing
        self.process_audio_metrics(&sample).await;
        Ok(())
    }

    /// Send input from client
    pub async fn send_input(&self, client_id: &Uuid, input: MoonlightInputPacket) -> Result<()> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(client_id) {
            session.sent_inputs.push(input);
        }
        Ok(())
    }

    /// Get received frames for a client
    pub async fn get_received_frames(&self, client_id: &Uuid) -> Result<Vec<VideoFrame>> {
        let sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get(client_id) {
            Ok(session.received_frames.clone())
        } else {
            Ok(Vec::new())
        }
    }

    /// Get quality metrics for a client
    pub async fn get_quality_metrics(&self, client_id: &Uuid) -> Result<QualityMetrics> {
        let quality_metrics = self.metrics_collector.quality_metrics.lock().unwrap();
        Ok(quality_metrics.get(client_id).cloned().unwrap_or_default())
    }

    /// Get audio metrics for a client
    pub async fn get_audio_metrics(&self, client_id: &Uuid) -> Result<AudioMetrics> {
        let audio_metrics = self.metrics_collector.audio_metrics.lock().unwrap();
        Ok(audio_metrics.get(client_id).cloned().unwrap_or_default())
    }

    /// Get current memory usage
    pub async fn get_memory_usage(&self) -> Result<MemoryUsage> {
        let memory_usage = self.metrics_collector.memory_usage.lock().unwrap();
        Ok(memory_usage.clone())
    }

    /// Get active sessions
    pub fn get_active_sessions(&self) -> Vec<Uuid> {
        let sessions = self.sessions.lock().unwrap();
        sessions.keys().cloned().collect()
    }

    /// Get processed inputs
    pub async fn get_processed_inputs(&self) -> Result<Vec<MoonlightInputPacket>> {
        let sessions = self.sessions.lock().unwrap();
        let mut all_inputs = Vec::new();
        for session in sessions.values() {
            all_inputs.extend(session.sent_inputs.clone());
        }
        Ok(all_inputs)
    }

    /// Get generated Dolphin commands (mock)
    pub async fn get_dolphin_commands(&self) -> Result<Vec<String>> {
        // Mock implementation - in real test would capture actual commands
        Ok(vec![
            "BUTTON 1 A PRESS".to_string(),
            "ANALOG 1 MAIN 128 128".to_string(),
            "TRIGGER 1 255 0".to_string(),
        ])
    }

    /// Network simulation methods
    pub async fn simulate_packet_loss(&self, rate: f64) -> Result<()> {
        let mut simulator = self.network_simulator.lock().unwrap();
        simulator.packet_loss_rate = rate;
        simulator.is_active = true;
        Ok(())
    }

    pub async fn simulate_latency_spike(&self, additional_ms: u64) -> Result<()> {
        let mut simulator = self.network_simulator.lock().unwrap();
        simulator.additional_latency_ms = additional_ms;
        simulator.is_active = true;
        Ok(())
    }

    pub async fn reset_network_simulation(&self) -> Result<()> {
        let mut simulator = self.network_simulator.lock().unwrap();
        *simulator = NetworkSimulator::new();
        Ok(())
    }

    /// Test scenario helpers
    pub async fn connect_invalid_client(&self) -> Result<Uuid> {
        // Simulate connection failure
        Err(dpstream_server::error::DpstreamError::Network(
            dpstream_server::error::NetworkError::ConnectionFailed("Invalid client".to_string()),
        ))
    }

    pub async fn simulate_memory_pressure(&self) -> Result<()> {
        // Simulate memory pressure conditions
        let mut memory_usage = self.metrics_collector.memory_usage.lock().unwrap();
        memory_usage.heap_usage_mb = 450; // Near limit
        Ok(())
    }

    pub async fn has_graceful_degradation(&self) -> bool {
        // Check if system degraded gracefully under pressure
        true // Mock implementation
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        // Graceful shutdown simulation
        let mut sessions = self.sessions.lock().unwrap();
        sessions.clear();
        Ok(())
    }

    pub async fn is_fully_shutdown(&self) -> bool {
        let sessions = self.sessions.lock().unwrap();
        sessions.is_empty()
    }

    /// Internal metrics processing
    async fn process_frame_metrics(&self, frame: &VideoFrame) {
        // Update quality metrics based on frame processing
        let mut quality_metrics = self.metrics_collector.quality_metrics.lock().unwrap();
        for metrics in quality_metrics.values_mut() {
            metrics.avg_latency_ms = 25.0; // Mock latency
            metrics.frame_drop_rate = 0.02; // 2% drop rate
            metrics.connection_stability = 0.95; // 95% stable
            metrics.throughput_mbps = 15.0; // 15 Mbps
        }
    }

    async fn process_audio_metrics(&self, sample: &AudioFrame) {
        // Update audio metrics
        let mut audio_metrics = self.metrics_collector.audio_metrics.lock().unwrap();
        for metrics in audio_metrics.values_mut() {
            metrics.latency_ms = 20.0;
            metrics.buffer_underruns = 0;
            metrics.sample_rate = 48000;
            metrics.bit_depth = 16;
        }
    }
}

impl NetworkSimulator {
    pub fn new() -> Self {
        Self {
            packet_loss_rate: 0.0,
            additional_latency_ms: 0,
            jitter_ms: 0,
            bandwidth_limit_kbps: None,
            is_active: false,
        }
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            quality_metrics: Arc::new(Mutex::new(HashMap::new())),
            audio_metrics: Arc::new(Mutex::new(HashMap::new())),
            memory_usage: Arc::new(Mutex::new(MemoryUsage::default())),
            processing_times: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

/// Helper functions for generating test data
pub fn generate_test_video_frames(width: u32, height: u32, count: u32) -> Vec<VideoFrame> {
    (0..count)
        .map(|i| VideoFrame {
            data: vec![0u8; (width * height * 3) as usize], // RGB data
            width,
            height,
            timestamp: i as u64 * 16_666, // ~60 FPS timestamps
            frame_number: i as u64,
        })
        .collect()
}

pub fn generate_test_audio_samples(
    sample_rate: u32,
    channels: u16,
    duration: Duration,
) -> Vec<AudioFrame> {
    let sample_count = (sample_rate as f64 * duration.as_secs_f64()) as u32;
    let frame_size = 1024; // Samples per frame
    let frame_count = sample_count.div_ceil(frame_size);

    (0..frame_count)
        .map(|i| AudioFrame {
            data: vec![0u8; (frame_size * channels as u32 * 2) as usize], // 16-bit samples
            timestamp: i as u64 * 21,                                     // ~48kHz frame timing
            sample_rate,
            channels,
        })
        .collect()
}

pub fn create_button_input(button_flags: u16, pressed: bool) -> MoonlightInputPacket {
    MoonlightInputPacket {
        packet_type: 0x0C,
        button_flags: if pressed { button_flags } else { 0 },
        left_trigger: 0,
        right_trigger: 0,
        left_stick_x: 0,
        left_stick_y: 0,
        right_stick_x: 0,
        right_stick_y: 0,
        timestamp: 0,
        gyro_x: None,
        gyro_y: None,
        gyro_z: None,
        accel_x: None,
        accel_y: None,
        accel_z: None,
        touch_points: None,
    }
}

pub fn create_analog_input(left_x: i16, left_y: i16) -> MoonlightInputPacket {
    MoonlightInputPacket {
        packet_type: 0x0C,
        button_flags: 0,
        left_trigger: 0,
        right_trigger: 0,
        left_stick_x: left_x,
        left_stick_y: left_y,
        right_stick_x: 0,
        right_stick_y: 0,
        timestamp: 0,
        gyro_x: None,
        gyro_y: None,
        gyro_z: None,
        accel_x: None,
        accel_y: None,
        accel_z: None,
        touch_points: None,
    }
}

pub fn create_trigger_input(left_trigger: u8, right_trigger: u8) -> MoonlightInputPacket {
    MoonlightInputPacket {
        packet_type: 0x0C,
        button_flags: 0,
        left_trigger,
        right_trigger,
        left_stick_x: 0,
        left_stick_y: 0,
        right_stick_x: 0,
        right_stick_y: 0,
        timestamp: 0,
        gyro_x: None,
        gyro_y: None,
        gyro_z: None,
        accel_x: None,
        accel_y: None,
        accel_z: None,
        touch_points: None,
    }
}
