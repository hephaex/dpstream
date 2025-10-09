//! Advanced memory optimization for streaming workloads
//!
//! Implements custom allocators, memory pools, and allocation patterns
//! optimized for high-performance streaming with minimal GC pressure.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicPtr, AtomicUsize, AtomicU64, Ordering};
use std::sync::Arc;
use std::ptr::{self, NonNull};
use std::mem::{self, MaybeUninit, ManuallyDrop};
use cache_padded::CachePadded;
use bumpalo::Bump;
use parking_lot::{Mutex, RwLock};
use smallvec::{SmallVec, smallvec};
use arrayvec::ArrayVec;
use slab::Slab;
use tracing::{debug, warn, error, info};

/// High-performance streaming allocator with optimized allocation patterns
pub struct StreamingAllocator {
    /// Video frame pool for zero-allocation frame processing
    frame_pool: Arc<Mutex<Slab<VideoFrameSlot>>>,
    /// Audio buffer pool for low-latency audio processing
    audio_pool: Arc<Mutex<Slab<AudioBufferSlot>>>,
    /// Network packet pool for zero-copy networking
    packet_pool: Arc<Mutex<Slab<PacketSlot>>>,
    /// Temporary allocation arena for short-lived objects
    temp_arena: Arc<Mutex<Bump>>,
    /// Long-lived allocation tracking
    persistent_allocations: Arc<RwLock<Vec<PersistentAllocation>>>,
    /// Memory usage statistics
    stats: Arc<MemoryStats>,
}

/// Memory allocation statistics with cache-aligned counters
#[derive(Debug)]
pub struct MemoryStats {
    pub total_allocations: CachePadded<AtomicU64>,
    pub total_deallocations: CachePadded<AtomicU64>,
    pub bytes_allocated: CachePadded<AtomicU64>,
    pub bytes_deallocated: CachePadded<AtomicU64>,
    pub pool_hits: CachePadded<AtomicU64>,
    pub pool_misses: CachePadded<AtomicU64>,
    pub arena_allocations: CachePadded<AtomicU64>,
    pub peak_memory_usage: CachePadded<AtomicU64>,
    pub current_memory_usage: CachePadded<AtomicU64>,
    pub fragmentation_score: CachePadded<AtomicU64>,
}

impl Default for MemoryStats {
    fn default() -> Self {
        Self {
            total_allocations: CachePadded::new(AtomicU64::new(0)),
            total_deallocations: CachePadded::new(AtomicU64::new(0)),
            bytes_allocated: CachePadded::new(AtomicU64::new(0)),
            bytes_deallocated: CachePadded::new(AtomicU64::new(0)),
            pool_hits: CachePadded::new(AtomicU64::new(0)),
            pool_misses: CachePadded::new(AtomicU64::new(0)),
            arena_allocations: CachePadded::new(AtomicU64::new(0)),
            peak_memory_usage: CachePadded::new(AtomicU64::new(0)),
            current_memory_usage: CachePadded::new(AtomicU64::new(0)),
            fragmentation_score: CachePadded::new(AtomicU64::new(0)),
        }
    }
}

/// Pre-allocated video frame slot with cache-aligned memory
#[repr(align(64))]
pub struct VideoFrameSlot {
    /// Pre-allocated frame data (4K RGBA maximum)
    data: Box<[MaybeUninit<u8>; 3840 * 2160 * 4]>,
    /// Actual data size
    size: AtomicUsize,
    /// Frame metadata
    width: u32,
    height: u32,
    timestamp: u64,
    frame_number: u64,
    /// Allocation tracking
    allocated_at: std::time::Instant,
    in_use: AtomicUsize,  // 0 = free, 1 = allocated
}

