use std::env;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VpnError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Environment variable missing: {0}")]
    EnvVar(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailscaleConfig {
    pub auth_key: String,
    pub hostname: String,
    pub advertise_routes: Vec<String>,
    pub accept_dns: bool,
}

impl TailscaleConfig {
    pub fn from_env() -> Result<Self, VpnError> {
        Ok(Self {
            auth_key: env::var("TAILSCALE_AUTH_KEY")
                .map_err(|_| VpnError::EnvVar("TAILSCALE_AUTH_KEY".to_string()))?,
            hostname: env::var("TAILSCALE_HOSTNAME")
                .unwrap_or_else(|_| "dpstream-server".to_string()),
            advertise_routes: env::var("TAILSCALE_ROUTES")
                .unwrap_or_else(|_| "192.168.1.0/24".to_string())
                .split(',')
                .map(String::from)
                .collect(),
            accept_dns: env::var("TAILSCALE_ACCEPT_DNS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
        })
    }
}

pub struct VpnManager {
    config: TailscaleConfig,
    connected: bool,
    tailscale_ip: Option<String>,
}

impl VpnManager {
    pub async fn new() -> Result<Self, VpnError> {
        let config = TailscaleConfig::from_env()?;

        Ok(Self {
            config,
            connected: false,
            tailscale_ip: None,
        })
    }

    pub async fn connect(&mut self) -> Result<String, VpnError> {
        // TODO: Implement actual Tailscale connection
        // For now, simulate connection and return a mock IP

        // This would normally interact with Tailscale daemon
        tracing::info!("Attempting to connect to Tailscale network...");
        tracing::info!("Hostname: {}", self.config.hostname);
        tracing::info!("Routes: {:?}", self.config.advertise_routes);

        // Simulate successful connection
        let mock_ip = "100.64.0.1".to_string();
        self.tailscale_ip = Some(mock_ip.clone());
        self.connected = true;

        tracing::info!("Successfully connected to Tailscale network");

        Ok(mock_ip)
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn get_ip(&self) -> Option<&String> {
        self.tailscale_ip.as_ref()
    }
}