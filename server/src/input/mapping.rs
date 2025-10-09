//! Controller mapping and configuration system
//!
//! Provides flexible mapping between Switch controllers and GameCube/Wii controllers

use crate::error::{Result, InputError};
use crate::input::processor::DolphinButton;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{info, warn, debug};

/// Controller mapping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerMapping {
    pub name: String,
    pub description: String,
    pub console_type: ConsoleType,

    // Button mappings
    pub a_button: DolphinButton,
    pub b_button: DolphinButton,
    pub x_button: DolphinButton,
    pub y_button: DolphinButton,
    pub z_button: DolphinButton,
    pub l_button: DolphinButton,
    pub r_button: DolphinButton,
    pub start_button: DolphinButton,

    // Analog stick settings
    pub main_stick_sensitivity: f32,
    pub c_stick_sensitivity: f32,
    pub trigger_sensitivity: f32,

    // Switch-specific features
    pub enable_gyro_pointer: bool,
    pub enable_touch_pointer: bool,
    pub gyro_sensitivity: f32,

    // Advanced settings
    pub deadzone: f32,
    pub vibration_strength: f32,
    pub invert_y_axis: bool,
}

impl ControllerMapping {
    /// Default GameCube controller mapping
    pub fn default_gamecube() -> Self {
        Self {
            name: "Default GameCube".to_string(),
            description: "Standard GameCube controller mapping".to_string(),
            console_type: ConsoleType::GameCube,

            // Standard button layout
            a_button: DolphinButton::A,
            b_button: DolphinButton::B,
            x_button: DolphinButton::X,
            y_button: DolphinButton::Y,
            z_button: DolphinButton::Z,
            l_button: DolphinButton::L,
            r_button: DolphinButton::R,
            start_button: DolphinButton::Start,

            // Default sensitivities
            main_stick_sensitivity: 1.0,
            c_stick_sensitivity: 1.0,
            trigger_sensitivity: 1.0,

            // Switch features disabled for GameCube
            enable_gyro_pointer: false,
            enable_touch_pointer: false,
            gyro_sensitivity: 1.0,

            // Standard settings
            deadzone: 0.1,
            vibration_strength: 1.0,
            invert_y_axis: false,
        }
    }

    /// Default Wii Remote mapping
    pub fn default_wii_remote() -> Self {
        Self {
            name: "Default Wii Remote".to_string(),
            description: "Standard Wii Remote with pointer support".to_string(),
            console_type: ConsoleType::Wii,

            // Wii Remote button layout (horizontal orientation)
            a_button: DolphinButton::A,
            b_button: DolphinButton::B,
            x_button: DolphinButton::Y, // 1 button
            y_button: DolphinButton::X, // 2 button
            z_button: DolphinButton::Z,
            l_button: DolphinButton::L,
            r_button: DolphinButton::R,
            start_button: DolphinButton::Start,

            // Lower sensitivity for pointer control
            main_stick_sensitivity: 0.8,
            c_stick_sensitivity: 0.8,
            trigger_sensitivity: 1.0,

            // Enable motion controls
            enable_gyro_pointer: true,
            enable_touch_pointer: true,
            gyro_sensitivity: 2.0,

            // More forgiving deadzone for motion
            deadzone: 0.05,
            vibration_strength: 0.8,
            invert_y_axis: false,
        }
    }

    /// Custom mapping for specific games
    pub fn for_game(game_id: &str) -> Self {
        match game_id {
            "GALE01" => Self::smash_melee_mapping(),
            "GM4E01" => Self::metroid_prime_mapping(),
            "GMPE01" => Self::metroid_prime_2_mapping(),
            "RSBE01" => Self::smash_brawl_mapping(),
            _ => Self::default_gamecube(),
        }
    }