/// Pre-allocated audio buffer slot
#[repr(align(64))]
pub struct AudioBufferSlot {
    /// Audio data (48kHz stereo, 1 second maximum)
    data: Box<[MaybeUninit<f32>; 48000 * 2]>,
    /// Actual sample count
    sample_count: AtomicUsize,
    /// Audio metadata
    sample_rate: u32,
    channels: u16,
    timestamp: u64,
    /// Allocation tracking
    allocated_at: std::time::Instant,
    in_use: AtomicUsize,
}

/// Pre-allocated network packet slot
#[repr(align(64))]
pub struct PacketSlot {
    /// Packet data (jumbo frame maximum)
    data: Box<[MaybeUninit<u8>; 9000]>,
    /// Actual packet size
    size: AtomicUsize,
    /// Packet metadata
    packet_type: u16,
    timestamp: u64,
    sequence: u32,
    /// Allocation tracking
    allocated_at: std::time::Instant,
    in_use: AtomicUsize,
}

/// Persistent allocation tracking for memory leak detection
#[derive(Debug, Clone)]
pub struct PersistentAllocation {
    ptr: NonNull<u8>,
    size: usize,
    layout: Layout,
    allocated_at: std::time::Instant,
    caller: &'static str,
}

impl StreamingAllocator {
    /// Create a new streaming allocator with optimized pools
    pub fn new() -> Self {
        info!("Initializing streaming allocator with optimized memory pools");

        // Pre-allocate frame pool (16 frames for 8+ concurrent clients)
        let mut frame_pool = Slab::with_capacity(16);
        for _ in 0..16 {
            frame_pool.insert(VideoFrameSlot {
                data: Box::new([MaybeUninit::uninit(); 3840 * 2160 * 4]),
                size: AtomicUsize::new(0),
                width: 0,
                height: 0,
                timestamp: 0,
                frame_number: 0,
                allocated_at: std::time::Instant::now(),
                in_use: AtomicUsize::new(0),
            });
        }

        // Pre-allocate audio pool (32 buffers for low-latency audio)
        let mut audio_pool = Slab::with_capacity(32);
        for _ in 0..32 {
            audio_pool.insert(AudioBufferSlot {
                data: Box::new([MaybeUninit::uninit(); 48000 * 2]),
                sample_count: AtomicUsize::new(0),
                sample_rate: 48000,
                channels: 2,
                timestamp: 0,
                allocated_at: std::time::Instant::now(),
                in_use: AtomicUsize::new(0),
            });
        }

        // Pre-allocate packet pool (128 packets for high-throughput networking)
        let mut packet_pool = Slab::with_capacity(128);
        for _ in 0..128 {
            packet_pool.insert(PacketSlot {
                data: Box::new([MaybeUninit::uninit(); 9000]),
                size: AtomicUsize::new(0),
                packet_type: 0,
                timestamp: 0,
                sequence: 0,
                allocated_at: std::time::Instant::now(),
                in_use: AtomicUsize::new(0),
            });
        }

        info!("Streaming allocator initialized with {} frame slots, {} audio slots, {} packet slots",
              frame_pool.len(), audio_pool.len(), packet_pool.len());

        Self {
            frame_pool: Arc::new(Mutex::new(frame_pool)),
            audio_pool: Arc::new(Mutex::new(audio_pool)),
            packet_pool: Arc::new(Mutex::new(packet_pool)),
            temp_arena: Arc::new(Mutex::new(Bump::with_capacity(64 * 1024))), // 64KB arena
            persistent_allocations: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(MemoryStats::default()),
        }
    }

