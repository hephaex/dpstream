//! Dolphin emulator input adapter
//!
//! Handles communication with Dolphin emulator for input injection

use crate::error::{EmulatorError, InputError, Result};
use crate::input::processor::{AnalogStick, DolphinButton, DolphinCommand};
use serde_json;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Adapter for sending input commands to Dolphin emulator
pub struct DolphinInputAdapter {
    connected_controllers: HashMap<u8, ControllerConnection>,
    command_sender: Option<mpsc::UnboundedSender<String>>,
    dolphin_process: Option<tokio::process::Child>,
    is_active: bool,
    connection_health: ConnectionHealth,
    last_command_time: std::time::Instant,
}

/// Connection health monitoring
#[derive(Debug, Clone)]
struct ConnectionHealth {
    commands_sent: u64,
    commands_failed: u64,
    last_health_check: std::time::Instant,
    is_healthy: bool,
}

impl DolphinInputAdapter {
    /// Create a new Dolphin input adapter
    pub fn new() -> Result<Self> {
        info!("Initializing Dolphin input adapter");

        Ok(Self {
            connected_controllers: HashMap::new(),
            command_sender: None,
            dolphin_process: None,
            is_active: false,
            connection_health: ConnectionHealth {
                commands_sent: 0,
                commands_failed: 0,
                last_health_check: std::time::Instant::now(),
                is_healthy: true,
            },
            last_command_time: std::time::Instant::now(),
        })
    }

    /// Initialize connection to Dolphin
    pub async fn initialize(&mut self, dolphin_path: &str) -> Result<()> {
        info!("Connecting to Dolphin emulator at: {}", dolphin_path);

        // Start Dolphin with pipe input enabled
        let mut dolphin = tokio::process::Command::new(dolphin_path)
            .arg("--interface=Pipe")
            .arg("--input-socket")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| EmulatorError::StartupFailed {
                reason: format!("Failed to start Dolphin with pipe input: {}", e),
            })?;

        // Set up command channel
        let (sender, mut receiver) = mpsc::unbounded_channel::<String>();

        // Get stdin handle for sending commands
        let mut stdin = dolphin
            .stdin
            .take()
            .ok_or_else(|| EmulatorError::StartupFailed {
                reason: "Failed to get Dolphin stdin".to_string(),
            })?;

        // Spawn task to handle command sending
        tokio::spawn(async move {
            while let Some(command) = receiver.recv().await {
                if let Err(e) = stdin.write_all(command.as_bytes()).await {
                    error!("Failed to send command to Dolphin: {}", e);
                    break;
                }
                if let Err(e) = stdin.write_all(b"\n").await {
                    error!("Failed to send newline to Dolphin: {}", e);
                    break;
                }
                if let Err(e) = stdin.flush().await {
                    error!("Failed to flush Dolphin stdin: {}", e);
                    break;
                }
            }
        });

        self.command_sender = Some(sender);
        self.dolphin_process = Some(dolphin);
        self.is_active = true;

