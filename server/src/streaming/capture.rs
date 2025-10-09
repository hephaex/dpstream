//! Video capture module for dpstream server
//!
//! Implements GStreamer-based video capture from Dolphin window with hardware acceleration

use crate::error::{Result, StreamingError};
use std::sync::Arc;
use parking_lot::{Mutex, RwLock};
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use tokio::sync::oneshot;
use tracing::{info, debug, error, warn};
use std::collections::VecDeque;

#[cfg(feature = "streaming")]
use gstreamer as gst;
#[cfg(feature = "streaming")]
use gstreamer_app as gst_app;
#[cfg(feature = "streaming")]
use gstreamer_video as gst_video;

/// Video frame data with optimized memory management
#[derive(Debug)]
pub struct VideoFrame {
    pub data: Arc<Vec<u8>>,  // Shared ownership for zero-copy
    pub width: u32,
    pub height: u32,
    pub timestamp: u64,
    pub frame_number: u64,
    pub priority: FramePriority,
}

impl Clone for VideoFrame {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
            width: self.width,
            height: self.height,
            timestamp: self.timestamp,
            frame_number: self.frame_number,
            priority: self.priority,
        }
    }
}

/// Frame priority for adaptive quality control
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FramePriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,  // Keyframes
}

/// High-performance video frame buffer pool
pub struct VideoFramePool {
    buffers: Arc<Mutex<VecDeque<Arc<Vec<u8>>>>>,
    buffer_size: usize,
    max_buffers: usize,
    allocated_count: Arc<parking_lot::Mutex<usize>>,
}

impl VideoFramePool {
    pub fn new(buffer_size: usize, max_buffers: usize) -> Self {
        let mut buffers = VecDeque::with_capacity(max_buffers);

        // Pre-allocate half the buffers
        for _ in 0..(max_buffers / 2) {
            buffers.push_back(Arc::new(vec![0u8; buffer_size]));
        }

        Self {
            buffers: Arc::new(Mutex::new(buffers)),
            buffer_size,
            max_buffers,
            allocated_count: Arc::new(parking_lot::Mutex::new(max_buffers / 2)),
        }
    }

    pub fn acquire(&self) -> Result<Arc<Vec<u8>>> {
        let mut buffers = self.buffers.lock();

        if let Some(buffer) = buffers.pop_front() {
            return Ok(buffer);
        }

        // Create new buffer if under limit
        let mut count = self.allocated_count.lock();
        if *count < self.max_buffers {
            *count += 1;
            drop(count);
            drop(buffers);
            return Ok(Arc::new(vec![0u8; self.buffer_size]));
        }

        Err(StreamingError::NoBuffersAvailable.into())
    }

    pub fn release(&self, buffer: Arc<Vec<u8>>) {
        // Only keep buffer if it's the right size and we're not at capacity
        if buffer.len() == self.buffer_size && Arc::strong_count(&buffer) == 1 {
            let mut buffers = self.buffers.lock();
            if buffers.len() < self.max_buffers / 2 {
                buffers.push_back(buffer);
                return;
            }
        }

        // Buffer will be dropped, decrease count
        let mut count = self.allocated_count.lock();
        *count = count.saturating_sub(1);
    }

    pub fn stats(&self) -> (usize, usize) {
        let available = self.buffers.lock().len();
        let allocated = *self.allocated_count.lock();
        (available, allocated)
    }
}

/// Video capture configuration
#[derive(Debug, Clone)]
pub struct VideoCaptureConfig {
    pub window_id: u64,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,
    pub encoder: VideoEncoder,
    pub quality_preset: QualityPreset,
}

#[derive(Debug, Clone, Copy)]
pub enum VideoEncoder {
    Software,  // x264
    NVENC,     // NVIDIA hardware encoder
    VAAPI,     // Intel/AMD hardware encoder
}

