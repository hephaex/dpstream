//! Hardware-accelerated video encoder module for dpstream server
//!
//! Supports NVENC, VAAPI, and software encoding with optimizations for streaming

use crate::error::{Result, StreamingError};
use crate::streaming::capture::VideoFrame;
use crate::streaming::{VideoBufferPool, ZeroCopyVideoBuffer, PoolConfig, SIMDVideoProcessor, CPUCapabilities};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{info, debug, error, warn};
use smallvec::smallvec;

#[cfg(feature = "streaming")]
use gstreamer as gst;
#[cfg(feature = "streaming")]
use gstreamer_app as gst_app;

/// Video encoder configuration
#[derive(Debug, Clone)]
pub struct EncoderConfig {
    pub encoder_type: EncoderType,
    pub codec: VideoCodec,
    pub bitrate: u32,       // kbps
    pub max_bitrate: u32,   // kbps for VBR
    pub rate_control: RateControlMode,
    pub preset: EncoderPreset,
    pub profile: H264Profile,
    pub level: H264Level,
    pub gop_size: u32,      // Keyframe interval
    pub b_frames: u32,      // B-frame count
    pub ref_frames: u32,    // Reference frame count
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub low_latency: bool,
    pub look_ahead: bool,
    pub adaptive_quantization: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EncoderType {
    NVENC,      // NVIDIA hardware encoder
    VAAPI,      // Intel/AMD hardware encoder
    QuickSync,  // Intel QuickSync
    Software,   // CPU-based x264/x265
}

#[derive(Debug, Clone, Copy)]
pub enum VideoCodec {
    H264,
    H265,
    AV1,
}

#[derive(Debug, Clone, Copy)]
pub enum RateControlMode {
    CBR,        // Constant bitrate
    VBR,        // Variable bitrate
    CQP,        // Constant quantization parameter
    VBR_HQ,     // High quality VBR
}

#[derive(Debug, Clone, Copy)]
pub enum EncoderPreset {
    UltraFast,
    SuperFast,
    VeryFast,
    Faster,
    Fast,
    Medium,
    Slow,
    Slower,
    VerySlow,
    Lossless,
    LosslessHP,
}

#[derive(Debug, Clone, Copy)]
pub enum H264Profile {
    Baseline,
    Main,
    High,
    High444,
}

#[derive(Debug, Clone, Copy)]
pub enum H264Level {
    Level3_0,
    Level3_1,
    Level3_2,
    Level4_0,
    Level4_1,
    Level4_2,
    Level5_0,
    Level5_1,
    Level5_2,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            encoder_type: EncoderType::Software,
            codec: VideoCodec::H264,
            bitrate: 15000,     // 15 Mbps
            max_bitrate: 20000, // 20 Mbps
            rate_control: RateControlMode::CBR,
            preset: EncoderPreset::Fast,
            profile: H264Profile::High,
            level: H264Level::Level4_1,
            gop_size: 60,       // 1 second at 60fps
            b_frames: 0,        // No B-frames for low latency
            ref_frames: 1,      // Minimum for low latency
            width: 1920,
            height: 1080,
            fps: 60,
            low_latency: true,
            look_ahead: false,
            adaptive_quantization: true,
        }
    }
}

/// High-performance zero-copy video encoder with SIMD acceleration
pub struct VideoEncoder {
    config: EncoderConfig,
    #[cfg(feature = "streaming")]
    pipeline: Option<gst::Pipeline>,
    #[cfg(feature = "streaming")]
    appsrc: Option<gst_app::AppSrc>,
    #[cfg(feature = "streaming")]
    appsink: Option<gst_app::AppSink>,
    frame_queue: Arc<Mutex<VecDeque<VideoFrame>>>,
    encoded_sender: Option<mpsc::UnboundedSender<EncodedFrame>>,
    is_encoding: Arc<Mutex<bool>>,
    stats: Arc<Mutex<EncoderStats>>,
    // New high-performance components
    buffer_pool: Arc<VideoBufferPool>,
    simd_processor: Arc<Mutex<SIMDVideoProcessor>>,
    cpu_capabilities: CPUCapabilities,
}

