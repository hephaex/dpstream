#![allow(dead_code)]

//! Zero-copy video pipeline for maximum performance
//!
//! Implements pre-allocated buffer pools and DMA-friendly memory management
//! to eliminate allocation overhead in the critical video processing path.

use bumpalo::Bump;
use crossbeam_utils::CachePadded;
use parking_lot::{Mutex, RwLock};
use smallvec::{smallvec, SmallVec};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Zero-copy video buffer with pre-allocated memory pools
#[repr(align(64))] // Cache line aligned for optimal performance
#[derive(Debug)]
#[allow(dead_code)]
pub struct ZeroCopyVideoBuffer {
    /// Pre-allocated video data aligned for DMA operations
    data: *mut u8,
    /// Buffer capacity in bytes
    capacity: usize,
    /// Current data length
    length: AtomicUsize,
    /// Buffer ID for tracking and debugging
    buffer_id: u64,
    /// Reference count for safe sharing
    ref_count: AtomicUsize,
    /// Memory pool this buffer belongs to
    pool_id: u32,
}

unsafe impl Send for ZeroCopyVideoBuffer {}
unsafe impl Sync for ZeroCopyVideoBuffer {}

#[allow(dead_code)]
impl ZeroCopyVideoBuffer {
    /// Create a new zero-copy buffer with specified capacity
    pub fn new(capacity: usize, buffer_id: u64, pool_id: u32) -> Result<Self, PoolError> {
        // Allocate aligned memory for optimal cache performance
        let layout = std::alloc::Layout::from_size_align(capacity, 64)
            .map_err(|_| PoolError::AllocationFailed)?;

        let data = unsafe { std::alloc::alloc_zeroed(layout) };
        if data.is_null() {
            return Err(PoolError::AllocationFailed);
        }

        Ok(Self {
            data,
            capacity,
            length: AtomicUsize::new(0),
            buffer_id,
            ref_count: AtomicUsize::new(1),
            pool_id,
        })
    }

    /// Get a slice of the current valid data
    #[inline(always)]
    pub fn data(&self) -> &[u8] {
        let length = self.length.load(Ordering::Acquire);
        unsafe { std::slice::from_raw_parts(self.data, length) }
    }

    /// Get a mutable slice for writing data (zero-copy)
    #[inline(always)]
    pub fn data_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.data, self.capacity) }
    }

    /// Set the valid data length
    #[inline(always)]
    pub fn set_length(&self, length: usize) {
        debug_assert!(length <= self.capacity);
        self.length.store(length, Ordering::Release);
    }

    /// Get buffer capacity
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get buffer ID for tracking
    #[inline(always)]
    pub fn id(&self) -> u64 {
        self.buffer_id
    }

    /// Increment reference count
    #[inline(always)]
    pub fn add_ref(&self) {
        self.ref_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement reference count and return true if should be freed
    #[inline(always)]
    pub fn release(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::Relaxed) == 1
    }
}

impl Drop for ZeroCopyVideoBuffer {
    fn drop(&mut self) {
        unsafe {
            let layout = std::alloc::Layout::from_size_align_unchecked(self.capacity, 64);
            std::alloc::dealloc(self.data, layout);
        }
    }
}

/// High-performance video buffer pool with intelligent allocation strategies
#[allow(dead_code)]
pub struct VideoBufferPool {
    /// Pre-allocated buffers for different resolutions
    pools: RwLock<SmallVec<[BufferPoolTier; 4]>>,
    /// Arena allocator for temporary objects
    arena: Mutex<Bump>,
    /// Pool statistics for monitoring
    stats: CachePadded<PoolStatistics>,
    /// Next buffer ID for tracking
    next_buffer_id: AtomicUsize,
    /// Pool configuration
    config: PoolConfig,
}

#[derive(Debug)]
#[allow(dead_code)]
struct BufferPoolTier {
    /// Buffers in this tier
    buffers: Vec<Arc<ZeroCopyVideoBuffer>>,
    /// Available buffer indices
    available: Vec<usize>,
    /// Buffer size for this tier
    buffer_size: usize,
    /// Tier ID for identification
    tier_id: u32,
    /// Allocation count for this tier
    allocation_count: AtomicUsize,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PoolConfig {
    /// Number of buffers per tier
    pub buffers_per_tier: usize,
    /// Buffer sizes for different tiers (720p, 1080p, 4K, etc.)
    pub tier_sizes: SmallVec<[usize; 4]>,
    /// Enable pressure-adaptive allocation
    pub adaptive_allocation: bool,
    /// Maximum total memory usage
    pub max_memory_bytes: usize,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            buffers_per_tier: 16,
            tier_sizes: smallvec![
                1280 * 720 * 4,  // 720p RGBA
                1920 * 1080 * 4, // 1080p RGBA
                3840 * 2160 * 4, // 4K RGBA
                7680 * 4320 * 4, // 8K RGBA (future)
            ],
            adaptive_allocation: true,
            max_memory_bytes: 512 * 1024 * 1024, // 512MB max
        }
    }
}