    /// Allocate video frame with zero-copy optimization
    pub fn allocate_video_frame(&self, width: u32, height: u32) -> Result<VideoFrameHandle, AllocationError> {
        let start_time = std::time::Instant::now();

        let required_size = (width * height * 4) as usize; // RGBA
        if required_size > 3840 * 2160 * 4 {
            return Err(AllocationError::SizeTooLarge(required_size));
        }

        let mut pool = self.frame_pool.lock();

        // Find available slot
        for (key, slot) in pool.iter_mut() {
            if slot.in_use.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_ok() {
                // Successfully claimed slot
                slot.size.store(required_size, Ordering::Release);
                slot.width = width;
                slot.height = height;
                slot.timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64;
                slot.allocated_at = std::time::Instant::now();

                self.stats.pool_hits.fetch_add(1, Ordering::Relaxed);
                self.stats.total_allocations.fetch_add(1, Ordering::Relaxed);
                self.stats.bytes_allocated.fetch_add(required_size as u64, Ordering::Relaxed);

                debug!("Allocated video frame {}x{} from pool slot {}", width, height, key);

                return Ok(VideoFrameHandle {
                    slot_key: key,
                    allocator: self,
                    allocated_at: start_time,
                });
            }
        }

        // Pool exhausted
        self.stats.pool_misses.fetch_add(1, Ordering::Relaxed);
        Err(AllocationError::PoolExhausted)
    }

    /// Allocate audio buffer with low-latency optimization
    pub fn allocate_audio_buffer(&self, sample_count: usize, sample_rate: u32, channels: u16) -> Result<AudioBufferHandle, AllocationError> {
        let start_time = std::time::Instant::now();

        if sample_count > 48000 * 2 {
            return Err(AllocationError::SizeTooLarge(sample_count));
        }

        let mut pool = self.audio_pool.lock();

        // Find available slot
        for (key, slot) in pool.iter_mut() {
            if slot.in_use.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_ok() {
                slot.sample_count.store(sample_count, Ordering::Release);
                slot.sample_rate = sample_rate;
                slot.channels = channels;
                slot.timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64;
                slot.allocated_at = std::time::Instant::now();

                self.stats.pool_hits.fetch_add(1, Ordering::Relaxed);
                self.stats.total_allocations.fetch_add(1, Ordering::Relaxed);
                self.stats.bytes_allocated.fetch_add((sample_count * mem::size_of::<f32>()) as u64, Ordering::Relaxed);

                debug!("Allocated audio buffer {} samples from pool slot {}", sample_count, key);

                return Ok(AudioBufferHandle {
                    slot_key: key,
                    allocator: self,
                    allocated_at: start_time,
                });
            }
        }

        self.stats.pool_misses.fetch_add(1, Ordering::Relaxed);
        Err(AllocationError::PoolExhausted)
    }

    /// Allocate network packet with zero-copy networking
    pub fn allocate_packet(&self, size: usize, packet_type: u16) -> Result<PacketHandle, AllocationError> {
        let start_time = std::time::Instant::now();

        if size > 9000 {
            return Err(AllocationError::SizeTooLarge(size));
        }

        let mut pool = self.packet_pool.lock();

        // Find available slot
        for (key, slot) in pool.iter_mut() {
            if slot.in_use.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_ok() {
                slot.size.store(size, Ordering::Release);
                slot.packet_type = packet_type;
                slot.timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64;
                slot.allocated_at = std::time::Instant::now();

                self.stats.pool_hits.fetch_add(1, Ordering::Relaxed);
                self.stats.total_allocations.fetch_add(1, Ordering::Relaxed);
                self.stats.bytes_allocated.fetch_add(size as u64, Ordering::Relaxed);

                debug!("Allocated packet {} bytes from pool slot {}", size, key);

                return Ok(PacketHandle {
                    slot_key: key,
                    allocator: self,
                    allocated_at: start_time,
                });
            }
        }

        self.stats.pool_misses.fetch_add(1, Ordering::Relaxed);
        Err(AllocationError::PoolExhausted)
    }