/// Encoded frame data
#[derive(Debug, Clone)]
pub struct EncodedFrame {
    pub data: Vec<u8>,
    pub timestamp: u64,
    pub frame_number: u64,
    pub is_keyframe: bool,
    pub encoding_time: Duration,
    pub size_bytes: usize,
}

/// Encoder statistics
#[derive(Debug, Clone, Default)]
pub struct EncoderStats {
    pub frames_encoded: u64,
    pub keyframes_encoded: u64,
    pub total_bytes: u64,
    pub dropped_frames: u64,
    pub average_encoding_time: Duration,
    pub average_bitrate: f32,
    pub current_fps: f32,
    pub quality_index: f32,
    pub buffer_fullness: f32,
}

/// Comprehensive encoder performance statistics including optimization metrics
#[derive(Debug, Clone)]
pub struct EncoderPerformanceStats {
    pub encoder_stats: EncoderStats,
    pub buffer_pool_hit_rate: f64,
    pub total_pool_allocations: usize,
    pub pool_hits: usize,
    pub pool_misses: usize,
    pub peak_buffer_usage: usize,
    pub current_buffer_usage: usize,
    pub cpu_capabilities: CPUCapabilities,
}

impl VideoEncoder {
    /// Create a new high-performance hardware encoder with zero-copy optimization
    pub fn new(config: EncoderConfig) -> Result<Self> {
        info!("Initializing high-performance {:?} video encoder", config.encoder_type);
        debug!("Configuration: {}x{} @ {}fps, {}kbps {:?}",
               config.width, config.height, config.fps, config.bitrate, config.codec);

        // Validate configuration
        Self::validate_config(&config)?;

        // Initialize CPU capabilities and SIMD processor
        let cpu_capabilities = CPUCapabilities::detect();
        info!("Detected CPU capabilities: AVX2={}, NEON={}",
              cpu_capabilities.has_avx2, cpu_capabilities.has_neon);

        let simd_processor = SIMDVideoProcessor::new(cpu_capabilities.clone())
            .map_err(|e| StreamingError::InitializationFailed {
                component: "SIMD Processor".to_string(),
                reason: e.to_string(),
            })?;

        // Create optimized buffer pool configuration
        let pool_config = PoolConfig {
            buffers_per_tier: 8, // Reduced for encoder use case
            tier_sizes: smallvec![
                (config.width * config.height * 3 / 2) as usize, // I420 format
                (config.width * config.height * 3) as usize,     // RGB24 format
                (config.width * config.height * 4) as usize,     // RGBA format
            ],
            adaptive_allocation: true,
            max_memory_bytes: 128 * 1024 * 1024, // 128MB for encoder
        };

        let buffer_pool = Arc::new(VideoBufferPool::new(pool_config)
            .map_err(|e| StreamingError::InitializationFailed {
                component: "Buffer Pool".to_string(),
                reason: e.to_string(),
            })?);

        info!("Zero-copy buffer pool initialized with optimized tiers");

        // Initialize GStreamer if available
        #[cfg(feature = "streaming")]
        {
            gst::init().map_err(|e| StreamingError::InitializationFailed {
                component: "GStreamer".to_string(),
                reason: e.to_string(),
            })?;
        }

        let (encoded_sender, _) = mpsc::unbounded_channel();

        Ok(Self {
            config,
            #[cfg(feature = "streaming")]
            pipeline: None,
            #[cfg(feature = "streaming")]
            appsrc: None,
            #[cfg(feature = "streaming")]
            appsink: None,
            frame_queue: Arc::new(Mutex::new(VecDeque::new())),
            encoded_sender: Some(encoded_sender),
            is_encoding: Arc::new(Mutex::new(false)),
            stats: Arc::new(Mutex::new(EncoderStats::default())),
            buffer_pool,
            simd_processor: Arc::new(Mutex::new(simd_processor)),
            cpu_capabilities,
        })
    }

