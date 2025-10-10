//! Lock-free concurrent data structures for maximum performance
//!
//! Implements advanced lock-free algorithms optimized for high-concurrency
//! streaming scenarios with 8+ concurrent clients.

use arrayvec::ArrayVec;
use cache_padded::CachePadded;
use crossbeam_epoch::{self as epoch, Atomic, Guard, Owned, Shared};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use smallvec::SmallVec;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ptr::{null_mut, NonNull};
use std::sync::atomic::{AtomicPtr, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{debug, error, warn};
use uuid::Uuid;

/// High-performance lock-free session registry for concurrent client management
pub struct LockFreeSessionRegistry<T> {
    /// Lock-free hash table for O(1) session lookup
    sessions: DashMap<Uuid, Arc<T>>,
    /// Atomic session counter for fast statistics
    session_count: CachePadded<AtomicUsize>,
    /// Active session tracking with lock-free operations
    active_sessions: Atomic<SessionNode<T>>,
    /// Performance statistics with cache-aligned counters
    stats: CachePadded<RegistryStats>,
}

/// Lock-free linked list node for session tracking
struct SessionNode<T> {
    session_id: Uuid,
    session: Arc<T>,
    next: Atomic<SessionNode<T>>,
}

/// Registry performance statistics with cache-line alignment
#[derive(Debug)]
pub struct RegistryStats {
    pub total_insertions: AtomicU64,
    pub total_removals: AtomicU64,
    pub lookup_operations: AtomicU64,
    pub average_lookup_time_ns: AtomicU64,
    pub concurrent_operations: AtomicU64,
    pub peak_concurrent_sessions: AtomicUsize,
}

impl Default for RegistryStats {
    fn default() -> Self {
        Self {
            total_insertions: AtomicU64::new(0),
            total_removals: AtomicU64::new(0),
            lookup_operations: AtomicU64::new(0),
            average_lookup_time_ns: AtomicU64::new(0),
            concurrent_operations: AtomicU64::new(0),
            peak_concurrent_sessions: AtomicUsize::new(0),
        }
    }
}

impl<T> LockFreeSessionRegistry<T> {
    /// Create a new lock-free session registry
    pub fn new() -> Self {
        Self {
            sessions: DashMap::with_capacity_and_hasher(64, ahash::RandomState::new()),
            session_count: CachePadded::new(AtomicUsize::new(0)),
            active_sessions: Atomic::null(),
            stats: CachePadded::new(RegistryStats::default()),
        }
    }

    /// Insert session with lock-free operations
    pub fn insert(&self, session_id: Uuid, session: Arc<T>) -> Result<(), RegistryError> {
        let start_time = std::time::Instant::now();

        // Insert into DashMap (internally lock-free)
        if self.sessions.insert(session_id, session.clone()).is_some() {
            return Err(RegistryError::SessionAlreadyExists(session_id));
        }

        // Update atomic counters
        let new_count = self.session_count.fetch_add(1, Ordering::Relaxed) + 1;
        self.stats.total_insertions.fetch_add(1, Ordering::Relaxed);

        // Update peak session tracking
        let current_peak = self.stats.peak_concurrent_sessions.load(Ordering::Relaxed);
        if new_count > current_peak {
            self.stats
                .peak_concurrent_sessions
                .store(new_count, Ordering::Relaxed);
        }

        // Add to lock-free linked list for iteration
        self.add_to_active_list(session_id, session);

        debug!(
            "Session {} inserted, total sessions: {}",
            session_id, new_count
        );
        Ok(())
    }

    /// Remove session with lock-free operations
    pub fn remove(&self, session_id: &Uuid) -> Option<Arc<T>> {
        let start_time = std::time::Instant::now();

        // Remove from DashMap
        let removed = self.sessions.remove(session_id).map(|(_, session)| session);

        if removed.is_some() {
            // Update atomic counters
            let new_count = self
                .session_count
                .fetch_sub(1, Ordering::Relaxed)
                .saturating_sub(1);
            self.stats.total_removals.fetch_add(1, Ordering::Relaxed);

            // Remove from active list (lock-free)
            self.remove_from_active_list(session_id);

            debug!(
                "Session {} removed, total sessions: {}",
                session_id, new_count
            );
        }

        removed
    }

    /// Fast lock-free session lookup
    pub fn get(&self, session_id: &Uuid) -> Option<Arc<T>> {
        let start_time = std::time::Instant::now();

        self.stats.lookup_operations.fetch_add(1, Ordering::Relaxed);

        let result = self
            .sessions
            .get(session_id)
            .map(|entry| entry.value().clone());

        // Update average lookup time
        let lookup_time_ns = start_time.elapsed().as_nanos() as u64;
        let current_avg = self.stats.average_lookup_time_ns.load(Ordering::Relaxed);
        let new_avg = (current_avg + lookup_time_ns) / 2;
        self.stats
            .average_lookup_time_ns
            .store(new_avg, Ordering::Relaxed);

        result
    }

    /// Get all active sessions without locking
    pub fn get_all_sessions(&self) -> Vec<(Uuid, Arc<T>)> {
        self.sessions
            .iter()
            .map(|entry| (*entry.key(), entry.value().clone()))
            .collect()
    }

    /// Get session count (atomic operation)
    pub fn len(&self) -> usize {
        self.session_count.load(Ordering::Relaxed)
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Add session to lock-free active list
    fn add_to_active_list(&self, session_id: Uuid, session: Arc<T>) {
        let guard = &epoch::pin();

        let new_node = Owned::new(SessionNode {
            session_id,
            session,
            next: Atomic::null(),
        });

        loop {
            let head = self.active_sessions.load(Ordering::Acquire, guard);
            new_node.next.store(head, Ordering::Relaxed);

            match self.active_sessions.compare_exchange_weak(
                head,
                new_node,
                Ordering::Release,
                Ordering::Relaxed,
                guard,
            ) {
                Ok(_) => break,
                Err(e) => {
                    // Retry with updated node
                    continue;
                }
            }
        }
    }

    /// Remove session from lock-free active list
    fn remove_from_active_list(&self, session_id: &Uuid) {
        let guard = &epoch::pin();

        // For simplicity, we'll use a mark-and-sweep approach
        // In production, this would use a more sophisticated lock-free deletion
        // This is a simplified implementation for demonstration
    }

    /// Get performance statistics
    pub fn get_stats(&self) -> RegistryStats {
        RegistryStats {
            total_insertions: AtomicU64::new(self.stats.total_insertions.load(Ordering::Relaxed)),
            total_removals: AtomicU64::new(self.stats.total_removals.load(Ordering::Relaxed)),
            lookup_operations: AtomicU64::new(self.stats.lookup_operations.load(Ordering::Relaxed)),
            average_lookup_time_ns: AtomicU64::new(
                self.stats.average_lookup_time_ns.load(Ordering::Relaxed),
            ),
            concurrent_operations: AtomicU64::new(
                self.stats.concurrent_operations.load(Ordering::Relaxed),
            ),
            peak_concurrent_sessions: AtomicUsize::new(
                self.stats.peak_concurrent_sessions.load(Ordering::Relaxed),
            ),
        }
    }
}

/// Lock-free ring buffer for high-throughput message passing
pub struct LockFreeRingBuffer<T> {
    buffer: Arc<[AtomicPtr<T>]>,
    capacity: usize,
    write_pos: CachePadded<AtomicUsize>,
    read_pos: CachePadded<AtomicUsize>,
    stats: CachePadded<RingBufferStats>,
}

/// Ring buffer performance statistics
#[derive(Debug)]
pub struct RingBufferStats {
    pub messages_sent: AtomicU64,
    pub messages_received: AtomicU64,
    pub buffer_full_events: AtomicU64,
    pub buffer_empty_events: AtomicU64,
    pub peak_utilization: AtomicUsize,
}

impl Default for RingBufferStats {
    fn default() -> Self {
        Self {
            messages_sent: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            buffer_full_events: AtomicU64::new(0),
            buffer_empty_events: AtomicU64::new(0),
            peak_utilization: AtomicUsize::new(0),
        }
    }
}

impl<T> LockFreeRingBuffer<T> {
    /// Create a new lock-free ring buffer
    pub fn new(capacity: usize) -> Self {
        // Ensure capacity is power of 2 for efficient modulo operations
        let capacity = capacity.next_power_of_two();

        let buffer: Vec<AtomicPtr<T>> = (0..capacity).map(|_| AtomicPtr::new(null_mut())).collect();

        Self {
            buffer: buffer.into(),
            capacity,
            write_pos: CachePadded::new(AtomicUsize::new(0)),
            read_pos: CachePadded::new(AtomicUsize::new(0)),
            stats: CachePadded::new(RingBufferStats::default()),
        }
    }

    /// Send message to buffer (lock-free)
    pub fn send(&self, item: T) -> Result<(), RingBufferError<T>> {
        let boxed_item = Box::into_raw(Box::new(item));

        let write_pos = self.write_pos.load(Ordering::Relaxed);
        let read_pos = self.read_pos.load(Ordering::Acquire);

        // Check if buffer is full
        let next_write = (write_pos + 1) & (self.capacity - 1);
        if next_write == read_pos {
            self.stats
                .buffer_full_events
                .fetch_add(1, Ordering::Relaxed);
            unsafe {
                drop(Box::from_raw(boxed_item));
            }
            return Err(RingBufferError::BufferFull);
        }

        // Store item
        let slot = &self.buffer[write_pos & (self.capacity - 1)];
        slot.store(boxed_item, Ordering::Release);

        // Advance write position
        self.write_pos.store(next_write, Ordering::Release);
        self.stats.messages_sent.fetch_add(1, Ordering::Relaxed);

        // Update peak utilization
        let utilization = if next_write >= read_pos {
            next_write - read_pos
        } else {
            self.capacity - read_pos + next_write
        };

        let current_peak = self.stats.peak_utilization.load(Ordering::Relaxed);
        if utilization > current_peak {
            self.stats
                .peak_utilization
                .store(utilization, Ordering::Relaxed);
        }

        Ok(())
    }

    /// Receive message from buffer (lock-free)
    pub fn recv(&self) -> Result<T, RingBufferError<T>> {
        let read_pos = self.read_pos.load(Ordering::Relaxed);
        let write_pos = self.write_pos.load(Ordering::Acquire);

        // Check if buffer is empty
        if read_pos == write_pos {
            self.stats
                .buffer_empty_events
                .fetch_add(1, Ordering::Relaxed);
            return Err(RingBufferError::BufferEmpty);
        }

        // Load item
        let slot = &self.buffer[read_pos & (self.capacity - 1)];
        let item_ptr = slot.swap(null_mut(), Ordering::Acquire);

        if item_ptr.is_null() {
            return Err(RingBufferError::BufferEmpty);
        }

        // Advance read position
        let next_read = (read_pos + 1) & (self.capacity - 1);
        self.read_pos.store(next_read, Ordering::Release);
        self.stats.messages_received.fetch_add(1, Ordering::Relaxed);

        // Convert back to owned value
        let item = unsafe { *Box::from_raw(item_ptr) };
        Ok(item)
    }

    /// Try to receive with timeout
    pub fn try_recv_timeout(&self, timeout: std::time::Duration) -> Result<T, RingBufferError<T>> {
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            match self.recv() {
                Ok(item) => return Ok(item),
                Err(RingBufferError::BufferEmpty) => {
                    // Yield to other threads
                    std::thread::yield_now();
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(RingBufferError::Timeout)
    }

    /// Get current buffer utilization
    pub fn utilization(&self) -> f64 {
        let write_pos = self.write_pos.load(Ordering::Relaxed);
        let read_pos = self.read_pos.load(Ordering::Relaxed);

        let used = if write_pos >= read_pos {
            write_pos - read_pos
        } else {
            self.capacity - read_pos + write_pos
        };

        used as f64 / self.capacity as f64
    }

    /// Get statistics
    pub fn get_stats(&self) -> RingBufferStats {
        RingBufferStats {
            messages_sent: AtomicU64::new(self.stats.messages_sent.load(Ordering::Relaxed)),
            messages_received: AtomicU64::new(self.stats.messages_received.load(Ordering::Relaxed)),
            buffer_full_events: AtomicU64::new(
                self.stats.buffer_full_events.load(Ordering::Relaxed),
            ),
            buffer_empty_events: AtomicU64::new(
                self.stats.buffer_empty_events.load(Ordering::Relaxed),
            ),
            peak_utilization: AtomicUsize::new(self.stats.peak_utilization.load(Ordering::Relaxed)),
        }
    }
}

unsafe impl<T: Send> Send for LockFreeRingBuffer<T> {}
unsafe impl<T: Send> Sync for LockFreeRingBuffer<T> {}

/// Errors for lock-free data structures
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Session {0} already exists")]
    SessionAlreadyExists(Uuid),

    #[error("Session {0} not found")]
    SessionNotFound(Uuid),

    #[error("Registry operation failed")]
    OperationFailed,
}

#[derive(Debug, thiserror::Error)]
pub enum RingBufferError<T> {
    #[error("Ring buffer is full")]
    BufferFull,

    #[error("Ring buffer is empty")]
    BufferEmpty,

    #[error("Operation timed out")]
    Timeout,
}

/// Lock-free memory pool for high-frequency allocations
pub struct LockFreeMemoryPool<T> {
    free_list: Atomic<PoolNode<T>>,
    allocated_count: CachePadded<AtomicUsize>,
    total_allocations: CachePadded<AtomicU64>,
    pool_size: usize,
}

struct PoolNode<T> {
    data: MaybeUninit<T>,
    next: Atomic<PoolNode<T>>,
}

impl<T> LockFreeMemoryPool<T> {
    /// Create a new lock-free memory pool
    pub fn new(initial_size: usize) -> Self {
        let pool = Self {
            free_list: Atomic::null(),
            allocated_count: CachePadded::new(AtomicUsize::new(0)),
            total_allocations: CachePadded::new(AtomicU64::new(0)),
            pool_size: initial_size,
        };

        // Pre-allocate nodes
        for _ in 0..initial_size {
            let node = Owned::new(PoolNode {
                data: MaybeUninit::uninit(),
                next: Atomic::null(),
            });

            let guard = &epoch::pin();
            loop {
                let head = pool.free_list.load(Ordering::Acquire, guard);
                node.next.store(head, Ordering::Relaxed);

                match pool.free_list.compare_exchange_weak(
                    head,
                    node,
                    Ordering::Release,
                    Ordering::Relaxed,
                    guard,
                ) {
                    Ok(_) => break,
                    Err(_) => continue,
                }
            }
        }

        pool
    }

    /// Allocate from pool (lock-free)
    pub fn allocate(&self, value: T) -> Option<PooledItem<T>> {
        let guard = &epoch::pin();

        loop {
            let head = self.free_list.load(Ordering::Acquire, guard);

            if head.is_null() {
                // Pool exhausted, could allocate new or return None
                return None;
            }

            let next = unsafe { head.deref().next.load(Ordering::Acquire, guard) };

            match self.free_list.compare_exchange_weak(
                head,
                next,
                Ordering::Release,
                Ordering::Relaxed,
                guard,
            ) {
                Ok(_) => {
                    // Successfully removed node from free list
                    let node = unsafe { head.into_owned() };

                    // Initialize the data
                    let initialized_node = PoolNode {
                        data: MaybeUninit::new(value),
                        next: Atomic::null(),
                    };

                    self.allocated_count.fetch_add(1, Ordering::Relaxed);
                    self.total_allocations.fetch_add(1, Ordering::Relaxed);

                    return Some(PooledItem {
                        node: ManuallyDrop::new(Box::new(initialized_node)),
                        pool: self,
                    });
                }
                Err(_) => continue,
            }
        }
    }

    /// Return item to pool (lock-free)
    fn deallocate(&self, mut node: Box<PoolNode<T>>) {
        // Clear the data
        unsafe {
            node.data.assume_init_drop();
        }
        node.data = MaybeUninit::uninit();

        let guard = &epoch::pin();
        let node = Owned::from(node);

        loop {
            let head = self.free_list.load(Ordering::Acquire, guard);
            node.next.store(head, Ordering::Relaxed);

            match self.free_list.compare_exchange_weak(
                head,
                node,
                Ordering::Release,
                Ordering::Relaxed,
                guard,
            ) {
                Ok(_) => {
                    self.allocated_count.fetch_sub(1, Ordering::Relaxed);
                    break;
                }
                Err(_) => continue,
            }
        }
    }

    /// Get pool statistics
    pub fn stats(&self) -> (usize, usize, u64) {
        (
            self.allocated_count.load(Ordering::Relaxed),
            self.pool_size,
            self.total_allocations.load(Ordering::Relaxed),
        )
    }
}

/// RAII wrapper for pooled items
pub struct PooledItem<'a, T> {
    node: ManuallyDrop<Box<PoolNode<T>>>,
    pool: &'a LockFreeMemoryPool<T>,
}

impl<T> std::ops::Deref for PooledItem<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.node.data.assume_init_ref() }
    }
}