    /// Super Smash Bros. Melee optimized mapping
    fn smash_melee_mapping() -> Self {
        let mut mapping = Self::default_gamecube();
        mapping.name = "Smash Bros. Melee".to_string();
        mapping.description = "Optimized for competitive Melee play".to_string();

        // Higher C-stick sensitivity for quick smash attacks
        mapping.c_stick_sensitivity = 1.2;
        // Lower deadzone for precise movement
        mapping.deadzone = 0.05;
        // Higher trigger sensitivity for L-canceling
        mapping.trigger_sensitivity = 1.1;

        mapping
    }

    /// Metroid Prime series mapping with motion controls
    fn metroid_prime_mapping() -> Self {
        let mut mapping = Self::default_gamecube();
        mapping.name = "Metroid Prime".to_string();
        mapping.description = "Enhanced with gyro aiming".to_string();

        // Enable gyro for aiming
        mapping.enable_gyro_pointer = true;
        mapping.gyro_sensitivity = 1.5;
        // Invert Y for FPS-style aiming
        mapping.invert_y_axis = true;

        mapping
    }

    /// Metroid Prime 2 with similar but tweaked settings
    fn metroid_prime_2_mapping() -> Self {
        let mut mapping = Self::metroid_prime_mapping();
        mapping.name = "Metroid Prime 2: Echoes".to_string();
        // Slightly different sensitivity for MP2
        mapping.gyro_sensitivity = 1.3;
        mapping
    }

    /// Super Smash Bros. Brawl with Wii Remote support
    fn smash_brawl_mapping() -> Self {
        let mut mapping = Self::default_wii_remote();
        mapping.name = "Smash Bros. Brawl".to_string();
        mapping.description = "Wii Remote + Nunchuk style".to_string();

        // Enable both gyro and touch for flexibility
        mapping.enable_gyro_pointer = true;
        mapping.enable_touch_pointer = true;
        mapping.gyro_sensitivity = 1.8;

        mapping
    }

    /// Save mapping to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| InputError::ConfigurationError {
                field: "mapping".to_string(),
                value: "json".to_string(),
                reason: e.to_string(),
            })?;

        fs::write(path, json)
            .map_err(|e| InputError::ConfigurationError {
                field: "file".to_string(),
                value: "write".to_string(),
                reason: e.to_string(),
            })?;

        Ok(())
    }

    /// Load mapping from file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| InputError::ConfigurationError {
                field: "file".to_string(),
                value: "read".to_string(),
                reason: e.to_string(),
            })?;

        let mapping: ControllerMapping = serde_json::from_str(&content)
            .map_err(|e| InputError::ConfigurationError {
                field: "mapping".to_string(),
                value: "json".to_string(),
                reason: e.to_string(),
            })?;

        Ok(mapping)
    }
}

/// Game-specific controller profiles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameProfile {
    pub game_id: String,
    pub game_name: String,
    pub console_type: ConsoleType,
    pub controller_mapping: ControllerMapping,
    pub recommended_settings: HashMap<String, String>,
}

impl GameProfile {
    /// Load profile for a specific game
    pub fn load_for_game(game_id: &str) -> Result<Self> {
        let profile_path = format!("profiles/{}.json", game_id);

        if Path::new(&profile_path).exists() {
            Self::load_from_file(profile_path)
        } else {
            // Create default profile
            Ok(Self::default_for_game(game_id))
        }
    }

    /// Create default profile for a game
    fn default_for_game(game_id: &str) -> Self {
        let (game_name, console_type, mapping) = match game_id {
            "GALE01" => (
                "Super Smash Bros. Melee".to_string(),
                ConsoleType::GameCube,
                ControllerMapping::smash_melee_mapping(),
            ),
            "GM4E01" => (
                "Metroid Prime".to_string(),
                ConsoleType::GameCube,
                ControllerMapping::metroid_prime_mapping(),
            ),
            "RSBE01" => (
                "Super Smash Bros. Brawl".to_string(),
                ConsoleType::Wii,
                ControllerMapping::smash_brawl_mapping(),
            ),
            _ => (
                "Unknown Game".to_string(),
                ConsoleType::GameCube,
                ControllerMapping::default_gamecube(),
            ),
        };

        let mut settings = HashMap::new();
        settings.insert("recommended_resolution".to_string(), "1080p".to_string());
        settings.insert("recommended_fps".to_string(), "60".to_string());

        Self {
            game_id: game_id.to_string(),
            game_name,
            console_type,
            controller_mapping: mapping,
            recommended_settings: settings,
        }
    }

