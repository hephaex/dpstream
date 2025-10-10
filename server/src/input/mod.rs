//! Input processing module for dpstream server
//!
//! Handles input events from Moonlight clients and routes them to Dolphin emulator

pub mod dolphin;
pub mod mapping;
pub mod processor;

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use uuid::Uuid;

pub use dolphin::DolphinInputAdapter;
pub use mapping::{ControllerMapping, GameProfile};
pub use processor::InputProcessor;

/// Main input manager for the server
pub struct ServerInputManager {
    processor: InputProcessor,
    sessions: HashMap<Uuid, ClientInputSession>,
    dolphin_adapter: DolphinInputAdapter,
    global_mapping: ControllerMapping,
}

impl ServerInputManager {
    /// Create a new server input manager
    pub fn new() -> Result<Self> {
        info!("Initializing server input manager");

        Ok(Self {
            processor: InputProcessor::new()?,
            sessions: HashMap::new(),
            dolphin_adapter: DolphinInputAdapter::new()?,
            global_mapping: ControllerMapping::default_gamecube(),
        })
    }

    /// Register a new client session
    pub fn register_client(
        &mut self,
        session_id: Uuid,
    ) -> Result<mpsc::UnboundedSender<MoonlightInputPacket>> {
        debug!("Registering input session: {}", session_id);

        let (sender, receiver) = mpsc::unbounded_channel();

        let player_slot = self.assign_player_slot();
        let session = ClientInputSession {
            id: session_id,
            receiver,
            mapping: self.global_mapping.clone(),
            player_slot,
            last_input_time: Instant::now(),
            is_active: true,
        };

        self.sessions.insert(session_id, session);

        info!(
            "Input session registered: {} (Player {})",
            session_id, player_slot
        );
        Ok(sender)
    }

    /// Remove a client session
    pub fn unregister_client(&mut self, session_id: &Uuid) -> Result<()> {
        if let Some(session) = self.sessions.remove(session_id) {
            info!(
                "Input session unregistered: {} (Player {})",
                session_id, session.player_slot
            );
            self.dolphin_adapter
                .disconnect_controller(session.player_slot)?;
        }
        Ok(())
    }

    /// Process input from all sessions with enhanced error resilience
    pub async fn process_inputs(&mut self) -> Result<()> {
        // Collect inputs from all active sessions
        let mut inputs_to_process = Vec::new();
        let mut sessions_to_remove = Vec::new();

        for (session_id, session) in self.sessions.iter_mut() {
            // Check for timeouts
            if session.last_input_time.elapsed().as_secs() > 30 {
                warn!("Input session timeout: {}", session_id);
                session.is_active = false;
                sessions_to_remove.push(*session_id);
                continue;
            }

            // Try to receive input without blocking
            let mut input_count = 0;
            while let Ok(input_packet) = session.receiver.try_recv() {
                session.last_input_time = Instant::now();
                inputs_to_process.push((
                    session.player_slot,
                    session.mapping.clone(),
                    input_packet,
                ));

                // Prevent excessive input processing in a single frame
                input_count += 1;
                if input_count >= 10 {
                    break;
                }
            }
        }

        // Remove timed out sessions
        for session_id in sessions_to_remove {
            self.unregister_client(&session_id)?;
        }

        // Process all collected inputs with error resilience
        let mut successful_inputs = 0;
        let mut failed_inputs = 0;

        for (player_slot, mapping, input_packet) in inputs_to_process {
            match self
                .processor
                .process_input(player_slot, mapping, input_packet)
                .await
            {
                Ok(_) => successful_inputs += 1,
                Err(e) => {
                    failed_inputs += 1;
                    warn!("Failed to process input for player {}: {}", player_slot, e);
                }
            }
        }

        if failed_inputs > 0 {
            debug!(
                "Input processing: {} successful, {} failed",
                successful_inputs, failed_inputs
            );
        }

        // Send processed inputs to Dolphin with batch size limit for performance
        match self.processor.get_dolphin_commands_batched(20).await {
            Ok(Some(dolphin_commands)) => {
                if let Err(e) = self.dolphin_adapter.send_commands(dolphin_commands).await {
                    warn!("Failed to send commands to Dolphin: {}", e);
                }
            }
            Ok(None) => {} // No commands to send
            Err(e) => {
                warn!("Failed to get Dolphin commands: {}", e);
            }
        }

        Ok(())
    }

    /// Update controller mapping for a session
    pub fn update_mapping(&mut self, session_id: &Uuid, mapping: ControllerMapping) -> Result<()> {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.mapping = mapping;
            debug!("Updated controller mapping for session: {}", session_id);
        }
        Ok(())
    }

    /// Load game-specific controller profile
    pub fn load_game_profile(&mut self, game_id: &str) -> Result<()> {
        if let Ok(profile) = GameProfile::load_for_game(game_id) {
            self.global_mapping = profile.controller_mapping;
            info!("Loaded controller profile for game: {}", game_id);

            // Update all active sessions
            for session in self.sessions.values_mut() {
                session.mapping = self.global_mapping.clone();
            }
        }
        Ok(())
    }

    /// Get input statistics
    pub fn get_stats(&self) -> InputStats {
        InputStats {
            active_sessions: self.sessions.values().filter(|s| s.is_active).count(),
            total_sessions: self.sessions.len(),
            processor_stats: self.processor.get_stats(),
        }
    }

    fn assign_player_slot(&self) -> u8 {
        // Find the first available player slot (1-4 for GameCube, 1-8 for Wii)
        for slot in 1..=4 {
            if !self
                .sessions
                .values()
                .any(|s| s.player_slot == slot && s.is_active)
            {
                return slot;
            }
        }
        1 // Default to player 1 if all slots taken
    }
}

/// Input session for a connected client
#[allow(dead_code)]
struct ClientInputSession {
    id: Uuid,
    receiver: mpsc::UnboundedReceiver<MoonlightInputPacket>,
    mapping: ControllerMapping,
    player_slot: u8,
    last_input_time: Instant,
    is_active: bool,
}

/// Moonlight input packet from client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoonlightInputPacket {
    pub packet_type: u8,
    pub button_flags: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub left_stick_x: i16,
    pub left_stick_y: i16,
    pub right_stick_x: i16,
    pub right_stick_y: i16,
    pub timestamp: u64,

    // Extended data for Switch-specific features
    pub gyro_x: Option<f32>,
    pub gyro_y: Option<f32>,
    pub gyro_z: Option<f32>,
    pub accel_x: Option<f32>,
    pub accel_y: Option<f32>,
    pub accel_z: Option<f32>,
    pub touch_points: Option<Vec<TouchPoint>>,
}

/// Touch point data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchPoint {
    pub x: u16,
    pub y: u16,
    pub pressure: u8,
}

/// Input processing statistics
#[derive(Debug, Clone)]
pub struct InputStats {
    pub active_sessions: usize,
    pub total_sessions: usize,
    pub processor_stats: processor::ProcessorStats,
}

impl Default for ServerInputManager {
    fn default() -> Self {
        Self::new().expect("Failed to create ServerInputManager")
    }
}
