use std::env;
use tracing::{info, error, warn, debug};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod emulator;
mod streaming;
mod network;
mod error;

use error::{Result, DpstreamError, ErrorReport};
use network::vpn::VpnManager;

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
    // TODO: Initialize streaming server with proper error handling

    // Initialize Dolphin emulator manager
    debug!("Initializing Dolphin emulator manager...");
    // TODO: Initialize Dolphin emulator manager

    info!("Server initialization complete");
    info!("Ready to accept client connections");

    // Setup graceful shutdown
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
        _ = wait_for_termination() => {
            warn!("Received termination signal");
        }
    }

    info!("Shutting down server gracefully...");

    // TODO: Cleanup resources

    info!("Server shutdown complete");
    Ok(())
}

/// Initialize enhanced logging with structured output
fn init_logging() -> Result<()> {
    let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

    // Create a file appender for persistent logs
    let log_dir = std::path::Path::new("logs");
    if !log_dir.exists() {
        std::fs::create_dir_all(log_dir).map_err(|e| {
            DpstreamError::Internal(format!("Failed to create log directory: {}", e))
        })?;
    }

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let log_file = format!("logs/dpstream_{}.log", timestamp);

    // Configure logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_thread_ids(true)
                .with_level(true)
                .compact()
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::fs::File::create(&log_file).map_err(|e| {
                    DpstreamError::Internal(format!("Failed to create log file: {}", e))
                })?)
                .with_target(true)
                .with_thread_ids(true)
                .json()
        )
        .with(tracing_subscriber::EnvFilter::new(&log_level))
        .try_init()
        .map_err(|e| DpstreamError::Internal(format!("Failed to initialize logging: {}", e)))?;

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
        let mut sigterm = signal(SignalKind::terminate()).expect("Failed to create SIGTERM handler");
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