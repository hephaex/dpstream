// Core modules that work with minimal dependencies
// pub mod audio;                 // Commented out: AudioFrame field mismatches
// pub mod capture;               // Commented out: VideoFrame field mismatches
// pub mod encoder;               // Commented out: depends on capture
pub mod error_recovery;
pub mod health_server;
pub mod moonlight;
// pub mod optimization;          // Commented out: depends on other modules
// pub mod rtp_optimization;      // Commented out: unsafe function call errors
// pub mod simd_ops;              // Commented out: borrow checker errors
pub mod sunshine;
pub mod zero_copy;

// Modules commented out due to missing optional dependencies
// These require additional crates not currently in Cargo.toml
// Uncomment and add dependencies when needed for full feature builds

// pub mod advanced_networking;      // Requires io_uring, RDMA dependencies
// pub mod compiler_optimization;    // Requires quantum_optimization module
// pub mod lock_free;                // Requires crossbeam_epoch
// pub mod memory_optimization;      // Requires slab
// pub mod production_monitoring;    // Requires axum, prometheus, opentelemetry
// pub mod quantum_optimization;     // Quantum computing dependencies

pub use health_server::HealthServer;
pub use moonlight::{MoonlightServer, ServerConfig};
// pub use rtp_optimization::{FastRtpPacket, RtpPacketBatch, SIMDRtpProcessor};
// pub use simd_ops::{CPUCapabilities, SIMDVideoProcessor};

// Commented out exports for disabled modules
// pub use advanced_networking::{AdvancedNetworkingSystem, IoUringSystem, PacketProcessor, RdmaSystem};
// pub use compiler_optimization::{BoltOptimizer, CompilerFlagOptimizer, CompilerOptimizationSystem, ProfileGuidedOptimizer};
// pub use lock_free::{LockFreeMemoryPool, LockFreeRingBuffer, LockFreeSessionRegistry};
// pub use memory_optimization::{AudioBufferHandle, PacketHandle, StreamingAllocator, VideoFrameHandle};
// pub use production_monitoring::{ApplicationMetrics, HealthCheck, ProductionMonitoringSystem};
