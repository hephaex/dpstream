pub mod advanced_networking;
pub mod audio;
pub mod capture;
pub mod compiler_optimization;
pub mod encoder;
pub mod error_recovery;
pub mod health_server;
pub mod lock_free;
pub mod memory_optimization;
pub mod moonlight;
pub mod optimization;
pub mod production_monitoring;
pub mod rtp_optimization;
pub mod simd_ops;
pub mod sunshine;
pub mod zero_copy;

pub use advanced_networking::{
    AdvancedNetworkingSystem, IoUringSystem, PacketProcessor, RdmaSystem,
};
pub use compiler_optimization::{
    BoltOptimizer, CompilerFlagOptimizer, CompilerOptimizationSystem, ProfileGuidedOptimizer,
};
pub use error_recovery::{CircuitBreaker, ErrorContext, ErrorRecoverySystem};
pub use health_server::HealthServer;
pub use lock_free::{LockFreeMemoryPool, LockFreeRingBuffer, LockFreeSessionRegistry};
pub use memory_optimization::{
    AudioBufferHandle, PacketHandle, StreamingAllocator, VideoFrameHandle,
};
pub use moonlight::{MoonlightServer, ServerConfig};
pub use production_monitoring::{ApplicationMetrics, HealthCheck, ProductionMonitoringSystem};
pub use rtp_optimization::{FastRtpPacket, RtpPacketBatch, SIMDRtpProcessor};
pub use simd_ops::{CPUCapabilities, SIMDVideoProcessor};
pub use zero_copy::{PoolConfig, VideoBufferPool, ZeroCopyVideoBuffer};
