// Centralized error handling for dpstream server
use std::fmt;
use std::time::Duration;
use thiserror::Error;

/// Main error type for dpstream server operations with enhanced error codes
#[derive(Error, Debug)]
pub enum DpstreamError {
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),

    #[error("Emulator error: {0}")]
    Emulator(#[from] EmulatorError),

    #[error("Streaming error: {0}")]
    Streaming(#[from] StreamingError),

    #[error("VPN error: {0}")]
    Vpn(#[from] VpnError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Resource exhaustion: {resource} - {details}")]
    ResourceExhaustion { resource: String, details: String },

    #[error("Hardware failure: {component} - {details}")]
    HardwareFailure { component: String, details: String },

    #[error("Memory allocation failed: requested {size} bytes")]
    MemoryAllocation { size: usize },

    #[error("Service unavailable: {service} - retry after {retry_after_ms}ms")]
    ServiceUnavailable { service: String, retry_after_ms: u64 },
}

impl DpstreamError {
    /// Get error code for programmatic handling
    pub fn error_code(&self) -> u32 {
        match self {
            Self::Network(_) => 1000,
            Self::Emulator(_) => 2000,
            Self::Streaming(_) => 3000,
            Self::Vpn(_) => 4000,
            Self::Config(_) => 5000,
            Self::Auth(_) => 6000,
            Self::Io(_) => 7000,
            Self::Serialization(_) => 8000,
            Self::Internal(_) => 9000,
            Self::ResourceExhaustion { .. } => 9100,
            Self::HardwareFailure { .. } => 9200,
            Self::MemoryAllocation { .. } => 9300,
            Self::ServiceUnavailable { .. } => 9400,
        }
    }

    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Network(net_err) => net_err.is_recoverable(),
            Self::Streaming(stream_err) => stream_err.is_recoverable(),
            Self::ServiceUnavailable { .. } => true,
            Self::ResourceExhaustion { .. } => true,
            Self::HardwareFailure { .. } => false,
            Self::MemoryAllocation { .. } => false,
            Self::Auth(_) => false,
            _ => false,
        }
    }

    /// Get recommended retry delay in milliseconds
    pub fn retry_delay_ms(&self) -> Option<u64> {
        match self {
            Self::Network(_) => Some(1000),
            Self::Streaming(_) => Some(500),
            Self::ServiceUnavailable { retry_after_ms, .. } => Some(*retry_after_ms),
            Self::ResourceExhaustion { .. } => Some(2000),
            _ => None,
        }
    }
}

/// Network-related errors
#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Socket bind error: {0}")]
    BindError(String),

    #[error("DNS resolution failed: {0}")]
    DnsResolution(String),

    #[error("Timeout: operation took longer than {timeout}ms")]
    Timeout { timeout: u64 },

    #[error("Service discovery failed: {0}")]
    Discovery(String),

    #[error("Protocol error: {0}")]
    Protocol(String),
}

impl NetworkError {
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::ConnectionFailed(_) => true,
            Self::Timeout { .. } => true,
            Self::Discovery(_) => true,
            Self::DnsResolution(_) => true,
            Self::BindError(_) => false,  // Port conflicts typically require intervention
            Self::Protocol(_) => false,   // Protocol errors usually indicate bugs
        }
    }
}

/// Emulator-related errors
#[derive(Error, Debug)]
pub enum EmulatorError {
    #[error("Dolphin executable not found at {path}")]
    ExecutableNotFound { path: String },

    #[error("Failed to start Dolphin: {reason}")]
    StartupFailed { reason: String },

    #[error("Dolphin process crashed with exit code {code}")]
    ProcessCrashed { code: i32 },

    #[error("Dolphin window not found after timeout: {timeout:?}")]
    WindowNotFound { timeout: Duration },

    #[error("ROM file not found: {path}")]
    RomNotFound { path: String },

    #[error("Dolphin startup timed out")]
    StartupTimeout,

