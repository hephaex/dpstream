use std::env;
use anyhow::Result;
use tracing::{info, error};

mod emulator;
mod streaming;
mod network;

use network::vpn::VpnManager;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::init();

    // Load environment variables
    dotenv::dotenv().ok();

    info!("Starting Dolphin Remote Gaming Server v1.0.0");

    // Initialize Tailscale VPN
    match VpnManager::new().await {
        Ok(mut vpn) => {
            info!("Tailscale VPN manager initialized");
            match vpn.connect().await {
                Ok(ip) => {
                    info!("Connected to Tailscale network with IP: {}", ip);
                }
                Err(e) => {
                    error!("Failed to connect to Tailscale: {}", e);
                    return Err(e);
                }
            }
        }
        Err(e) => {
            error!("Failed to initialize VPN manager: {}", e);
            return Err(e);
        }
    }

    // TODO: Initialize streaming server
    // TODO: Initialize Dolphin emulator manager

    info!("Server initialization complete");

    // Keep the server running
    tokio::signal::ctrl_c().await?;
    info!("Server shutting down");

    Ok(())
}