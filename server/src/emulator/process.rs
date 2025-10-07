use std::process::{Child, Command, Stdio};
use std::env;
use anyhow::{Result, anyhow};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DolphinError {
    #[error("Dolphin executable not found: {0}")]
    NotFound(String),
    #[error("Failed to start Dolphin: {0}")]
    StartFailed(String),
    #[error("Dolphin process error: {0}")]
    ProcessError(String),
}

pub struct DolphinManager {
    process: Option<Child>,
    window_id: Option<u64>,
    tailscale_ip: String,
    executable_path: String,
    rom_path: String,
    save_path: String,
}

impl DolphinManager {
    pub fn new(tailscale_ip: String) -> Result<Self, DolphinError> {
        let executable_path = env::var("DOLPHIN_PATH")
            .unwrap_or_else(|_| "/usr/bin/dolphin-emu".to_string());

        let rom_path = env::var("ROM_PATH")
            .unwrap_or_else(|_| "/srv/games/gc-wii".to_string());

        let save_path = env::var("SAVE_PATH")
            .unwrap_or_else(|_| "/srv/saves".to_string());

        // Verify Dolphin executable exists
        if !std::path::Path::new(&executable_path).exists() {
            return Err(DolphinError::NotFound(executable_path));
        }

        Ok(Self {
            process: None,
            window_id: None,
            tailscale_ip,
            executable_path,
            rom_path,
            save_path,
        })
    }

    pub async fn start_game(&mut self, rom_name: &str) -> Result<(), DolphinError> {
        let rom_path = format!("{}/{}", self.rom_path, rom_name);

        if !std::path::Path::new(&rom_path).exists() {
            return Err(DolphinError::ProcessError(format!("ROM not found: {}", rom_path)));
        }

        tracing::info!("Starting Dolphin with ROM: {}", rom_path);

        let mut cmd = Command::new(&self.executable_path);
        cmd.arg("--exec")
           .arg(&rom_path)
           .arg("--nogui")
           .arg("--user")
           .arg(&self.save_path)
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        match cmd.spawn() {
            Ok(child) => {
                tracing::info!("Dolphin process started with PID: {}", child.id());
                self.process = Some(child);

                // Give Dolphin time to start and create window
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

                // Find the Dolphin window
                self.find_dolphin_window()?;

                Ok(())
            }
            Err(e) => Err(DolphinError::StartFailed(e.to_string())),
        }
    }

    pub fn stop_game(&mut self) -> Result<(), DolphinError> {
        if let Some(mut process) = self.process.take() {
            tracing::info!("Stopping Dolphin process");

            match process.kill() {
                Ok(_) => {
                    process.wait().ok();
                    self.window_id = None;
                    tracing::info!("Dolphin process stopped");
                    Ok(())
                }
                Err(e) => Err(DolphinError::ProcessError(e.to_string())),
            }
        } else {
            Ok(()) // Already stopped
        }
    }

    pub fn is_running(&mut self) -> bool {
        if let Some(process) = &mut self.process {
            match process.try_wait() {
                Ok(Some(_)) => {
                    // Process has exited
                    self.process = None;
                    self.window_id = None;
                    false
                }
                Ok(None) => true, // Still running
                Err(_) => {
                    // Error checking status, assume not running
                    self.process = None;
                    self.window_id = None;
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

    fn find_dolphin_window(&mut self) -> Result<(), DolphinError> {
        // TODO: Implement X11 window finding
        // This would use X11 APIs to find the Dolphin window by process ID or title

        // Mock implementation for now
        self.window_id = Some(0x12345678);
        tracing::info!("Found Dolphin window with ID: {:x}", self.window_id.unwrap());

        Ok(())
    }
}

impl Drop for DolphinManager {
    fn drop(&mut self) {
        if self.process.is_some() {
            let _ = self.stop_game();
        }
    }
}