use std::env;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod emulator;
mod error;
mod health;
mod input;
mod network;
mod streaming;

use emulator::{DolphinConfig, DolphinManager};
use error::{DpstreamError, ErrorReport, Result};
use health::{run_health_monitoring, HealthMonitor};
use input::ServerInputManager;
use network::VpnManager;
use std::sync::Arc;
use streaming::{HealthServer, MoonlightServer, ServerConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize enhanced logging
    init_logging()?;

    // Load environment variables
    dotenv::dotenv().ok();

    info!("Starting Dolphin Remote Gaming Server v1.0.0");
    info!("Platform: {}", env::consts::OS);
    info!("Architecture: {}", env::consts::ARCH);

    // Generate correlation ID for this session
    let session_id = uuid::Uuid::new_v4().to_string();
    info!("Session ID: {}", session_id);

    // Initialize Tailscale VPN
    debug!("Initializing Tailscale VPN manager...");
    let mut vpn = VpnManager::new().await.map_err(|e| {
        let report = ErrorReport::new(e)
            .with_context("Failed to initialize VPN manager".to_string())
            .with_correlation_id(session_id.clone());
        error!("{}", report.format_for_log());
        report.error
    })?;

    info!("Tailscale VPN manager initialized");

    // Connect to VPN
    debug!("Connecting to Tailscale network...");
    let tailscale_ip = vpn.connect().await.map_err(|e| {
        let report = ErrorReport::new(e)
            .with_context("Failed to connect to Tailscale network".to_string())
            .with_correlation_id(session_id.clone());
        error!("{}", report.format_for_log());
        report.error
    })?;

    info!("Connected to Tailscale network with IP: {}", tailscale_ip);

    // Initialize streaming server
    debug!("Initializing streaming server...");
    let streaming_config = ServerConfig {
        bind_addr: tailscale_ip.clone(),
        port: env::var("SERVER_PORT")
            .unwrap_or_else(|_| "47989".to_string())
            .parse()
            .map_err(|e| DpstreamError::Config(format!("Invalid SERVER_PORT: {e}")))?,
        max_clients: env::var("MAX_CLIENTS")
            .unwrap_or_else(|_| "4".to_string())
            .parse()
            .map_err(|e| DpstreamError::Config(format!("Invalid MAX_CLIENTS: {e}")))?,
        enable_encryption: true,
        enable_authentication: true,
        stream_timeout_ms: 30000,
    };

    let mut streaming_server = MoonlightServer::new(streaming_config).await.map_err(|e| {
        let report = ErrorReport::new(e)
            .with_context("Failed to initialize streaming server".to_string())
            .with_correlation_id(session_id.clone());
        error!("{}", report.format_for_log());
        report.error
    })?;

    info!(
        "Streaming server initialized on {}:{}",
        tailscale_ip,
        streaming_server.port()
    );

    // Initialize Dolphin emulator manager
    debug!("Initializing Dolphin emulator manager...");
    let dolphin_config = DolphinConfig {
        executable_path: env::var("DOLPHIN_PATH")
            .unwrap_or_else(|_| "/usr/bin/dolphin-emu".to_string()),
        rom_directory: env::var("ROM_PATH").unwrap_or_else(|_| "/srv/games/gc-wii".to_string()),
        save_directory: env::var("SAVE_PATH").unwrap_or_else(|_| "/srv/saves".to_string()),
        window_title: "Dolphin Remote Gaming".to_string(),
        enable_graphics_mods: true,
        enable_netplay: false,
        audio_backend: "pulse".to_string(),
        video_backend: "OpenGL".to_string(),
    };

    let mut dolphin_manager = DolphinManager::new(dolphin_config).map_err(|e| {
        let report = ErrorReport::new(e)
            .with_context("Failed to initialize Dolphin manager".to_string())
            .with_correlation_id(session_id.clone());
        error!("{}", report.format_for_log());
        report.error
    })?;

    info!("Dolphin emulator manager initialized");

    // Initialize input manager
    debug!("Initializing input manager...");
    let input_manager = ServerInputManager::new().map_err(|e| {
        let report = ErrorReport::new(e)
            .with_context("Failed to initialize input manager".to_string())
            .with_correlation_id(session_id.clone());
        error!("{}", report.format_for_log());
        report.error
    })?;

    info!("Input manager initialized");

    // Initialize health monitoring
    debug!("Initializing health monitor...");
    let health_monitor = Arc::new(HealthMonitor::new("1.0.0".to_string()));

    // Start health monitoring background task
    let health_monitor_clone = health_monitor.clone();
    tokio::spawn(async move {
        run_health_monitoring(health_monitor_clone).await;
    });

    info!("Health monitoring initialized");

    // Start health server
    debug!("Starting health check server...");
    let health_server = HealthServer::new(health_monitor.clone(), 8080);
    tokio::spawn(async move {
        if let Err(e) = health_server.run().await {
            error!("Health server error: {}", e);
        }
    });

    info!("Health server started on port 8080");

    // Connect input manager to streaming server
    streaming_server.set_input_manager(input_manager);
    streaming_server.set_health_monitor(health_monitor);

    info!("Server initialization complete");
    info!("Ready to accept client connections");

    // Start the streaming server
    let server_handle = tokio::spawn(async move {
        if let Err(e) = streaming_server.run().await {
            error!("Streaming server error: {}", e);
        }
    });

    // Setup graceful shutdown
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal (Ctrl+C)");
        }
        _ = wait_for_termination() => {
            warn!("Received termination signal");
        }
        result = server_handle => {
            match result {
                Ok(_) => info!("Streaming server completed"),
                Err(e) => error!("Streaming server task error: {}", e),
            }
        }
    }

    info!("Shutting down server gracefully...");

    // Cleanup resources in proper order
    info!("Stopping Dolphin emulator instances...");
    if let Err(e) = dolphin_manager.shutdown().await {
        warn!("Error stopping Dolphin manager: {}", e);
    }

    info!("Disconnecting from Tailscale...");
    if let Err(e) = vpn.disconnect().await {
        warn!("Error disconnecting from Tailscale: {}", e);
    }

    info!("Cleanup complete");
    info!("Server shutdown complete");
    Ok(())
}

