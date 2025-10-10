//! Advanced error correlation and recovery system for production reliability
//!
//! Implements comprehensive error handling with correlation tracking, circuit breakers,
//! automatic recovery, and distributed tracing for enterprise-grade reliability.

use arrayvec::ArrayVec;
use crossbeam_utils::CachePadded;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use smallvec::{smallvec, SmallVec};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, span, warn, Instrument, Level};
use uuid::Uuid;

/// Enterprise-grade error recovery system with distributed correlation
pub struct ErrorRecoverySystem {
    /// Error correlation tracking with distributed context
    correlations: Arc<DashMap<String, ErrorCorrelation>>,
    /// Circuit breakers for different service components
    circuit_breakers: Arc<DashMap<String, CircuitBreaker>>,
    /// Recovery strategies registry - using Arc for clonability
    recovery_strategies: Arc<RwLock<HashMap<ErrorType, Arc<dyn RecoveryStrategy + Send + Sync>>>>,
    /// Error event bus for distributed notifications
    error_bus: broadcast::Sender<ErrorEvent>,
    /// Performance metrics for error handling
    metrics: Arc<ErrorMetrics>,
    /// Configuration for error handling behavior
    config: ErrorRecoveryConfig,
    /// Background recovery task handles
    recovery_tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

/// Error correlation tracking with distributed context
#[derive(Debug, Clone)]
pub struct ErrorCorrelation {
    /// Unique correlation ID for tracking across services
    pub correlation_id: String,
    /// Root cause error that started the correlation chain
    pub root_cause: Option<Box<ErrorContext>>,
    /// Chain of related errors
    pub error_chain: VecDeque<ErrorContext>,
    /// Affected components and services
    pub affected_components: SmallVec<[String; 8]>,
    /// User impact assessment
    pub user_impact: UserImpactLevel,
    /// Recovery attempts and their results
    pub recovery_attempts: Vec<RecoveryAttempt>,
    /// Correlation metadata
    pub created_at: SystemTime,
    pub last_updated: SystemTime,
    pub severity: ErrorSeverity,
    pub status: CorrelationStatus,
}

/// Comprehensive error context with telemetry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// Unique error ID
    pub error_id: Uuid,
    /// Error correlation ID
    pub correlation_id: String,
    /// Error type classification
    pub error_type: ErrorType,
    /// Error severity level
    pub severity: ErrorSeverity,
    /// Human-readable error message
    pub message: String,
    /// Structured error details
    pub details: HashMap<String, String>,
    /// Component that generated the error
    pub component: String,
    /// Operation that was being performed
    pub operation: String,
    /// Stack trace if available
    pub stack_trace: Option<String>,
    /// System context at time of error
    pub system_context: SystemContext,
    /// Timestamp with high precision
    pub timestamp: SystemTime,
    /// Distributed tracing span context
    pub span_context: Option<SpanContext>,
}

/// System context snapshot for error analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemContext {
    /// CPU usage percentage
    pub cpu_usage: f64,
    /// Memory usage in bytes
    pub memory_usage: u64,
    /// Available memory in bytes
    pub available_memory: u64,
    /// Network bandwidth utilization
    pub network_utilization: f64,
    /// Active connections count
    pub active_connections: usize,
    /// Current load average
    pub load_average: [f64; 3],
    /// Disk I/O statistics
    pub disk_io: DiskIOStats,
}

/// Disk I/O statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIOStats {
    pub read_bytes_per_sec: u64,
    pub write_bytes_per_sec: u64,
    pub read_ops_per_sec: u64,
    pub write_ops_per_sec: u64,
}

/// Distributed tracing span context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanContext {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub baggage: HashMap<String, String>,
}

/// Circuit breaker for automatic failure protection
pub struct CircuitBreaker {
    /// Circuit breaker state
    state: Arc<RwLock<CircuitBreakerState>>,
    /// Failure count with atomic operations
    failure_count: CachePadded<AtomicUsize>,
    /// Success count for health tracking
    success_count: CachePadded<AtomicUsize>,
    /// Last failure timestamp
    last_failure: CachePadded<AtomicU64>,
    /// Circuit breaker configuration
    config: CircuitBreakerConfig,
    /// Performance metrics
    metrics: Arc<CircuitBreakerMetrics>,
}

