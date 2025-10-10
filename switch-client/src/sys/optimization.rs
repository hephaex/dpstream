//! Performance optimization utilities for Nintendo Switch
//!
//! Provides memory management and performance optimization patterns for embedded gaming

#![no_std]

use crate::error::{MemoryError, Result};
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use cache_padded::CachePadded;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use heapless::Vec as HeaplessVec;
use once_cell::race::OnceNonZeroUsize;
use spin::{Mutex, RwLock};
use tinyvec::TinyVec;

/// Cache-optimized lock-free ring buffer for high-performance frame streaming
#[repr(align(64))] // Cache line alignment
pub struct LockFreeRingBuffer<T> {
    buffer: Vec<T>,
    capacity: usize,
    // Cache-pad atomic variables to prevent false sharing
    read_pos: CachePadded<AtomicUsize>,
    write_pos: CachePadded<AtomicUsize>,
    size: CachePadded<AtomicUsize>,
}

impl<T: Default + Clone> LockFreeRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        let mut buffer = Vec::with_capacity(capacity);
        buffer.resize(capacity, T::default());

        Self {
            buffer,
            capacity,
            read_pos: CachePadded::new(AtomicUsize::new(0)),
            write_pos: CachePadded::new(AtomicUsize::new(0)),
            size: CachePadded::new(AtomicUsize::new(0)),
        }
    }

    pub fn push(&self, item: T) -> bool {
        let current_size = self.size.load(Ordering::Relaxed);
        if current_size >= self.capacity {
            return false; // Buffer full
        }

        let write_pos = self.write_pos.load(Ordering::Relaxed);

        // Safety: We know the buffer has this capacity and we checked bounds
        unsafe {
            let ptr = self.buffer.as_ptr().add(write_pos) as *mut T;
            ptr.write(item);
        }

        let next_pos = (write_pos + 1) % self.capacity;
        self.write_pos.store(next_pos, Ordering::Release);
        self.size.fetch_add(1, Ordering::Relaxed);

        true
    }

    pub fn pop(&self) -> Option<T> {
        let current_size = self.size.load(Ordering::Relaxed);
        if current_size == 0 {
            return None;
        }

        let read_pos = self.read_pos.load(Ordering::Relaxed);

        // Safety: We know there's at least one item
        let item = unsafe {
            let ptr = self.buffer.as_ptr().add(read_pos);
            ptr.read()
        };

        let next_pos = (read_pos + 1) % self.capacity;
        self.read_pos.store(next_pos, Ordering::Release);
        self.size.fetch_sub(1, Ordering::Relaxed);

        Some(item)
    }

    pub fn len(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_full(&self) -> bool {
        self.len() >= self.capacity
    }
}

/// Switch-optimized memory pool for video frame buffers
/// Uses stack-allocated pools and cache-aligned allocations
pub struct OptimizedFramePool {
    buffers: RwLock<TinyVec<[NonNull<u8>; 8]>>, // Stack-allocated for small pools
    buffer_size: usize,
    total_buffers: usize,
    allocated_count: CachePadded<AtomicUsize>,
    stats: RwLock<PoolStats>,
    // Pre-allocated static pools to reduce dynamic allocation
    small_pool: CachePadded<RwLock<HeaplessVec<NonNull<u8>, 4>>>, // 720p frames
    large_pool: CachePadded<RwLock<HeaplessVec<NonNull<u8>, 2>>>, // 1080p frames
}

/// Cache-optimized performance metrics
#[derive(Debug, Clone)]
pub struct SwitchPerformanceMetrics {
    pub frame_decode_time_us: u32,
    pub memory_pressure: u8,       // 0-100 percentage
    pub cpu_temperature: u8,       // Temperature in Celsius
    pub gpu_frequency_mhz: u16,    // Current GPU frequency
    pub memory_frequency_mhz: u16, // Current memory frequency
    pub power_consumption_mw: u16, // Power consumption in milliwatts
}

static PERFORMANCE_CACHE: OnceNonZeroUsize = OnceNonZeroUsize::new();

#[derive(Debug, Default)]
pub struct PoolStats {
    pub total_allocations: u64,
    pub current_allocations: usize,
    pub peak_allocations: usize,
    pub allocation_failures: u64,
    pub pool_hits: u64,
    pub pool_misses: u64,
}

