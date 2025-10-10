//! Input handling for Nintendo Switch
//!
//! Manages Joy-Con, Pro Controller, and touch input

use crate::error::{InputError, Result};
use bitflags::bitflags;

/// Main input manager
pub struct InputManager {
    current_state: InputState,
    previous_state: InputState,
    controllers: [Controller; 8], // Up to 8 controllers
}

impl InputManager {
    /// Initialize input system
    pub fn new() -> Result<Self> {
        // In real implementation: hidInitialize()
        Ok(Self {
            current_state: InputState::default(),
            previous_state: InputState::default(),
            controllers: [Controller::default(); 8],
        })
    }

    /// Update input state (call once per frame)
    pub fn update(&mut self) -> Result<()> {
        self.previous_state = self.current_state;

        // Scan input devices
        self.scan_input_devices()?;

        // Update controller states
        for i in 0..self.controllers.len() {
            if self.controllers[i].is_connected {
                self.update_controller_state(i)?;
            }
        }

        // Update consolidated input state
        self.update_consolidated_state()?;

        Ok(())
    }

    /// Scan for connected input devices
    fn scan_input_devices(&mut self) -> Result<()> {
        // In real implementation: hidScanInput()

        // Check handheld mode (built-in Joy-Cons)
        self.controllers[0] = Controller {
            controller_type: ControllerType::Handheld,
            is_connected: true,
            player_number: 0,
            battery_level: 100, // Mock full battery
            ..Default::default()
        };

        // Check for separate Joy-Cons and Pro Controllers
        for i in 1..4 {
            // Mock detection logic - in real implementation:
            // Use hidGetControllerColors, hidGetControllerType, etc.
            self.controllers[i].is_connected = false; // No additional controllers for now
        }

        Ok(())
    }

    /// Update state for a specific controller
    fn update_controller_state(&mut self, controller_index: usize) -> Result<()> {
        let controller = &mut self.controllers[controller_index];

        // In real implementation: hidGetControllerState()
        // For now, mock the state update

        match controller.controller_type {
            ControllerType::Handheld => {
                self.update_handheld_state(controller)?;
            }
            ControllerType::JoyConLeft | ControllerType::JoyConRight => {
                self.update_joycon_state(controller)?;
            }
            ControllerType::ProController => {
                self.update_pro_controller_state(controller)?;
            }
            ControllerType::None => {}
        }

        Ok(())
    }

    /// Update handheld mode input (built-in Joy-Cons)
    fn update_handheld_state(&mut self, controller: &mut Controller) -> Result<()> {
        // In real implementation:
        // - hidGetControllerState() for buttons and sticks
        // - hidGetSixAxisSensorValues() for gyro/accelerometer
        // - hidGetTouchScreenStates() for touch input

        // Mock implementation - in production this would read actual hardware
        controller.buttons = Buttons::empty();
        controller.left_stick = AnalogStick { x: 0, y: 0 };
        controller.right_stick = AnalogStick { x: 0, y: 0 };
        controller.left_trigger = 0;
        controller.right_trigger = 0;

        // Mock gyroscope data (would come from hidGetSixAxisSensorValues)
        controller.gyro = SixAxisSensor {
            angular_velocity_x: 0.0,
            angular_velocity_y: 0.0,
            angular_velocity_z: 0.0,
            acceleration_x: 0.0,
            acceleration_y: 0.0,
            acceleration_z: 9.8, // Gravity
            orientation_w: 1.0,
            orientation_x: 0.0,
            orientation_y: 0.0,
            orientation_z: 0.0,
        };

        // Update touch points (would come from hidGetTouchScreenStates)
        controller.touch_points.clear();

        Ok(())
    }

    /// Update Joy-Con specific state
    fn update_joycon_state(&mut self, controller: &mut Controller) -> Result<()> {
        // Joy-Con specific logic
        // Similar to handheld but with individual Joy-Con handling
        self.update_handheld_state(controller)
    }

    /// Update Pro Controller state
    fn update_pro_controller_state(&mut self, controller: &mut Controller) -> Result<()> {
        // Pro Controller specific logic
        // Higher precision sticks, HD Rumble support
        self.update_handheld_state(controller)
    }

    /// Update consolidated input state from all controllers
    fn update_consolidated_state(&mut self) -> Result<()> {
        // Combine input from all connected controllers
        // Priority: Handheld > Pro Controller > Joy-Cons

        for controller in &self.controllers {
            if controller.is_connected {
                // Use the first connected controller as primary
                self.current_state = InputState {
                    buttons: controller.buttons,
                    left_stick: controller.left_stick,
                    right_stick: controller.right_stick,
                    left_trigger: controller.left_trigger,
                    right_trigger: controller.right_trigger,
                    gyro: controller.gyro,
                    touch_points: controller.touch_points.clone(),
                };
                break;
            }
        }

        Ok(())
    }

    /// Get current input state
    pub fn get_current_state(&self) -> &InputState {
        &self.current_state
    }

    /// Check if a button was just pressed this frame
    pub fn is_button_pressed(&self, button: Buttons) -> bool {
        self.current_state.buttons.contains(button) && !self.previous_state.buttons.contains(button)
    }