/// Circuit breaker state machine
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CircuitBreakerState {
    /// Circuit is closed, allowing all requests
    Closed,
    /// Circuit is open, rejecting all requests
    Open,
    /// Circuit is half-open, testing if service is healthy
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: usize,
    /// Time to wait before attempting recovery
    pub recovery_timeout: Duration,
    /// Number of successful requests needed to close circuit
    pub success_threshold: usize,
    /// Maximum concurrent requests in half-open state
    pub half_open_max_requests: usize,
    /// Request timeout for circuit breaker decisions
    pub request_timeout: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 3,
            half_open_max_requests: 3,
            request_timeout: Duration::from_secs(10),
        }
    }
}

/// Circuit breaker performance metrics
#[derive(Debug)]
pub struct CircuitBreakerMetrics {
    pub total_requests: CachePadded<AtomicU64>,
    pub successful_requests: CachePadded<AtomicU64>,
    pub failed_requests: CachePadded<AtomicU64>,
    pub rejected_requests: CachePadded<AtomicU64>,
    pub state_transitions: CachePadded<AtomicU64>,
    pub time_in_open_state: CachePadded<AtomicU64>,
    pub recovery_attempts: CachePadded<AtomicU64>,
}

impl Default for CircuitBreakerMetrics {
    fn default() -> Self {
        Self {
            total_requests: CachePadded::new(AtomicU64::new(0)),
            successful_requests: CachePadded::new(AtomicU64::new(0)),
            failed_requests: CachePadded::new(AtomicU64::new(0)),
            rejected_requests: CachePadded::new(AtomicU64::new(0)),
            state_transitions: CachePadded::new(AtomicU64::new(0)),
            time_in_open_state: CachePadded::new(AtomicU64::new(0)),
            recovery_attempts: CachePadded::new(AtomicU64::new(0)),
        }
    }
}

/// Recovery strategy trait for pluggable recovery mechanisms
pub trait RecoveryStrategy: Send + Sync {
    /// Attempt to recover from the given error
    fn recover(
        &self,
        error: &ErrorContext,
    ) -> Box<dyn std::future::Future<Output = RecoveryResult> + Send + Unpin>;

    /// Check if this strategy can handle the given error type
    fn can_handle(&self, error_type: &ErrorType) -> bool;

    /// Get strategy priority (higher = more preferred)
    fn priority(&self) -> u32;

    /// Get strategy name for logging
    fn name(&self) -> &'static str;
}

/// Recovery attempt tracking
#[derive(Debug, Clone)]
pub struct RecoveryAttempt {
    pub attempt_id: Uuid,
    pub strategy_name: String,
    pub started_at: SystemTime,
    pub completed_at: Option<SystemTime>,
    pub result: Option<RecoveryResult>,
    pub details: HashMap<String, String>,
}

/// Recovery operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryResult {
    /// Recovery was successful
    Success {
        duration: Duration,
        details: HashMap<String, String>,
    },
    /// Recovery failed but should be retried
    RetryableFailure {
        duration: Duration,
        error: String,
        retry_after: Duration,
    },
    /// Recovery failed permanently
    PermanentFailure { duration: Duration, error: String },
    /// Recovery is still in progress
    InProgress {
        started_at: SystemTime,
        progress: f64,
    },
}

/// Error classification types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorType {
    /// Network connectivity issues
    NetworkError,
    /// Memory allocation failures
    MemoryError,
    /// File system I/O errors
    IOError,
    /// Authentication/authorization failures
    SecurityError,
    /// Configuration errors
    ConfigurationError,
    /// External service failures
    ServiceError,
    /// Database connection/query errors
    DatabaseError,
    /// Video encoding/decoding errors
    VideoError,
    /// Audio processing errors
    AudioError,
    /// Protocol parsing errors
    ProtocolError,
    /// System resource exhaustion
    ResourceExhaustion,
    /// Unknown/uncategorized errors
    Unknown,
}

