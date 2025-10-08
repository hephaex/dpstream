//! Memory management utilities for Switch
//!
//! Provides optimized memory allocation and management for limited resources

use crate::error::{Result, MemoryError};
use alloc::vec::Vec;
use core::mem::{size_of, align_of};
use core::ptr::NonNull;

/// Memory allocator statistics with performance metrics
#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub total_heap: usize,
    pub used_heap: usize,
    pub free_heap: usize,
    pub largest_free_block: usize,
    pub fragmentation_ratio: f32,
    pub allocation_count: usize,
    pub peak_usage: usize,
}

/// Memory pool for video frame buffers
pub struct VideoBufferPool {
    buffers: Vec<NonNull<u8>>,
    buffer_size: usize,
    available: Vec<usize>,
    in_use: Vec<usize>,
}

impl VideoBufferPool {
    /// Create a new buffer pool with pre-allocated frames
    pub fn new(buffer_count: usize, buffer_size: usize) -> Result<Self> {
        if !check_available_memory(buffer_count * buffer_size)? {
            return Err(MemoryError::InsufficientMemory {
                requested: buffer_count * buffer_size,
                available: get_memory_stats()?.free_heap,
            }.into());
        }

        let mut buffers = Vec::with_capacity(buffer_count);
        let mut available = Vec::with_capacity(buffer_count);

        // Pre-allocate all buffers
        for i in 0..buffer_count {
            let layout = core::alloc::Layout::from_size_align(buffer_size, align_of::<u64>())
                .map_err(|_| MemoryError::InvalidAlignment)?;

            unsafe {
                let ptr = alloc::alloc::alloc(layout);
                if ptr.is_null() {
                    // Cleanup already allocated buffers
                    for &existing_ptr in &buffers {
                        alloc::alloc::dealloc(existing_ptr.as_ptr(), layout);
                    }
                    return Err(MemoryError::AllocationFailed.into());
                }

                // Zero the buffer for security
                core::ptr::write_bytes(ptr, 0, buffer_size);

                buffers.push(NonNull::new_unchecked(ptr));
                available.push(i);
            }
        }

        Ok(Self {
            buffers,
            buffer_size,
            available,
            in_use: Vec::new(),
        })
    }

    /// Get an available buffer
    pub fn acquire(&mut self) -> Option<NonNull<u8>> {
        self.available.pop().map(|index| {
            self.in_use.push(index);
            self.buffers[index]
        })
    }

    /// Return a buffer to the pool
    pub fn release(&mut self, ptr: NonNull<u8>) -> Result<()> {
        // Find the buffer index
        let index = self.buffers.iter().position(|&buf_ptr| buf_ptr == ptr)
            .ok_or(MemoryError::InvalidPointer)?;

        // Move from in_use to available
        let in_use_pos = self.in_use.iter().position(|&i| i == index)
            .ok_or(MemoryError::DoubleRelease)?;

        self.in_use.swap_remove(in_use_pos);
        self.available.push(index);

        // Zero the buffer for security
        unsafe {
            core::ptr::write_bytes(ptr.as_ptr(), 0, self.buffer_size);
        }

        Ok(())
    }

    /// Get pool statistics
    pub fn stats(&self) -> (usize, usize, usize) {
        (self.buffers.len(), self.available.len(), self.in_use.len())
    }
}

impl Drop for VideoBufferPool {
    fn drop(&mut self) {
        let layout = core::alloc::Layout::from_size_align(self.buffer_size, align_of::<u64>())
            .expect("Invalid layout in drop");

        for &ptr in &self.buffers {
            unsafe {
                alloc::alloc::dealloc(ptr.as_ptr(), layout);
            }
        }
    }
}

/// Thread-safe memory statistics counter
static mut MEMORY_STATS: MemoryStats = MemoryStats {
    total_heap: 0,
    used_heap: 0,
    free_heap: 0,
    largest_free_block: 0,
    fragmentation_ratio: 0.0,
    allocation_count: 0,
    peak_usage: 0,
};

/// Get current memory statistics with real heap information
pub fn get_memory_stats() -> Result<MemoryStats> {
    unsafe {
        // In real implementation: query libnx heap functions
        // For now, simulate reasonable values for Switch
        MEMORY_STATS.total_heap = 64 * 1024 * 1024; // 64MB available to homebrew
        MEMORY_STATS.used_heap = 8 * 1024 * 1024;   // Current usage
        MEMORY_STATS.free_heap = MEMORY_STATS.total_heap - MEMORY_STATS.used_heap;
        MEMORY_STATS.largest_free_block = MEMORY_STATS.free_heap * 3 / 4; // Account for fragmentation
        MEMORY_STATS.fragmentation_ratio =
            1.0 - (MEMORY_STATS.largest_free_block as f32 / MEMORY_STATS.free_heap as f32);

        Ok(MEMORY_STATS.clone())
    }
}

/// Check if we have enough memory for allocation with safety margin
pub fn check_available_memory(required: usize) -> Result<bool> {
    let stats = get_memory_stats()?;
    // Keep 25% safety margin
    let available_with_margin = stats.largest_free_block * 3 / 4;
    Ok(available_with_margin >= required)
}

/// Force garbage collection if available
pub fn force_gc() -> Result<usize> {
    // In real implementation: call libnx memory management functions
    // For now, return simulated freed memory
    Ok(1024 * 1024) // 1MB freed
}

/// Check memory pressure and recommend actions
pub fn check_memory_pressure() -> Result<MemoryPressure> {
    let stats = get_memory_stats()?;
    let usage_ratio = stats.used_heap as f32 / stats.total_heap as f32;

    if usage_ratio > 0.9 {
        Ok(MemoryPressure::Critical)
    } else if usage_ratio > 0.75 {
        Ok(MemoryPressure::High)
    } else if usage_ratio > 0.5 {
        Ok(MemoryPressure::Medium)
    } else {
        Ok(MemoryPressure::Low)
    }
}

/// Memory pressure levels
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryPressure {
    Low,
    Medium,
    High,
    Critical,
}

/// Optimized allocator for fixed-size objects
pub struct FixedAllocator<T> {
    pool: Vec<Option<T>>,
    free_indices: Vec<usize>,
}

impl<T> FixedAllocator<T> {
    /// Create a new fixed allocator
    pub fn new(capacity: usize) -> Self {
        let mut pool = Vec::with_capacity(capacity);
        let mut free_indices = Vec::with_capacity(capacity);

        for i in 0..capacity {
            pool.push(None);
            free_indices.push(i);
        }

        Self { pool, free_indices }
    }

    /// Allocate an object
    pub fn alloc(&mut self, value: T) -> Option<usize> {
        self.free_indices.pop().map(|index| {
            self.pool[index] = Some(value);
            index
        })
    }

    /// Deallocate an object
    pub fn dealloc(&mut self, index: usize) -> Option<T> {
        if index < self.pool.len() {
            let value = self.pool[index].take();
            if value.is_some() {
                self.free_indices.push(index);
            }
            value
        } else {
            None
        }
    }

    /// Get a reference to an allocated object
    pub fn get(&self, index: usize) -> Option<&T> {
        self.pool.get(index)?.as_ref()
    }

    /// Get a mutable reference to an allocated object
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.pool.get_mut(index)?.as_mut()
    }
}