#[derive(Debug, Clone, Copy)]
pub enum QualityPreset {
    UltraFast,
    SuperFast,
    VeryFast,
    Faster,
    Fast,
    Medium,
    Slow,
    Slower,
    VerySlow,
}

/// Video capture pipeline using GStreamer with optimized performance
pub struct VideoCapture {
    config: VideoCaptureConfig,
    #[cfg(feature = "streaming")]
    pipeline: Option<gst::Pipeline>,
    #[cfg(feature = "streaming")]
    appsink: Option<gst_app::AppSink>,
    frame_sender: Option<Sender<VideoFrame>>,
    frame_receiver: Option<Receiver<VideoFrame>>,
    is_capturing: Arc<parking_lot::RwLock<bool>>,
    frame_counter: Arc<parking_lot::Mutex<u64>>,
    buffer_pool: Arc<VideoFramePool>,
    stats: Arc<Mutex<CaptureStats>>,
}

/// Capture performance statistics
#[derive(Debug, Default)]
pub struct CaptureStats {
    pub frames_captured: u64,
    pub frames_dropped: u64,
    pub bytes_processed: u64,
    pub avg_frame_time_ms: f64,
    pub last_frame_timestamp: u64,
    pub buffer_pool_hits: u64,
    pub buffer_pool_misses: u64,
}

impl VideoCapture {
    /// Create a new video capture instance with optimized performance
    pub fn new(config: VideoCaptureConfig) -> Result<Self> {
        info!("Initializing optimized video capture for window 0x{:x}", config.window_id);
        debug!("Configuration: {}x{} @ {}fps, bitrate: {}kbps",
               config.width, config.height, config.fps, config.bitrate);

        // Initialize GStreamer if available
        #[cfg(feature = "streaming")]
        {
            gst::init().map_err(|e| StreamingError::InitializationFailed {
                component: "GStreamer".to_string(),
                reason: e.to_string(),
            })?;
        }

        // Calculate optimal buffer pool size based on configuration
        let frame_size = (config.width * config.height * 4) as usize; // RGBA
        let max_buffers = ((config.fps as usize * 2).max(8)).min(32); // 2 seconds worth, 8-32 range
        let buffer_pool = Arc::new(VideoFramePool::new(frame_size, max_buffers));

        info!("Created buffer pool: {} buffers of {} KB each",
              max_buffers, frame_size / 1024);

        // Use high-performance bounded channel with backpressure
        let (frame_sender, frame_receiver) = bounded(max_buffers);

        Ok(Self {
            config,
            #[cfg(feature = "streaming")]
            pipeline: None,
            #[cfg(feature = "streaming")]
            appsink: None,
            frame_sender: Some(frame_sender),
            frame_receiver: Some(frame_receiver),
            is_capturing: Arc::new(parking_lot::RwLock::new(false)),
            frame_counter: Arc::new(parking_lot::Mutex::new(0)),
            buffer_pool,
            stats: Arc::new(Mutex::new(CaptureStats::default())),
        })
    }

    /// Start video capture pipeline
    pub async fn start_capture(&mut self) -> Result<()> {
        debug!("Starting video capture pipeline");

        #[cfg(feature = "streaming")]
        {
            self.setup_gstreamer_pipeline().await?;
            self.start_gstreamer_pipeline().await?;
        }

        #[cfg(not(feature = "streaming"))]
        {
            self.simulate_capture().await?;
        }

        *self.is_capturing.write() = true;
        info!("Video capture started for {}x{} at {}fps",
              self.config.width, self.config.height, self.config.fps);

        Ok(())
    }

    /// Stop video capture
    pub async fn stop_capture(&mut self) -> Result<()> {
        debug!("Stopping video capture");

        *self.is_capturing.write() = false;

        #[cfg(feature = "streaming")]
        {
            if let Some(pipeline) = &self.pipeline {
                pipeline.set_state(gst::State::Null)
                    .map_err(|e| StreamingError::PipelineError {
                        operation: "stop".to_string(),
                        reason: e.to_string(),
                    })?;
            }
            self.pipeline = None;
            self.appsink = None;
        }

        info!("Video capture stopped");
        Ok(())
    }