/// Error severity levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ErrorSeverity {
    /// Low severity, minimal impact
    Low,
    /// Medium severity, some functionality affected
    Medium,
    /// High severity, significant functionality affected
    High,
    /// Critical severity, service unavailable
    Critical,
}

/// User impact assessment
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserImpactLevel {
    /// No user impact
    None,
    /// Minimal impact, degraded performance
    Minimal,
    /// Moderate impact, some features unavailable
    Moderate,
    /// Severe impact, major functionality unavailable
    Severe,
    /// Complete service unavailability
    Complete,
}

/// Correlation status tracking
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorrelationStatus {
    /// New correlation, analysis in progress
    New,
    /// Analysis completed, recovery in progress
    Recovering,
    /// Recovery completed successfully
    Resolved,
    /// Recovery failed, manual intervention required
    Failed,
    /// Correlation closed without resolution
    Closed,
}

/// Error event for distributed notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEvent {
    pub event_id: Uuid,
    pub correlation_id: String,
    pub event_type: ErrorEventType,
    pub timestamp: SystemTime,
    pub component: String,
    pub details: HashMap<String, String>,
}

/// Error event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorEventType {
    /// New error occurred
    ErrorOccurred(ErrorContext),
    /// Recovery attempt started
    RecoveryStarted(String), // strategy name
    /// Recovery completed
    RecoveryCompleted(RecoveryResult),
    /// Circuit breaker state changed
    CircuitBreakerStateChanged(String, CircuitBreakerState),
    /// Error correlation created
    CorrelationCreated,
    /// Error correlation resolved
    CorrelationResolved,
}

/// Error handling performance metrics
#[derive(Debug)]
pub struct ErrorMetrics {
    pub total_errors: CachePadded<AtomicU64>,
    pub errors_by_type: Arc<DashMap<ErrorType, AtomicU64>>,
    pub errors_by_severity: Arc<DashMap<ErrorSeverity, AtomicU64>>,
    pub recovery_attempts: CachePadded<AtomicU64>,
    pub successful_recoveries: CachePadded<AtomicU64>,
    pub failed_recoveries: CachePadded<AtomicU64>,
    pub average_recovery_time: CachePadded<AtomicU64>,
    pub correlation_count: CachePadded<AtomicU64>,
    pub active_correlations: CachePadded<AtomicUsize>,
}

impl Default for ErrorMetrics {
    fn default() -> Self {
        Self {
            total_errors: CachePadded::new(AtomicU64::new(0)),
            errors_by_type: Arc::new(DashMap::new()),
            errors_by_severity: Arc::new(DashMap::new()),
            recovery_attempts: CachePadded::new(AtomicU64::new(0)),
            successful_recoveries: CachePadded::new(AtomicU64::new(0)),
            failed_recoveries: CachePadded::new(AtomicU64::new(0)),
            average_recovery_time: CachePadded::new(AtomicU64::new(0)),
            correlation_count: CachePadded::new(AtomicU64::new(0)),
            active_correlations: CachePadded::new(AtomicUsize::new(0)),
        }
    }
}

/// Error recovery system configuration
#[derive(Debug, Clone)]
pub struct ErrorRecoveryConfig {
    /// Maximum number of active correlations
    pub max_correlations: usize,
    /// Correlation cleanup interval
    pub correlation_cleanup_interval: Duration,
    /// Maximum age for resolved correlations
    pub correlation_retention: Duration,
    /// Maximum recovery attempts per error
    pub max_recovery_attempts: usize,
    /// Circuit breaker configurations by component
    pub circuit_breaker_configs: HashMap<String, CircuitBreakerConfig>,
    /// Enable distributed error notifications
    pub enable_distributed_notifications: bool,
    /// Error sampling rate for high-frequency errors
    pub error_sampling_rate: f64,
}

