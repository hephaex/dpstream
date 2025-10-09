//! Audio capture and streaming for Dolphin Remote Gaming System
//!
//! Implements PulseAudio-based audio capture with hardware acceleration support

use crate::error::{Result, StreamingError};
use crate::streaming::moonlight::{AudioFrame, AudioCodec};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::time::sleep;
use parking_lot::Mutex;
use crossbeam_channel::{Receiver, Sender, bounded};

/// Audio capture configuration
#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u8,
    pub bit_depth: u8,
    pub codec: AudioCodec,
    pub bitrate: u32,
    pub buffer_size: usize,
    pub enable_hardware_acceleration: bool,
    pub low_latency_mode: bool,
    pub noise_suppression: bool,
    pub echo_cancellation: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,  // 48kHz for gaming audio
            channels: 2,         // Stereo
            bit_depth: 16,       // 16-bit samples
            codec: AudioCodec::Opus,
            bitrate: 128000,     // 128 kbps
            buffer_size: 1024,   // 1024 samples per buffer
            enable_hardware_acceleration: true,
            low_latency_mode: true,
            noise_suppression: false,  // Disable for gaming
            echo_cancellation: false,  // Disable for gaming
        }
    }
}

/// Audio capture statistics
#[derive(Debug, Clone, Default)]
pub struct AudioStats {
    pub frames_captured: u64,
    pub frames_dropped: u64,
    pub bytes_processed: u64,
    pub average_latency_ms: f32,
    pub buffer_underruns: u64,
    pub encoding_errors: u64,
    pub last_capture_time: Option<Instant>,
}

/// Audio capture and encoding system
pub struct AudioCapture {
    config: AudioConfig,
    #[cfg(feature = "streaming")]
    pipeline: Option<gst::Pipeline>,
    #[cfg(feature = "streaming")]
    appsink: Option<gst_app::AppSink>,
    frame_sender: Option<broadcast::Sender<AudioFrame>>,
    sample_queue: Arc<Mutex<VecDeque<AudioSample>>>,
    is_capturing: Arc<Mutex<bool>>,
    stats: Arc<Mutex<AudioStats>>,
    encoder: Option<AudioEncoder>,
}

/// Raw audio sample
#[derive(Debug, Clone)]
pub struct AudioSample {
    pub data: Vec<u8>,
    pub timestamp: u64,
    pub sample_rate: u32,
    pub channels: u8,
    pub samples_per_channel: usize,
}

/// Audio encoder for various codecs
pub struct AudioEncoder {
    config: AudioConfig,
    #[cfg(feature = "streaming")]
    encoder_pipeline: Option<gst::Pipeline>,
    encode_queue: Arc<Mutex<VecDeque<AudioSample>>>,
    stats: Arc<Mutex<AudioStats>>,
}

impl AudioCapture {
    /// Create a new audio capture system
    pub fn new(config: AudioConfig) -> Result<Self> {
        let (frame_sender, _) = broadcast::channel(32);

        Ok(Self {
            config,
            #[cfg(feature = "streaming")]
            pipeline: None,
            #[cfg(feature = "streaming")]
            appsink: None,
            frame_sender: Some(frame_sender),
            sample_queue: Arc::new(Mutex::new(VecDeque::new())),
            is_capturing: Arc::new(Mutex::new(false)),
            stats: Arc::new(Mutex::new(AudioStats::default())),
            encoder: None,
        })
    }

    /// Initialize the audio capture system
    pub async fn initialize(&mut self) -> Result<()> {
        #[cfg(feature = "streaming")]
        {
            // Initialize GStreamer for audio
            gst::init().map_err(|e| StreamingError::InitializationFailed {
                component: "GStreamer".to_string(),
                reason: e.to_string(),
            })?;

            // Create audio capture pipeline
            self.create_audio_pipeline().await?;
        }

        // Initialize audio encoder
        self.encoder = Some(AudioEncoder::new(self.config.clone()).await?);

        Ok(())
    }

