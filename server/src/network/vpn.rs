use crate::error::{Result, VpnError};
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailscaleConfig {
    pub auth_key: String,
    pub hostname: String,
    pub advertise_routes: Vec<String>,
    pub accept_dns: bool,
}

impl TailscaleConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            auth_key: env::var("TAILSCALE_AUTH_KEY").map_err(|_| {
                VpnError::Config("TAILSCALE_AUTH_KEY environment variable not set".to_string())
            })?,
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
                .map_err(|_| VpnError::Config("Invalid TAILSCALE_ACCEPT_DNS value".to_string()))?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailscaleStatus {
    pub backend_state: String,
    pub health: Vec<String>,
    pub magicsock: MagicsockStatus,
    pub tailscale_ips: Vec<String>,
    pub hostname: String,
    pub os: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagicsockStatus {
    pub derp: String,
    pub endpoints: Vec<String>,
}

pub struct VpnManager {
    config: TailscaleConfig,
    connected: bool,
    tailscale_ip: Option<String>,
    last_status: Option<TailscaleStatus>,
}

impl VpnManager {
    pub async fn new() -> Result<Self> {
        let config = TailscaleConfig::from_env()?;

        Ok(Self {
            config,
            connected: false,
            tailscale_ip: None,
            last_status: None,
        })
    }

    pub async fn connect(&mut self) -> Result<String> {
        info!("Attempting to connect to Tailscale network...");
        info!("Hostname: {}", self.config.hostname);
        info!("Routes: {:?}", self.config.advertise_routes);

        // Check if tailscale is installed
        self.check_tailscale_installation().await?;

        // Check current status first
        if let Ok(status) = self.get_status().await {
            if status.backend_state == "Running" {
                info!("Tailscale is already running");
                if let Some(ip) = status.tailscale_ips.first() {
                    let ip_clone = ip.clone();
                    self.tailscale_ip = Some(ip_clone.clone());
                    self.connected = true;
                    self.last_status = Some(status);
                    return Ok(ip_clone);
                }
            }
        }

        // Start Tailscale if not running
        self.start_tailscale().await?;

        // Authenticate if needed
        self.authenticate().await?;

        // Configure hostname and routes
        self.configure_node().await?;

        // Wait for connection to establish
        let ip = self.wait_for_connection().await?;

        info!(
            "Successfully connected to Tailscale network with IP: {}",
            ip
        );
        Ok(ip)
    }

    async fn check_tailscale_installation(&self) -> Result<()> {
        debug!("Checking Tailscale installation");

        let output = timeout(Duration::from_secs(5), async {
            Command::new("tailscale").arg("version").output().await
        })
        .await
        .map_err(|_| VpnError::TailscaleNotAvailable("Command timeout".to_string()))?
        .map_err(|e| {
            VpnError::TailscaleNotAvailable(format!("Failed to run tailscale command: {e}"))
        })?;

        if !output.status.success() {
            return Err(VpnError::TailscaleNotAvailable(
                "Tailscale not installed or not accessible".to_string(),
            )
            .into());
        }

        let version = String::from_utf8_lossy(&output.stdout);
        info!("Tailscale version: {}", version.trim());
        Ok(())
    }

    async fn start_tailscale(&self) -> Result<()> {
        debug!("Starting Tailscale daemon");

        // Start tailscaled if not already running
        let output = Command::new("sudo")
            .args(["systemctl", "start", "tailscaled"])
            .output()
            .await
            .map_err(|e| {
                VpnError::TailscaleNotAvailable(format!("Failed to start tailscaled: {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to start tailscaled via systemctl: {}", stderr);
            // Continue anyway as it might already be running
        }

        // Wait a moment for the daemon to start
        tokio::time::sleep(Duration::from_secs(2)).await;

        info!("Tailscale daemon started");
        Ok(())
    }

    async fn authenticate(&mut self) -> Result<()> {
        debug!("Authenticating with Tailscale");

        let output = timeout(Duration::from_secs(30), async {
            Command::new("tailscale")
                .args(["up", "--auth-key", &self.config.auth_key])
                .output()
                .await
        })
        .await
        .map_err(|_| VpnError::AuthFailed("Authentication timeout".to_string()))?
        .map_err(|e| VpnError::AuthFailed(format!("Failed to authenticate: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VpnError::AuthFailed(format!("Authentication failed: {stderr}")).into());
        }

        info!("Successfully authenticated with Tailscale");
        Ok(())
    }

    async fn configure_node(&self) -> Result<()> {
        debug!("Configuring Tailscale node");

        // Set hostname
        let output = Command::new("tailscale")
            .args(["set", "--hostname", &self.config.hostname])
            .output()
            .await
            .map_err(|e| VpnError::Config(format!("Failed to set hostname: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to set hostname: {}", stderr);
        }

        // Advertise routes if configured
        if !self.config.advertise_routes.is_empty() {
            let routes = self.config.advertise_routes.join(",");
            let output = Command::new("tailscale")
                .args(["set", "--advertise-routes", &routes])
                .output()
                .await
                .map_err(|e| VpnError::Config(format!("Failed to advertise routes: {e}")))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to advertise routes: {}", stderr);
            } else {
                info!("Advertising routes: {}", routes);
            }
        }

        // Configure DNS settings
        if self.config.accept_dns {
            let output = Command::new("tailscale")
                .args(["set", "--accept-dns"])
                .output()
                .await
                .map_err(|e| VpnError::Config(format!("Failed to configure DNS: {e}")))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to accept DNS: {}", stderr);
            }
        }

        info!("Node configuration completed");
        Ok(())
    }

    async fn wait_for_connection(&mut self) -> Result<String> {
        debug!("Waiting for Tailscale connection to establish");

        let max_attempts = 30;
        let wait_interval = Duration::from_secs(2);

        for attempt in 1..=max_attempts {
            debug!("Connection attempt {}/{}", attempt, max_attempts);

            if let Ok(status) = self.get_status().await {
                if status.backend_state == "Running" && !status.tailscale_ips.is_empty() {
                    let ip = status.tailscale_ips[0].clone();
                    self.tailscale_ip = Some(ip.clone());
                    self.connected = true;
                    self.last_status = Some(status);
                    return Ok(ip);
                }
            }

            if attempt < max_attempts {
                tokio::time::sleep(wait_interval).await;
            }
        }

        Err(VpnError::Timeout.into())
    }

    pub async fn get_status(&self) -> Result<TailscaleStatus> {
        debug!("Getting Tailscale status");

        let output = timeout(Duration::from_secs(10), async {
            Command::new("tailscale")
                .args(["status", "--json"])
                .output()
                .await
        })
        .await
        .map_err(|_| VpnError::Timeout)?
        .map_err(|e| VpnError::TailscaleNotAvailable(format!("Failed to get status: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VpnError::TailscaleNotAvailable(format!(
                "Status command failed: {stderr}"
            ))
            .into());
        }

        let status_json = String::from_utf8_lossy(&output.stdout);

        // Parse the complex Tailscale status JSON into our simplified structure
        let status = self.parse_tailscale_status(&status_json)?;

        Ok(status)
    }

    fn parse_tailscale_status(&self, json_str: &str) -> Result<TailscaleStatus> {
        // This is a simplified parser for Tailscale status JSON
        // In a real implementation, we'd use the full Tailscale API types

        use serde_json::Value;

        let json: Value = serde_json::from_str(json_str)
            .map_err(|e| VpnError::Config(format!("Failed to parse status JSON: {e}")))?;

        let backend_state = json["BackendState"]
            .as_str()
            .unwrap_or("Unknown")
            .to_string();

        let mut tailscale_ips = Vec::new();
        if let Some(self_node) = json["Self"].as_object() {
            if let Some(ips) = self_node["TailscaleIPs"].as_array() {
                for ip in ips {
                    if let Some(ip_str) = ip.as_str() {
                        tailscale_ips.push(ip_str.to_string());
                    }
                }
            }
        }

        let hostname = json["Self"]["HostName"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        let os = json["Self"]["OS"].as_str().unwrap_or("unknown").to_string();
        let version = json["Version"].as_str().unwrap_or("unknown").to_string();

        Ok(TailscaleStatus {
            backend_state,
            health: vec![], // Simplified - would parse health warnings
            magicsock: MagicsockStatus {
                derp: "unknown".to_string(), // Simplified
                endpoints: vec![],           // Simplified
            },
            tailscale_ips,
            hostname,
            os,
            version,
        })
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from Tailscale network");

        let output = Command::new("tailscale")
            .arg("down")
            .output()
            .await
            .map_err(|e| VpnError::TailscaleNotAvailable(format!("Failed to disconnect: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(
                VpnError::TailscaleNotAvailable(format!("Disconnect failed: {stderr}")).into(),
            );
        }

        self.connected = false;
        self.tailscale_ip = None;
        self.last_status = None;

        info!("Successfully disconnected from Tailscale network");
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn get_ip(&self) -> Option<&String> {
        self.tailscale_ip.as_ref()
    }

    pub fn get_status_snapshot(&self) -> Option<&TailscaleStatus> {
        self.last_status.as_ref()
    }

    pub async fn refresh_status(&mut self) -> Result<()> {
        match self.get_status().await {
            Ok(status) => {
                self.last_status = Some(status);
                Ok(())
            }
            Err(e) => {
                error!("Failed to refresh status: {}", e);
                Err(e)
            }
        }
    }

    pub async fn ping_peer(&self, peer_ip: &str) -> Result<bool> {
        debug!("Pinging peer: {}", peer_ip);

        let output = timeout(Duration::from_secs(5), async {
            Command::new("tailscale")
                .args(["ping", peer_ip])
                .output()
                .await
        })
        .await
        .map_err(|_| VpnError::Timeout)?
        .map_err(|e| VpnError::NetworkUnreachable(format!("Failed to ping peer: {e}")))?;

        let success = output.status.success();
        if success {
            info!("Successfully pinged peer: {}", peer_ip);
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to ping peer {}: {}", peer_ip, stderr);
        }

        Ok(success)
    }

    pub async fn restart(&mut self) -> Result<String> {
        info!("Restarting Tailscale connection");

        if self.connected {
            self.disconnect().await?;
        }

        // Wait a moment before reconnecting
        tokio::time::sleep(Duration::from_secs(2)).await;

        self.connect().await
    }

    pub fn get_config(&self) -> &TailscaleConfig {
        &self.config
    }
}