    /// Allocate temporary object in arena (very fast, bulk deallocation)
    pub fn allocate_temp<T>(&self, value: T) -> ArenaHandle<T> {
        let arena = self.temp_arena.lock();
        let allocated = arena.alloc(value);

        self.stats.arena_allocations.fetch_add(1, Ordering::Relaxed);
        self.stats.bytes_allocated.fetch_add(mem::size_of::<T>() as u64, Ordering::Relaxed);

        ArenaHandle {
            ptr: allocated as *const T,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Reset temporary arena (bulk deallocation)
    pub fn reset_temp_arena(&self) {
        let mut arena = self.temp_arena.lock();
        let bytes_deallocated = arena.allocated_bytes();
        arena.reset();

        self.stats.bytes_deallocated.fetch_add(bytes_deallocated as u64, Ordering::Relaxed);
        debug!("Reset temporary arena, deallocated {} bytes", bytes_deallocated);
    }

    /// Release video frame back to pool
    fn release_video_frame(&self, slot_key: usize) {
        let pool = self.frame_pool.lock();
        if let Some(slot) = pool.get(slot_key) {
            let size = slot.size.load(Ordering::Acquire);
            slot.in_use.store(0, Ordering::Release);

            self.stats.total_deallocations.fetch_add(1, Ordering::Relaxed);
            self.stats.bytes_deallocated.fetch_add(size as u64, Ordering::Relaxed);

            debug!("Released video frame from pool slot {}", slot_key);
        }
    }

    /// Release audio buffer back to pool
    fn release_audio_buffer(&self, slot_key: usize) {
        let pool = self.audio_pool.lock();
        if let Some(slot) = pool.get(slot_key) {
            let sample_count = slot.sample_count.load(Ordering::Acquire);
            slot.in_use.store(0, Ordering::Release);

            self.stats.total_deallocations.fetch_add(1, Ordering::Relaxed);
            self.stats.bytes_deallocated.fetch_add((sample_count * mem::size_of::<f32>()) as u64, Ordering::Relaxed);

            debug!("Released audio buffer from pool slot {}", slot_key);
        }
    }

    /// Release packet back to pool
    fn release_packet(&self, slot_key: usize) {
        let pool = self.packet_pool.lock();
        if let Some(slot) = pool.get(slot_key) {
            let size = slot.size.load(Ordering::Acquire);
            slot.in_use.store(0, Ordering::Release);

            self.stats.total_deallocations.fetch_add(1, Ordering::Relaxed);
            self.stats.bytes_deallocated.fetch_add(size as u64, Ordering::Relaxed);

            debug!("Released packet from pool slot {}", slot_key);
        }
    }

    /// Get memory usage statistics
    pub fn get_stats(&self) -> MemoryUsageStats {
        let current_usage = self.stats.current_memory_usage.load(Ordering::Relaxed);
        let peak_usage = self.stats.peak_memory_usage.load(Ordering::Relaxed);

        if current_usage > peak_usage {
            self.stats.peak_memory_usage.store(current_usage, Ordering::Relaxed);
        }

        MemoryUsageStats {
            total_allocations: self.stats.total_allocations.load(Ordering::Relaxed),
            total_deallocations: self.stats.total_deallocations.load(Ordering::Relaxed),
            bytes_allocated: self.stats.bytes_allocated.load(Ordering::Relaxed),
            bytes_deallocated: self.stats.bytes_deallocated.load(Ordering::Relaxed),
            pool_hits: self.stats.pool_hits.load(Ordering::Relaxed),
            pool_misses: self.stats.pool_misses.load(Ordering::Relaxed),
            arena_allocations: self.stats.arena_allocations.load(Ordering::Relaxed),
            peak_memory_usage: peak_usage,
            current_memory_usage: current_usage,
            pool_hit_rate: self.pool_hit_rate(),
        }
    }

    /// Calculate pool hit rate percentage
    pub fn pool_hit_rate(&self) -> f64 {
        let hits = self.stats.pool_hits.load(Ordering::Relaxed) as f64;
        let misses = self.stats.pool_misses.load(Ordering::Relaxed) as f64;
        let total = hits + misses;

        if total > 0.0 {
            (hits / total) * 100.0
        } else {
            0.0
        }
    }

    /// Reset all statistics
    pub fn reset_stats(&self) {
        self.stats.total_allocations.store(0, Ordering::Relaxed);
        self.stats.total_deallocations.store(0, Ordering::Relaxed);
        self.stats.bytes_allocated.store(0, Ordering::Relaxed);
        self.stats.bytes_deallocated.store(0, Ordering::Relaxed);
        self.stats.pool_hits.store(0, Ordering::Relaxed);
        self.stats.pool_misses.store(0, Ordering::Relaxed);
        self.stats.arena_allocations.store(0, Ordering::Relaxed);
        // Don't reset peak_memory_usage as it's a high water mark
    }
}

/// RAII handle for video frame allocation
pub struct VideoFrameHandle<'a> {
    slot_key: usize,
    allocator: &'a StreamingAllocator,
    allocated_at: std::time::Instant,
}

impl<'a> VideoFrameHandle<'a> {
    /// Get mutable access to frame data
    pub fn data_mut(&mut self) -> &mut [u8] {
        let pool = self.allocator.frame_pool.lock();
        let slot = &pool[self.slot_key];
        let size = slot.size.load(Ordering::Acquire);

        unsafe {
            std::slice::from_raw_parts_mut(
                slot.data.as_ptr() as *mut u8,
                size
            )
        }
    }