    /// Start audio capture
    pub async fn start_capture(&mut self) -> Result<()> {
        let mut is_capturing = self.is_capturing.lock().await;
        if *is_capturing {
            return Ok(());
        }

        #[cfg(feature = "streaming")]
        {
            if let Some(ref pipeline) = self.pipeline {
                pipeline.set_state(gst::State::Playing)
                    .map_err(|e| StreamingError::CaptureStartFailed {
                        reason: e.to_string(),
                    })?;
            }
        }

        *is_capturing = true;

        // Start processing loop
        let sample_queue = Arc::clone(&self.sample_queue);
        let frame_sender = self.frame_sender.clone().unwrap();
        let stats = Arc::clone(&self.stats);
        let config = self.config.clone();

        tokio::spawn(async move {
            Self::process_audio_samples(sample_queue, frame_sender, stats, config).await;
        });

        Ok(())
    }

    /// Stop audio capture
    pub async fn stop_capture(&mut self) -> Result<()> {
        let mut is_capturing = self.is_capturing.lock().await;
        if !*is_capturing {
            return Ok(());
        }

        #[cfg(feature = "streaming")]
        {
            if let Some(ref pipeline) = self.pipeline {
                pipeline.set_state(gst::State::Null)
                    .map_err(|e| StreamingError::CaptureStopFailed {
                        reason: e.to_string(),
                    })?;
            }
        }

        *is_capturing = false;
        Ok(())
    }

    /// Get audio frame receiver
    pub fn subscribe(&self) -> broadcast::Receiver<AudioFrame> {
        self.frame_sender.as_ref().unwrap().subscribe()
    }

    /// Get capture statistics
    pub async fn get_stats(&self) -> AudioStats {
        self.stats.lock().await.clone()
    }

    #[cfg(feature = "streaming")]
    async fn create_audio_pipeline(&mut self) -> Result<()> {
        use gst::prelude::*;

        // Create pipeline elements
        let pipeline = gst::Pipeline::builder()
            .name("audio-capture-pipeline")
            .build();

        // Audio source - use PulseAudio
        let source = gst::ElementFactory::make("pulsesrc")
            .name("audio-source")
            .property("device", "dolphin-emu.monitor")  // Capture Dolphin audio
            .build()
            .map_err(|e| StreamingError::InitializationFailed {
                component: "PulseAudio source".to_string(),
                reason: e.to_string(),
            })?;

        // Audio converter
        let audioconvert = gst::ElementFactory::make("audioconvert")
            .name("audio-convert")
            .build()
            .map_err(|e| StreamingError::InitializationFailed {
                component: "Audio converter".to_string(),
                reason: e.to_string(),
            })?;

        // Audio resampler
        let audioresample = gst::ElementFactory::make("audioresample")
            .name("audio-resample")
            .build()
            .map_err(|e| StreamingError::InitializationFailed {
                component: "Audio resampler".to_string(),
                reason: e.to_string(),
            })?;

        // Audio filter for format specification
        let caps_filter = gst::ElementFactory::make("capsfilter")
            .name("audio-caps")
            .build()
            .map_err(|e| StreamingError::InitializationFailed {
                component: "Audio caps filter".to_string(),
                reason: e.to_string(),
            })?;

        // Set audio format caps
        let caps = gst::Caps::builder("audio/x-raw")
            .field("format", "S16LE")
            .field("rate", self.config.sample_rate as i32)
            .field("channels", self.config.channels as i32)
            .field("layout", "interleaved")
            .build();

        caps_filter.set_property("caps", &caps);

        // Application sink for receiving audio data
        let appsink = gst_app::AppSink::builder()
            .name("audio-sink")
            .caps(&caps)
            .build();

        // Configure appsink for low latency
        appsink.set_property("emit-signals", true);
        appsink.set_property("sync", false);
        appsink.set_property("async", false);
        appsink.set_property("max-buffers", 3u32);  // Small buffer for low latency

        // Add elements to pipeline
        pipeline.add_many(&[
            &source,
            &audioconvert,
            &audioresample,
            &caps_filter,
            appsink.upcast_ref(),
        ]).map_err(|e| StreamingError::InitializationFailed {
            component: "Pipeline assembly".to_string(),
            reason: e.to_string(),
        })?;

        // Link elements
        gst::Element::link_many(&[
            &source,
            &audioconvert,
            &audioresample,
            &caps_filter,
            appsink.upcast_ref(),
        ]).map_err(|e| StreamingError::InitializationFailed {
            component: "Pipeline linking".to_string(),
            reason: e.to_string(),
        })?;

        // Set up sample callback
        let sample_queue = Arc::clone(&self.sample_queue);
        let stats = Arc::clone(&self.stats);
        let config = self.config.clone();

        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample_queue = Arc::clone(&sample_queue);
                    let stats = Arc::clone(&stats);
                    let config = config.clone();

                    tokio::spawn(async move {
                        if let Ok(sample) = appsink.pull_sample() {
                            Self::handle_audio_sample(sample, sample_queue, stats, config).await;
                        }
                    });

                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        self.pipeline = Some(pipeline);
        self.appsink = Some(appsink);

        Ok(())
    }

