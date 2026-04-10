use core::fmt;

use alloc::string::{String, ToString};

/// Transport layer errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// Connection establishment failed.
    ConnectionFailed,
    /// Connection lost unexpectedly.
    ConnectionLost,
    /// TLS handshake failed.
    TlsHandshakeFailed,
    /// Operation timed out.
    Timeout,
    /// Generic I/O error.
    Io(String),
    /// DNS resolution failed.
    DnsResolutionFailed,
    /// Certificate validation failed.
    CertificateValidationFailed,
    /// Network is unreachable.
    NetworkUnreachable,
    /// Connection was refused.
    ConnectionRefused,
    /// Other transport-specific error.
    Other(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionFailed => write!(f, "Connection failed"),
            Self::ConnectionLost => write!(f, "Connection lost"),
            Self::TlsHandshakeFailed => write!(f, "TLS handshake failed"),
            Self::Timeout => write!(f, "Operation timed out"),
            Self::Io(msg) => write!(f, "IO error: {msg}"),
            Self::DnsResolutionFailed => write!(f, "DNS resolution failed"),
            Self::CertificateValidationFailed => write!(f, "Certificate validation failed"),
            Self::NetworkUnreachable => write!(f, "Network unreachable"),
            Self::ConnectionRefused => write!(f, "Connection refused"),
            Self::Other(msg) => write!(f, "Transport error: {msg}"),
        }
    }
}

impl From<embedded_io::ErrorKind> for TransportError {
    fn from(kind: embedded_io::ErrorKind) -> Self {
        use embedded_io::ErrorKind;
        match kind {
            ErrorKind::TimedOut => Self::Timeout,
            ErrorKind::ConnectionAborted | ErrorKind::ConnectionReset => Self::ConnectionLost,
            ErrorKind::NotConnected => Self::ConnectionFailed,
            ErrorKind::ConnectionRefused => Self::ConnectionRefused,
            _ => Self::Io(alloc::format!("{kind:?}")),
        }
    }
}

#[cfg(feature = "std")]
impl From<std::io::Error> for TransportError {
    fn from(err: std::io::Error) -> Self {
        use std::io::ErrorKind;
        match err.kind() {
            ErrorKind::TimedOut => TransportError::Timeout,
            ErrorKind::ConnectionRefused => TransportError::ConnectionRefused,
            ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted | ErrorKind::NotConnected => {
                TransportError::ConnectionLost
            }
            _ => TransportError::Io(err.to_string()),
        }
    }
}

#[cfg(feature = "embassy")]
impl From<embassy_net::tcp::ConnectError> for TransportError {
    fn from(e: embassy_net::tcp::ConnectError) -> Self {
        match e {
            embassy_net::tcp::ConnectError::NoRoute => Self::NetworkUnreachable,
            embassy_net::tcp::ConnectError::TimedOut => Self::Timeout,
            embassy_net::tcp::ConnectError::ConnectionReset => Self::ConnectionLost,
            embassy_net::tcp::ConnectError::InvalidState => {
                Self::Other("Invalid TCP socket state".into())
            }
        }
    }
}

#[cfg(feature = "embassy")]
impl From<embassy_net::tcp::Error> for TransportError {
    fn from(e: embassy_net::tcp::Error) -> Self {
        match e {
            embassy_net::tcp::Error::ConnectionReset => Self::ConnectionLost,
        }
    }
}

impl embedded_io::Error for TransportError {
    fn kind(&self) -> embedded_io::ErrorKind {
        match self {
            Self::ConnectionFailed => embedded_io::ErrorKind::NotConnected,
            Self::ConnectionLost => embedded_io::ErrorKind::ConnectionAborted,
            Self::TlsHandshakeFailed => embedded_io::ErrorKind::Other,
            Self::Timeout => embedded_io::ErrorKind::TimedOut,
            Self::Io(_) => embedded_io::ErrorKind::Other,
            Self::DnsResolutionFailed => embedded_io::ErrorKind::Other,
            Self::CertificateValidationFailed => embedded_io::ErrorKind::Other,
            Self::NetworkUnreachable => embedded_io::ErrorKind::Other,
            Self::ConnectionRefused => embedded_io::ErrorKind::ConnectionRefused,
            Self::Other(_) => embedded_io::ErrorKind::Other,
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TransportError {}