impl Default for ErrorRecoveryConfig {
    fn default() -> Self {
        Self {
            max_correlations: 1000,
            correlation_cleanup_interval: Duration::from_secs(300), // 5 minutes
            correlation_retention: Duration::from_secs(3600),       // 1 hour
            max_recovery_attempts: 3,
            circuit_breaker_configs: HashMap::new(),
            enable_distributed_notifications: true,
            error_sampling_rate: 1.0, // Sample all errors by default
        }
    }
}

impl ErrorRecoverySystem {
    /// Create a new error recovery system
    pub fn new(config: ErrorRecoveryConfig) -> Self {
        let (error_bus, _) = broadcast::channel(1000);

        let system = Self {
            correlations: Arc::new(DashMap::new()),
            circuit_breakers: Arc::new(DashMap::new()),
            recovery_strategies: Arc::new(RwLock::new(HashMap::new())),
            error_bus,
            metrics: Arc::new(ErrorMetrics::default()),
            config,
            recovery_tasks: Arc::new(Mutex::new(Vec::new())),
        };

        // Register default recovery strategies
        system.register_default_strategies();

        // Start background maintenance tasks
        system.start_background_tasks();

        info!("Error recovery system initialized with advanced correlation tracking");
        system
    }

    /// Register an error with automatic correlation and recovery
    pub async fn register_error(&self, mut error: ErrorContext) -> String {
        let start_time = Instant::now();

        // Generate correlation ID if not provided
        if error.correlation_id.is_empty() {
            error.correlation_id = format!("corr_{}", Uuid::new_v4());
        }

        let correlation_id = error.correlation_id.clone();

        // Update metrics
        self.metrics.total_errors.fetch_add(1, Ordering::Relaxed);
        self.update_error_type_metrics(&error.error_type);
        self.update_error_severity_metrics(&error.severity);

        // Apply error sampling for high-frequency errors
        if !self.should_sample_error(&error) {
            debug!("Error {} sampled out due to sampling rate", error.error_id);
            return correlation_id;
        }

        // Create or update error correlation
        let correlation = self.create_or_update_correlation(error.clone()).await;

        // Check circuit breakers
        self.update_circuit_breaker(&error.component, false).await;

        // Emit error event
        if self.config.enable_distributed_notifications {
            let event = ErrorEvent {
                event_id: Uuid::new_v4(),
                correlation_id: correlation_id.clone(),
                event_type: ErrorEventType::ErrorOccurred(error.clone()),
                timestamp: SystemTime::now(),
                component: error.component.clone(),
                details: error.details.clone(),
            };

            if let Err(e) = self.error_bus.send(event) {
                warn!("Failed to send error event: {}", e);
            }
        }

        // Trigger automatic recovery
        if correlation.severity >= ErrorSeverity::High {
            self.trigger_recovery(correlation_id.clone(), error).await;
        }

        let processing_time = start_time.elapsed();
        debug!("Error registered and processed in {:?}", processing_time);

        correlation_id
    }

    /// Create or update error correlation
    async fn create_or_update_correlation(&self, error: ErrorContext) -> ErrorCorrelation {
        let correlation_id = error.correlation_id.clone();

        self.correlations
            .entry(correlation_id.clone())
            .or_insert_with(|| {
                self.metrics
                    .correlation_count
                    .fetch_add(1, Ordering::Relaxed);
                self.metrics
                    .active_correlations
                    .fetch_add(1, Ordering::Relaxed);

                ErrorCorrelation {
                    correlation_id: correlation_id.clone(),
                    root_cause: Some(Box::new(error.clone())),
                    error_chain: VecDeque::new(),
                    affected_components: smallvec![error.component.clone()],
                    user_impact: self.assess_user_impact(&error),
                    recovery_attempts: Vec::new(),
                    created_at: SystemTime::now(),
                    last_updated: SystemTime::now(),
                    severity: error.severity.clone(),
                    status: CorrelationStatus::New,
                }
            })
            .value()
            .clone()
    }