    #[cfg(feature = "streaming")]
    async fn handle_audio_sample(
        sample: gst::Sample,
        sample_queue: Arc<Mutex<VecDeque<AudioSample>>>,
        stats: Arc<Mutex<AudioStats>>,
        config: AudioConfig,
    ) {
        if let Some(buffer) = sample.buffer() {
            if let Ok(map) = buffer.map_readable() {
                let data = map.as_slice().to_vec();
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_micros() as u64;

                let samples_per_channel = data.len() / (config.channels as usize * 2); // 16-bit = 2 bytes

                let audio_sample = AudioSample {
                    data,
                    timestamp,
                    sample_rate: config.sample_rate,
                    channels: config.channels,
                    samples_per_channel,
                };

                // Add to queue
                let mut queue = sample_queue.lock().await;
                if queue.len() >= 32 {  // Prevent buffer overflow
                    queue.pop_front();

                    let mut stats = stats.lock().await;
                    stats.frames_dropped += 1;
                    stats.buffer_underruns += 1;
                } else {
                    queue.push_back(audio_sample);

                    let mut stats = stats.lock().await;
                    stats.frames_captured += 1;
                    stats.bytes_processed += data.len() as u64;
                    stats.last_capture_time = Some(Instant::now());
                }
            }
        }
    }

    async fn process_audio_samples(
        sample_queue: Arc<Mutex<VecDeque<AudioSample>>>,
        frame_sender: broadcast::Sender<AudioFrame>,
        stats: Arc<Mutex<AudioStats>>,
        config: AudioConfig,
    ) {
        let mut encoder = match AudioEncoder::new(config.clone()).await {
            Ok(encoder) => encoder,
            Err(_) => return,
        };

        loop {
            let sample = {
                let mut queue = sample_queue.lock().await;
                queue.pop_front()
            };

            if let Some(sample) = sample {
                match encoder.encode_sample(sample).await {
                    Ok(Some(frame)) => {
                        if frame_sender.send(frame).is_err() {
                            break; // No receivers
                        }
                    }
                    Ok(None) => {
                        // Frame not ready yet
                    }
                    Err(_) => {
                        let mut stats = stats.lock().await;
                        stats.encoding_errors += 1;
                    }
                }
            } else {
                sleep(Duration::from_millis(1)).await;
            }
        }
    }
}

impl AudioEncoder {
    async fn new(config: AudioConfig) -> Result<Self> {
        Ok(Self {
            config,
            #[cfg(feature = "streaming")]
            encoder_pipeline: None,
            encode_queue: Arc::new(Mutex::new(VecDeque::new())),
            stats: Arc::new(Mutex::new(AudioStats::default())),
        })
    }

    async fn encode_sample(&mut self, sample: AudioSample) -> Result<Option<AudioFrame>> {
        match self.config.codec {
            AudioCodec::Opus => self.encode_opus(sample).await,
            AudioCodec::AAC => self.encode_aac(sample).await,
            AudioCodec::PCM => self.encode_pcm(sample).await,
        }
    }

