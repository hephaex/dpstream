// mDNS/UPnP service discovery using modern service discovery protocols
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, warn, error, debug};
use crate::error::{Result, NetworkError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub hostname: String,
    pub ip: String,
    pub port: u16,
    pub version: String,
    pub capabilities: Vec<String>,
}

pub struct DiscoveryService {
    server_info: ServerInfo,
    #[cfg(feature = "discovery")]
    mdns_daemon: Option<mdns_sd::ServiceDaemon>,
    advertising: bool,
}

impl DiscoveryService {
    pub fn new(ip: String, port: u16) -> Result<Self> {
        let server_info = ServerInfo {
            hostname: hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "dpstream-server".to_string()),
            ip,
            port,
            version: env!("CARGO_PKG_VERSION").to_string(),
            capabilities: vec![
                "dolphin".to_string(),
                "gamecube".to_string(),
                "wii".to_string(),
                "h264".to_string(),
                "nvenc".to_string(),
                "tailscale".to_string(),
            ],
        };

        info!("Initializing discovery service for {}", server_info.hostname);
        debug!("Server capabilities: {:?}", server_info.capabilities);

        Ok(Self {
            server_info,
            #[cfg(feature = "discovery")]
            mdns_daemon: None,
            advertising: false,
        })
    }

    pub async fn start_advertising(&mut self) -> Result<()> {
        if self.advertising {
            warn!("Discovery service already advertising");
            return Ok(());
        }

        info!("Starting service discovery advertising");
        info!("Service: {}:{}", self.server_info.ip, self.server_info.port);

        #[cfg(feature = "discovery")]
        {
            self.start_mdns_advertising().await?;
        }

        #[cfg(not(feature = "discovery"))]
        {
            info!("mDNS discovery feature not enabled, using simulation mode");
            self.simulate_advertising().await?;
        }

        self.advertising = true;
        info!("Service discovery advertising started successfully");
        Ok(())
    }

    #[cfg(feature = "discovery")]
    async fn start_mdns_advertising(&mut self) -> Result<()> {
        use mdns_sd::{ServiceDaemon, ServiceInfo};

        let daemon = ServiceDaemon::new().map_err(|e| {
            NetworkError::Discovery(format!("Failed to create mDNS daemon: {}", e))
        })?;

        let service_type = "_nvstream._tcp.local.";
        let instance_name = format!("{}._nvstream._tcp.local.", self.server_info.hostname);

        let properties = vec![
            ("hostname".to_string(), self.server_info.hostname.clone()),
            ("version".to_string(), self.server_info.version.clone()),
            ("capabilities".to_string(), self.server_info.capabilities.join(",")),
            ("GfeVersion".to_string(), "3.20.4.14".to_string()), // GameStream compatibility
            ("mac".to_string(), "00:11:22:33:44:55".to_string()), // Placeholder MAC
        ];

        let service_info = ServiceInfo::new(
            service_type,
            &instance_name,
            &self.server_info.hostname,
            &self.server_info.ip,
            self.server_info.port,
            &properties,
        ).map_err(|e| {
            NetworkError::Discovery(format!("Failed to create service info: {}", e))
        })?;

        daemon.register(service_info).map_err(|e| {
            NetworkError::Discovery(format!("Failed to register service: {}", e))
        })?;

        self.mdns_daemon = Some(daemon);
        info!("mDNS service registered: {}", instance_name);
        Ok(())
    }

    #[cfg(not(feature = "discovery"))]
    async fn simulate_advertising(&self) -> Result<()> {
        info!("Simulating mDNS advertising for development");
        info!("Service would be advertised as:");
        info!("  Type: _nvstream._tcp.local.");
        info!("  Instance: {}._nvstream._tcp.local.", self.server_info.hostname);
        info!("  Address: {}:{}", self.server_info.ip, self.server_info.port);
        info!("  Properties:");
        info!("    hostname: {}", self.server_info.hostname);
        info!("    version: {}", self.server_info.version);
        info!("    capabilities: {}", self.server_info.capabilities.join(","));

        // Simulate some delay
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }

    pub async fn stop_advertising(&mut self) -> Result<()> {
        if !self.advertising {
            debug!("Discovery service not currently advertising");
            return Ok(());
        }

        info!("Stopping service discovery advertising");

        #[cfg(feature = "discovery")]
        {
            if let Some(daemon) = self.mdns_daemon.take() {
                // The daemon will automatically unregister services when dropped
                drop(daemon);
                info!("mDNS service unregistered");
            }
        }

        self.advertising = false;
        info!("Service discovery advertising stopped");
        Ok(())
    }

    pub fn is_advertising(&self) -> bool {
        self.advertising
    }

    pub fn get_server_info(&self) -> &ServerInfo {
        &self.server_info
    }

    pub async fn discover_servers(timeout: Duration) -> Result<Vec<ServerInfo>> {
        info!("Discovering dpstream servers on network (timeout: {:?})", timeout);

        #[cfg(feature = "discovery")]
        {
            Self::discover_mdns_servers(timeout).await
        }

        #[cfg(not(feature = "discovery"))]
        {
            warn!("Discovery feature not enabled, returning empty server list");
            Ok(vec![])
        }
    }

    #[cfg(feature = "discovery")]
    async fn discover_mdns_servers(timeout: Duration) -> Result<Vec<ServerInfo>> {
        use mdns_sd::{ServiceDaemon, ServiceEvent};
        use tokio::time::timeout as async_timeout;

        let daemon = ServiceDaemon::new().map_err(|e| {
            NetworkError::Discovery(format!("Failed to create discovery daemon: {}", e))
        })?;

        let service_type = "_nvstream._tcp.local.";
        let receiver = daemon.browse(service_type).map_err(|e| {
            NetworkError::Discovery(format!("Failed to start service browsing: {}", e))
        })?;

        let mut servers = Vec::new();

        info!("Browsing for {} services...", service_type);

        match async_timeout(timeout, async {
            while let Ok(event) = receiver.recv_async().await {
                match event {
                    ServiceEvent::ServiceResolved(service) => {
                        debug!("Discovered service: {}", service.get_fullname());

                        let properties = service.get_properties();
                        let hostname = properties.get("hostname")
                            .unwrap_or(&service.get_hostname().to_string())
                            .clone();

                        let server_info = ServerInfo {
                            hostname,
                            ip: service.get_addresses().iter().next()
                                .map(|addr| addr.to_string())
                                .unwrap_or_else(|| "unknown".to_string()),
                            port: service.get_port(),
                            version: properties.get("version")
                                .unwrap_or(&"unknown".to_string())
                                .clone(),
                            capabilities: properties.get("capabilities")
                                .map(|caps| caps.split(',').map(String::from).collect())
                                .unwrap_or_default(),
                        };

                        info!("Found server: {} at {}:{}",
                              server_info.hostname, server_info.ip, server_info.port);
                        servers.push(server_info);
                    }
                    ServiceEvent::ServiceRemoved(_, _) => {
                        debug!("Service removed from network");
                    }
                    _ => {}
                }
            }
        }).await {
            Ok(_) => {}
            Err(_) => {
                debug!("Discovery timeout reached");
            }
        }

        info!("Discovery completed, found {} servers", servers.len());
        Ok(servers)
    }
}

impl Drop for DiscoveryService {
    fn drop(&mut self) {
        if self.advertising {
            // Note: In async context, we can't await here
            // The mDNS daemon will clean up automatically when dropped
            debug!("Discovery service dropping, advertising will stop");
        }
    }
}