    /// Get read-only access to frame data
    pub fn data(&self) -> &[u8] {
        let pool = self.allocator.frame_pool.lock();
        let slot = &pool[self.slot_key];
        let size = slot.size.load(Ordering::Acquire);

        unsafe {
            std::slice::from_raw_parts(
                slot.data.as_ptr() as *const u8,
                size
            )
        }
    }

    /// Get frame metadata
    pub fn metadata(&self) -> (u32, u32, u64) {
        let pool = self.allocator.frame_pool.lock();
        let slot = &pool[self.slot_key];
        (slot.width, slot.height, slot.timestamp)
    }
}

impl<'a> Drop for VideoFrameHandle<'a> {
    fn drop(&mut self) {
        let allocation_duration = self.allocated_at.elapsed();
        debug!("Video frame allocated for {:?}", allocation_duration);
        self.allocator.release_video_frame(self.slot_key);
    }
}

/// RAII handle for audio buffer allocation
pub struct AudioBufferHandle<'a> {
    slot_key: usize,
    allocator: &'a StreamingAllocator,
    allocated_at: std::time::Instant,
}

impl<'a> AudioBufferHandle<'a> {
    /// Get mutable access to audio samples
    pub fn samples_mut(&mut self) -> &mut [f32] {
        let pool = self.allocator.audio_pool.lock();
        let slot = &pool[self.slot_key];
        let sample_count = slot.sample_count.load(Ordering::Acquire);

        unsafe {
            std::slice::from_raw_parts_mut(
                slot.data.as_ptr() as *mut f32,
                sample_count
            )
        }
    }

    /// Get read-only access to audio samples
    pub fn samples(&self) -> &[f32] {
        let pool = self.allocator.audio_pool.lock();
        let slot = &pool[self.slot_key];
        let sample_count = slot.sample_count.load(Ordering::Acquire);

        unsafe {
            std::slice::from_raw_parts(
                slot.data.as_ptr() as *const f32,
                sample_count
            )
        }
    }

    /// Get audio metadata
    pub fn metadata(&self) -> (u32, u16, u64) {
        let pool = self.allocator.audio_pool.lock();
        let slot = &pool[self.slot_key];
        (slot.sample_rate, slot.channels, slot.timestamp)
    }
}

impl<'a> Drop for AudioBufferHandle<'a> {
    fn drop(&mut self) {
        let allocation_duration = self.allocated_at.elapsed();
        debug!("Audio buffer allocated for {:?}", allocation_duration);
        self.allocator.release_audio_buffer(self.slot_key);
    }
}

