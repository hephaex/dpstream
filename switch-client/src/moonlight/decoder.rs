//! Hardware H264 decoder for Nintendo Switch
//!
//! Implements NVDEC hardware-accelerated video decoding on Tegra X1

use crate::error::{Result, VideoError};
use crate::sys::memory::{check_memory_pressure, MemoryPressure, VideoBufferPool};
use alloc::vec::Vec;
use core::ptr::NonNull;
use core::time::Duration;

/// Video decoder configuration
#[derive(Debug, Clone)]
pub struct DecoderConfig {
    pub codec: VideoCodec,
    pub max_width: u32,
    pub max_height: u32,
    pub buffer_count: usize,
    pub enable_hardware_acceleration: bool,
    pub error_concealment: bool,
    pub low_latency_mode: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum VideoCodec {
    H264,
    H265,
}

/// Encoded video packet
#[derive(Debug, Clone)]
pub struct EncodedPacket {
    pub data: Vec<u8>,
    pub timestamp: u64,
    pub is_keyframe: bool,
    pub frame_number: u64,
}

/// Decoded video frame
#[derive(Debug)]
pub struct DecodedFrame {
    pub buffer: NonNull<u8>,
    pub width: u32,
    pub height: u32,
    pub timestamp: u64,
    pub frame_number: u64,
    pub format: PixelFormat,
}

#[derive(Debug, Clone, Copy)]
pub enum PixelFormat {
    NV12,   // Semi-planar YUV 4:2:0
    YUV420, // Planar YUV 4:2:0
    RGBA,   // 32-bit RGBA
}

/// Hardware video decoder using Tegra X1 NVDEC
pub struct VideoDecoder {
    config: DecoderConfig,
    buffer_pool: Option<VideoBufferPool>,
    is_initialized: bool,
    frames_decoded: u64,
    frames_dropped: u64,
    last_keyframe: Option<u64>,
    decoder_state: DecoderState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DecoderState {
    Uninitialized,
    Initialized,
    WaitingForKeyframe,
    Decoding,
    Error,
}

impl Default for DecoderConfig {
    fn default() -> Self {
        Self {
            codec: VideoCodec::H264,
            max_width: 1920,
            max_height: 1080,
            buffer_count: 8, // Multiple buffers for smooth playback
            enable_hardware_acceleration: true,
            error_concealment: true,
            low_latency_mode: true,
        }
    }
}

impl VideoDecoder {
    /// Create a new hardware video decoder
    pub fn new(config: DecoderConfig) -> Result<Self> {
        // Validate configuration
        if config.max_width == 0 || config.max_height == 0 {
            return Err(VideoError::InvalidConfiguration {
                reason: "Width and height must be greater than 0".into(),
            }
            .into());
        }

        if config.buffer_count < 2 {
            return Err(VideoError::InvalidConfiguration {
                reason: "Buffer count must be at least 2".into(),
            }
            .into());
        }

        Ok(Self {
            config,
            buffer_pool: None,
            is_initialized: false,
            frames_decoded: 0,
            frames_dropped: 0,
            last_keyframe: None,
            decoder_state: DecoderState::Uninitialized,
        })
    }

    /// Initialize the hardware decoder
    pub fn initialize(&mut self) -> Result<()> {
        if self.is_initialized {
            return Ok(());
        }

        // Calculate buffer size for the maximum resolution
        let buffer_size = match self.config.codec {
            VideoCodec::H264 | VideoCodec::H265 => {
                // NV12 format: Y plane + UV plane
                let y_size = (self.config.max_width * self.config.max_height) as usize;
                let uv_size = y_size / 2; // UV plane is half the size for 4:2:0
                y_size + uv_size
            }
        };

        // Check if we have enough memory
        let total_memory_needed = buffer_size * self.config.buffer_count;
        if !crate::sys::memory::check_available_memory(total_memory_needed)? {
            return Err(VideoError::InsufficientMemory {
                requested: total_memory_needed,
                available: crate::sys::memory::get_memory_stats()?.free_heap,
            }
            .into());
        }

        // Initialize video buffer pool
        self.buffer_pool = Some(VideoBufferPool::new(self.config.buffer_count, buffer_size)?);

        // Initialize hardware decoder
        if self.config.enable_hardware_acceleration {
            self.initialize_hardware_decoder()?;
        }

        self.is_initialized = true;
        self.decoder_state = DecoderState::WaitingForKeyframe;

        Ok(())
    }