        info!("Successfully connected to Dolphin emulator");
        Ok(())
    }

    /// Connect a controller to a specific player slot
    pub fn connect_controller(&mut self, player_slot: u8) -> Result<()> {
        if player_slot == 0 || player_slot > 4 {
            return Err(InputError::InvalidPlayer {
                player: player_slot,
            }
            .into());
        }

        let connection = ControllerConnection {
            player_slot,
            is_connected: true,
            controller_type: DolphinControllerType::Standard,
        };

        self.connected_controllers.insert(player_slot, connection);

        // Send connection command to Dolphin
        let command = format!("SET CONTROLLER {} STANDARD", player_slot);
        self.send_command(command)?;

        info!("Connected controller for player {}", player_slot);
        Ok(())
    }

    /// Disconnect controller from player slot
    pub fn disconnect_controller(&mut self, player_slot: u8) -> Result<()> {
        if let Some(_) = self.connected_controllers.remove(&player_slot) {
            let command = format!("SET CONTROLLER {} NONE", player_slot);
            self.send_command(command)?;
            info!("Disconnected controller for player {}", player_slot);
        }
        Ok(())
    }

    /// Send multiple commands to Dolphin
    pub async fn send_commands(&mut self, commands: Vec<DolphinCommand>) -> Result<()> {
        if !self.is_active {
            return Err(InputError::AdapterNotInitialized.into());
        }

        for command in commands {
            self.send_dolphin_command(command).await?;
        }

        Ok(())
    }

    /// Send a single command to Dolphin
    async fn send_dolphin_command(&mut self, command: DolphinCommand) -> Result<()> {
        let dolphin_cmd = match command {
            DolphinCommand::ButtonPress {
                player,
                button,
                pressed,
            } => {
                if !self.connected_controllers.contains_key(&player) {
                    self.connect_controller(player)?;
                }
                self.format_button_command(player, button, pressed)
            }
            DolphinCommand::AnalogInput {
                player,
                stick,
                x,
                y,
            } => self.format_analog_command(player, stick, x, y),
            DolphinCommand::TriggerInput {
                player,
                left_trigger,
                right_trigger,
            } => self.format_trigger_command(player, left_trigger, right_trigger),
            DolphinCommand::DPadInput {
                player,
                up,
                down,
                left,
                right,
            } => self.format_dpad_command(player, up, down, left, right),
            DolphinCommand::WiiPointerInput { player, x, y, z } => {
                self.format_pointer_command(player, x, y, z)
            }
        };

        self.send_command(dolphin_cmd)?;
        Ok(())
    }

    fn format_button_command(&self, player: u8, button: DolphinButton, pressed: bool) -> String {
        let button_name = match button {
            DolphinButton::A => "A",
            DolphinButton::B => "B",
            DolphinButton::X => "X",
            DolphinButton::Y => "Y",
            DolphinButton::Z => "Z",
            DolphinButton::L => "L",
            DolphinButton::R => "R",
            DolphinButton::Start => "START",
            DolphinButton::Up => "UP",
            DolphinButton::Down => "DOWN",
            DolphinButton::Left => "LEFT",
            DolphinButton::Right => "RIGHT",
        };

        let state = if pressed { "PRESS" } else { "RELEASE" };
        format!("BUTTON {} {} {}", player, button_name, state)
    }

    fn format_analog_command(&self, player: u8, stick: AnalogStick, x: f32, y: f32) -> String {
        let stick_name = match stick {
            AnalogStick::Main => "MAIN",
            AnalogStick::CStick => "C",
        };

        // Convert -1.0 to 1.0 range to 0-255 range for Dolphin
        let x_val = ((x + 1.0) * 127.5) as u8;
        let y_val = ((y + 1.0) * 127.5) as u8;

        format!("ANALOG {} {} {} {}", player, stick_name, x_val, y_val)
    }

    fn format_trigger_command(&self, player: u8, left: f32, right: f32) -> String {
        // Convert 0.0-1.0 to 0-255 for Dolphin
        let left_val = (left * 255.0) as u8;
        let right_val = (right * 255.0) as u8;

        format!("TRIGGER {} {} {}", player, left_val, right_val)
    }

    fn format_dpad_command(
        &self,
        player: u8,
        up: bool,
        down: bool,
        left: bool,
        right: bool,
    ) -> String {
        let mut dpad_val = 0u8;
        if up {
            dpad_val |= 1;
        }
        if down {
            dpad_val |= 2;
        }
        if left {
            dpad_val |= 4;
        }
        if right {
            dpad_val |= 8;
        }

        format!("DPAD {} {}", player, dpad_val)
    }

    fn format_pointer_command(&self, player: u8, x: f32, y: f32, z: f32) -> String {
        // Convert -1.0 to 1.0 coordinates to screen coordinates
        let screen_x = ((x + 1.0) * 512.0) as u16; // Assuming 1024x768 screen
        let screen_y = ((y + 1.0) * 384.0) as u16;

        format!(
            "POINTER {} {} {} {}",
            player,
            screen_x,
            screen_y,
            (z * 100.0) as u8
        )
    }

    fn send_command(&mut self, command: String) -> Result<()> {
        if let Some(sender) = &self.command_sender {
            debug!("Sending command to Dolphin: {}", command);

            match sender.send(command) {
                Ok(_) => {
                    self.connection_health.commands_sent += 1;
                    self.last_command_time = std::time::Instant::now();

                    // Mark as healthy if we successfully send commands
                    if !self.connection_health.is_healthy {
                        info!("Dolphin connection restored");
                        self.connection_health.is_healthy = true;
                    }
                }
                Err(e) => {
                    self.connection_health.commands_failed += 1;
                    self.connection_health.is_healthy = false;

                    return Err(InputError::CommandSendFailed {
                        reason: e.to_string(),
                    }
                    .into());
                }
            }
        } else {
            return Err(InputError::AdapterNotInitialized.into());
        }
        Ok(())
    }

    /// Check connection health and attempt recovery if needed
    pub fn check_health(&mut self) -> bool {
        let now = std::time::Instant::now();

        // Check if too much time has passed since last command
        if now.duration_since(self.last_command_time).as_secs() > 5 {
            // Send a heartbeat command to verify connection
            if let Err(_) = self.send_command("HEARTBEAT".to_string()) {
                warn!("Dolphin connection health check failed");
                self.connection_health.is_healthy = false;
            }
        }

        self.connection_health.is_healthy
    }

    /// Get adapter status
    pub fn get_status(&self) -> AdapterStatus {
        AdapterStatus {
            is_active: self.is_active,
            connected_controllers: self.connected_controllers.len(),
            has_dolphin_process: self.dolphin_process.is_some(),
        }
    }

    /// Shutdown the adapter
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down Dolphin input adapter");

        self.is_active = false;

        // Disconnect all controllers
        let player_slots: Vec<u8> = self.connected_controllers.keys().cloned().collect();
        for slot in player_slots {
            self.disconnect_controller(slot)?;
        }

        // Close command sender
        self.command_sender = None;

        // Terminate Dolphin process if we started it
        if let Some(mut process) = self.dolphin_process.take() {
            if let Err(e) = process.kill().await {
                warn!("Failed to kill Dolphin process: {}", e);
            }
        }

        info!("Dolphin input adapter shutdown complete");
        Ok(())
    }
}