/// RAII handle for packet allocation
pub struct PacketHandle<'a> {
    slot_key: usize,
    allocator: &'a StreamingAllocator,
    allocated_at: std::time::Instant,
}

impl<'a> PacketHandle<'a> {
    /// Get mutable access to packet data
    pub fn data_mut(&mut self) -> &mut [u8] {
        let pool = self.allocator.packet_pool.lock();
        let slot = &pool[self.slot_key];
        let size = slot.size.load(Ordering::Acquire);

        unsafe {
            std::slice::from_raw_parts_mut(
                slot.data.as_ptr() as *mut u8,
                size
            )
        }
    }

    /// Get read-only access to packet data
    pub fn data(&self) -> &[u8] {
        let pool = self.allocator.packet_pool.lock();
        let slot = &pool[self.slot_key];
        let size = slot.size.load(Ordering::Acquire);

        unsafe {
            std::slice::from_raw_parts(
                slot.data.as_ptr() as *const u8,
                size
            )
        }
    }

    /// Get packet metadata
    pub fn metadata(&self) -> (u16, u64, u32) {
        let pool = self.allocator.packet_pool.lock();
        let slot = &pool[self.slot_key];
        (slot.packet_type, slot.timestamp, slot.sequence)
    }
}

impl<'a> Drop for PacketHandle<'a> {
    fn drop(&mut self) {
        let allocation_duration = self.allocated_at.elapsed();
        debug!("Packet allocated for {:?}", allocation_duration);
        self.allocator.release_packet(self.slot_key);
    }
}

/// Handle for arena-allocated objects
pub struct ArenaHandle<T> {
    ptr: *const T,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> std::ops::Deref for ArenaHandle<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

// Arena handles are automatically freed when arena is reset

/// Memory usage statistics
#[derive(Debug, Clone)]
pub struct MemoryUsageStats {
    pub total_allocations: u64,
    pub total_deallocations: u64,
    pub bytes_allocated: u64,
    pub bytes_deallocated: u64,
    pub pool_hits: u64,
    pub pool_misses: u64,
    pub arena_allocations: u64,
    pub peak_memory_usage: u64,
    pub current_memory_usage: u64,
    pub pool_hit_rate: f64,
}

/// Allocation errors
#[derive(Debug, thiserror::Error)]
pub enum AllocationError {
    #[error("Requested size {0} is too large")]
    SizeTooLarge(usize),

    #[error("Memory pool is exhausted")]
    PoolExhausted,

    #[error("Invalid allocation parameters")]
    InvalidParameters,

    #[error("Allocation failed: {0}")]
    AllocationFailed(String),
}

/// Global streaming allocator instance
pub static STREAMING_ALLOCATOR: once_cell::sync::Lazy<StreamingAllocator> =
    once_cell::sync::Lazy::new(|| StreamingAllocator::new());

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::sync::Arc;

    #[test]
    fn test_video_frame_allocation() {
        let allocator = StreamingAllocator::new();

        // Test allocation
        let mut frame = allocator.allocate_video_frame(1920, 1080).unwrap();
        let data = frame.data_mut();
        assert_eq!(data.len(), 1920 * 1080 * 4);

        // Test metadata
        let (width, height, _) = frame.metadata();
        assert_eq!(width, 1920);
        assert_eq!(height, 1080);

        // Frame should be released when dropped
        drop(frame);

        let stats = allocator.get_stats();
        assert_eq!(stats.total_allocations, 1);
        assert_eq!(stats.total_deallocations, 1);
    }

    #[test]
    fn test_audio_buffer_allocation() {
        let allocator = StreamingAllocator::new();

        // Test allocation
        let mut buffer = allocator.allocate_audio_buffer(1024, 48000, 2).unwrap();
        let samples = buffer.samples_mut();
        assert_eq!(samples.len(), 1024);

        // Fill with test data
        for (i, sample) in samples.iter_mut().enumerate() {
            *sample = (i as f32) * 0.001;
        }

        // Test metadata
        let (sample_rate, channels, _) = buffer.metadata();
        assert_eq!(sample_rate, 48000);
        assert_eq!(channels, 2);

        drop(buffer);

        let stats = allocator.get_stats();
        assert_eq!(stats.total_allocations, 1);
        assert_eq!(stats.total_deallocations, 1);
    }

