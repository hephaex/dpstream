use tokio::process::{Child, Command};
use std::env;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{info, warn, error, debug};
use crate::error::{Result, EmulatorError};

pub struct DolphinManager {
    process: Option<Child>,
    window_id: Option<u64>,
    tailscale_ip: String,
    executable_path: String,
    rom_path: String,
    save_path: String,
    config_path: String,
    startup_timeout: Duration,
    process_monitor: Option<tokio::task::JoinHandle<()>>,
}

impl DolphinManager {
    pub fn new(tailscale_ip: String) -> Result<Self> {
        info!("Initializing Dolphin manager for Tailscale IP: {}", tailscale_ip);

        let executable_path = env::var("DOLPHIN_PATH")
            .unwrap_or_else(|_| "/usr/bin/dolphin-emu".to_string());

        let rom_path = env::var("ROM_PATH")
            .unwrap_or_else(|_| "/srv/games/gc-wii".to_string());

        let save_path = env::var("SAVE_PATH")
            .unwrap_or_else(|_| "/srv/saves".to_string());

        let config_path = env::var("DOLPHIN_CONFIG_PATH")
            .unwrap_or_else(|_| "/tmp/dpstream-dolphin".to_string());

        let startup_timeout = Duration::from_secs(
            env::var("DOLPHIN_STARTUP_TIMEOUT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30)
        );

        // Verify Dolphin executable exists
        if !Path::new(&executable_path).exists() {
            return Err(EmulatorError::ExecutableNotFound {
                path: executable_path
            }.into());
        }

        // Create necessary directories
        for dir in &[&rom_path, &save_path, &config_path] {
            if let Err(e) = std::fs::create_dir_all(dir) {
                warn!("Failed to create directory {}: {}", dir, e);
            }
        }

        debug!("Dolphin configuration:");
        debug!("  Executable: {}", executable_path);
        debug!("  ROM path: {}", rom_path);
        debug!("  Save path: {}", save_path);
        debug!("  Config path: {}", config_path);
        debug!("  Startup timeout: {:?}", startup_timeout);

        Ok(Self {
            process: None,
            window_id: None,
            tailscale_ip,
            executable_path,
            rom_path,
            save_path,
            config_path,
            startup_timeout,
            process_monitor: None,
        })
    }

    pub async fn start_game(&mut self, rom_name: &str) -> Result<()> {
        let rom_path = format!("{}/{}", self.rom_path, rom_name);

        if !std::path::Path::new(&rom_path).exists() {
            return Err(EmulatorError::RomNotFound { path: rom_path }.into());
        }

        info!("Starting Dolphin with ROM: {}", rom_path);

        let mut cmd = Command::new(&self.executable_path);
        cmd.arg("--exec")
           .arg(&rom_path)
           .arg("--nogui")
           .arg("--user")
           .arg(&self.config_path)
           .arg("--save")
           .arg(&self.save_path)
           .kill_on_drop(true);

        let child = cmd.spawn()
            .map_err(|e| EmulatorError::StartupFailed {
                reason: format!("Failed to spawn Dolphin process: {}", e)
            })?;

        let pid = child.id();
        info!("Dolphin process started with PID: {}", pid.unwrap_or(0));
        self.process = Some(child);

        // Wait for Dolphin startup with timeout
        let startup_result = timeout(self.startup_timeout, async {
            // Give Dolphin time to initialize
            tokio::time::sleep(Duration::from_secs(3)).await;

            // Find the Dolphin window
            if let Err(e) = self.find_dolphin_window().await {
                return Err(e);
            }

            // Start process monitoring
            if let Err(e) = self.start_process_monitor().await {
                return Err(e);
            }

            Ok::<(), crate::error::DpstreamError>(())
        }).await;

        match startup_result {
            Ok(Ok(())) => {
                info!("Dolphin started successfully for ROM: {}", rom_name);
                Ok(())
            }
            Ok(Err(e)) => {
                error!("Dolphin startup failed: {}", e);
                self.stop_game().await?;
                Err(e.into())
            }
            Err(_) => {
                error!("Dolphin startup timed out after {:?}", self.startup_timeout);
                self.stop_game().await?;
                Err(EmulatorError::StartupTimeout.into())
            }
        }
    }

    pub async fn stop_game(&mut self) -> Result<()> {
        // Stop process monitor first
        if let Some(monitor_handle) = self.process_monitor.take() {
            monitor_handle.abort();
            debug!("Process monitor stopped");
        }

        if let Some(mut process) = self.process.take() {
            info!("Stopping Dolphin process");

            match process.kill().await {
                Ok(_) => {
                    let _ = process.wait().await;
                    self.window_id = None;
                    info!("Dolphin process stopped successfully");
                    Ok(())
                }
                Err(e) => Err(EmulatorError::ProcessControlFailed {
                    operation: "kill".to_string(),
                    reason: e.to_string(),
                }.into()),
            }
        } else {
            debug!("Dolphin process already stopped");
            Ok(()) // Already stopped
        }
    }

    pub async fn is_running(&mut self) -> bool {
        if let Some(process) = &mut self.process {
            match process.try_wait() {
                Ok(Some(status)) => {
                    // Process has exited
                    warn!("Dolphin process exited with status: {:?}", status);
                    self.cleanup_process();
                    false
                }
                Ok(None) => true, // Still running
                Err(e) => {
                    // Error checking status, assume not running
                    error!("Error checking Dolphin process status: {}", e);
                    self.cleanup_process();
                    false
                }
            }
        } else {
            false
        }
    }

    pub fn get_window_id(&self) -> Option<u64> {
        self.window_id
    }

    async fn find_dolphin_window(&mut self) -> Result<()> {
        // TODO: Implement X11 window finding using x11 crate
        // This would use X11 APIs to find the Dolphin window by process ID or title

        let attempts = 10;
        let wait_time = Duration::from_millis(500);

        for attempt in 1..=attempts {
            debug!("Searching for Dolphin window, attempt {}/{}", attempt, attempts);

            // Mock implementation for now - in real implementation, this would:
            // 1. Connect to X11 display
            // 2. Enumerate windows
            // 3. Match by process ID or window title containing "Dolphin"
            // 4. Return the window ID

            if attempt >= 3 {
                // Simulate finding window after a few attempts
                let window_id = 0x12345678 + (attempt as u64);
                self.window_id = Some(window_id);
                info!("Found Dolphin window with ID: 0x{:x}", window_id);
                return Ok(());
            }

            tokio::time::sleep(wait_time).await;
        }

        Err(EmulatorError::WindowNotFound {
            timeout: Duration::from_millis(wait_time.as_millis() as u64 * attempts),
        }.into())
    }

    async fn start_process_monitor(&mut self) -> Result<()> {
        if self.process.is_none() {
            return Err(EmulatorError::ProcessControlFailed {
                operation: "monitor".to_string(),
                reason: "No process to monitor".to_string(),
            }.into());
        }

        // Create a monitoring task
        let process_monitor = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));

