#![allow(dead_code)]

//! Input processing and transformation module
//!
//! Converts Moonlight input packets to Dolphin-compatible commands

use crate::error::Result;
use crate::input::mapping::ControllerMapping;
use crate::input::{MoonlightInputPacket, TouchPoint};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// Input processor that handles transformation and buffering
#[allow(dead_code)]
pub struct InputProcessor {
    command_buffer: VecDeque<DolphinCommand>,
    stats: ProcessorStats,
    last_process_time: Instant,
    deadzone_threshold: f32,
}

impl InputProcessor {
    /// Create a new input processor
    pub fn new() -> Result<Self> {
        Ok(Self {
            command_buffer: VecDeque::with_capacity(1000),
            stats: ProcessorStats::default(),
            last_process_time: Instant::now(),
            deadzone_threshold: 0.1, // 10% deadzone
        })
    }

    /// Create a new input processor with custom buffer size for performance tuning
    pub fn with_capacity(capacity: usize) -> Result<Self> {
        Ok(Self {
            command_buffer: VecDeque::with_capacity(capacity),
            stats: ProcessorStats::default(),
            last_process_time: Instant::now(),
            deadzone_threshold: 0.1,
        })
    }

    /// Process a single input packet
    pub async fn process_input(
        &mut self,
        player_slot: u8,
        mapping: ControllerMapping,
        input_packet: MoonlightInputPacket,
    ) -> Result<()> {
        let start_time = Instant::now();

        // Update statistics
        self.stats.packets_processed += 1;

        // Convert Moonlight input to Dolphin commands
        let commands = self.convert_to_dolphin_commands(player_slot, mapping, input_packet)?;

        // Buffer commands for batch processing
        for command in commands {
            if self.command_buffer.len() >= self.command_buffer.capacity() {
                // Drop oldest command if buffer is full
                self.command_buffer.pop_front();
                self.stats.commands_dropped += 1;
                warn!("Input command buffer overflow, dropping oldest command");
            }
            self.command_buffer.push_back(command);
        }

        // Update processing time statistics
        let processing_time = start_time.elapsed();
        self.stats.total_processing_time += processing_time;
        self.stats.average_processing_time =
            self.stats.total_processing_time / self.stats.packets_processed as u32;

        Ok(())
    }

    /// Get buffered commands for Dolphin
    pub async fn get_dolphin_commands(&mut self) -> Result<Option<Vec<DolphinCommand>>> {
        if self.command_buffer.is_empty() {
            return Ok(None);
        }

        // Drain all buffered commands
        let commands: Vec<DolphinCommand> = self.command_buffer.drain(..).collect();
        self.stats.commands_sent += commands.len() as u64;

        debug!("Sending {} commands to Dolphin", commands.len());
        Ok(Some(commands))
    }

    /// Get buffered commands with batch size limit for performance control
    pub async fn get_dolphin_commands_batched(
        &mut self,
        batch_size: usize,
    ) -> Result<Option<Vec<DolphinCommand>>> {
        if self.command_buffer.is_empty() {
            return Ok(None);
        }

        // Drain up to batch_size commands
        let mut commands = Vec::with_capacity(batch_size.min(self.command_buffer.len()));
        let actual_batch_size = batch_size.min(self.command_buffer.len());

        for _ in 0..actual_batch_size {
            if let Some(command) = self.command_buffer.pop_front() {
                commands.push(command);
            }
        }

        self.stats.commands_sent += commands.len() as u64;

        debug!("Sending {} commands to Dolphin (batched)", commands.len());
        Ok(Some(commands))
    }

    /// Get processing statistics
    pub fn get_stats(&self) -> ProcessorStats {
        self.stats.clone()
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = ProcessorStats::default();
    }