    #[error("Process control operation '{operation}' failed: {reason}")]
    ProcessControlFailed { operation: String, reason: String },

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Streaming-related errors
#[derive(Error, Debug)]
pub enum StreamingError {
    #[error("Video encoding failed: {0}")]
    VideoEncodingFailed(String),

    #[error("Audio encoding failed: {0}")]
    AudioEncodingFailed(String),

    #[error("Capture initialization failed: {0}")]
    CaptureInitFailed(String),

    #[error("Stream setup failed: {0}")]
    StreamSetupFailed(String),

    #[error("Codec not supported: {codec}")]
    UnsupportedCodec { codec: String },

    #[error("Client disconnected: {client_id}")]
    ClientDisconnected { client_id: String },

    #[error("Bandwidth exceeded: current {current}bps, max {max}bps")]
    BandwidthExceeded { current: u64, max: u64 },

    #[error("No buffers available for processing")]
    NoBuffersAvailable,

    #[error("Hardware acceleration not available: {reason}")]
    HardwareAccelerationUnavailable { reason: String },

    #[error("Frame processing failed: {reason}")]
    FrameProcessingFailed { reason: String },
}

impl StreamingError {
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::VideoEncodingFailed(_) => true,   // Can retry with different settings
            Self::AudioEncodingFailed(_) => true,   // Can retry with different settings
            Self::CaptureInitFailed(_) => false,    // Hardware/setup issue
            Self::StreamSetupFailed(_) => false,    // Configuration issue
            Self::UnsupportedCodec { .. } => false, // Client compatibility issue
            Self::ClientDisconnected { .. } => false, // Client initiated
            Self::BandwidthExceeded { .. } => true, // Can reduce quality
            Self::NoBuffersAvailable => true,       // Temporary resource issue
            Self::HardwareAccelerationUnavailable { .. } => false, // Hardware limitation
            Self::FrameProcessingFailed { .. } => true, // Can retry
        }
    }
}

/// VPN-related errors
#[derive(Error, Debug)]
pub enum VpnError {
    #[error("Tailscale not available: {0}")]
    TailscaleNotAvailable(String),

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Network unreachable: {0}")]
    NetworkUnreachable(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Connection timeout")]
    Timeout,
}

/// Result type alias for dpstream operations
pub type Result<T> = std::result::Result<T, DpstreamError>;

/// Error context extension for better error reporting
pub trait ErrorContext<T> {
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: Into<DpstreamError>,
{
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| {
            let base_error = e.into();
            let context = f();
            DpstreamError::Internal(format!("{}: {}", context, base_error))
        })
    }
}

/// Error severity levels for logging
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Low,      // Recoverable errors, warnings
    Medium,   // Errors that affect functionality but don't crash
    High,     // Critical errors that may cause service interruption
    Critical, // Fatal errors that require immediate attention
}

impl DpstreamError {
    /// Get the severity level of this error
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            DpstreamError::Network(NetworkError::Timeout { .. }) => ErrorSeverity::Low,
            DpstreamError::Network(NetworkError::Discovery(_)) => ErrorSeverity::Medium,
            DpstreamError::Network(_) => ErrorSeverity::High,

            DpstreamError::Emulator(EmulatorError::ConfigError(_)) => ErrorSeverity::Medium,
            DpstreamError::Emulator(EmulatorError::ProcessCrashed { .. }) => ErrorSeverity::High,
            DpstreamError::Emulator(_) => ErrorSeverity::Critical,

            DpstreamError::Streaming(StreamingError::ClientDisconnected { .. }) => {
                ErrorSeverity::Low
            }
            DpstreamError::Streaming(StreamingError::BandwidthExceeded { .. }) => {
                ErrorSeverity::Medium
            }
            DpstreamError::Streaming(_) => ErrorSeverity::High,

            DpstreamError::Vpn(VpnError::Timeout) => ErrorSeverity::Medium,
            DpstreamError::Vpn(_) => ErrorSeverity::High,

