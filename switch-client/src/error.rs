//! Error handling for Switch client
//!
//! No-std compatible error types for the dpstream Switch client

use core::fmt;

/// Result type alias for client operations
pub type Result<T> = core::result::Result<T, ClientError>;

/// Main error type for client operations
#[derive(Debug, Clone)]
pub enum ClientError {
    /// System-level errors (libnx, memory, etc.)
    System(SystemError),
    /// Network and communication errors
    Network(NetworkError),
    /// Moonlight protocol errors
    Moonlight(MoonlightError),
    /// Display and rendering errors
    Display(DisplayError),
    /// Input handling errors
    Input(InputError),
    /// Memory allocation errors
    Memory(MemoryError),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::System(e) => write!(f, "System error: {}", e),
            ClientError::Network(e) => write!(f, "Network error: {}", e),
            ClientError::Moonlight(e) => write!(f, "Moonlight error: {}", e),
            ClientError::Display(e) => write!(f, "Display error: {}", e),
            ClientError::Input(e) => write!(f, "Input error: {}", e),
            ClientError::Memory(e) => write!(f, "Memory error: {}", e),
        }
    }
}

/// System-level errors
#[derive(Debug, Clone)]
pub enum SystemError {
    InitializationFailed,
    ServiceUnavailable(&'static str),
    LibnxError(i32),
    InvalidState,
}

impl fmt::Display for SystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SystemError::InitializationFailed => write!(f, "System initialization failed"),
            SystemError::ServiceUnavailable(service) => write!(f, "Service unavailable: {}", service),
            SystemError::LibnxError(code) => write!(f, "Libnx error: {}", code),
            SystemError::InvalidState => write!(f, "Invalid system state"),
        }
    }
}

/// Network communication errors
#[derive(Debug, Clone)]
pub enum NetworkError {
    ConnectionFailed,
    Timeout,
    InvalidAddress,
    ProtocolError,
    TlsError,
    ServerUnreachable,
    PacketLoss,
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkError::ConnectionFailed => write!(f, "Connection failed"),
            NetworkError::Timeout => write!(f, "Network timeout"),
            NetworkError::InvalidAddress => write!(f, "Invalid network address"),
            NetworkError::ProtocolError => write!(f, "Protocol error"),
            NetworkError::TlsError => write!(f, "TLS/encryption error"),
            NetworkError::ServerUnreachable => write!(f, "Server unreachable"),
            NetworkError::PacketLoss => write!(f, "Packet loss detected"),
        }
    }
}

/// Moonlight protocol errors
#[derive(Debug, Clone)]
pub enum MoonlightError {
    HandshakeFailed,
    AuthenticationFailed,
    UnsupportedCodec,
    StreamingError,
    DecodingError,
    ServerIncompatible,
    SessionTimeout,
}

impl fmt::Display for MoonlightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MoonlightError::HandshakeFailed => write!(f, "Moonlight handshake failed"),
            MoonlightError::AuthenticationFailed => write!(f, "Authentication failed"),
            MoonlightError::UnsupportedCodec => write!(f, "Unsupported video codec"),
            MoonlightError::StreamingError => write!(f, "Streaming error"),
            MoonlightError::DecodingError => write!(f, "Video decoding error"),
            MoonlightError::ServerIncompatible => write!(f, "Server incompatible"),
            MoonlightError::SessionTimeout => write!(f, "Session timeout"),
        }
    }
}

/// Display and rendering errors
#[derive(Debug, Clone)]
pub enum DisplayError {
    InitializationFailed,
    RenderingFailed,
    UnsupportedFormat,
    BufferOverflow,
    SurfaceError,
    FramebufferError,
}

impl fmt::Display for DisplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DisplayError::InitializationFailed => write!(f, "Display initialization failed"),
            DisplayError::RenderingFailed => write!(f, "Rendering failed"),
            DisplayError::UnsupportedFormat => write!(f, "Unsupported video format"),
            DisplayError::BufferOverflow => write!(f, "Display buffer overflow"),
            DisplayError::SurfaceError => write!(f, "Surface error"),
            DisplayError::FramebufferError => write!(f, "Framebuffer error"),
        }
    }
}

/// Input handling errors
#[derive(Debug, Clone)]
pub enum InputError {
    InitializationFailed,
    ControllerDisconnected,
    InvalidInputData,
    CalibrationError,
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InputError::InitializationFailed => write!(f, "Input initialization failed"),
            InputError::ControllerDisconnected => write!(f, "Controller disconnected"),
            InputError::InvalidInputData => write!(f, "Invalid input data"),
            InputError::CalibrationError => write!(f, "Controller calibration error"),
        }
    }
}

/// Memory allocation errors
#[derive(Debug, Clone)]
pub enum MemoryError {
    OutOfMemory,
    AllocationFailed,
    InvalidSize,
    FragmentationError,
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryError::OutOfMemory => write!(f, "Out of memory"),
            MemoryError::AllocationFailed => write!(f, "Memory allocation failed"),
            MemoryError::InvalidSize => write!(f, "Invalid allocation size"),
            MemoryError::FragmentationError => write!(f, "Memory fragmentation error"),
        }
    }
}

// Error conversions
impl From<SystemError> for ClientError {
    fn from(err: SystemError) -> Self {
        ClientError::System(err)
    }
}

impl From<NetworkError> for ClientError {
    fn from(err: NetworkError) -> Self {
        ClientError::Network(err)
    }
}

impl From<MoonlightError> for ClientError {
    fn from(err: MoonlightError) -> Self {
        ClientError::Moonlight(err)
    }
}

impl From<DisplayError> for ClientError {
    fn from(err: DisplayError) -> Self {
        ClientError::Display(err)
    }
}

impl From<InputError> for ClientError {
    fn from(err: InputError) -> Self {
        ClientError::Input(err)
    }
}

impl From<MemoryError> for ClientError {
    fn from(err: MemoryError) -> Self {
        ClientError::Memory(err)
    }
}

/// Helper macro for easy error creation
#[macro_export]
macro_rules! client_error {
    (system, $variant:ident) => {
        $crate::error::ClientError::System($crate::error::SystemError::$variant)
    };
    (network, $variant:ident) => {
        $crate::error::ClientError::Network($crate::error::NetworkError::$variant)
    };
    (moonlight, $variant:ident) => {
        $crate::error::ClientError::Moonlight($crate::error::MoonlightError::$variant)
    };
    (display, $variant:ident) => {
        $crate::error::ClientError::Display($crate::error::DisplayError::$variant)
    };
    (input, $variant:ident) => {
        $crate::error::ClientError::Input($crate::error::InputError::$variant)
    };
    (memory, $variant:ident) => {
        $crate::error::ClientError::Memory($crate::error::MemoryError::$variant)
    };
}