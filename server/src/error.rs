// Centralized error handling for dpstream server
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

    #[error("Input error: {0}")]
    Input(#[from] InputError),

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
    ServiceUnavailable {
        service: String,
        retry_after_ms: u64,
    },
}

impl DpstreamError {
    /// Get error code for programmatic handling
    pub fn error_code(&self) -> u32 {
        match self {
            Self::Network(_) => 1000,
            Self::Emulator(_) => 2000,
            Self::Streaming(_) => 3000,
            Self::Input(_) => 3500,
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
            Self::Input(_) => true, // Most input errors are recoverable
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

    /// Get error severity level for monitoring and alerting
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::HardwareFailure { .. } | Self::MemoryAllocation { .. } => ErrorSeverity::Critical,
            Self::Auth(_) | Self::Config(_) => ErrorSeverity::High,
            Self::Network(_) | Self::Streaming(_) | Self::ServiceUnavailable { .. } => {
                ErrorSeverity::Medium
            }
            Self::Input(_) | Self::Serialization(_) => ErrorSeverity::Low,
            _ => ErrorSeverity::Medium,
        }
    }
}

/// Error severity levels for monitoring and alerting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorSeverity {
    Low,      // Recoverable, minimal impact
    Medium,   // May impact performance
    High,     // Significant impact on functionality
    Critical, // System failure, immediate attention required
}
/// Enhanced error reporting with context and correlation tracking
#[derive(Debug)]
pub struct ErrorReport {
    pub error: DpstreamError,
    pub context: Vec<String>,
    pub correlation_id: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub component: Option<String>,
    pub retry_count: u32,
}

impl ErrorReport {
    pub fn new(error: DpstreamError) -> Self {
        Self {
            error,
            context: Vec::new(),
            correlation_id: None,
            timestamp: chrono::Utc::now(),
            component: None,
            retry_count: 0,
        }
    }

    pub fn with_context(mut self, context: String) -> Self {
        self.context.push(context);
        self
    }

    pub fn with_correlation_id(mut self, correlation_id: String) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    pub fn with_component(mut self, component: String) -> Self {
        self.component = Some(component);
        self
    }

    pub fn with_retry_count(mut self, retry_count: u32) -> Self {
        self.retry_count = retry_count;
        self
    }