            DpstreamError::Config(_) => ErrorSeverity::High,
            DpstreamError::Auth(_) => ErrorSeverity::High,
            DpstreamError::Io(_) => ErrorSeverity::Medium,
            DpstreamError::Serialization(_) => ErrorSeverity::Low,
            DpstreamError::Internal(_) => ErrorSeverity::Critical,
        }
    }

    /// Get suggested recovery actions
    pub fn recovery_suggestions(&self) -> Vec<String> {
        match self {
            DpstreamError::Network(NetworkError::Timeout { .. }) => {
                vec!["Retry the operation".to_string(), "Check network connectivity".to_string()]
            }
            DpstreamError::Network(NetworkError::Discovery(_)) => {
                vec!["Ensure Tailscale is running".to_string()]
            }
            DpstreamError::Emulator(EmulatorError::ExecutableNotFound { .. }) => {
                vec!["Install Dolphin Emulator".to_string(), "Check DOLPHIN_PATH configuration".to_string()]
            }
            DpstreamError::Vpn(VpnError::AuthFailed(_)) => {
                vec!["Check Tailscale authentication".to_string(), "Regenerate auth key".to_string()]
            }
            _ => vec!["Check logs for more details".to_string()],
        }
    }

    /// Convert to a user-friendly message
    pub fn user_message(&self) -> String {
        match self {
            DpstreamError::Network(NetworkError::Timeout { .. }) => {
                "Network operation timed out. Please check your connection.".to_string()
            }
            DpstreamError::Emulator(EmulatorError::ExecutableNotFound { .. }) => {
                "Dolphin Emulator not found. Please ensure it's installed.".to_string()
            }
            DpstreamError::Vpn(VpnError::AuthFailed(_)) => {
                "VPN authentication failed. Please check your Tailscale configuration.".to_string()
            }
            _ => format!("An error occurred: {}", self),
        }
    }
}

/// Enhanced error reporting with context
#[derive(Debug)]
pub struct ErrorReport {
    pub error: DpstreamError,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub context: Option<String>,
    pub correlation_id: Option<String>,
}

impl ErrorReport {
    pub fn new(error: DpstreamError) -> Self {
        Self {
            error,
            timestamp: chrono::Utc::now(),
            context: None,
            correlation_id: None,
        }
    }

    pub fn with_context(mut self, context: String) -> Self {
        self.context = Some(context);
        self
    }

    pub fn with_correlation_id(mut self, id: String) -> Self {
        self.correlation_id = Some(id);
        self
    }

    /// Format error for logging
    pub fn format_for_log(&self) -> String {
        let mut msg = format!("ERROR [{}] {}", self.timestamp.format("%Y-%m-%d %H:%M:%S UTC"), self.error);

        if let Some(context) = &self.context {
            msg.push_str(&format!(" | Context: {}", context));
        }

        if let Some(correlation_id) = &self.correlation_id {
            msg.push_str(&format!(" | ID: {}", correlation_id));
        }

        msg.push_str(&format!(" | Severity: {:?}", self.error.severity()));

        let suggestions = self.error.recovery_suggestions();
        if !suggestions.is_empty() {
            msg.push_str(&format!(" | Suggestions: {}", suggestions.join(", ")));
        }

        msg
    }
}

/// Macro for easy error creation with context
#[macro_export]
macro_rules! dpstream_error {
    ($err:expr) => {
        crate::error::DpstreamError::Internal($err.to_string())
    };
    ($err:expr, $context:expr) => {
        crate::error::DpstreamError::Internal(format!("{}: {}", $context, $err))
    };
}

/// Macro for creating error reports
#[macro_export]
macro_rules! error_report {
    ($err:expr) => {
        crate::error::ErrorReport::new($err)
    };
    ($err:expr, $context:expr) => {
        crate::error::ErrorReport::new($err).with_context($context.to_string())
    };
}