    /// Get the next video frame (non-blocking)
    pub fn get_frame(&mut self) -> Option<VideoFrame> {
        if let Some(receiver) = &self.frame_receiver {
            receiver.try_recv().ok()
        } else {
            None
        }
    }

    /// Check if capture is active
    pub fn is_capturing(&self) -> bool {
        *self.is_capturing.read()
    }

    /// Get capture statistics
    pub fn get_stats(&self) -> CaptureStats {
        let frame_count = *self.frame_counter.lock().unwrap();
        CaptureStats {
            frames_captured: frame_count,
            is_active: self.is_capturing(),
            config: self.config.clone(),
        }
    }

    #[cfg(feature = "streaming")]
    async fn setup_gstreamer_pipeline(&mut self) -> Result<()> {
        debug!("Setting up GStreamer pipeline");

        // Create pipeline elements
        let pipeline = gst::Pipeline::new(Some("video-capture"));

        // Video source (X11 screen capture)
        let video_src = gst::ElementFactory::make("ximagesrc", Some("video-source"))
            .map_err(|e| StreamingError::PipelineError {
                operation: "create ximagesrc".to_string(),
                reason: e.to_string(),
            })?;

        // Configure source for specific window
        video_src.set_property("xid", self.config.window_id);
        video_src.set_property("use-damage", true);

        // Video rate control
        let videorate = gst::ElementFactory::make("videorate", Some("rate-control"))
            .map_err(|e| StreamingError::PipelineError {
                operation: "create videorate".to_string(),
                reason: e.to_string(),
            })?;

        // Video scale and format conversion
        let videoconvert = gst::ElementFactory::make("videoconvert", Some("converter"))
            .map_err(|e| StreamingError::PipelineError {
                operation: "create videoconvert".to_string(),
                reason: e.to_string(),
            })?;

        let videoscale = gst::ElementFactory::make("videoscale", Some("scaler"))
            .map_err(|e| StreamingError::PipelineError {
                operation: "create videoscale".to_string(),
                reason: e.to_string(),
            })?;

        // Encoder based on configuration
        let encoder = self.create_encoder()?;

        // Output sink
        let appsink = gst_app::AppSink::builder()
            .name("video-sink")
            .sync(false)
            .emit_signals(true)
            .build();

        // Configure caps for the pipeline
        let caps = gst_video::VideoCapsBuilder::new()
            .format(gst_video::VideoFormat::I420)
            .width(self.config.width as i32)
            .height(self.config.height as i32)
            .framerate(gst::Fraction::new(self.config.fps as i32, 1))
            .build();

        appsink.set_caps(Some(&caps));

        // Add elements to pipeline
        pipeline.add_many(&[
            &video_src,
            &videorate,
            &videoconvert,
            &videoscale,
            &encoder,
            appsink.upcast_ref(),
        ]).map_err(|e| StreamingError::PipelineError {
            operation: "add elements".to_string(),
            reason: e.to_string(),
        })?;

        // Link elements
        gst::Element::link_many(&[
            &video_src,
            &videorate,
            &videoconvert,
            &videoscale,
            &encoder,
            appsink.upcast_ref(),
        ]).map_err(|e| StreamingError::PipelineError {
            operation: "link elements".to_string(),
            reason: e.to_string(),
        })?;

        // Set up frame callback
        let frame_sender = self.frame_sender.take().unwrap();
        let frame_counter = Arc::clone(&self.frame_counter);

        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Error)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;

                    let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
                    let data = map.as_slice().to_vec();

                    let mut counter = frame_counter.lock().unwrap();
                    *counter += 1;

                    let frame = VideoFrame {
                        data,
                        width: 1920, // TODO: Get from caps
                        height: 1080,
                        timestamp: buffer.pts().unwrap_or(gst::ClockTime::ZERO).nseconds(),
                        frame_number: *counter,
                    };