    /// Save profile to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| InputError::ConfigurationError {
                field: "profile".to_string(),
                value: "json".to_string(),
                reason: e.to_string(),
            })?;

        // Ensure profiles directory exists
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)
                .map_err(|e| InputError::ConfigurationError {
                    field: "directory".to_string(),
                    value: "create".to_string(),
                    reason: e.to_string(),
                })?;
        }

        fs::write(path, json)
            .map_err(|e| InputError::ConfigurationError {
                field: "file".to_string(),
                value: "write".to_string(),
                reason: e.to_string(),
            })?;

        Ok(())
    }

    /// Load profile from file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| InputError::ConfigurationError {
                field: "file".to_string(),
                value: "read".to_string(),
                reason: e.to_string(),
            })?;

        let profile: GameProfile = serde_json::from_str(&content)
            .map_err(|e| InputError::ConfigurationError {
                field: "profile".to_string(),
                value: "json".to_string(),
                reason: e.to_string(),
            })?;

        Ok(profile)
    }
}

/// Console type for mapping
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ConsoleType {
    GameCube,
    Wii,
    WiiU, // For future expansion
}

/// Calibration data for analog inputs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationData {
    pub stick_center_x: i16,
    pub stick_center_y: i16,
    pub stick_range_x: i16,
    pub stick_range_y: i16,
    pub trigger_range: u8,
}

impl Default for CalibrationData {
    fn default() -> Self {
        Self {
            stick_center_x: 0,
            stick_center_y: 0,
            stick_range_x: 32767,
            stick_range_y: 32767,
            trigger_range: 255,
        }
    }
}

/// Mapping presets for quick setup
pub struct MappingPresets;

impl MappingPresets {
    /// Get all available presets
    pub fn get_all() -> Vec<ControllerMapping> {
        vec![
            ControllerMapping::default_gamecube(),
            ControllerMapping::default_wii_remote(),
            ControllerMapping::for_game("GALE01"), // Melee
            ControllerMapping::for_game("GM4E01"), // Metroid Prime
            ControllerMapping::for_game("RSBE01"), // Brawl
        ]
    }

    /// Get preset by name
    pub fn get_by_name(name: &str) -> Option<ControllerMapping> {
        Self::get_all().into_iter().find(|m| m.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_gamecube_mapping() {
        let mapping = ControllerMapping::default_gamecube();
        assert_eq!(mapping.console_type, ConsoleType::GameCube);
        assert!(!mapping.enable_gyro_pointer);
        assert_eq!(mapping.deadzone, 0.1);
    }

    #[test]
    fn test_wii_remote_mapping() {
        let mapping = ControllerMapping::default_wii_remote();
        assert_eq!(mapping.console_type, ConsoleType::Wii);
        assert!(mapping.enable_gyro_pointer);
        assert!(mapping.enable_touch_pointer);
    }

    #[test]
    fn test_game_specific_mapping() {
        let melee = ControllerMapping::for_game("GALE01");
        assert_eq!(melee.name, "Smash Bros. Melee");
        assert_eq!(melee.deadzone, 0.05); // Lower deadzone for precision

        let prime = ControllerMapping::for_game("GM4E01");
        assert!(prime.enable_gyro_pointer);
        assert!(prime.invert_y_axis);
    }

    #[test]
    fn test_mapping_serialization() {
        let mapping = ControllerMapping::default_gamecube();
        let json = serde_json::to_string(&mapping).unwrap();
        let deserialized: ControllerMapping = serde_json::from_str(&json).unwrap();

        assert_eq!(mapping.name, deserialized.name);
        assert_eq!(mapping.deadzone, deserialized.deadzone);
    }
}