    /// Format error for structured logging
    pub fn format_for_log(&self) -> String {
        let context_str = if self.context.is_empty() {
            String::new()
        } else {
            format!(" | Context: {}", self.context.join(" -> "))
        };

        let correlation_str = if let Some(ref id) = self.correlation_id {
            format!(" | Correlation: {}", id)
        } else {
            String::new()
        };

        let component_str = if let Some(ref comp) = self.component {
            format!(" | Component: {}", comp)
        } else {
            String::new()
        };

        let retry_str = if self.retry_count > 0 {
            format!(" | Retry: {}", self.retry_count)
        } else {
            String::new()
        };

        format!(
            "[{}] {} (Code: {}, Severity: {:?}){}{}{}{}",
            self.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            self.error,
            self.error.error_code(),
            self.error.severity(),
            context_str,
            correlation_str,
            component_str,
            retry_str
        )
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
            Self::BindError(_) => false, // Port conflicts typically require intervention
            Self::Protocol(_) => false,  // Protocol errors usually indicate bugs
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

/// Input-related errors
#[derive(Error, Debug)]
pub enum InputError {
    #[error("Input initialization failed: {reason}")]
    InitializationFailed { reason: String },

    #[error("Invalid player slot: {player} (must be 1-4)")]
    InvalidPlayer { player: u8 },

    #[error("Controller not connected for player {player}")]
    ControllerNotConnected { player: u8 },

    #[error("Input adapter not initialized")]
    AdapterNotInitialized,

    #[error("Command send failed: {reason}")]
    CommandSendFailed { reason: String },

    #[error("Configuration error in {field} with value '{value}': {reason}")]
    ConfigurationError {
        field: String,
        value: String,
        reason: String,
    },

    #[error("Calibration failed: {reason}")]
    CalibrationFailed { reason: String },

    #[error("Input mapping error: {reason}")]
    MappingError { reason: String },
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

    #[error("Initialization failed for {component}: {reason}")]
    InitializationFailed { component: String, reason: String },

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

    #[error("Pipeline error in {operation}: {reason}")]
    PipelineError { operation: String, reason: String },

    #[error("Encoder not available: {encoder} - {reason}")]
    EncoderNotAvailable { encoder: String, reason: String },

    #[error("Invalid packet data")]
    InvalidPacket,

    #[error("Configuration error in {field}: {reason}")]
    ConfigurationError { field: String, reason: String },

    #[error("Capture start failed: {reason}")]
    CaptureStartFailed { reason: String },

    #[error("Capture stop failed: {reason}")]
    CaptureStopFailed { reason: String },
}

impl StreamingError {
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::VideoEncodingFailed(_) => true, // Can retry with different settings
            Self::AudioEncodingFailed(_) => true, // Can retry with different settings
            Self::CaptureInitFailed(_) => false,  // Hardware/setup issue
            Self::InitializationFailed { .. } => false, // Setup/configuration issue
            Self::StreamSetupFailed(_) => false,  // Configuration issue
            Self::UnsupportedCodec { .. } => false, // Client compatibility issue
            Self::ClientDisconnected { .. } => false, // Client initiated
            Self::BandwidthExceeded { .. } => true, // Can reduce quality
            Self::NoBuffersAvailable => true,     // Temporary resource issue
            Self::HardwareAccelerationUnavailable { .. } => false, // Hardware limitation
            Self::FrameProcessingFailed { .. } => true, // Can retry
            Self::PipelineError { .. } => false,  // Pipeline configuration issue
            Self::EncoderNotAvailable { .. } => false, // Hardware/driver issue
            Self::InvalidPacket => true,          // Data corruption, can retry
            Self::ConfigurationError { .. } => false, // Configuration issue
            Self::CaptureStartFailed { .. } => false, // Setup issue
            Self::CaptureStopFailed { .. } => true, // Can force stop
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

impl DpstreamError {
    /// Get suggested recovery actions
    pub fn recovery_suggestions(&self) -> Vec<String> {
        match self {
            DpstreamError::Network(NetworkError::Timeout { .. }) => {
                vec![
                    "Retry the operation".to_string(),
                    "Check network connectivity".to_string(),
                ]
            }
            DpstreamError::Network(NetworkError::Discovery(_)) => {
                vec!["Ensure Tailscale is running".to_string()]
            }
            DpstreamError::Emulator(EmulatorError::ExecutableNotFound { .. }) => {
                vec![
                    "Install Dolphin Emulator".to_string(),
                    "Check DOLPHIN_PATH configuration".to_string(),
                ]
            }
            DpstreamError::Vpn(VpnError::AuthFailed(_)) => {
                vec![
                    "Check Tailscale authentication".to_string(),
                    "Regenerate auth key".to_string(),
                ]
            }
            DpstreamError::Input(_) => {
                vec![
                    "Check controller connection".to_string(),
                    "Verify input configuration".to_string(),
                ]
            }
            DpstreamError::ResourceExhaustion { .. } => {
                vec![
                    "Free up system resources".to_string(),
                    "Close unused applications".to_string(),
                ]
            }
            DpstreamError::HardwareFailure { .. } => {
                vec![
                    "Check hardware connections".to_string(),
                    "Contact system administrator".to_string(),
                ]
            }
            DpstreamError::MemoryAllocation { .. } => {
                vec![
                    "Free system memory".to_string(),
                    "Restart the application".to_string(),
                ]
            }
            DpstreamError::ServiceUnavailable { .. } => {
                vec![
                    "Wait for service to become available".to_string(),
                    "Check service status".to_string(),
                ]
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
            DpstreamError::Input(_) => {
                "Controller input error. Please check your input device connections.".to_string()
            }
            DpstreamError::ResourceExhaustion { .. } => {
                "System resources exhausted. Please free up memory or CPU.".to_string()
            }
            DpstreamError::HardwareFailure { .. } => {
                "Hardware failure detected. Please check your system hardware.".to_string()
            }
            DpstreamError::MemoryAllocation { .. } => {
                "Memory allocation failed. System may be low on memory.".to_string()
            }
            DpstreamError::ServiceUnavailable { .. } => {
                "Service temporarily unavailable. Please try again later.".to_string()
            }
            _ => format!("An error occurred: {}", self),
        }
    }
}

/// Macro for easy error creation with context
#[macro_export]
macro_rules! dpstream_error {
    ($err:expr) => {
        $crate::error::DpstreamError::Internal($err.to_string())
    };
    ($err:expr, $context:expr) => {
        $crate::error::DpstreamError::Internal(format!("{}: {}", $context, $err))
    };
}

/// Macro for creating error reports
#[macro_export]
macro_rules! error_report {
    ($err:expr) => {
        $crate::error::ErrorReport::new($err)
    };
    ($err:expr, $context:expr) => {
        $crate::error::ErrorReport::new($err).with_context($context.to_string())
    };
}