impl OptimizedFramePool {
    pub fn new(buffer_size: usize, total_buffers: usize) -> Result<Self> {
        let mut buffers = VecDeque::with_capacity(total_buffers);

        // Pre-allocate all buffers
        for _ in 0..total_buffers {
            let layout = core::alloc::Layout::from_size_align(buffer_size, 8)
                .map_err(|_| MemoryError::AllocationFailed)?;

            let ptr = unsafe { alloc::alloc::alloc(layout) };
            if ptr.is_null() {
                return Err(MemoryError::AllocationFailed.into());
            }

            buffers.push_back(NonNull::new(ptr).unwrap());
        }

        Ok(Self {
            buffers: RwLock::new(buffers),
            buffer_size,
            total_buffers,
            allocated_count: AtomicUsize::new(0),
            stats: RwLock::new(PoolStats::default()),
        })
    }

    pub fn acquire(&self) -> Option<NonNull<u8>> {
        let mut buffers = self.buffers.write();

        let mut stats = self.stats.write();
        stats.total_allocations += 1;

        if let Some(buffer) = buffers.pop_front() {
            let current = self.allocated_count.fetch_add(1, Ordering::Relaxed) + 1;
            stats.current_allocations = current;
            if current > stats.peak_allocations {
                stats.peak_allocations = current;
            }
            stats.pool_hits += 1;

            Some(buffer)
        } else {
            stats.allocation_failures += 1;
            None
        }
    }

    pub fn release(&self, buffer: NonNull<u8>) {
        let mut buffers = self.buffers.write();
        buffers.push_back(buffer);

        self.allocated_count.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn stats(&self) -> PoolStats {
        let stats = self.stats.read();
        *stats
    }

    pub fn available_buffers(&self) -> usize {
        self.buffers.read().len()
    }
}

impl Drop for OptimizedFramePool {
    fn drop(&mut self) {
        let layout = core::alloc::Layout::from_size_align(self.buffer_size, 8).unwrap();
        let mut buffers = self.buffers.write();

        while let Some(buffer) = buffers.pop_front() {
            unsafe {
                alloc::alloc::dealloc(buffer.as_ptr(), layout);
            }
        }
    }
}

/// Adaptive quality controller for limited resources
pub struct AdaptiveQualityController {
    current_quality: AtomicUsize, // 0-100
    memory_pressure_threshold: f32,
    cpu_usage_threshold: f32,
    frame_drop_count: AtomicUsize,
    last_adjustment: AtomicUsize, // Timestamp approximation
}

impl AdaptiveQualityController {
    pub fn new() -> Self {
        Self {
            current_quality: AtomicUsize::new(75), // Start at 75% quality
            memory_pressure_threshold: 80.0,
            cpu_usage_threshold: 85.0,
            frame_drop_count: AtomicUsize::new(0),
            last_adjustment: AtomicUsize::new(0),
        }
    }

    pub fn update_metrics(
        &self,
        memory_usage_percent: f32,
        cpu_usage_percent: f32,
        timestamp: usize,
    ) {
        let last_adjustment = self.last_adjustment.load(Ordering::Relaxed);

        // Only adjust every ~2 seconds (assuming timestamp is in ms)
        if timestamp.saturating_sub(last_adjustment) < 2000 {
            return;
        }

        let current_quality = self.current_quality.load(Ordering::Relaxed);
        let mut new_quality = current_quality;

        // Decrease quality if under pressure
        if memory_usage_percent > self.memory_pressure_threshold
            || cpu_usage_percent > self.cpu_usage_threshold
        {
            new_quality = new_quality.saturating_sub(10);
        }
        // Increase quality if resources are available
        else if memory_usage_percent < 60.0 && cpu_usage_percent < 60.0 {
            new_quality = (new_quality + 5).min(100);
        }

        // Factor in frame drops
        let frame_drops = self.frame_drop_count.load(Ordering::Relaxed);
        if frame_drops > 10 {
            new_quality = new_quality.saturating_sub(15);
            self.frame_drop_count.store(0, Ordering::Relaxed);
        }

        if new_quality != current_quality {
            self.current_quality.store(new_quality, Ordering::Relaxed);
            self.last_adjustment.store(timestamp, Ordering::Relaxed);
        }
    }

    pub fn get_quality(&self) -> u8 {
        self.current_quality.load(Ordering::Relaxed) as u8
    }