                    if frame_sender.send(frame).is_err() {
                        warn!("Failed to send video frame to receiver");
                    }

                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        self.pipeline = Some(pipeline);
        self.appsink = Some(appsink);

        info!("GStreamer pipeline configured successfully");
        Ok(())
    }

    #[cfg(feature = "streaming")]
    fn create_encoder(&self) -> Result<gst::Element> {
        match self.config.encoder {
            VideoEncoder::NVENC => {
                debug!("Creating NVENC H264 encoder");
                let encoder = gst::ElementFactory::make("nvh264enc", Some("encoder"))
                    .map_err(|e| StreamingError::EncoderNotAvailable {
                        encoder: "NVENC".to_string(),
                        reason: e.to_string(),
                    })?;

                // Configure NVENC settings
                encoder.set_property("bitrate", self.config.bitrate);
                encoder.set_property("preset", self.quality_preset_to_nvenc());
                encoder.set_property("rc-mode", "cbr"); // Constant bitrate
                encoder.set_property("gop-size", 60); // 1 second at 60fps
                encoder.set_property("b-frames", 0); // Low latency
                encoder.set_property("zerolatency", true);

                Ok(encoder)
            }
            VideoEncoder::VAAPI => {
                debug!("Creating VAAPI H264 encoder");
                let encoder = gst::ElementFactory::make("vaapih264enc", Some("encoder"))
                    .map_err(|e| StreamingError::EncoderNotAvailable {
                        encoder: "VAAPI".to_string(),
                        reason: e.to_string(),
                    })?;

                encoder.set_property("bitrate", self.config.bitrate);
                encoder.set_property("rate-control", "cbr");

                Ok(encoder)
            }
            VideoEncoder::Software => {
                debug!("Creating software H264 encoder");
                let encoder = gst::ElementFactory::make("x264enc", Some("encoder"))
                    .map_err(|e| StreamingError::EncoderNotAvailable {
                        encoder: "x264".to_string(),
                        reason: e.to_string(),
                    })?;

                encoder.set_property("bitrate", self.config.bitrate);
                encoder.set_property("speed-preset", self.quality_preset_to_x264());
                encoder.set_property("tune", "zerolatency");
                encoder.set_property("key-int-max", 60);
                encoder.set_property("b-frames", 0);

                Ok(encoder)
            }
        }
    }

    #[cfg(feature = "streaming")]
    async fn start_gstreamer_pipeline(&mut self) -> Result<()> {
        if let Some(pipeline) = &self.pipeline {
            debug!("Starting GStreamer pipeline");

            pipeline.set_state(gst::State::Playing)
                .map_err(|e| StreamingError::PipelineError {
                    operation: "start".to_string(),
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
                                operation: "pipeline startup".to_string(),
                                reason: format!("{}: {}", err.error(), err.debug().unwrap_or_default()),
                            }.into());
                        }
                        _ => {
                            debug!("Pipeline started successfully");
                        }
                    }
                }
                None => {
                    warn!("Pipeline startup timeout");
                }
            }
        }

        Ok(())
    }

    #[cfg(not(feature = "streaming"))]
    async fn simulate_capture(&mut self) -> Result<()> {
        debug!("Simulating video capture (GStreamer not available)");

        let frame_sender = self.frame_sender.take().unwrap();
        let frame_counter = Arc::clone(&self.frame_counter);
        let is_capturing = Arc::clone(&self.is_capturing);
        let config = self.config.clone();

        // Spawn a task to simulate frame generation
        tokio::spawn(async move {
            let frame_interval = std::time::Duration::from_millis(1000 / config.fps as u64);

            while *is_capturing.read() {
                let mut counter = frame_counter.lock().unwrap();
                *counter += 1;

                // Generate dummy frame data
                let frame_size = (config.width * config.height * 3 / 2) as usize; // I420 format
                let data = vec![128; frame_size]; // Gray frame

                let frame = VideoFrame {
                    data,
                    width: config.width,
                    height: config.height,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos() as u64,
                    frame_number: *counter,
                };

                if frame_sender.send(frame).is_err() {
                    break;
                }

                drop(counter);
                tokio::time::sleep(frame_interval).await;
            }
        });

        Ok(())
    }

    fn quality_preset_to_nvenc(&self) -> &'static str {
        match self.config.quality_preset {
            QualityPreset::UltraFast => "hp",
            QualityPreset::SuperFast => "hq",
            QualityPreset::VeryFast => "hq",
            QualityPreset::Faster => "hq",
            QualityPreset::Fast => "hq",
            QualityPreset::Medium => "hq",
            QualityPreset::Slow => "lossless-hp",
            QualityPreset::Slower => "lossless-hq",
            QualityPreset::VerySlow => "lossless-hq",
        }
    }

    fn quality_preset_to_x264(&self) -> u32 {
        match self.config.quality_preset {
            QualityPreset::UltraFast => 1,
            QualityPreset::SuperFast => 2,
            QualityPreset::VeryFast => 3,
            QualityPreset::Faster => 4,
            QualityPreset::Fast => 5,
            QualityPreset::Medium => 6,
            QualityPreset::Slow => 7,
            QualityPreset::Slower => 8,
            QualityPreset::VerySlow => 9,
        }
    }
}