    /// Decode an encoded video packet
    pub fn decode_packet(&mut self, packet: EncodedPacket) -> Result<Option<DecodedFrame>> {
        if !self.is_initialized {
            return Err(VideoError::DecoderNotInitialized.into());
        }

        // Check memory pressure and take action if needed
        match check_memory_pressure()? {
            MemoryPressure::Critical => {
                // Drop frame and try to recover memory
                self.frames_dropped += 1;
                crate::sys::memory::force_gc()?;
                return Ok(None);
            }
            MemoryPressure::High => {
                // Only decode keyframes under high pressure
                if !packet.is_keyframe {
                    self.frames_dropped += 1;
                    return Ok(None);
                }
            }
            _ => {} // Normal operation
        }

        // Handle decoder state transitions
        match self.decoder_state {
            DecoderState::WaitingForKeyframe => {
                if !packet.is_keyframe {
                    // Drop non-keyframes until we get a keyframe
                    return Ok(None);
                }
                self.decoder_state = DecoderState::Decoding;
                self.last_keyframe = Some(packet.frame_number);
            }
            DecoderState::Decoding => {
                if packet.is_keyframe {
                    self.last_keyframe = Some(packet.frame_number);
                }
            }
            DecoderState::Error => {
                // Try to recover by waiting for next keyframe
                if packet.is_keyframe {
                    self.decoder_state = DecoderState::Decoding;
                    self.last_keyframe = Some(packet.frame_number);
                } else {
                    return Ok(None);
                }
            }
            _ => return Err(VideoError::InvalidDecoderState.into()),
        }

        // Attempt to decode the packet
        match self.decode_packet_internal(&packet) {
            Ok(frame) => {
                self.frames_decoded += 1;
                Ok(frame)
            }
            Err(e) => {
                self.decoder_state = DecoderState::Error;
                // Don't propagate decoding errors immediately, try to recover
                if self.config.error_concealment {
                    Ok(None) // Return no frame but don't fail
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Get decoder statistics
    pub fn get_stats(&self) -> DecoderStats {
        DecoderStats {
            frames_decoded: self.frames_decoded,
            frames_dropped: self.frames_dropped,
            last_keyframe: self.last_keyframe,
            decoder_state: self.decoder_state,
            buffer_pool_stats: self.buffer_pool.as_ref().map(|pool| pool.stats()),
        }
    }

    /// Release a decoded frame back to the buffer pool
    pub fn release_frame(&mut self, frame: DecodedFrame) -> Result<()> {
        if let Some(ref mut pool) = self.buffer_pool {
            pool.release(frame.buffer)?;
        }
        Ok(())
    }

    /// Flush the decoder and reset state
    pub fn flush(&mut self) -> Result<()> {
        if self.config.enable_hardware_acceleration {
            self.flush_hardware_decoder()?;
        }

        self.decoder_state = DecoderState::WaitingForKeyframe;
        Ok(())
    }

    /// Shutdown the decoder and cleanup resources
    pub fn shutdown(&mut self) -> Result<()> {
        if !self.is_initialized {
            return Ok(());
        }

        if self.config.enable_hardware_acceleration {
            self.shutdown_hardware_decoder()?;
        }

        // Buffer pool will be automatically cleaned up when dropped
        self.buffer_pool = None;
        self.is_initialized = false;
        self.decoder_state = DecoderState::Uninitialized;

        Ok(())
    }

    fn decode_packet_internal(&mut self, packet: &EncodedPacket) -> Result<Option<DecodedFrame>> {
        // Get a buffer from the pool
        let buffer_ptr = self
            .buffer_pool
            .as_mut()
            .ok_or(VideoError::DecoderNotInitialized)?
            .acquire()
            .ok_or(VideoError::NoBuffersAvailable)?;

        if self.config.enable_hardware_acceleration {
            self.decode_with_hardware(packet, buffer_ptr)
        } else {
            self.decode_with_software(packet, buffer_ptr)
        }
    }

    fn decode_with_hardware(
        &self,
        packet: &EncodedPacket,
        buffer: NonNull<u8>,
    ) -> Result<Option<DecodedFrame>> {
        // In a real implementation, this would:
        // 1. Set up NVDEC command buffer
        // 2. Configure decoding parameters
        // 3. Submit H264 bitstream to hardware
        // 4. Wait for completion or use async notification
        // 5. Handle any decoding errors

        // For now, simulate hardware decoding
        self.simulate_hardware_decode(packet, buffer)
    }

    fn decode_with_software(
        &self,
        packet: &EncodedPacket,
        buffer: NonNull<u8>,
    ) -> Result<Option<DecodedFrame>> {
        // Software fallback (very limited on Switch)
        // In practice, this would use a minimal software decoder
        // or return an error if hardware is not available

        self.simulate_software_decode(packet, buffer)
    }

    fn simulate_hardware_decode(
        &self,
        packet: &EncodedPacket,
        buffer: NonNull<u8>,
    ) -> Result<Option<DecodedFrame>> {
        // Simulate NVDEC hardware decoding
        // In reality, this would involve:
        // - NVDEC command buffer setup
        // - DMA transfers
        // - Hardware synchronization

        // For simulation, just fill buffer with test pattern
        unsafe {
            let buffer_size = self.config.max_width * self.config.max_height * 3 / 2; // NV12
            core::ptr::write_bytes(buffer.as_ptr(), 128, buffer_size as usize); // Gray frame
        }

        let decoded_frame = DecodedFrame {
            buffer,
            width: self.config.max_width,
            height: self.config.max_height,
            timestamp: packet.timestamp,
            frame_number: packet.frame_number,
            format: PixelFormat::NV12,
        };

        Ok(Some(decoded_frame))
    }

    fn simulate_software_decode(
        &self,
        packet: &EncodedPacket,
        buffer: NonNull<u8>,
    ) -> Result<Option<DecodedFrame>> {
        // Simulate limited software decoding
        // On Switch, software decoding is very limited due to CPU constraints

        if packet.data.len() > 1024 * 1024 {
            // Too large for software decoding
            return Err(VideoError::SoftwareDecodingFailed {
                reason: "Packet too large for software decoder".into(),
            }
            .into());
        }

        // Simulate software decode (much slower)
        unsafe {
            let buffer_size = self.config.max_width * self.config.max_height * 3 / 2;
            core::ptr::write_bytes(buffer.as_ptr(), 64, buffer_size as usize); // Darker gray for software
        }

        let decoded_frame = DecodedFrame {
            buffer,
            width: self.config.max_width,
            height: self.config.max_height,
            timestamp: packet.timestamp,
            frame_number: packet.frame_number,
            format: PixelFormat::NV12,
        };

        Ok(Some(decoded_frame))
    }

    fn initialize_hardware_decoder(&self) -> Result<()> {
        // In a real implementation, this would:
        // 1. Initialize NVDEC hardware unit
        // 2. Set up command buffers
        // 3. Configure decoder for H264/H265
        // 4. Set up DMA channels
        // 5. Configure memory mapping

        // Simulate initialization delay
        // In reality, this would be hardware register access
        Ok(())
    }

    fn flush_hardware_decoder(&self) -> Result<()> {
        // In a real implementation:
        // 1. Send flush command to NVDEC
        // 2. Wait for all pending operations to complete
        // 3. Clear internal buffers
        // 4. Reset decoder state

        Ok(())
    }

    fn shutdown_hardware_decoder(&self) -> Result<()> {
        // In a real implementation:
        // 1. Flush all pending operations
        // 2. Disable NVDEC unit
        // 3. Free hardware resources
        // 4. Unmap memory regions

        Ok(())
    }
}

impl Drop for VideoDecoder {
    fn drop(&mut self) {
        if self.is_initialized {
            let _ = self.shutdown();
        }
    }
}

/// Decoder statistics
#[derive(Debug, Clone)]
pub struct DecoderStats {
    pub frames_decoded: u64,
    pub frames_dropped: u64,
    pub last_keyframe: Option<u64>,
    pub decoder_state: DecoderState,
    pub buffer_pool_stats: Option<(usize, usize, usize)>, // (total, available, in_use)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_creation() {
        let config = DecoderConfig::default();
        let result = VideoDecoder::new(config);
        assert!(result.is_ok());

        let decoder = result.unwrap();
        assert!(!decoder.is_initialized);
        assert_eq!(decoder.decoder_state, DecoderState::Uninitialized);
    }

    #[test]
    fn test_decoder_initialization() {
        let config = DecoderConfig {
            max_width: 1280,
            max_height: 720,
            buffer_count: 4,
            ..Default::default()
        };

        let mut decoder = VideoDecoder::new(config).unwrap();
        let result = decoder.initialize();
        assert!(result.is_ok());
        assert!(decoder.is_initialized);
        assert_eq!(decoder.decoder_state, DecoderState::WaitingForKeyframe);
    }

    #[test]
    fn test_keyframe_requirement() {
        let config = DecoderConfig {
            max_width: 640,
            max_height: 480,
            buffer_count: 2,
            ..Default::default()
        };

        let mut decoder = VideoDecoder::new(config).unwrap();
        decoder.initialize().unwrap();

        // Non-keyframe should be dropped initially
        let non_keyframe = EncodedPacket {
            data: vec![0; 1024],
            timestamp: 1000,
            is_keyframe: false,
            frame_number: 1,
        };

        let result = decoder.decode_packet(non_keyframe);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        // Keyframe should be decoded
        let keyframe = EncodedPacket {
            data: vec![0; 1024],
            timestamp: 2000,
            is_keyframe: true,
            frame_number: 2,
        };

        let result = decoder.decode_packet(keyframe);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn test_invalid_config() {
        let config = DecoderConfig {
            max_width: 0,
            max_height: 720,
            ..Default::default()
        };

        let result = VideoDecoder::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_buffer_management() {
        let config = DecoderConfig {
            max_width: 320,
            max_height: 240,
            buffer_count: 2,
            ..Default::default()
        };

        let mut decoder = VideoDecoder::new(config).unwrap();
        decoder.initialize().unwrap();

        let keyframe = EncodedPacket {
            data: vec![0; 512],
            timestamp: 1000,
            is_keyframe: true,
            frame_number: 1,
        };

        // Decode a frame
        let result = decoder.decode_packet(keyframe);
        assert!(result.is_ok());

        if let Some(frame) = result.unwrap() {
            // Release the frame back to the pool
            let release_result = decoder.release_frame(frame);
            assert!(release_result.is_ok());
        }
    }
}