    #[test]
    fn test_packet_allocation() {
        let allocator = StreamingAllocator::new();

        // Test allocation
        let mut packet = allocator.allocate_packet(1500, 96).unwrap();
        let data = packet.data_mut();
        assert_eq!(data.len(), 1500);

        // Fill with test data
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }

        // Test metadata
        let (packet_type, _, _) = packet.metadata();
        assert_eq!(packet_type, 96);

        drop(packet);

        let stats = allocator.get_stats();
        assert_eq!(stats.total_allocations, 1);
        assert_eq!(stats.total_deallocations, 1);
    }

    #[test]
    fn test_arena_allocation() {
        let allocator = StreamingAllocator::new();

        // Test temporary allocations
        {
            let temp1 = allocator.allocate_temp(42u32);
            let temp2 = allocator.allocate_temp("hello".to_string());

            assert_eq!(*temp1, 42);
            assert_eq!(*temp2, "hello");

            let stats = allocator.get_stats();
            assert_eq!(stats.arena_allocations, 2);
        }

        // Reset arena
        allocator.reset_temp_arena();

        let stats = allocator.get_stats();
        assert!(stats.bytes_deallocated > 0);
    }

    #[test]
    fn test_concurrent_allocations() {
        let allocator = Arc::new(StreamingAllocator::new());
        let num_threads = 8;
        let allocations_per_thread = 100;

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let allocator = allocator.clone();
                thread::spawn(move || {
                    for _ in 0..allocations_per_thread {
                        // Test video frame allocation
                        if let Ok(frame) = allocator.allocate_video_frame(1280, 720) {
                            // Use the frame briefly
                            let _ = frame.data().len();
                            drop(frame);
                        }

                        // Test audio buffer allocation
                        if let Ok(buffer) = allocator.allocate_audio_buffer(512, 48000, 2) {
                            let _ = buffer.samples().len();
                            drop(buffer);
                        }

                        // Test packet allocation
                        if let Ok(packet) = allocator.allocate_packet(1400, 96) {
                            let _ = packet.data().len();
                            drop(packet);
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = allocator.get_stats();
        assert!(stats.total_allocations > 0);
        assert_eq!(stats.total_allocations, stats.total_deallocations);
        assert!(stats.pool_hit_rate > 50.0); // Should have good hit rate
    }

    #[test]
    fn test_pool_exhaustion() {
        let allocator = StreamingAllocator::new();

        // Allocate all frames without releasing
        let mut frames = Vec::new();
        for _ in 0..16 {
            frames.push(allocator.allocate_video_frame(1920, 1080).unwrap());
        }

        // Next allocation should fail
        assert!(matches!(
            allocator.allocate_video_frame(1920, 1080),
            Err(AllocationError::PoolExhausted)
        ));

        // Release one frame
        frames.pop();

        // Should be able to allocate again
        assert!(allocator.allocate_video_frame(1920, 1080).is_ok());
    }

    #[test]
    fn test_size_limits() {
        let allocator = StreamingAllocator::new();

        // Test oversized allocations
        assert!(matches!(
            allocator.allocate_video_frame(8000, 8000), // Too large
            Err(AllocationError::SizeTooLarge(_))
        ));

        assert!(matches!(
            allocator.allocate_audio_buffer(100000, 48000, 2), // Too many samples
            Err(AllocationError::SizeTooLarge(_))
        ));

        assert!(matches!(
            allocator.allocate_packet(10000, 96), // Larger than jumbo frame
            Err(AllocationError::SizeTooLarge(_))
        ));
    }
}