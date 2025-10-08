//! Time utilities for Switch
//!
//! Provides timing functions without std

use crate::error::{Result, SystemError};

/// Get current timestamp in milliseconds
pub fn get_time_ms() -> Result<u64> {
    // In real implementation: svcGetSystemTick() and convert to milliseconds
    Ok(0) // Mock implementation
}

/// Get current timestamp in microseconds
pub fn get_time_us() -> Result<u64> {
    // In real implementation: high-resolution timer
    Ok(0) // Mock implementation
}

/// Sleep for specified milliseconds
pub async fn sleep_ms(ms: u64) -> Result<()> {
    // In real implementation: svcSleepThread()
    Ok(())
}

/// Simple timer for measuring elapsed time
pub struct Timer {
    start_time: u64,
}

impl Timer {
    /// Create and start a new timer
    pub fn new() -> Result<Self> {
        Ok(Self {
            start_time: get_time_us()?,
        })
    }

    /// Get elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> Result<u64> {
        let current = get_time_us()?;
        Ok((current - self.start_time) / 1000)
    }

    /// Get elapsed time in microseconds
    pub fn elapsed_us(&self) -> Result<u64> {
        let current = get_time_us()?;
        Ok(current - self.start_time)
    }

    /// Reset the timer
    pub fn reset(&mut self) -> Result<()> {
        self.start_time = get_time_us()?;
        Ok(())
    }
}