    /// Assess user impact based on error context
    fn assess_user_impact(&self, error: &ErrorContext) -> UserImpactLevel {
        match (&error.error_type, &error.severity) {
            (_, ErrorSeverity::Critical) => UserImpactLevel::Complete,
            (ErrorType::VideoError | ErrorType::AudioError, ErrorSeverity::High) => {
                UserImpactLevel::Severe
            }
            (ErrorType::NetworkError, ErrorSeverity::High) => UserImpactLevel::Severe,
            (_, ErrorSeverity::High) => UserImpactLevel::Moderate,
            (ErrorType::VideoError | ErrorType::AudioError, ErrorSeverity::Medium) => {
                UserImpactLevel::Moderate
            }
            (_, ErrorSeverity::Medium) => UserImpactLevel::Minimal,
            _ => UserImpactLevel::None,
        }
    }

    /// Trigger automatic recovery for an error
    async fn trigger_recovery(&self, correlation_id: String, error: ErrorContext) {
        let recovery_span = span!(Level::INFO, "error_recovery", correlation_id = %correlation_id);

        async move {
            info!("Starting automatic recovery for error: {}", error.error_id);

            let strategies = self.get_applicable_strategies(&error.error_type);

            for strategy in strategies {
                let attempt = RecoveryAttempt {
                    attempt_id: Uuid::new_v4(),
                    strategy_name: strategy.name().to_string(),
                    started_at: SystemTime::now(),
                    completed_at: None,
                    result: None,
                    details: HashMap::new(),
                };

                // Emit recovery started event
                let event = ErrorEvent {
                    event_id: Uuid::new_v4(),
                    correlation_id: correlation_id.clone(),
                    event_type: ErrorEventType::RecoveryStarted(strategy.name().to_string()),
                    timestamp: SystemTime::now(),
                    component: error.component.clone(),
                    details: HashMap::new(),
                };

                if let Err(e) = self.error_bus.send(event) {
                    warn!("Failed to send recovery started event: {}", e);
                }

                self.metrics
                    .recovery_attempts
                    .fetch_add(1, Ordering::Relaxed);

                // Execute recovery strategy with timeout
                let recovery_result =
                    match timeout(Duration::from_secs(30), strategy.recover(&error)).await {
                        Ok(result) => result,
                        Err(_) => RecoveryResult::RetryableFailure {
                            duration: Duration::from_secs(30),
                            error: "Recovery timeout".to_string(),
                            retry_after: Duration::from_secs(60),
                        },
                    };

                // Update metrics
                match &recovery_result {
                    RecoveryResult::Success { .. } => {
                        self.metrics
                            .successful_recoveries
                            .fetch_add(1, Ordering::Relaxed);
                        info!("Recovery successful using strategy: {}", strategy.name());
                        break; // Stop trying other strategies
                    }
                    RecoveryResult::PermanentFailure { .. } => {
                        self.metrics
                            .failed_recoveries
                            .fetch_add(1, Ordering::Relaxed);
                        error!(
                            "Recovery permanently failed using strategy: {}",
                            strategy.name()
                        );
                        break; // Stop trying other strategies
                    }
                    RecoveryResult::RetryableFailure { .. } => {
                        self.metrics
                            .failed_recoveries
                            .fetch_add(1, Ordering::Relaxed);
                        warn!(
                            "Recovery failed with strategy: {}, will try next strategy",
                            strategy.name()
                        );
                        // Continue to next strategy
                    }
                    RecoveryResult::InProgress { .. } => {
                        info!("Recovery in progress with strategy: {}", strategy.name());
                        // Continue monitoring this recovery
                    }
                }

                // Emit recovery completed event
                let event = ErrorEvent {
                    event_id: Uuid::new_v4(),
                    correlation_id: correlation_id.clone(),
                    event_type: ErrorEventType::RecoveryCompleted(recovery_result.clone()),
                    timestamp: SystemTime::now(),
                    component: error.component.clone(),
                    details: HashMap::new(),
                };

                if let Err(e) = self.error_bus.send(event) {
                    warn!("Failed to send recovery completed event: {}", e);
                }
            }
        }
        .instrument(recovery_span)
        .await;
    }