    fn convert_to_dolphin_commands(
        &self,
        player_slot: u8,
        mapping: ControllerMapping,
        input: MoonlightInputPacket,
    ) -> Result<Vec<DolphinCommand>> {
        let mut commands = Vec::new();

        // Convert button inputs
        commands.extend(self.convert_buttons(player_slot, &mapping, input.button_flags)?);

        // Convert analog inputs
        commands.extend(self.convert_analog_inputs(player_slot, &mapping, &input)?);

        // Convert triggers
        commands.extend(self.convert_triggers(player_slot, &mapping, &input)?);

        // Handle special Switch features
        if let (Some(gx), Some(gy), Some(gz)) = (input.gyro_x, input.gyro_y, input.gyro_z) {
            commands.extend(self.convert_gyro_input(player_slot, &mapping, gx, gy, gz)?);
        }

        if let Some(touch_points) = input.touch_points {
            commands.extend(self.convert_touch_input(player_slot, &mapping, touch_points)?);
        }

        Ok(commands)
    }

    fn convert_buttons(
        &self,
        player_slot: u8,
        mapping: &ControllerMapping,
        button_flags: u16,
    ) -> Result<Vec<DolphinCommand>> {
        let mut commands = Vec::new();

        // Moonlight button flags to GameCube buttons mapping
        let button_mappings = [
            (0x1000, &mapping.a_button),     // A
            (0x2000, &mapping.b_button),     // B
            (0x4000, &mapping.x_button),     // X
            (0x8000, &mapping.y_button),     // Y
            (0x0100, &mapping.l_button),     // L shoulder
            (0x0200, &mapping.r_button),     // R shoulder
            (0x0020, &mapping.z_button),     // Back (mapped to Z)
            (0x0010, &mapping.start_button), // Start
        ];

        for (flag, dolphin_button) in button_mappings {
            let is_pressed = (button_flags & flag) != 0;
            commands.push(DolphinCommand::ButtonPress {
                player: player_slot,
                button: *dolphin_button,
                pressed: is_pressed,
            });
        }

        // D-Pad mapping
        let dpad_up = (button_flags & 0x0001) != 0;
        let dpad_down = (button_flags & 0x0002) != 0;
        let dpad_left = (button_flags & 0x0004) != 0;
        let dpad_right = (button_flags & 0x0008) != 0;

        commands.push(DolphinCommand::DPadInput {
            player: player_slot,
            up: dpad_up,
            down: dpad_down,
            left: dpad_left,
            right: dpad_right,
        });

        Ok(commands)
    }

    fn convert_analog_inputs(
        &self,
        player_slot: u8,
        _mapping: &ControllerMapping,
        input: &MoonlightInputPacket,
    ) -> Result<Vec<DolphinCommand>> {
        let mut commands = Vec::new();

        // Convert left stick (GameCube main analog stick)
        let left_x = self.apply_deadzone(input.left_stick_x as f32 / 32767.0);
        let left_y = self.apply_deadzone(input.left_stick_y as f32 / 32767.0);

        commands.push(DolphinCommand::AnalogInput {
            player: player_slot,
            stick: AnalogStick::Main,
            x: left_x,
            y: left_y,
        });

        // Convert right stick (GameCube C-stick)
        let right_x = self.apply_deadzone(input.right_stick_x as f32 / 32767.0);
        let right_y = self.apply_deadzone(input.right_stick_y as f32 / 32767.0);

        commands.push(DolphinCommand::AnalogInput {
            player: player_slot,
            stick: AnalogStick::CStick,
            x: right_x,
            y: right_y,
        });

        Ok(commands)
    }

    fn convert_triggers(
        &self,
        player_slot: u8,
        _mapping: &ControllerMapping,
        input: &MoonlightInputPacket,
    ) -> Result<Vec<DolphinCommand>> {
        let mut commands = Vec::new();

        // GameCube triggers are analog (0.0 to 1.0)
        let left_trigger = input.left_trigger as f32 / 255.0;
        let right_trigger = input.right_trigger as f32 / 255.0;

        commands.push(DolphinCommand::TriggerInput {
            player: player_slot,
            left_trigger,
            right_trigger,
        });

        Ok(commands)
    }