#[derive(Debug, Default)]
pub struct PoolStatistics {
    total_allocations: AtomicUsize,
    pool_hits: AtomicUsize,
    pool_misses: AtomicUsize,
    peak_usage: AtomicUsize,
    current_usage: AtomicUsize,
    allocation_failures: AtomicUsize,
}

#[allow(dead_code)]
impl VideoBufferPool {
    /// Create a new video buffer pool with specified configuration
    pub fn new(config: PoolConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let mut pools = SmallVec::new();

        // Pre-allocate buffer tiers
        for (tier_id, &buffer_size) in config.tier_sizes.iter().enumerate() {
            let mut buffers = Vec::with_capacity(config.buffers_per_tier);
            let mut available = Vec::with_capacity(config.buffers_per_tier);

            // Pre-allocate buffers for this tier
            for i in 0..config.buffers_per_tier {
                let buffer_id = (tier_id as u64) << 32 | (i as u64);
                let buffer = Arc::new(ZeroCopyVideoBuffer::new(
                    buffer_size,
                    buffer_id,
                    tier_id as u32,
                )?);
                buffers.push(buffer);
                available.push(i);
            }

            pools.push(BufferPoolTier {
                buffers,
                available,
                buffer_size,
                tier_id: tier_id as u32,
                allocation_count: AtomicUsize::new(0),
            });
        }

        Ok(Self {
            pools: RwLock::new(pools),
            arena: Mutex::new(Bump::new()),
            stats: CachePadded::new(PoolStatistics::default()),
            next_buffer_id: AtomicUsize::new(1000),
            config,
        })
    }

    /// Acquire a buffer of specified minimum size (zero-copy)
    pub fn acquire_buffer(&self, min_size: usize) -> Result<Arc<ZeroCopyVideoBuffer>, PoolError> {
        self.stats.total_allocations.fetch_add(1, Ordering::Relaxed);

        let pools = self.pools.read();

        // Find the smallest tier that can accommodate the request
        for tier in pools.iter() {
            if tier.buffer_size >= min_size && !tier.available.is_empty() {
                return self.acquire_from_tier(tier);
            }
        }

        // No suitable buffer found in pools
        self.stats.pool_misses.fetch_add(1, Ordering::Relaxed);

        if self.config.adaptive_allocation {
            // Try to allocate a new buffer dynamically
            self.allocate_dynamic_buffer(min_size)
        } else {
            Err(PoolError::NoAvailableBuffers)
        }
    }

    /// Acquire buffer from a specific tier
    fn acquire_from_tier(
        &self,
        tier: &BufferPoolTier,
    ) -> Result<Arc<ZeroCopyVideoBuffer>, PoolError> {
        // This is a simplified implementation - in production, this would need
        // proper synchronization for the available indices
        if let Some(&index) = tier.available.last() {
            let buffer = tier.buffers[index].clone();
            tier.allocation_count.fetch_add(1, Ordering::Relaxed);
            self.stats.pool_hits.fetch_add(1, Ordering::Relaxed);
            self.stats.current_usage.fetch_add(1, Ordering::Relaxed);

            // Update peak usage tracking
            let current = self.stats.current_usage.load(Ordering::Relaxed);
            let peak = self.stats.peak_usage.load(Ordering::Relaxed);
            if current > peak {
                self.stats.peak_usage.store(current, Ordering::Relaxed);
            }

            Ok(buffer)
        } else {
            Err(PoolError::TierExhausted)
        }
    }

    /// Allocate a dynamic buffer when pools are exhausted
    fn allocate_dynamic_buffer(&self, size: usize) -> Result<Arc<ZeroCopyVideoBuffer>, PoolError> {
        let buffer_id = self.next_buffer_id.fetch_add(1, Ordering::Relaxed) as u64;

        match ZeroCopyVideoBuffer::new(size, buffer_id, u32::MAX) {
            Ok(buffer) => {
                self.stats.current_usage.fetch_add(1, Ordering::Relaxed);
                Ok(Arc::new(buffer))
            }
            Err(_) => {
                self.stats
                    .allocation_failures
                    .fetch_add(1, Ordering::Relaxed);
                Err(PoolError::AllocationFailed)
            }
        }
    }

