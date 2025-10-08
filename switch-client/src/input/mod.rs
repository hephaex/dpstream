//! Input handling for Nintendo Switch
//!
//! Manages Joy-Con, Pro Controller, and touch input

use crate::error::{Result, InputError};
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

        // In real implementation:
        // hidScanInput();
        // for each controller: hidGetControllerState()

        // Mock input for now
        self.current_state = InputState::default();

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

    /// Get touch screen state
    pub fn get_touch_state(&self) -> &TouchState {
        &self.current_state.touch
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
    pub touch: TouchState,
    pub gyro: GyroState,
    pub accelerometer: AccelState,
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
        (
            self.x as f32 / 32767.0,
            self.y as f32 / 32767.0,
        )
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
    pub id: u8,
    pub controller_type: ControllerType,
    pub is_connected: bool,
    pub battery_level: u8,
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
        if self.buttons.contains(Buttons::A) { gamepad_buttons |= 0x1000; } // A
        if self.buttons.contains(Buttons::B) { gamepad_buttons |= 0x2000; } // B
        if self.buttons.contains(Buttons::X) { gamepad_buttons |= 0x4000; } // X
        if self.buttons.contains(Buttons::Y) { gamepad_buttons |= 0x8000; } // Y

        if self.buttons.contains(Buttons::L) { gamepad_buttons |= 0x0100; } // Left shoulder
        if self.buttons.contains(Buttons::R) { gamepad_buttons |= 0x0200; } // Right shoulder

        if self.buttons.contains(Buttons::MINUS) { gamepad_buttons |= 0x0020; } // Back
        if self.buttons.contains(Buttons::PLUS) { gamepad_buttons |= 0x0010; } // Start

        if self.buttons.contains(Buttons::L_STICK) { gamepad_buttons |= 0x0040; } // Left thumb
        if self.buttons.contains(Buttons::R_STICK) { gamepad_buttons |= 0x0080; } // Right thumb

        if self.buttons.contains(Buttons::D_UP) { gamepad_buttons |= 0x0001; }
        if self.buttons.contains(Buttons::D_DOWN) { gamepad_buttons |= 0x0002; }
        if self.buttons.contains(Buttons::D_LEFT) { gamepad_buttons |= 0x0004; }
        if self.buttons.contains(Buttons::D_RIGHT) { gamepad_buttons |= 0x0008; }

        MoonlightInput {
            packet_type: 0x0C, // Multi-controller packet
            button_flags: gamepad_buttons,
            left_trigger: if self.buttons.contains(Buttons::ZL) { 255 } else { 0 },
            right_trigger: if self.buttons.contains(Buttons::ZR) { 255 } else { 0 },
            left_stick_x: self.left_stick.x,
            left_stick_y: self.left_stick.y,
            right_stick_x: self.right_stick.x,
            right_stick_y: self.right_stick.y,
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
}