/// Controller connection information
#[derive(Debug, Clone)]
struct ControllerConnection {
    player_slot: u8,
    is_connected: bool,
    controller_type: DolphinControllerType,
}

/// Types of controllers Dolphin supports
#[derive(Debug, Clone, Copy)]
enum DolphinControllerType {
    None,
    Standard,          // Standard GameCube controller
    WaveBird,          // Wireless GameCube controller
    DKBongos,          // DK Bongos
    WiiRemote,         // Wii Remote
    WiiRemoteNunchuk,  // Wii Remote + Nunchuk
    ClassicController, // Wii Classic Controller
}

/// Adapter status information
#[derive(Debug, Clone)]
pub struct AdapterStatus {
    pub is_active: bool,
    pub connected_controllers: usize,
    pub has_dolphin_process: bool,
}

impl Drop for DolphinInputAdapter {
    fn drop(&mut self) {
        if self.is_active {
            debug!("Dolphin input adapter dropped while active");
            // Note: Cannot use async in drop, but Dolphin process will be terminated
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = DolphinInputAdapter::new();
        assert!(adapter.is_ok());

        let adapter = adapter.unwrap();
        assert!(!adapter.is_active);
        assert!(adapter.connected_controllers.is_empty());
    }

    #[test]
    fn test_button_command_formatting() {
        let adapter = DolphinInputAdapter::new().unwrap();

        let cmd = adapter.format_button_command(1, DolphinButton::A, true);
        assert_eq!(cmd, "BUTTON 1 A PRESS");

        let cmd = adapter.format_button_command(2, DolphinButton::Start, false);
        assert_eq!(cmd, "BUTTON 2 START RELEASE");
    }

    #[test]
    fn test_analog_command_formatting() {
        let adapter = DolphinInputAdapter::new().unwrap();

        let cmd = adapter.format_analog_command(1, AnalogStick::Main, 0.0, 0.0);
        assert_eq!(cmd, "ANALOG 1 MAIN 127 127"); // Center position

        let cmd = adapter.format_analog_command(1, AnalogStick::CStick, 1.0, -1.0);
        assert_eq!(cmd, "ANALOG 1 C 255 0"); // Full right, full down
    }

    #[test]
    fn test_trigger_command_formatting() {
        let adapter = DolphinInputAdapter::new().unwrap();

        let cmd = adapter.format_trigger_command(1, 0.5, 1.0);
        assert_eq!(cmd, "TRIGGER 1 127 255"); // Half left, full right
    }
}