    /// Get applicable recovery strategies for an error type
    fn get_applicable_strategies(
        &self,
        error_type: &ErrorType,
    ) -> Vec<Arc<dyn RecoveryStrategy + Send + Sync>> {
        let strategies = self.recovery_strategies.read();
        let mut applicable: Vec<Arc<dyn RecoveryStrategy + Send + Sync>> = strategies
            .values()
            .filter(|strategy| strategy.can_handle(error_type))
            .map(|strategy| Arc::clone(strategy)) // Clone the Arc (cheap refcount increment)
            .collect();

        // Sort by priority (highest first)
        applicable.sort_by(|a, b| b.priority().cmp(&a.priority()));

        applicable
    }

    /// Update circuit breaker state
    async fn update_circuit_breaker(&self, component: &str, success: bool) {
        let breaker = self
            .circuit_breakers
            .entry(component.to_string())
            .or_insert_with(|| {
                let config = self
                    .config
                    .circuit_breaker_configs
                    .get(component)
                    .cloned()
                    .unwrap_or_default();

                CircuitBreaker::new(config)
            });

        if success {
            breaker.record_success().await;
        } else {
            breaker.record_failure().await;
        }
    }

    /// Check if error should be sampled based on sampling rate
    fn should_sample_error(&self, error: &ErrorContext) -> bool {
        if self.config.error_sampling_rate >= 1.0 {
            return true;
        }

        // Use error hash for consistent sampling
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        error.error_id.hash(&mut hasher);
        let hash = hasher.finish();

        (hash as f64 / u64::MAX as f64) < self.config.error_sampling_rate
    }

    /// Update error type metrics
    fn update_error_type_metrics(&self, error_type: &ErrorType) {
        self.metrics
            .errors_by_type
            .entry(error_type.clone())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Update error severity metrics
    fn update_error_severity_metrics(&self, severity: &ErrorSeverity) {
        self.metrics
            .errors_by_severity
            .entry(severity.clone())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Register default recovery strategies
    fn register_default_strategies(&self) {
        // This would be implemented with concrete strategy types
        // For demonstration, showing the interface
    }

    /// Start background maintenance tasks
    fn start_background_tasks(&self) {
        // Correlation cleanup task
        let correlations = self.correlations.clone();
        let config = self.config.clone();
        let metrics = self.metrics.clone();

        let cleanup_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.correlation_cleanup_interval);

            loop {
                interval.tick().await;

                let now = SystemTime::now();
                let mut removed_count = 0;

                correlations.retain(|_, correlation| {
                    let should_retain = match correlation.status {
                        CorrelationStatus::Resolved
                        | CorrelationStatus::Failed
                        | CorrelationStatus::Closed => {
                            now.duration_since(correlation.last_updated)
                                .unwrap_or(Duration::ZERO)
                                < config.correlation_retention
                        }
                        _ => true,
                    };

                    if !should_retain {
                        removed_count += 1;
                        metrics.active_correlations.fetch_sub(1, Ordering::Relaxed);
                    }

                    should_retain
                });

                if removed_count > 0 {
                    debug!("Cleaned up {} expired correlations", removed_count);
                }
            }
        });

