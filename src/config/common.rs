/// Transport layer configuration.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Connection timeout (ms).
    pub connect_timeout_ms: u32,
    /// Read timeout (ms).
    pub read_timeout_ms: u32,
    /// Write timeout (ms).
    pub write_timeout_ms: u32,
    /// Enables TCP Keep-Alive.
    pub tcp_keepalive: bool,
    /// TCP Keep-Alive interval (seconds).
    pub tcp_keepalive_interval_secs: u16,
    /// Number of TCP Keep-Alive probes.
    pub tcp_keepalive_probes: u8,
    /// Read buffer size (bytes).
    pub read_buffer_size: usize,
    /// Write buffer size (bytes).
    pub write_buffer_size: usize,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            connect_timeout_ms: 30_000,
            read_timeout_ms: 30_000,
            write_timeout_ms: 30_000,
            tcp_keepalive: true,
            tcp_keepalive_interval_secs: 60,
            tcp_keepalive_probes: 3,
            read_buffer_size: 1024,
            write_buffer_size: 1024,
        }
    }
}

impl TransportConfig {
    /// Creates a configuration for low-latency scenarios.
    pub fn low_latency() -> Self {
        Self {
            connect_timeout_ms: 5_000,
            read_timeout_ms: 5_000,
            write_timeout_ms: 5_000,
            tcp_keepalive: true,
            tcp_keepalive_interval_secs: 30,
            tcp_keepalive_probes: 3,
            read_buffer_size: 512,
            write_buffer_size: 512,
        }
    }

    /// Creates a configuration for high-throughput scenarios.
    pub fn high_throughput() -> Self {
        Self {
            connect_timeout_ms: 60_000,
            read_timeout_ms: 60_000,
            write_timeout_ms: 60_000,
            tcp_keepalive: true,
            tcp_keepalive_interval_secs: 120,
            tcp_keepalive_probes: 5,
            read_buffer_size: 4096,
            write_buffer_size: 4096,
        }
    }
}

/// Automatic reconnect configuration.
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Enable automatic reconnect.
    pub enabled: bool,
    /// Maximum number of reconnect attempts. `0` means unlimited.
    pub max_attempts: u32,
    /// Initial reconnect interval (ms).
    pub initial_interval_ms: u32,
    /// Maximum reconnect interval (ms).
    pub max_interval_ms: u32,
    /// Multiplier for exponential backoff.
    pub backoff_multiplier: f32,
    /// Enable jitter to avoid thundering herd problem.
    pub enable_jitter: bool,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_attempts: 0, // unlimited
            initial_interval_ms: 1_000,
            max_interval_ms: 60_000,
            backoff_multiplier: 2.0,
            enable_jitter: true,
        }
    }
}

impl ReconnectConfig {
    /// Disables automatic reconnect.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Creates a reconnect configuration with a fixed interval.
    pub fn simple(interval_ms: u32, max_attempts: u32) -> Self {
        Self {
            enabled: true,
            max_attempts,
            initial_interval_ms: interval_ms,
            max_interval_ms: interval_ms,
            backoff_multiplier: 1.0,
            enable_jitter: false,
        }
    }

    /// Creates a reconnect configuration with exponential backoff.
    pub fn exponential_backoff(
        initial_ms: u32,
        max_ms: u32,
        multiplier: f32,
        max_attempts: u32,
    ) -> Self {
        Self {
            enabled: true,
            max_attempts,
            initial_interval_ms: initial_ms,
            max_interval_ms: max_ms,
            backoff_multiplier: multiplier,
            enable_jitter: true,
        }
    }
}