    async fn encode_opus(&self, sample: AudioSample) -> Result<Option<AudioFrame>> {
        // In a real implementation, this would use libopus
        // For now, simulate Opus encoding
        let compressed_size = sample.data.len() / 8; // Simulate compression
        let compressed_data = vec![0u8; compressed_size];

        Ok(Some(AudioFrame {
            data: compressed_data,
            timestamp: sample.timestamp,
            codec: AudioCodec::Opus,
            sample_rate: sample.sample_rate,
            channels: sample.channels,
            samples: sample.samples_per_channel,
        }))
    }

    async fn encode_aac(&self, sample: AudioSample) -> Result<Option<AudioFrame>> {
        // In a real implementation, this would use libfdk-aac
        // For now, simulate AAC encoding
        let compressed_size = sample.data.len() / 6; // Simulate compression
        let compressed_data = vec![0u8; compressed_size];

        Ok(Some(AudioFrame {
            data: compressed_data,
            timestamp: sample.timestamp,
            codec: AudioCodec::AAC,
            sample_rate: sample.sample_rate,
            channels: sample.channels,
            samples: sample.samples_per_channel,
        }))
    }

    async fn encode_pcm(&self, sample: AudioSample) -> Result<Option<AudioFrame>> {
        // PCM is uncompressed
        Ok(Some(AudioFrame {
            data: sample.data,
            timestamp: sample.timestamp,
            codec: AudioCodec::PCM,
            sample_rate: sample.sample_rate,
            channels: sample.channels,
            samples: sample.samples_per_channel,
        }))
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        #[cfg(feature = "streaming")]
        {
            if let Some(ref pipeline) = self.pipeline {
                let _ = pipeline.set_state(gst::State::Null);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_audio_capture_creation() {
        let config = AudioConfig::default();
        let result = AudioCapture::new(config);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_audio_config_defaults() {
        let config = AudioConfig::default();
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.channels, 2);
        assert_eq!(config.bit_depth, 16);
        assert_eq!(config.codec, AudioCodec::Opus);
    }

    #[tokio::test]
    async fn test_audio_encoder_creation() {
        let config = AudioConfig::default();
        let result = AudioEncoder::new(config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_pcm_encoding() {
        let config = AudioConfig {
            codec: AudioCodec::PCM,
            ..Default::default()
        };
        let encoder = AudioEncoder::new(config).await.unwrap();

        let sample = AudioSample {
            data: vec![0, 1, 2, 3, 4, 5, 6, 7],
            timestamp: 1000,
            sample_rate: 48000,
            channels: 2,
            samples_per_channel: 2,
        };

        let result = encoder.encode_pcm(sample).await;
        assert!(result.is_ok());

        let frame = result.unwrap().unwrap();
        assert_eq!(frame.codec, AudioCodec::PCM);
        assert_eq!(frame.data.len(), 8);
    }

    #[tokio::test]
    async fn test_opus_encoding_simulation() {
        let config = AudioConfig {
            codec: AudioCodec::Opus,
            ..Default::default()
        };
        let encoder = AudioEncoder::new(config).await.unwrap();

        let sample = AudioSample {
            data: vec![0u8; 1024],  // 1024 bytes of audio
            timestamp: 1000,
            sample_rate: 48000,
            channels: 2,
            samples_per_channel: 256,
        };

        let result = encoder.encode_opus(sample).await;
        assert!(result.is_ok());

        let frame = result.unwrap().unwrap();
        assert_eq!(frame.codec, AudioCodec::Opus);
        assert_eq!(frame.data.len(), 128);  // Compressed to 1/8 size
    }

    #[tokio::test]
    async fn test_audio_stats() {
        let config = AudioConfig::default();
        let mut capture = AudioCapture::new(config).unwrap();

        let stats = capture.get_stats().await;
        assert_eq!(stats.frames_captured, 0);
        assert_eq!(stats.frames_dropped, 0);
        assert_eq!(stats.bytes_processed, 0);
    }
}