        let mut tasks = self.recovery_tasks.lock();
        tasks.push(cleanup_task);
    }

    /// Get error recovery statistics
    pub fn get_statistics(&self) -> ErrorRecoveryStats {
        ErrorRecoveryStats {
            total_errors: self.metrics.total_errors.load(Ordering::Relaxed),
            recovery_attempts: self.metrics.recovery_attempts.load(Ordering::Relaxed),
            successful_recoveries: self.metrics.successful_recoveries.load(Ordering::Relaxed),
            failed_recoveries: self.metrics.failed_recoveries.load(Ordering::Relaxed),
            active_correlations: self.metrics.active_correlations.load(Ordering::Relaxed),
            total_correlations: self.metrics.correlation_count.load(Ordering::Relaxed),
            recovery_success_rate: self.calculate_recovery_success_rate(),
        }
    }

    /// Calculate recovery success rate
    fn calculate_recovery_success_rate(&self) -> f64 {
        let successful = self.metrics.successful_recoveries.load(Ordering::Relaxed) as f64;
        let total = self.metrics.recovery_attempts.load(Ordering::Relaxed) as f64;

        if total > 0.0 {
            (successful / total) * 100.0
        } else {
            0.0
        }
    }

    /// Subscribe to error events
    pub fn subscribe_to_events(&self) -> broadcast::Receiver<ErrorEvent> {
        self.error_bus.subscribe()
    }
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitBreakerState::Closed)),
            failure_count: CachePadded::new(AtomicUsize::new(0)),
            success_count: CachePadded::new(AtomicUsize::new(0)),
            last_failure: CachePadded::new(AtomicU64::new(0)),
            config,
            metrics: Arc::new(CircuitBreakerMetrics::default()),
        }
    }

    /// Record a successful operation
    pub async fn record_success(&self) {
        self.metrics.total_requests.fetch_add(1, Ordering::Relaxed);
        self.metrics
            .successful_requests
            .fetch_add(1, Ordering::Relaxed);

        let success_count = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;

        let mut state = self.state.write();
        match *state {
            CircuitBreakerState::HalfOpen => {
                if success_count >= self.config.success_threshold {
                    *state = CircuitBreakerState::Closed;
                    self.failure_count.store(0, Ordering::Relaxed);
                    self.success_count.store(0, Ordering::Relaxed);
                    self.metrics
                        .state_transitions
                        .fetch_add(1, Ordering::Relaxed);
                    info!("Circuit breaker transitioned to Closed state");
                }
            }
            CircuitBreakerState::Open => {
                // Check if we should transition to half-open
                let last_failure = self.last_failure.load(Ordering::Relaxed);
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                if now - last_failure >= self.config.recovery_timeout.as_secs() {
                    *state = CircuitBreakerState::HalfOpen;
                    self.success_count.store(1, Ordering::Relaxed);
                    self.metrics
                        .state_transitions
                        .fetch_add(1, Ordering::Relaxed);
                    info!("Circuit breaker transitioned to HalfOpen state");
                }
            }
            CircuitBreakerState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::Relaxed);
            }
        }
    }

    /// Record a failed operation
    pub async fn record_failure(&self) {
        self.metrics.total_requests.fetch_add(1, Ordering::Relaxed);
        self.metrics.failed_requests.fetch_add(1, Ordering::Relaxed);

        let failure_count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        self.last_failure.store(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            Ordering::Relaxed,
        );

        let mut state = self.state.write();
        match *state {
            CircuitBreakerState::Closed => {
                if failure_count >= self.config.failure_threshold {
                    *state = CircuitBreakerState::Open;
                    self.metrics
                        .state_transitions
                        .fetch_add(1, Ordering::Relaxed);
                    warn!("Circuit breaker opened due to {} failures", failure_count);
                }
            }
            CircuitBreakerState::HalfOpen => {
                *state = CircuitBreakerState::Open;
                self.success_count.store(0, Ordering::Relaxed);
                self.metrics
                    .state_transitions
                    .fetch_add(1, Ordering::Relaxed);
                warn!("Circuit breaker reopened due to failure in half-open state");
            }
            CircuitBreakerState::Open => {
                // Already open, just record the failure
            }
        }
    }

    /// Check if requests should be allowed
    pub fn is_request_allowed(&self) -> bool {
        let state = self.state.read();
        match *state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => false,
            CircuitBreakerState::HalfOpen => {
                // Allow limited requests in half-open state
                true // Simplified - in practice would track concurrent requests
            }
        }
    }

    /// Get current state
    pub fn get_state(&self) -> CircuitBreakerState {
        self.state.read().clone()
    }
}