    /// Initialize the encoder pipeline
    pub async fn initialize(&mut self) -> Result<()> {
        debug!("Initializing encoder pipeline");

        #[cfg(feature = "streaming")]
        {
            self.setup_gstreamer_pipeline().await?;
            self.start_encoding_pipeline().await?;
        }

        #[cfg(not(feature = "streaming"))]
        {
            self.simulate_encoder().await?;
        }

        *self.is_encoding.lock().unwrap() = true;
        info!("Video encoder initialized successfully");
        Ok(())
    }

    /// High-performance zero-copy frame encoding with SIMD acceleration
    pub async fn encode_frame_optimized(&mut self, frame: VideoFrame) -> Result<Option<EncodedFrame>> {
        if !*self.is_encoding.lock().unwrap() {
            return Err(StreamingError::VideoEncodingFailed(
                "Encoder not initialized".to_string()
            ).into());
        }

        let start_time = Instant::now();

        // Get zero-copy buffer from pool
        let required_size = (self.config.width * self.config.height * 3 / 2) as usize;
        let zero_copy_buffer = self.buffer_pool
            .acquire_buffer(required_size)
            .map_err(|e| StreamingError::VideoEncodingFailed(
                format!("Failed to acquire zero-copy buffer: {}", e)
            ))?;

        // Perform SIMD-accelerated color space conversion if needed
        let processed_frame = if frame.data.len() != required_size {
            debug!("Performing SIMD-accelerated format conversion");
            let mut simd_processor = self.simd_processor.lock().unwrap();

            // Convert using SIMD operations (example: RGB to YUV420)
            match simd_processor.convert_rgb24_to_yuv420(
                &frame.data,
                frame.width as usize,
                frame.height as usize
            ) {
                Ok(converted_data) => {
                    // Copy converted data to zero-copy buffer
                    let buffer_data = unsafe {
                        std::slice::from_raw_parts_mut(
                            zero_copy_buffer.data().as_ptr() as *mut u8,
                            converted_data.len()
                        )
                    };
                    buffer_data.copy_from_slice(&converted_data);
                    zero_copy_buffer.set_length(converted_data.len());

                    VideoFrame {
                        data: zero_copy_buffer.data().to_vec(),
                        width: frame.width,
                        height: frame.height,
                        timestamp: frame.timestamp,
                        frame_number: frame.frame_number,
                    }
                }
                Err(e) => {
                    warn!("SIMD conversion failed, using fallback: {}", e);
                    frame
                }
            }
        } else {
            frame
        };

        // Proceed with regular encoding
        self.encode_frame_internal(processed_frame, start_time).await
    }

    /// Encode a video frame (legacy method for compatibility)
    pub async fn encode_frame(&mut self, frame: VideoFrame) -> Result<Option<EncodedFrame>> {
        let start_time = Instant::now();
        self.encode_frame_internal(frame, start_time).await
    }

