use core::fmt;

use alloc::string::ToString;

mod config;
mod protocol;
mod session;
mod transport;

pub use config::ConfigError;
pub use protocol::ProtocolError;
pub use session::SessionError;
pub use transport::TransportError;

/// Top-level MQTT error.
#[derive(Debug, Clone)]
pub enum MqttError {
    /// Configuration validation error.
    Config(ConfigError),
    /// Transport layer error.
    Transport(TransportError),
    /// Protocol-related error.
    Protocol(ProtocolError),
    /// Session state error.
    Session(SessionError),
    /// Invalid connection state.
    InvalidState,
    /// Client is not connected to the broker.
    NotConnected,
    /// Authentication failed.
    AuthenticationFailed,
    /// Subscription operation failed.
    SubscriptionFailed,
    /// Publish operation failed.
    PublishFailed,
    /// Flow control limit exceeded.
    FlowControl,
    /// Operation timed out.
    Timeout,
    /// Internal library error.
    Internal,
}

impl fmt::Display for MqttError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(err) => write!(f, "Configuration error: {err}"),
            Self::Transport(err) => write!(f, "Transport error: {err}"),
            Self::Protocol(err) => write!(f, "Protocol error: {err}"),
            Self::Session(err) => write!(f, "Session error: {err}"),
            Self::InvalidState => write!(f, "Invalid connection state"),
            Self::NotConnected => write!(f, "Client not connected"),
            Self::AuthenticationFailed => write!(f, "Authentication failed"),
            Self::SubscriptionFailed => write!(f, "Subscription failed"),
            Self::PublishFailed => write!(f, "Publish failed"),
            Self::FlowControl => write!(f, "Flow control limit exceeded"),
            Self::Timeout => write!(f, "Operation timed out"),
            Self::Internal => write!(f, "Internal error"),
        }
    }
}

// Error conversion implementations
impl From<ConfigError> for MqttError {
    fn from(err: ConfigError) -> Self {
        Self::Config(err)
    }
}

impl From<TransportError> for MqttError {
    fn from(err: TransportError) -> Self {
        Self::Transport(err)
    }
}

impl From<ProtocolError> for MqttError {
    fn from(err: ProtocolError) -> Self {
        Self::Protocol(err)
    }
}

impl From<SessionError> for MqttError {
    fn from(err: SessionError) -> Self {
        Self::Session(err)
    }
}

impl From<mqtt_proto::Error> for MqttError {
    fn from(err: mqtt_proto::Error) -> Self {
        Self::Protocol(ProtocolError::InvalidPacket(err.to_string()))
    }
}

impl From<mqtt_proto::v5::ErrorV5> for MqttError {
    fn from(err: mqtt_proto::v5::ErrorV5) -> Self {
        Self::Protocol(ProtocolError::V5Specific(err.to_string()))
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MqttError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(err) => Some(err),
            Self::Transport(err) => Some(err),
            Self::Protocol(err) => Some(err),
            Self::Session(err) => Some(err),
            _ => None,
        }
    }
}

/// Result type for MQTT operations.
pub type Result<T> = core::result::Result<T, MqttError>;