    pub fn record_frame_drop(&self) {
        self.frame_drop_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn should_drop_frame(&self, frame_priority: u8) -> bool {
        let quality = self.get_quality();

        // Drop lower priority frames based on quality setting
        match quality {
            0..=25 => frame_priority < 7,  // Only keep critical frames
            26..=50 => frame_priority < 5, // Keep high and critical
            51..=75 => frame_priority < 3, // Keep normal, high, and critical
            _ => false,                    // Keep all frames
        }
    }
}

/// SIMD-optimized operations for ARM NEON (Tegra X1)
pub struct SIMDOptimizations;

impl SIMDOptimizations {
    /// Fast memory copy using ARM NEON when available
    #[inline(always)]
    pub fn fast_memcpy(dst: *mut u8, src: *const u8, len: usize) {
        // On real hardware, this would use ARM NEON SIMD instructions
        // For simulation, use regular copy
        unsafe {
            core::ptr::copy_nonoverlapping(src, dst, len);
        }
    }

    /// Optimized YUV to RGB conversion using NEON
    #[inline(always)]
    pub fn yuv_to_rgb_neon(yuv_data: &[u8], rgb_data: &mut [u8], width: usize, height: usize) {
        // In real implementation, this would use ARM NEON intrinsics
        // for vectorized YUV->RGB conversion

        let pixels = width * height;
        for i in 0..pixels {
            let y = yuv_data[i] as i32;
            let u = yuv_data[pixels + i / 4] as i32 - 128;
            let v = yuv_data[pixels + pixels / 4 + i / 4] as i32 - 128;

            let r = (y + (1.370705 * v as f32) as i32).clamp(0, 255);
            let g = (y - (0.698001 * v as f32) as i32 - (0.337633 * u as f32) as i32).clamp(0, 255);
            let b = (y + (1.732446 * u as f32) as i32).clamp(0, 255);

            let rgb_idx = i * 3;
            if rgb_idx + 2 < rgb_data.len() {
                rgb_data[rgb_idx] = r as u8;
                rgb_data[rgb_idx + 1] = g as u8;
                rgb_data[rgb_idx + 2] = b as u8;
            }
        }
    }

    /// Vectorized alpha blending for UI elements
    #[inline(always)]
    pub fn alpha_blend_neon(src: &[u8], dst: &mut [u8], alpha: u8) {
        // NEON-optimized alpha blending would go here
        let alpha_f = alpha as f32 / 255.0;
        let inv_alpha_f = 1.0 - alpha_f;

        for i in 0..dst.len().min(src.len()) {
            dst[i] = ((src[i] as f32 * alpha_f) + (dst[i] as f32 * inv_alpha_f)) as u8;
        }
    }
}

/// Cache-optimized data structures for better memory access patterns
pub struct CacheOptimizedQueue<T> {
    // Align to cache line boundaries
    #[repr(align(64))]
    data: HeaplessVec<T, 32>,
    head: usize,
    tail: usize,
}

impl<T: Default + Clone> CacheOptimizedQueue<T> {
    pub fn new() -> Self {
        Self {
            data: HeaplessVec::new(),
            head: 0,
            tail: 0,
        }
    }

    pub fn push(&mut self, item: T) -> Result<(), T> {
        self.data.push(item)
    }

    pub fn pop(&mut self) -> Option<T> {
        if !self.is_empty() {
            let item = self.data[self.head].clone();
            self.head = (self.head + 1) % self.data.capacity();
            Some(item)
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.head == self.tail && self.data.is_empty()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_lockfree_ring_buffer() {
        let buffer = LockFreeRingBuffer::new(4);

        assert!(buffer.push(1));
        assert!(buffer.push(2));
        assert!(buffer.push(3));
        assert!(buffer.push(4));
        assert!(!buffer.push(5)); // Should fail - buffer full

        assert_eq!(buffer.pop(), Some(1));
        assert_eq!(buffer.pop(), Some(2));
        assert!(buffer.push(5)); // Should succeed now

        assert_eq!(buffer.len(), 3);
    }

    #[test]
    fn test_adaptive_quality_controller() {
        let controller = AdaptiveQualityController::new();

        // Simulate high memory pressure
        controller.update_metrics(90.0, 50.0, 1000);
        assert!(controller.get_quality() < 75);

        // Simulate frame drops
        for _ in 0..15 {
            controller.record_frame_drop();
        }
        controller.update_metrics(70.0, 50.0, 3000);
        assert!(controller.get_quality() < 60);
    }

    #[test]
    fn test_optimized_frame_pool() {
        let pool = OptimizedFramePool::new(1024, 4).unwrap();

        let buf1 = pool.acquire().unwrap();
        let buf2 = pool.acquire().unwrap();

        assert_eq!(pool.available_buffers(), 2);

        pool.release(buf1);
        assert_eq!(pool.available_buffers(), 3);

        let stats = pool.stats();
        assert_eq!(stats.total_allocations, 2);
        assert_eq!(stats.pool_hits, 2);
    }
}