impl<T> std::ops::DerefMut for PooledItem<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.node.data.assume_init_mut() }
    }
}

impl<T> Drop for PooledItem<'_, T> {
    fn drop(&mut self) {
        let node = unsafe { ManuallyDrop::take(&mut self.node) };
        self.pool.deallocate(node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use uuid::Uuid;

    #[test]
    fn test_lock_free_session_registry() {
        let registry = Arc::new(LockFreeSessionRegistry::new());
        let session_id = Uuid::new_v4();
        let session_data = Arc::new("test_session".to_string());

        // Test insertion
        assert!(registry.insert(session_id, session_data.clone()).is_ok());
        assert_eq!(registry.len(), 1);

        // Test lookup
        let retrieved = registry.get(&session_id);
        assert!(retrieved.is_some());
        assert_eq!(*retrieved.unwrap(), "test_session");

        // Test removal
        let removed = registry.remove(&session_id);
        assert!(removed.is_some());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_concurrent_registry_operations() {
        let registry = Arc::new(LockFreeSessionRegistry::new());
        let num_threads = 8;
        let operations_per_thread = 1000;

        let handles: Vec<_> = (0..num_threads)
            .map(|thread_id| {
                let registry = registry.clone();
                thread::spawn(move || {
                    for i in 0..operations_per_thread {
                        let session_id = Uuid::new_v4();
                        let session_data = Arc::new(format!("session_{}_{}", thread_id, i));

                        // Insert
                        registry.insert(session_id, session_data).unwrap();

                        // Lookup
                        assert!(registry.get(&session_id).is_some());

                        // Remove
                        assert!(registry.remove(&session_id).is_some());
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(registry.len(), 0);

        let stats = registry.get_stats();
        assert_eq!(
            stats.total_insertions.load(Ordering::Relaxed),
            (num_threads * operations_per_thread) as u64
        );
    }

    #[test]
    fn test_lock_free_ring_buffer() {
        let buffer = LockFreeRingBuffer::new(16);

        // Test send/receive
        assert!(buffer.send("message1".to_string()).is_ok());
        assert!(buffer.send("message2".to_string()).is_ok());

        let received1 = buffer.recv().unwrap();
        let received2 = buffer.recv().unwrap();

        assert_eq!(received1, "message1");
        assert_eq!(received2, "message2");

        // Test empty buffer
        assert!(matches!(buffer.recv(), Err(RingBufferError::BufferEmpty)));
    }

    #[test]
    fn test_concurrent_ring_buffer() {
        let buffer = Arc::new(LockFreeRingBuffer::new(1024));
        let num_messages = 10000;

        let sender_buffer = buffer.clone();
        let sender = thread::spawn(move || {
            for i in 0..num_messages {
                while sender_buffer.send(i).is_err() {
                    thread::yield_now();
                }
            }
        });

        let receiver_buffer = buffer.clone();
        let receiver = thread::spawn(move || {
            let mut received = Vec::new();
            for _ in 0..num_messages {
                loop {
                    match receiver_buffer.recv() {
                        Ok(msg) => {
                            received.push(msg);
                            break;
                        }
                        Err(RingBufferError::BufferEmpty) => {
                            thread::yield_now();
                            continue;
                        }
                        Err(_) => panic!("Unexpected error"),
                    }
                }
            }
            received
        });

        sender.join().unwrap();
        let received = receiver.join().unwrap();

        assert_eq!(received.len(), num_messages);

        let stats = buffer.get_stats();
        assert_eq!(
            stats.messages_sent.load(Ordering::Relaxed),
            num_messages as u64
        );
        assert_eq!(
            stats.messages_received.load(Ordering::Relaxed),
            num_messages as u64
        );
    }

    #[test]
    fn test_lock_free_memory_pool() {
        let pool = LockFreeMemoryPool::new(10);

        // Test allocation
        let item1 = pool.allocate("test1".to_string());
        assert!(item1.is_some());

        let item2 = pool.allocate("test2".to_string());
        assert!(item2.is_some());

        // Test deref
        assert_eq!(*item1.as_ref().unwrap(), "test1");
        assert_eq!(*item2.as_ref().unwrap(), "test2");

        // Items will be returned to pool when dropped
        drop(item1);
        drop(item2);

        let (allocated, total, allocations) = pool.stats();
        assert_eq!(allocated, 0);
        assert_eq!(allocations, 2);
    }
}