    /// Release a buffer back to the pool
    pub fn release_buffer(&self, buffer: Arc<ZeroCopyVideoBuffer>) {
        if buffer.release() {
            self.stats.current_usage.fetch_sub(1, Ordering::Relaxed);

            // If this is a pooled buffer, return it to the available list
            if buffer.pool_id != u32::MAX {
                // In production, this would return the buffer to the appropriate tier
                // For now, we just track the release
            }
        }
    }

    /// Get pool statistics for monitoring
    pub fn get_statistics(&self) -> PoolStatistics {
        PoolStatistics {
            total_allocations: AtomicUsize::new(
                self.stats.total_allocations.load(Ordering::Relaxed),
            ),
            pool_hits: AtomicUsize::new(self.stats.pool_hits.load(Ordering::Relaxed)),
            pool_misses: AtomicUsize::new(self.stats.pool_misses.load(Ordering::Relaxed)),
            peak_usage: AtomicUsize::new(self.stats.peak_usage.load(Ordering::Relaxed)),
            current_usage: AtomicUsize::new(self.stats.current_usage.load(Ordering::Relaxed)),
            allocation_failures: AtomicUsize::new(
                self.stats.allocation_failures.load(Ordering::Relaxed),
            ),
        }
    }

    /// Calculate pool hit rate as percentage
    pub fn hit_rate(&self) -> f64 {
        let hits = self.stats.pool_hits.load(Ordering::Relaxed) as f64;
        let total = self.stats.total_allocations.load(Ordering::Relaxed) as f64;

        if total > 0.0 {
            (hits / total) * 100.0
        } else {
            0.0
        }
    }

    /// Reset pool statistics
    pub fn reset_statistics(&self) {
        self.stats.total_allocations.store(0, Ordering::Relaxed);
        self.stats.pool_hits.store(0, Ordering::Relaxed);
        self.stats.pool_misses.store(0, Ordering::Relaxed);
        self.stats.allocation_failures.store(0, Ordering::Relaxed);
        // Don't reset peak_usage and current_usage as they reflect current state
    }
}

/// Errors that can occur during buffer pool operations
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum PoolError {
    #[error("No available buffers in any tier")]
    NoAvailableBuffers,

    #[error("Requested tier is exhausted")]
    TierExhausted,

    #[error("Memory allocation failed")]
    AllocationFailed,

    #[error("Invalid buffer size: {size} bytes")]
    InvalidSize { size: usize },

    #[error("Pool capacity exceeded")]
    CapacityExceeded,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_copy_buffer_creation() {
        let buffer = ZeroCopyVideoBuffer::new(1024, 1, 0).unwrap();
        assert_eq!(buffer.capacity(), 1024);
        assert_eq!(buffer.id(), 1);
        assert_eq!(buffer.data().len(), 0);
    }

    #[test]
    fn test_buffer_pool_creation() {
        let config = PoolConfig::default();
        let pool = VideoBufferPool::new(config).unwrap();

        // Test basic allocation
        let buffer = pool.acquire_buffer(1280 * 720 * 4).unwrap();
        assert!(buffer.capacity() >= 1280 * 720 * 4);

        // Test statistics
        let stats = pool.get_statistics();
        assert_eq!(stats.total_allocations.load(Ordering::Relaxed), 1);
        assert_eq!(stats.pool_hits.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_pool_hit_rate() {
        let config = PoolConfig::default();
        let pool = VideoBufferPool::new(config).unwrap();

        // Allocate several buffers
        for _ in 0..5 {
            let _buffer = pool.acquire_buffer(1280 * 720 * 4).unwrap();
        }

        let hit_rate = pool.hit_rate();
        assert!(hit_rate > 90.0); // Should have high hit rate for same-size allocations
    }

    #[test]
    fn test_buffer_reference_counting() {
        let buffer = Arc::new(ZeroCopyVideoBuffer::new(1024, 1, 0).unwrap());

        // Test reference counting
        buffer.add_ref();
        assert!(!buffer.release()); // Should not be ready for release
        assert!(buffer.release()); // Should be ready for release now
    }
}