    /// Internal frame encoding implementation shared by optimized and legacy methods
    async fn encode_frame_internal(&mut self, frame: VideoFrame, start_time: Instant) -> Result<Option<EncodedFrame>> {
        if !*self.is_encoding.lock().unwrap() {
            return Err(StreamingError::VideoEncodingFailed(
                "Encoder not initialized".to_string()
            ).into());
        }

        // Add frame to queue
        {
            let mut queue = self.frame_queue.lock().unwrap();
            queue.push_back(frame.clone());

            // Limit queue size to prevent memory buildup
            while queue.len() > 10 {
                let dropped = queue.pop_front();
                if dropped.is_some() {
                    let mut stats = self.stats.lock().unwrap();
                    stats.dropped_frames += 1;
                    warn!("Dropped frame due to encoder backlog");
                }
            }
        }

        #[cfg(feature = "streaming")]
        {
            if let Some(appsrc) = &self.appsrc {
                self.push_frame_to_pipeline(appsrc, &frame).await?;
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.lock().unwrap();
            stats.frames_encoded += 1;
            let encoding_time = start_time.elapsed();
            stats.average_encoding_time =
                (stats.average_encoding_time + encoding_time) / 2;
        }

        // For now, return a simulated encoded frame
        #[cfg(not(feature = "streaming"))]
        {
            let encoded = EncodedFrame {
                data: self.simulate_encoded_frame(&frame),
                timestamp: frame.timestamp,
                frame_number: frame.frame_number,
                is_keyframe: frame.frame_number % self.config.gop_size as u64 == 0,
                encoding_time: start_time.elapsed(),
                size_bytes: frame.data.len() / 8, // Simulate compression
            };
            return Ok(Some(encoded));
        }

        #[cfg(feature = "streaming")]
        {
            // In real implementation, this would pull from appsink
            Ok(None)
        }
    }

    /// Get encoder statistics
    pub fn get_stats(&self) -> EncoderStats {
        self.stats.lock().unwrap().clone()
    }

    /// Get comprehensive performance statistics including zero-copy buffer pool metrics
    pub fn get_performance_stats(&self) -> EncoderPerformanceStats {
        let encoder_stats = self.stats.lock().unwrap().clone();
        let pool_stats = self.buffer_pool.get_statistics();
        let pool_hit_rate = self.buffer_pool.hit_rate();

        EncoderPerformanceStats {
            encoder_stats,
            buffer_pool_hit_rate: pool_hit_rate,
            total_pool_allocations: pool_stats.total_allocations.load(std::sync::atomic::Ordering::Relaxed),
            pool_hits: pool_stats.pool_hits.load(std::sync::atomic::Ordering::Relaxed),
            pool_misses: pool_stats.pool_misses.load(std::sync::atomic::Ordering::Relaxed),
            peak_buffer_usage: pool_stats.peak_usage.load(std::sync::atomic::Ordering::Relaxed),
            current_buffer_usage: pool_stats.current_usage.load(std::sync::atomic::Ordering::Relaxed),
            cpu_capabilities: self.cpu_capabilities.clone(),
        }
    }

    /// Reset all performance statistics including buffer pool
    pub fn reset_performance_stats(&self) {
        {
            let mut stats = self.stats.lock().unwrap();
            *stats = EncoderStats::default();
        }
        self.buffer_pool.reset_statistics();
    }

    /// Update encoder bitrate dynamically
    pub fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        info!("Updating encoder bitrate to {}kbps", bitrate);

        #[cfg(feature = "streaming")]
        {
            if let Some(pipeline) = &self.pipeline {
                // Find encoder element and update bitrate
                if let Some(encoder) = pipeline.by_name("encoder") {
                    encoder.set_property("bitrate", bitrate);
                    debug!("Encoder bitrate updated successfully");
                }
            }
        }

        self.config.bitrate = bitrate;
        Ok(())
    }

    /// Shutdown the encoder
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down video encoder");

        *self.is_encoding.lock().unwrap() = false;

        #[cfg(feature = "streaming")]
        {
            if let Some(pipeline) = &self.pipeline {
                pipeline.set_state(gst::State::Null)
                    .map_err(|e| StreamingError::PipelineError {
                        operation: "shutdown".to_string(),
                        reason: e.to_string(),
                    })?;
            }
            self.pipeline = None;
            self.appsrc = None;
            self.appsink = None;
        }

        // Clear frame queue
        self.frame_queue.lock().unwrap().clear();

