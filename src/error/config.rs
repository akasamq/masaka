use core::fmt;

use alloc::string::String;

/// Configuration validation errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Invalid client ID.
    InvalidClientId(String),
    /// Invalid keep-alive setting.
    InvalidKeepAlive(String),
    /// Invalid will message.
    InvalidWillMessage(String),
    /// Invalid limit setting.
    InvalidLimit(String),
    /// Invalid timeout setting.
    InvalidTimeout(String),
    /// Invalid protocol configuration.
    InvalidProtocol(String),
    /// Reconnect functionality is disabled.
    ReconnectDisabled,
    /// Invalid authentication configuration.
    InvalidAuthentication(String),
    /// Invalid transport configuration.
    InvalidTransport(String),
    /// Invalid QoS configuration.
    InvalidQoS(String),
    /// Invalid topic configuration.
    InvalidTopic(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidClientId(msg) => write!(f, "Invalid client ID: {msg}"),
            Self::InvalidKeepAlive(msg) => write!(f, "Invalid keep-alive setting: {msg}"),
            Self::InvalidWillMessage(msg) => write!(f, "Invalid will message: {msg}"),
            Self::InvalidLimit(msg) => write!(f, "Invalid limit setting: {msg}"),
            Self::InvalidTimeout(msg) => write!(f, "Invalid timeout setting: {msg}"),
            Self::InvalidProtocol(msg) => write!(f, "Invalid protocol configuration: {msg}"),
            Self::ReconnectDisabled => write!(f, "Auto-reconnect is disabled in the configuration"),
            Self::InvalidAuthentication(msg) => write!(f, "Invalid auth configuration: {msg}"),
            Self::InvalidTransport(msg) => write!(f, "Invalid transport configuration: {msg}"),
            Self::InvalidQoS(msg) => write!(f, "Invalid QoS configuration: {msg}"),
            Self::InvalidTopic(msg) => write!(f, "Invalid topic configuration: {msg}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ConfigError {}