    fn convert_gyro_input(
        &self,
        player_slot: u8,
        mapping: &ControllerMapping,
        gyro_x: f32,
        gyro_y: f32,
        _gyro_z: f32,
    ) -> Result<Vec<DolphinCommand>> {
        let mut commands = Vec::new();

        // For Wii games, gyro can be mapped to Wii Remote pointer
        if mapping.enable_gyro_pointer {
            // Convert gyro angular velocity to pointer position
            // This is a simplified conversion - real implementation would need calibration
            let pointer_x = (gyro_y * 0.1).clamp(-1.0, 1.0);
            let pointer_y = (gyro_x * 0.1).clamp(-1.0, 1.0);

            commands.push(DolphinCommand::WiiPointerInput {
                player: player_slot,
                x: pointer_x,
                y: pointer_y,
                z: 0.0, // Distance from screen
            });
        }

        Ok(commands)
    }

    fn convert_touch_input(
        &self,
        player_slot: u8,
        mapping: &ControllerMapping,
        touch_points: Vec<TouchPoint>,
    ) -> Result<Vec<DolphinCommand>> {
        let mut commands = Vec::new();

        if mapping.enable_touch_pointer && !touch_points.is_empty() {
            // Use first touch point for pointer control
            let touch = &touch_points[0];

            // Convert touch coordinates to Wii pointer coordinates
            // Switch touch screen is 1280x720, normalize to -1.0 to 1.0
            let pointer_x = (touch.x as f32 / 1280.0) * 2.0 - 1.0;
            let pointer_y = (touch.y as f32 / 720.0) * 2.0 - 1.0;

            commands.push(DolphinCommand::WiiPointerInput {
                player: player_slot,
                x: pointer_x,
                y: pointer_y,
                z: 0.0,
            });

            // Touch pressure can be mapped to A button press
            if touch.pressure > 128 {
                commands.push(DolphinCommand::ButtonPress {
                    player: player_slot,
                    button: DolphinButton::A,
                    pressed: true,
                });
            }
        }

        Ok(commands)
    }

    fn apply_deadzone(&self, value: f32) -> f32 {
        let abs_value = value.abs();
        if abs_value < self.deadzone_threshold {
            0.0
        } else {
            // Scale the remaining range to 0.0-1.0
            let sign = value.signum();
            let scaled = (abs_value - self.deadzone_threshold) / (1.0 - self.deadzone_threshold);
            sign * scaled.min(1.0)
        }
    }
}

/// Commands that can be sent to Dolphin emulator
#[derive(Debug, Clone)]
pub enum DolphinCommand {
    ButtonPress {
        player: u8,
        button: DolphinButton,
        pressed: bool,
    },
    AnalogInput {
        player: u8,
        stick: AnalogStick,
        x: f32,
        y: f32,
    },
    TriggerInput {
        player: u8,
        left_trigger: f32,
        right_trigger: f32,
    },
    DPadInput {
        player: u8,
        up: bool,
        down: bool,
        left: bool,
        right: bool,
    },
    WiiPointerInput {
        player: u8,
        x: f32,
        y: f32,
        z: f32,
    },
}

/// GameCube/Wii controller buttons
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum DolphinButton {
    A,
    B,
    X,
    Y,
    Z,
    L,
    R,
    Start,
    Up,
    Down,
    Left,
    Right,
}

/// Analog stick types
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum AnalogStick {
    Main,   // Left stick (GameCube main analog)
    CStick, // Right stick (GameCube C-stick)
}

/// Input processing statistics
#[derive(Debug, Clone, Default)]
pub struct ProcessorStats {
    pub packets_processed: u64,
    pub commands_sent: u64,
    pub commands_dropped: u64,
    pub total_processing_time: Duration,
    pub average_processing_time: Duration,
}