        info!("Video encoder shutdown complete");
        Ok(())
    }

    fn validate_config(config: &EncoderConfig) -> Result<()> {
        if config.bitrate == 0 {
            return Err(StreamingError::ConfigurationError {
                field: "bitrate".to_string(),
                value: config.bitrate.to_string(),
                reason: "Bitrate must be greater than 0".to_string(),
            }.into());
        }

        if config.width == 0 || config.height == 0 {
            return Err(StreamingError::ConfigurationError {
                field: "resolution".to_string(),
                value: format!("{}x{}", config.width, config.height),
                reason: "Resolution must be greater than 0".to_string(),
            }.into());
        }

        if config.fps == 0 {
            return Err(StreamingError::ConfigurationError {
                field: "fps".to_string(),
                value: config.fps.to_string(),
                reason: "FPS must be greater than 0".to_string(),
            }.into());
        }

        Ok(())
    }

    #[cfg(feature = "streaming")]
    async fn setup_gstreamer_pipeline(&mut self) -> Result<()> {
        debug!("Setting up GStreamer encoding pipeline");

        let pipeline = gst::Pipeline::new(Some("video-encoder"));

        // Create appsrc for input frames
        let appsrc = gst_app::AppSrc::builder()
            .name("video-input")
            .format(gst::Format::Time)
            .build();

        // Configure input caps
        let input_caps = gst::Caps::builder("video/x-raw")
            .field("format", "I420")
            .field("width", self.config.width as i32)
            .field("height", self.config.height as i32)
            .field("framerate", gst::Fraction::new(self.config.fps as i32, 1))
            .build();

        appsrc.set_caps(Some(&input_caps));

        // Create encoder element
        let encoder = self.create_encoder_element()?;

        // Create output sink
        let appsink = gst_app::AppSink::builder()
            .name("encoded-output")
            .sync(false)
            .emit_signals(true)
            .build();

        // Add elements to pipeline
        pipeline.add_many(&[
            appsrc.upcast_ref(),
            &encoder,
            appsink.upcast_ref(),
        ]).map_err(|e| StreamingError::PipelineError {
            operation: "add elements".to_string(),
            reason: e.to_string(),
        })?;

        // Link elements
        gst::Element::link_many(&[
            appsrc.upcast_ref(),
            &encoder,
            appsink.upcast_ref(),
        ]).map_err(|e| StreamingError::PipelineError {
            operation: "link elements".to_string(),
            reason: e.to_string(),
        })?;

        // Set up encoded frame callback
        let encoded_sender = self.encoded_sender.take().unwrap();
        let stats = Arc::clone(&self.stats);

        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let encode_start = Instant::now();
                    let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Error)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;

                    let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
                    let data = map.as_slice().to_vec();

                    let frame_flags = buffer.flags();
                    let is_keyframe = !frame_flags.contains(gst::BufferFlags::DELTA_UNIT);

                    let frame_number = {
                        let mut stats_guard = stats.lock().unwrap();
                        stats_guard.frames_encoded += 1;
                        stats_guard.frames_encoded
                    };

                    let encoding_time = encode_start.elapsed();

                    let encoded_frame = EncodedFrame {
                        data,
                        timestamp: buffer.pts().unwrap_or(gst::ClockTime::ZERO).nseconds(),
                        frame_number,
                        is_keyframe,
                        encoding_time,
                        size_bytes: map.size(),
                    };

                    // Update statistics
                    {
                        let mut stats_guard = stats.lock().unwrap();
                        stats_guard.total_bytes += encoded_frame.size_bytes as u64;
                        if encoded_frame.is_keyframe {
                            stats_guard.keyframes_encoded += 1;
                        }
                    }

                    if encoded_sender.send(encoded_frame).is_err() {
                        warn!("Failed to send encoded frame");
                    }

                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        self.pipeline = Some(pipeline);
        self.appsrc = Some(appsrc);
        self.appsink = Some(appsink);

        info!("GStreamer encoding pipeline configured");
        Ok(())
    }

    #[cfg(feature = "streaming")]
    fn create_encoder_element(&self) -> Result<gst::Element> {
        let encoder = match self.config.encoder_type {
            EncoderType::NVENC => {
                debug!("Creating NVENC H264 encoder");
                let encoder = gst::ElementFactory::make("nvh264enc", Some("encoder"))
                    .map_err(|e| StreamingError::EncoderNotAvailable {
                        encoder: "NVENC".to_string(),
                        reason: e.to_string(),
                    })?;

                // Configure NVENC settings for low latency
                encoder.set_property("bitrate", self.config.bitrate);
                encoder.set_property("preset", self.nvenc_preset());
                encoder.set_property("rc-mode", self.nvenc_rate_control());
                encoder.set_property("gop-size", self.config.gop_size);
                encoder.set_property("b-frames", self.config.b_frames);
                encoder.set_property("zerolatency", self.config.low_latency);
                encoder.set_property("aud", true); // Access unit delimiters
                encoder.set_property("cabac", false); // Disable for baseline profile

                if self.config.low_latency {
                    encoder.set_property("tune", "ultra-low-latency");
                }

                encoder
            }
            EncoderType::VAAPI => {
                debug!("Creating VAAPI H264 encoder");
                let encoder = gst::ElementFactory::make("vaapih264enc", Some("encoder"))
                    .map_err(|e| StreamingError::EncoderNotAvailable {
                        encoder: "VAAPI".to_string(),
                        reason: e.to_string(),
                    })?;

                encoder.set_property("bitrate", self.config.bitrate);
                encoder.set_property("rate-control", self.vaapi_rate_control());
                encoder.set_property("keyframe-period", self.config.gop_size);

                encoder
            }
            EncoderType::QuickSync => {
                debug!("Creating QuickSync H264 encoder");
                let encoder = gst::ElementFactory::make("mfh264enc", Some("encoder"))
                    .map_err(|e| StreamingError::EncoderNotAvailable {
                        encoder: "QuickSync".to_string(),
                        reason: e.to_string(),
                    })?;

                encoder.set_property("bitrate", self.config.bitrate);

                encoder
            }
            EncoderType::Software => {
                debug!("Creating software H264 encoder");
                let encoder = gst::ElementFactory::make("x264enc", Some("encoder"))
                    .map_err(|e| StreamingError::EncoderNotAvailable {
                        encoder: "x264".to_string(),
                        reason: e.to_string(),
                    })?;

                encoder.set_property("bitrate", self.config.bitrate);
                encoder.set_property("speed-preset", self.x264_preset());
                encoder.set_property("tune", if self.config.low_latency { "zerolatency" } else { "film" });
                encoder.set_property("key-int-max", self.config.gop_size);
                encoder.set_property("b-frames", self.config.b_frames);
                encoder.set_property("ref", self.config.ref_frames);

                encoder
            }
        };

        Ok(encoder)
    }

    #[cfg(feature = "streaming")]
    async fn start_encoding_pipeline(&mut self) -> Result<()> {
        if let Some(pipeline) = &self.pipeline {
            debug!("Starting encoding pipeline");

            pipeline.set_state(gst::State::Playing)
                .map_err(|e| StreamingError::PipelineError {
                    operation: "start encoding".to_string(),
                    reason: e.to_string(),
                })?;

            // Wait for pipeline to reach playing state
            let bus = pipeline.bus().unwrap();
            let state_change = pipeline.state_change_timeout();

            match bus.timed_pop_filtered(state_change, &[gst::MessageType::StateChanged, gst::MessageType::Error]) {
                Some(msg) => {
                    match msg.view() {
                        gst::MessageView::Error(err) => {
                            return Err(StreamingError::PipelineError {
                                operation: "encoding pipeline startup".to_string(),
                                reason: format!("{}: {}", err.error(), err.debug().unwrap_or_default()),
                            }.into());
                        }
                        _ => {
                            debug!("Encoding pipeline started successfully");
                        }
                    }
                }
                None => {
                    warn!("Encoding pipeline startup timeout");
                }
            }
        }

        Ok(())
    }

    #[cfg(feature = "streaming")]
    async fn push_frame_to_pipeline(&self, appsrc: &gst_app::AppSrc, frame: &VideoFrame) -> Result<()> {
        let buffer = gst::Buffer::from_slice(frame.data.clone());

        // Set buffer timestamp
        let mut buffer = buffer.into_mut();
        buffer.set_pts(gst::ClockTime::from_nseconds(frame.timestamp));

        match appsrc.push_buffer(buffer.into()) {
            Ok(_) => Ok(()),
            Err(gst::FlowError::Flushing) => {
                debug!("Pipeline is flushing, dropping frame");
                Ok(())
            }
            Err(e) => Err(StreamingError::VideoEncodingFailed(
                format!("Failed to push frame to encoder: {:?}", e)
            ).into())
        }
    }

    #[cfg(not(feature = "streaming"))]
    async fn simulate_encoder(&mut self) -> Result<()> {
        debug!("Simulating video encoder (GStreamer not available)");
        // Encoder simulation is handled in encode_frame method
        Ok(())
    }

    #[cfg(not(feature = "streaming"))]
    fn simulate_encoded_frame(&self, frame: &VideoFrame) -> Vec<u8> {
        // Simulate compression (roughly 1:8 ratio for H264)
        let compressed_size = frame.data.len() / 8;
        vec![0x42; compressed_size.max(1024)] // Fake H264 data
    }

    fn nvenc_preset(&self) -> &'static str {
        match self.config.preset {
            EncoderPreset::UltraFast => "hp",
            EncoderPreset::SuperFast => "hq",
            EncoderPreset::VeryFast => "hq",
            EncoderPreset::Faster => "hq",
            EncoderPreset::Fast => "hq",
            EncoderPreset::Medium => "hq",
            EncoderPreset::Slow => "lossless-hp",
            EncoderPreset::Slower => "lossless-hq",
            EncoderPreset::VerySlow => "lossless-hq",
            EncoderPreset::Lossless => "lossless-hq",
            EncoderPreset::LosslessHP => "lossless-hp",
        }
    }

    fn nvenc_rate_control(&self) -> &'static str {
        match self.config.rate_control {
            RateControlMode::CBR => "cbr",
            RateControlMode::VBR => "vbr",
            RateControlMode::CQP => "cqp",
            RateControlMode::VBR_HQ => "vbr-hq",
        }
    }

    fn vaapi_rate_control(&self) -> &'static str {
        match self.config.rate_control {
            RateControlMode::CBR => "cbr",
            RateControlMode::VBR => "vbr",
            RateControlMode::CQP => "cqp",
            RateControlMode::VBR_HQ => "vbr",
        }
    }

    fn x264_preset(&self) -> u32 {
        match self.config.preset {
            EncoderPreset::UltraFast => 1,
            EncoderPreset::SuperFast => 2,
            EncoderPreset::VeryFast => 3,
            EncoderPreset::Faster => 4,
            EncoderPreset::Fast => 5,
            EncoderPreset::Medium => 6,
            EncoderPreset::Slow => 7,
            EncoderPreset::Slower => 8,
            EncoderPreset::VerySlow => 9,
            _ => 5, // Default to Fast
        }
    }
}

