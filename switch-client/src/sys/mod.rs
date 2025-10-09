//! System interface module for Nintendo Switch
//!
//! Provides abstractions over libnx and Switch system services

pub mod libnx;
pub mod memory;
pub mod time;
pub mod optimization;

pub use libnx::LibnxSystem;