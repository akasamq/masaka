use core::fmt;

use alloc::string::String;

#[derive(Debug, Clone)]
pub enum ConfigError {
    /// Invalid client ID
    InvalidClientId(String),
    /// Invalid keep alive setting
    InvalidKeepAlive(String),
    /// Invalid will message
    InvalidWillMessage(String),
    /// Invalid limit setting
    InvalidLimit(String),
    /// Invalid timeout setting
    InvalidTimeout(String),
    /// Reconnect is disabled
    ReconnectDisabled,
}

impl core::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidClientId(msg) => write!(f, "Invalid client ID: {}", msg),
            Self::InvalidKeepAlive(msg) => write!(f, "Invalid keep alive: {}", msg),
            Self::InvalidWillMessage(msg) => write!(f, "Invalid will message: {}", msg),
            Self::InvalidLimit(msg) => write!(f, "Invalid limit: {}", msg),
            Self::InvalidTimeout(msg) => write!(f, "Invalid timeout: {}", msg),
            Self::ReconnectDisabled => write!(f, "Auto-reconnect is disabled in configuration"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// Connection failed
    ConnectionFailed,
    /// Connection lost
    ConnectionLost,
    /// TLS handshake failed
    TlsHandshakeFailed,
    /// Timeout
    Timeout,
    /// IO error
    Io(embedded_io::ErrorKind),
    /// Other transport error
    Other(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionFailed => write!(f, "Connection failed"),
            Self::ConnectionLost => write!(f, "Connection lost"),
            Self::TlsHandshakeFailed => write!(f, "TLS handshake failed"),
            Self::Timeout => write!(f, "Operation timeout"),
            Self::Io(kind) => write!(f, "IO error: {:?}", kind),
            Self::Other(msg) => write!(f, "Transport error: {}", msg),
        }
    }
}

impl From<embedded_io::ErrorKind> for TransportError {
    fn from(kind: embedded_io::ErrorKind) -> Self {
        Self::Io(kind)
    }
}

impl embedded_io::Error for TransportError {
    fn kind(&self) -> embedded_io::ErrorKind {
        match self {
            Self::ConnectionFailed => embedded_io::ErrorKind::Other,
            Self::ConnectionLost => embedded_io::ErrorKind::ConnectionAborted,
            Self::TlsHandshakeFailed => embedded_io::ErrorKind::Other,
            Self::Timeout => embedded_io::ErrorKind::TimedOut,
            Self::Io(kind) => *kind,
            Self::Other(_) => embedded_io::ErrorKind::Other,
        }
    }
}

#[derive(Debug, Clone)]
pub enum MqttError {
    /// Protocol error
    Protocol(String),
    /// Transport error
    Transport(TransportError),
    /// Invalid connection state
    InvalidState,
    /// Not connected
    NotConnected,
    /// Authentication failed
    AuthenticationFailed,
    /// Subscription failed
    SubscriptionFailed,
    /// Publish failed
    PublishFailed,
    /// Configuration error
    Config(ConfigError),
    /// Internal error
    Internal,
}

impl fmt::Display for MqttError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(err) => write!(f, "Protocol error: {}", err),
            Self::Transport(err) => write!(f, "Transport error: {}", err),
            Self::InvalidState => write!(f, "Invalid connection state"),
            Self::NotConnected => write!(f, "Client not connected"),
            Self::AuthenticationFailed => write!(f, "Authentication failed"),
            Self::SubscriptionFailed => write!(f, "Subscription failed"),
            Self::PublishFailed => write!(f, "Publish failed"),
            Self::Config(err) => write!(f, "Configuration error: {}", err),
            Self::Internal => write!(f, "Internal error"),
        }
    }
}

impl From<mqtt_proto::Error> for MqttError {
    fn from(err: mqtt_proto::Error) -> Self {
        Self::Protocol(err.to_string())
    }
}

impl From<mqtt_proto::v5::ErrorV5> for MqttError {
    fn from(err: mqtt_proto::v5::ErrorV5) -> Self {
        Self::Protocol(err.to_string())
    }
}

impl From<TransportError> for MqttError {
    fn from(err: TransportError) -> Self {
        Self::Transport(err)
    }
}

impl From<ConfigError> for MqttError {
    fn from(err: ConfigError) -> Self {
        Self::Config(err)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ConfigError {}

#[cfg(feature = "std")]
impl std::error::Error for MqttError {}

#[cfg(feature = "std")]
impl std::error::Error for TransportError {}
