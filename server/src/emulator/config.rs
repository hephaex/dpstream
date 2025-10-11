#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DolphinConfig {
    pub video: VideoConfig,
    pub audio: AudioConfig,
    pub controls: ControlsConfig,
    pub general: GeneralConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoConfig {
    pub backend: String,               // "Vulkan", "OpenGL", "D3D11"
    pub adapter: Option<String>,       // GPU adapter
    pub resolution_scale: u32,         // 1x, 2x, 3x, etc.
    pub anti_aliasing: String,         // "None", "FXAA", "MSAA"
    pub anisotropic_filtering: String, // "1x", "2x", "4x", "8x", "16x"
    pub vsync: bool,
    pub fullscreen: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub backend: String, // "Pulse", "ALSA", "Cubeb"
    pub volume: u8,      // 0-100
    pub dsp_hle: bool,   // High-level DSP emulation
    pub latency: u32,    // Audio latency in ms
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlsConfig {
    pub gamecube_adapter: bool,
    pub wiimote_source: String, // "None", "Emulated", "Real"
    pub players: HashMap<u8, PlayerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerConfig {
    pub device: String,  // "None", "Standard Controller", "GC Adapter"
    pub profile: String, // Controller profile name
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub dual_core: bool,
    pub idle_skipping: bool,
    pub cheats_enabled: bool,
    pub discord_presence: bool,
    pub auto_update: bool,
}

#[allow(dead_code)]
impl DolphinConfig {
    pub fn default_streaming() -> Self {
        let mut players = HashMap::new();
        players.insert(
            1,
            PlayerConfig {
                device: "Standard Controller".to_string(),
                profile: "Remote Player".to_string(),
            },
        );

        Self {
            video: VideoConfig {
                backend: "Vulkan".to_string(),
                adapter: None,
                resolution_scale: 2, // 2x native resolution
                anti_aliasing: "FXAA".to_string(),
                anisotropic_filtering: "4x".to_string(),
                vsync: false, // Disable for streaming
                fullscreen: true,
            },
            audio: AudioConfig {
                backend: "Pulse".to_string(),
                volume: 80,
                dsp_hle: true,
                latency: 32, // Low latency for streaming
            },
            controls: ControlsConfig {
                gamecube_adapter: false,
                wiimote_source: "Emulated".to_string(),
                players,
            },
            general: GeneralConfig {
                dual_core: true,
                idle_skipping: true,
                cheats_enabled: false,
                discord_presence: false,
                auto_update: false,
            },
        }
    }

    pub fn save_to_file(&self, path: &str) -> Result<()> {
        // TODO: Convert to Dolphin's INI format and save
        tracing::info!("Saving Dolphin config to: {}", path);
        Ok(())
    }

    pub fn load_from_file(path: &str) -> Result<Self> {
        // TODO: Load from Dolphin's INI format
        tracing::info!("Loading Dolphin config from: {}", path);
        Ok(Self::default_streaming())
    }
}