impl Drop for VideoEncoder {
    fn drop(&mut self) {
        if *self.is_encoding.lock().unwrap() {
            debug!("Stopping encoder on drop");
            *self.is_encoding.lock().unwrap() = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_encoder_creation() {
        let config = EncoderConfig::default();
        let result = VideoEncoder::new(config);
        assert!(result.is_ok(), "Encoder creation should succeed");
    }

    #[tokio::test]
    async fn test_encoder_lifecycle() {
        let mut config = EncoderConfig::default();
        config.encoder_type = EncoderType::Software;

        let mut encoder = VideoEncoder::new(config).unwrap();

        // Initialize
        let init_result = encoder.initialize().await;
        assert!(init_result.is_ok(), "Encoder initialization should succeed");

        // Encode frame
        let frame = VideoFrame {
            data: vec![128; 1920 * 1080 * 3 / 2], // I420 frame
            width: 1920,
            height: 1080,
            timestamp: 12345,
            frame_number: 1,
        };

        let encode_result = encoder.encode_frame(frame).await;
        assert!(encode_result.is_ok(), "Frame encoding should succeed");

        // Shutdown
        let shutdown_result = encoder.shutdown().await;
        assert!(shutdown_result.is_ok(), "Encoder shutdown should succeed");
    }

    #[tokio::test]
    async fn test_config_validation() {
        let mut config = EncoderConfig::default();
        config.bitrate = 0;

        let result = VideoEncoder::new(config);
        assert!(result.is_err(), "Should fail with invalid bitrate");
    }
}