    /// Check if a button is currently held
    pub fn is_button_held(&self, button: Buttons) -> bool {
        self.current_state.buttons.contains(button)
    }

    /// Convenience methods for common buttons
    pub fn is_a_pressed(&self) -> bool {
        self.is_button_pressed(Buttons::A)
    }

    pub fn is_b_pressed(&self) -> bool {
        self.is_button_pressed(Buttons::B)
    }

    pub fn is_x_pressed(&self) -> bool {
        self.is_button_pressed(Buttons::X)
    }

    pub fn is_y_pressed(&self) -> bool {
        self.is_button_pressed(Buttons::Y)
    }

    pub fn is_home_pressed(&self) -> bool {
        self.is_button_pressed(Buttons::HOME)
    }

    pub fn is_plus_pressed(&self) -> bool {
        self.is_button_pressed(Buttons::PLUS)
    }

    pub fn is_minus_pressed(&self) -> bool {
        self.is_button_pressed(Buttons::MINUS)
    }

    /// Get left analog stick position
    pub fn get_left_stick(&self) -> AnalogStick {
        self.current_state.left_stick
    }

    /// Get right analog stick position
    pub fn get_right_stick(&self) -> AnalogStick {
        self.current_state.right_stick
    }

    /// Get touch points
    pub fn get_touch_points(&self) -> &alloc::vec::Vec<TouchPoint> {
        &self.current_state.touch_points
    }

    /// Get gyroscope data
    pub fn get_gyro(&self) -> &SixAxisSensor {
        &self.current_state.gyro
    }

    /// Cleanup input system
    pub fn cleanup(&mut self) -> Result<()> {
        // In real implementation: hidExit()
        Ok(())
    }
}

/// Complete input state for one frame
#[derive(Debug, Clone, Default)]
pub struct InputState {
    pub buttons: Buttons,
    pub left_stick: AnalogStick,
    pub right_stick: AnalogStick,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub gyro: SixAxisSensor,
    pub touch_points: alloc::vec::Vec<TouchPoint>,
}

/// Button flags
bitflags! {
    #[derive(Default)]
    pub struct Buttons: u32 {
        const A = 1 << 0;
        const B = 1 << 1;
        const X = 1 << 2;
        const Y = 1 << 3;
        const L_STICK = 1 << 4;
        const R_STICK = 1 << 5;
        const L = 1 << 6;
        const R = 1 << 7;
        const ZL = 1 << 8;
        const ZR = 1 << 9;
        const PLUS = 1 << 10;
        const MINUS = 1 << 11;
        const D_LEFT = 1 << 12;
        const D_UP = 1 << 13;
        const D_RIGHT = 1 << 14;
        const D_DOWN = 1 << 15;
        const L_STICK_LEFT = 1 << 16;
        const L_STICK_UP = 1 << 17;
        const L_STICK_RIGHT = 1 << 18;
        const L_STICK_DOWN = 1 << 19;
        const R_STICK_LEFT = 1 << 20;
        const R_STICK_UP = 1 << 21;
        const R_STICK_RIGHT = 1 << 22;
        const R_STICK_DOWN = 1 << 23;
        const SL_LEFT = 1 << 24;
        const SR_LEFT = 1 << 25;
        const SL_RIGHT = 1 << 26;
        const SR_RIGHT = 1 << 27;
        const HOME = 1 << 28;
        const CAPTURE = 1 << 29;
    }
}

/// Analog stick position
#[derive(Debug, Clone, Copy, Default)]
pub struct AnalogStick {
    pub x: i16, // -32768 to 32767
    pub y: i16, // -32768 to 32767
}

impl AnalogStick {
    /// Get normalized float values (-1.0 to 1.0)
    pub fn normalized(&self) -> (f32, f32) {
        (self.x as f32 / 32767.0, self.y as f32 / 32767.0)
    }

    /// Get magnitude of stick deflection (0.0 to 1.0)
    pub fn magnitude(&self) -> f32 {
        let (x, y) = self.normalized();
        (x * x + y * y).sqrt().min(1.0)
    }
}

/// Touch screen state
#[derive(Debug, Clone, Default)]
pub struct TouchState {
    pub touches: heapless::Vec<TouchPoint, 10>, // Up to 10 touch points
}

/// Individual touch point
#[derive(Debug, Clone, Copy)]
pub struct TouchPoint {
    pub id: u32,
    pub x: u16,
    pub y: u16,
    pub diameter_x: u16,
    pub diameter_y: u16,
    pub rotation_angle: u16,
}

/// Gyroscope state
#[derive(Debug, Clone, Copy, Default)]
pub struct GyroState {
    pub x: f32, // Angular velocity around X axis (rad/s)
    pub y: f32, // Angular velocity around Y axis (rad/s)
    pub z: f32, // Angular velocity around Z axis (rad/s)
}

/// Accelerometer state
#[derive(Debug, Clone, Copy, Default)]
pub struct AccelState {
    pub x: f32, // Acceleration along X axis (m/s²)
    pub y: f32, // Acceleration along Y axis (m/s²)
    pub z: f32, // Acceleration along Z axis (m/s²)
}