impl Drop for VideoCapture {
    fn drop(&mut self) {
        if self.is_capturing() {
            debug!("Stopping video capture on drop");
            // Note: Can't use async in drop, but pipeline will be cleaned up automatically
            *self.is_capturing.write() = false;
        }
    }
}

/// Capture statistics
#[derive(Debug, Clone)]
pub struct CaptureStats {
    pub frames_captured: u64,
    pub is_active: bool,
    pub config: VideoCaptureConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_video_capture_creation() {
        let config = VideoCaptureConfig {
            window_id: 0x12345678,
            width: 1920,
            height: 1080,
            fps: 60,
            bitrate: 15000,
            encoder: VideoEncoder::Software,
            quality_preset: QualityPreset::Fast,
        };

        let result = VideoCapture::new(config);
        assert!(result.is_ok(), "VideoCapture creation should succeed");

        let capture = result.unwrap();
        assert!(!capture.is_capturing(), "Should not be capturing initially");
    }

    #[tokio::test]
    async fn test_video_capture_lifecycle() {
        let config = VideoCaptureConfig {
            window_id: 0x12345678,
            width: 1280,
            height: 720,
            fps: 30,
            bitrate: 5000,
            encoder: VideoEncoder::Software,
            quality_preset: QualityPreset::VeryFast,
        };

        let mut capture = VideoCapture::new(config).unwrap();

        // Start capture
        let start_result = capture.start_capture().await;
        assert!(start_result.is_ok(), "Capture start should succeed");
        assert!(capture.is_capturing(), "Should be capturing after start");

        // Stop capture
        let stop_result = capture.stop_capture().await;
        assert!(stop_result.is_ok(), "Capture stop should succeed");
        assert!(!capture.is_capturing(), "Should not be capturing after stop");
    }

    #[tokio::test]
    async fn test_frame_generation() {
        let config = VideoCaptureConfig {
            window_id: 0x12345678,
            width: 640,
            height: 480,
            fps: 15,
            bitrate: 2000,
            encoder: VideoEncoder::Software,
            quality_preset: QualityPreset::UltraFast,
        };

        let mut capture = VideoCapture::new(config).unwrap();
        capture.start_capture().await.unwrap();

        // Wait for some frames
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Check that frames are being generated
        let stats = capture.get_stats();
        assert!(stats.frames_captured > 0, "Should have captured some frames");

        capture.stop_capture().await.unwrap();
    }
}