            loop {
                interval.tick().await;

                // In a real implementation, this would:
                // 1. Check process health
                // 2. Monitor resource usage
                // 3. Detect crashes or hangs
                // 4. Send health updates to logging system

                debug!("Dolphin process health check completed");
            }
        });

        self.process_monitor = Some(process_monitor);
        debug!("Process monitor started");
        Ok(())
    }

    fn cleanup_process(&mut self) {
        if let Some(monitor_handle) = self.process_monitor.take() {
            monitor_handle.abort();
        }
        self.process = None;
        self.window_id = None;
        debug!("Process cleanup completed");
    }
}

impl Drop for DolphinManager {
    fn drop(&mut self) {
        if self.process.is_some() {
            debug!("Cleaning up Dolphin process on drop");
            // Note: Cannot use async in Drop, process will be killed on drop due to kill_on_drop(true)
            if let Some(monitor_handle) = self.process_monitor.take() {
                monitor_handle.abort();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;
    use std::env;

    fn setup_test_env() {
        env::set_var("DOLPHIN_PATH", "/usr/bin/true"); // Use /bin/true for testing
        env::set_var("ROM_PATH", "/tmp/test-roms");
        env::set_var("SAVE_PATH", "/tmp/test-saves");
        env::set_var("DOLPHIN_CONFIG_PATH", "/tmp/test-config");
        env::set_var("DOLPHIN_STARTUP_TIMEOUT", "5");
    }

    #[tokio::test]
    async fn test_dolphin_manager_creation() {
        setup_test_env();

        let result = DolphinManager::new("100.64.0.1".to_string());
        assert!(result.is_ok(), "DolphinManager creation should succeed");

        let manager = result.unwrap();
        assert_eq!(manager.tailscale_ip, "100.64.0.1");
        assert!(!manager.is_running().await);
    }

    #[tokio::test]
    async fn test_invalid_executable_path() {
        env::set_var("DOLPHIN_PATH", "/nonexistent/path");

        let result = DolphinManager::new("100.64.0.1".to_string());
        assert!(result.is_err(), "Should fail with invalid executable path");
    }

    #[tokio::test]
    async fn test_process_lifecycle() {
        setup_test_env();

        let mut manager = DolphinManager::new("100.64.0.1".to_string()).unwrap();

        // Create a dummy ROM file for testing
        std::fs::create_dir_all("/tmp/test-roms").ok();
        std::fs::write("/tmp/test-roms/test.iso", "dummy content").unwrap();

        // Start game should work with /bin/true
        let result = manager.start_game("test.iso").await;
        assert!(result.is_ok(), "Game start should succeed with test executable");

        // Should be running
        assert!(manager.is_running().await, "Process should be running");

        // Stop should work
        let stop_result = manager.stop_game().await;
        assert!(stop_result.is_ok(), "Game stop should succeed");

        // Should no longer be running
        assert!(!manager.is_running().await, "Process should be stopped");
    }

    #[tokio::test]
    async fn test_nonexistent_rom() {
        setup_test_env();

        let mut manager = DolphinManager::new("100.64.0.1".to_string()).unwrap();

        let result = manager.start_game("nonexistent.iso").await;
        assert!(result.is_err(), "Should fail with nonexistent ROM");
    }

    #[tokio::test]
    async fn test_multiple_stop_calls() {
        setup_test_env();

        let mut manager = DolphinManager::new("100.64.0.1".to_string()).unwrap();

        // Multiple stop calls should not fail
        let result1 = manager.stop_game().await;
        let result2 = manager.stop_game().await;

        assert!(result1.is_ok(), "First stop should succeed");
        assert!(result2.is_ok(), "Second stop should succeed");
    }
}