/// Individual controller information
#[derive(Debug, Clone, Default)]
pub struct Controller {
    pub controller_type: ControllerType,
    pub is_connected: bool,
    pub player_number: u8,
    pub battery_level: u8,
    pub buttons: Buttons,
    pub left_stick: AnalogStick,
    pub right_stick: AnalogStick,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub gyro: SixAxisSensor,
    pub touch_points: alloc::vec::Vec<TouchPoint>,
}

/// Six-axis sensor data (gyroscope + accelerometer + orientation)
#[derive(Debug, Clone, Copy, Default)]
pub struct SixAxisSensor {
    pub angular_velocity_x: f32, // rad/s
    pub angular_velocity_y: f32, // rad/s
    pub angular_velocity_z: f32, // rad/s
    pub acceleration_x: f32,     // m/s²
    pub acceleration_y: f32,     // m/s²
    pub acceleration_z: f32,     // m/s²
    pub orientation_w: f32,      // Quaternion
    pub orientation_x: f32,      // Quaternion
    pub orientation_y: f32,      // Quaternion
    pub orientation_z: f32,      // Quaternion
}

/// Controller types
#[derive(Debug, Clone, Default)]
pub enum ControllerType {
    #[default]
    None,
    JoyConLeft,
    JoyConRight,
    JoyConPair,
    ProController,
    Handheld,
}

/// Convert input state to Moonlight protocol format
impl InputState {
    /// Convert to Moonlight input packet
    pub fn to_moonlight_input(&self) -> MoonlightInput {
        let mut gamepad_buttons = 0u16;

        // Map Switch buttons to Xbox controller buttons (Moonlight format)
        if self.buttons.contains(Buttons::A) {
            gamepad_buttons |= 0x1000;
        } // A
        if self.buttons.contains(Buttons::B) {
            gamepad_buttons |= 0x2000;
        } // B
        if self.buttons.contains(Buttons::X) {
            gamepad_buttons |= 0x4000;
        } // X
        if self.buttons.contains(Buttons::Y) {
            gamepad_buttons |= 0x8000;
        } // Y

        if self.buttons.contains(Buttons::L) {
            gamepad_buttons |= 0x0100;
        } // Left shoulder
        if self.buttons.contains(Buttons::R) {
            gamepad_buttons |= 0x0200;
        } // Right shoulder

        if self.buttons.contains(Buttons::MINUS) {
            gamepad_buttons |= 0x0020;
        } // Back
        if self.buttons.contains(Buttons::PLUS) {
            gamepad_buttons |= 0x0010;
        } // Start

        if self.buttons.contains(Buttons::L_STICK) {
            gamepad_buttons |= 0x0040;
        } // Left thumb
        if self.buttons.contains(Buttons::R_STICK) {
            gamepad_buttons |= 0x0080;
        } // Right thumb

        if self.buttons.contains(Buttons::D_UP) {
            gamepad_buttons |= 0x0001;
        }
        if self.buttons.contains(Buttons::D_DOWN) {
            gamepad_buttons |= 0x0002;
        }
        if self.buttons.contains(Buttons::D_LEFT) {
            gamepad_buttons |= 0x0004;
        }
        if self.buttons.contains(Buttons::D_RIGHT) {
            gamepad_buttons |= 0x0008;
        }

        MoonlightInput {
            packet_type: 0x0C, // Multi-controller packet
            button_flags: gamepad_buttons,
            left_trigger: if self.buttons.contains(Buttons::ZL) {
                255
            } else {
                0
            },
            right_trigger: if self.buttons.contains(Buttons::ZR) {
                255
            } else {
                0
            },
            left_stick_x: self.left_stick.x,
            left_stick_y: self.left_stick.y,
            right_stick_x: self.right_stick.x,
            right_stick_y: self.right_stick.y,
            timestamp: 0, // Will be set by network layer
            gyro_x: Some(self.gyro.angular_velocity_x),
            gyro_y: Some(self.gyro.angular_velocity_y),
            gyro_z: Some(self.gyro.angular_velocity_z),
            accel_x: Some(self.gyro.acceleration_x),
            accel_y: Some(self.gyro.acceleration_y),
            accel_z: Some(self.gyro.acceleration_z),
            touch_points: if !self.touch_points.is_empty() {
                Some(self.touch_points.clone())
            } else {
                None
            },
        }
    }
}

/// Moonlight input packet format
#[derive(Debug, Clone)]
pub struct MoonlightInput {
    pub packet_type: u8,
    pub button_flags: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub left_stick_x: i16,
    pub left_stick_y: i16,
    pub right_stick_x: i16,
    pub right_stick_y: i16,
    pub timestamp: u64,

    // Extended Switch-specific data
    pub gyro_x: Option<f32>,
    pub gyro_y: Option<f32>,
    pub gyro_z: Option<f32>,
    pub accel_x: Option<f32>,
    pub accel_y: Option<f32>,
    pub accel_z: Option<f32>,
    pub touch_points: Option<alloc::vec::Vec<TouchPoint>>,
}
