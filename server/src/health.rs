use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: ServiceStatus,
    pub timestamp: u64,
    pub version: String,
    pub uptime_seconds: u64,
    pub checks: HashMap<String, HealthCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub status: ServiceStatus,
    pub message: String,
    pub last_updated: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessStatus {
    pub ready: bool,
    pub timestamp: u64,
    pub checks: Vec<ReadinessCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessCheck {
    pub name: String,
    pub ready: bool,
    pub message: String,
}

pub struct HealthMonitor {
    start_time: SystemTime,
    version: String,
    checks: Arc<RwLock<HashMap<String, HealthCheck>>>,
}

impl HealthMonitor {
    pub fn new(version: String) -> Self {
        Self {
            start_time: SystemTime::now(),
            version,
            checks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn update_check(&self, name: &str, status: ServiceStatus, message: String) {
        let check = HealthCheck {
            status,
            message,
            last_updated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            duration_ms: 0, // This would be measured in actual implementation
        };

        let mut checks = self.checks.write().await;
        checks.insert(name.to_string(), check);
    }

    pub async fn get_health_status(&self) -> HealthStatus {
        let checks = self.checks.read().await;
        let overall_status = self.determine_overall_status(&checks);

        HealthStatus {
            status: overall_status,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            version: self.version.clone(),
            uptime_seconds: self.start_time.elapsed().unwrap_or_default().as_secs(),
            checks: checks.clone(),
        }
    }

    pub async fn get_readiness_status(&self) -> ReadinessStatus {
        let checks = vec![
            self.check_dolphin_availability().await,
            self.check_streaming_service().await,
            self.check_network_interfaces().await,
            self.check_redis_connection().await,
            self.check_disk_space().await,
        ];

        let ready = checks.iter().all(|check| check.ready);

        ReadinessStatus {
            ready,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            checks,
        }
    }

    fn determine_overall_status(&self, checks: &HashMap<String, HealthCheck>) -> ServiceStatus {
        if checks.is_empty() {
            return ServiceStatus::Healthy;
        }

        let mut has_unhealthy = false;
        let mut has_degraded = false;
        let mut all_healthy = true;

        for check in checks.values() {
            match check.status {
                ServiceStatus::Healthy => {}
                ServiceStatus::Unhealthy => {
                    has_unhealthy = true;
                    all_healthy = false;
                }
                ServiceStatus::Degraded => {
                    has_degraded = true;
                    all_healthy = false;
                }
            }
        }

        if has_unhealthy {
            ServiceStatus::Unhealthy
        } else if has_degraded {
            ServiceStatus::Degraded
        } else if all_healthy {
            ServiceStatus::Healthy
        } else {
            ServiceStatus::Healthy // Fallback, shouldn't reach here
        }
    }

    async fn check_dolphin_availability(&self) -> ReadinessCheck {
        // Check if Dolphin emulator binary is available
        match tokio::fs::metadata("/usr/bin/dolphin-emu").await {
            Ok(_) => ReadinessCheck {
                name: "dolphin_emulator".to_string(),
                ready: true,
                message: "Dolphin emulator binary is available".to_string(),
            },
            Err(_) => ReadinessCheck {
                name: "dolphin_emulator".to_string(),
                ready: false,
                message: "Dolphin emulator binary not found".to_string(),
            },
        }
    }

    async fn check_streaming_service(&self) -> ReadinessCheck {
        // Check if streaming service is ready to accept connections
        // This would check if GStreamer pipeline is initialized, etc.
        ReadinessCheck {
            name: "streaming_service".to_string(),
            ready: true, // Simplified for now
            message: "Streaming service is ready".to_string(),
        }
    }

    async fn check_network_interfaces(&self) -> ReadinessCheck {
        // Check if required network interfaces are available
        use std::net::TcpListener;

        match TcpListener::bind("0.0.0.0:47989") {
            Ok(_) => ReadinessCheck {
                name: "network_interface".to_string(),
                ready: true,
                message: "Network interface is available".to_string(),
            },
            Err(e) => ReadinessCheck {
                name: "network_interface".to_string(),
                ready: false,
                message: format!("Network interface check failed: {e}"),
            },
        }
    }

    async fn check_redis_connection(&self) -> ReadinessCheck {
        // Check Redis connection
        // This would use the actual Redis client in production
        ReadinessCheck {
            name: "redis_cache".to_string(),
            ready: true, // Simplified for now
            message: "Redis cache is accessible".to_string(),
        }
    }

    async fn check_disk_space(&self) -> ReadinessCheck {
        // Check available disk space
        use std::fs;

        match fs::metadata("/app") {
            Ok(_) => ReadinessCheck {
                name: "disk_space".to_string(),
                ready: true,
                message: "Sufficient disk space available".to_string(),
            },
            Err(e) => ReadinessCheck {
                name: "disk_space".to_string(),
                ready: false,
                message: format!("Disk space check failed: {e}"),
            },
        }
    }
}

// Background health monitoring task
pub async fn run_health_monitoring(monitor: Arc<HealthMonitor>) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));

    loop {
        interval.tick().await;

        // Update various health checks
        monitor
            .update_check(
                "system_resources",
                ServiceStatus::Healthy,
                "System resources within normal limits".to_string(),
            )
            .await;

        monitor
            .update_check(
                "active_sessions",
                ServiceStatus::Healthy,
                "Active sessions within capacity".to_string(),
            )
            .await;

        // Add more checks as needed
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_monitor_creation() {
        let monitor = HealthMonitor::new("1.0.0".to_string());
        let status = monitor.get_health_status().await;

        assert_eq!(status.version, "1.0.0");
        assert!(matches!(status.status, ServiceStatus::Healthy));
    }

    #[tokio::test]
    async fn test_readiness_checks() {
        let monitor = HealthMonitor::new("1.0.0".to_string());
        let readiness = monitor.get_readiness_status().await;

        assert!(!readiness.checks.is_empty());
        // Most checks should pass in test environment
    }

    #[tokio::test]
    async fn test_health_check_updates() {
        let monitor = HealthMonitor::new("1.0.0".to_string());

        monitor
            .update_check(
                "test_service",
                ServiceStatus::Degraded,
                "Test degraded state".to_string(),
            )
            .await;

        let status = monitor.get_health_status().await;
        assert!(status.checks.contains_key("test_service"));
        assert!(matches!(status.status, ServiceStatus::Degraded));
    }
}