/// Initialize enhanced logging with structured output
fn init_logging() -> Result<()> {
    let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

    // Create a file appender for persistent logs
    let log_dir = std::path::Path::new("logs");
    if !log_dir.exists() {
        std::fs::create_dir_all(log_dir)
            .map_err(|e| DpstreamError::Internal(format!("Failed to create log directory: {e}")))?;
    }

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let log_file = format!("logs/dpstream_{timestamp}.log");

    // Configure logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_thread_ids(true)
                .with_level(true)
                .compact(),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::fs::File::create(&log_file).map_err(|e| {
                    DpstreamError::Internal(format!("Failed to create log file: {e}"))
                })?)
                .with_target(true)
                .with_thread_ids(true)
                .json(),
        )
        .with(tracing_subscriber::EnvFilter::new(&log_level))
        .try_init()
        .map_err(|e| DpstreamError::Internal(format!("Failed to initialize logging: {e}")))?;

    info!("Logging initialized");
    info!("Log level: {}", log_level);
    info!("Log file: {}", log_file);

    Ok(())
}

/// Wait for SIGTERM or other termination signals
async fn wait_for_termination() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm =
            signal(SignalKind::terminate()).expect("Failed to create SIGTERM handler");
        let mut sigint = signal(SignalKind::interrupt()).expect("Failed to create SIGINT handler");

        tokio::select! {
            _ = sigterm.recv() => {
                info!("Received SIGTERM");
            }
            _ = sigint.recv() => {
                info!("Received SIGINT");
            }
        }
    }

    #[cfg(not(unix))]
    {
        // On non-Unix platforms, just wait indefinitely
        tokio::time::sleep(tokio::time::Duration::from_secs(u64::MAX)).await;
    }
}