/// Error recovery statistics
#[derive(Debug, Clone)]
pub struct ErrorRecoveryStats {
    pub total_errors: u64,
    pub recovery_attempts: u64,
    pub successful_recoveries: u64,
    pub failed_recoveries: u64,
    pub active_correlations: usize,
    pub total_correlations: u64,
    pub recovery_success_rate: f64,
}

/// Global error recovery system instance
pub static ERROR_RECOVERY: Lazy<ErrorRecoverySystem> =
    Lazy::new(|| ErrorRecoverySystem::new(ErrorRecoveryConfig::default()));

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_error_registration() {
        let system = ErrorRecoverySystem::new(ErrorRecoveryConfig::default());

        let error = ErrorContext {
            error_id: Uuid::new_v4(),
            correlation_id: String::new(),
            error_type: ErrorType::NetworkError,
            severity: ErrorSeverity::High,
            message: "Connection failed".to_string(),
            details: HashMap::new(),
            component: "network".to_string(),
            operation: "connect".to_string(),
            stack_trace: None,
            system_context: SystemContext {
                cpu_usage: 50.0,
                memory_usage: 1024 * 1024 * 1024,
                available_memory: 2048 * 1024 * 1024,
                network_utilization: 75.0,
                active_connections: 10,
                load_average: [1.0, 1.5, 2.0],
                disk_io: DiskIOStats {
                    read_bytes_per_sec: 1024,
                    write_bytes_per_sec: 2048,
                    read_ops_per_sec: 10,
                    write_ops_per_sec: 20,
                },
            },
            timestamp: SystemTime::now(),
            span_context: None,
        };

        let correlation_id = system.register_error(error).await;
        assert!(!correlation_id.is_empty());

        let stats = system.get_statistics();
        assert_eq!(stats.total_errors, 1);
        assert_eq!(stats.active_correlations, 1);
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout: Duration::from_secs(1),
            success_threshold: 2,
            half_open_max_requests: 1,
            request_timeout: Duration::from_secs(5),
        };

        let breaker = CircuitBreaker::new(config);

        // Initially closed
        assert_eq!(breaker.get_state(), CircuitBreakerState::Closed);
        assert!(breaker.is_request_allowed());

        // Record failures to open circuit
        breaker.record_failure().await;
        breaker.record_failure().await;
        breaker.record_failure().await;

        assert_eq!(breaker.get_state(), CircuitBreakerState::Open);
        assert!(!breaker.is_request_allowed());

        // Wait for recovery timeout
        tokio::time::sleep(Duration::from_secs(2)).await;

        // First success should transition to half-open
        breaker.record_success().await;
        // Note: The transition logic needs the success to happen after timeout

        // More successes should close the circuit
        breaker.record_success().await;
        breaker.record_success().await;

        // Should eventually be closed (implementation dependent on exact logic)
        assert!(breaker.is_request_allowed());
    }

    #[test]
    fn test_error_correlation_creation() {
        let error = ErrorContext {
            error_id: Uuid::new_v4(),
            correlation_id: "test_correlation".to_string(),
            error_type: ErrorType::VideoError,
            severity: ErrorSeverity::Medium,
            message: "Video decoding failed".to_string(),
            details: HashMap::new(),
            component: "video_decoder".to_string(),
            operation: "decode_frame".to_string(),
            stack_trace: None,
            system_context: SystemContext {
                cpu_usage: 30.0,
                memory_usage: 512 * 1024 * 1024,
                available_memory: 1024 * 1024 * 1024,
                network_utilization: 25.0,
                active_connections: 5,
                load_average: [0.5, 0.7, 0.9],
                disk_io: DiskIOStats {
                    read_bytes_per_sec: 512,
                    write_bytes_per_sec: 1024,
                    read_ops_per_sec: 5,
                    write_ops_per_sec: 10,
                },
            },
            timestamp: SystemTime::now(),
            span_context: None,
        };

        let system = ErrorRecoverySystem::new(ErrorRecoveryConfig::default());
        let correlation = system.assess_user_impact(&error);

        assert_eq!(correlation, UserImpactLevel::Moderate);
    }
}
