use core::fmt;

use alloc::string::String;

/// MQTT protocol errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    /// Invalid packet format.
    InvalidPacket(String),
    /// Invalid packet header.
    InvalidHeader,
    /// Invalid packet length.
    InvalidLength,
    /// Invalid packet ID.
    InvalidPacketId,
    /// Unsupported protocol version.
    UnsupportedVersion(String),
    /// MQTT v5.0 specific error.
    V5Specific(String),
    /// Encoding error.
    EncodingError(String),
    /// Decoding error.
    DecodingError(String),
    /// Invalid QoS level.
    InvalidQoS(u8),
    /// Invalid topic name.
    InvalidTopic(String),
    /// Invalid topic filter.
    InvalidTopicFilter(String),
    /// Connection state error.
    ConnectionState(String),
    /// Subscription limit exceeded.
    SubscriptionLimitExceeded,
    /// Packet is too large.
    PacketTooLarge(usize),
    /// Invalid payload format.
    InvalidPayload(String),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPacket(msg) => write!(f, "Invalid packet: {msg}"),
            Self::InvalidHeader => write!(f, "Invalid packet header"),
            Self::InvalidLength => write!(f, "Invalid packet length"),
            Self::InvalidPacketId => write!(f, "Invalid packet ID"),
            Self::UnsupportedVersion(ver) => write!(f, "Unsupported protocol version: {ver}"),
            Self::V5Specific(msg) => write!(f, "MQTT v5.0 error: {msg}"),
            Self::EncodingError(msg) => write!(f, "Encoding error: {msg}"),
            Self::DecodingError(msg) => write!(f, "Decoding error: {msg}"),
            Self::InvalidQoS(qos) => write!(f, "Invalid QoS level: {qos}"),
            Self::InvalidTopic(topic) => write!(f, "Invalid topic: {topic}"),
            Self::InvalidTopicFilter(filter) => write!(f, "Invalid topic filter: {filter}"),
            Self::ConnectionState(msg) => write!(f, "Connection state error: {msg}"),
            Self::SubscriptionLimitExceeded => write!(f, "Subscription limit exceeded"),
            Self::PacketTooLarge(size) => write!(f, "Packet too large: {size} bytes"),
            Self::InvalidPayload(msg) => write!(f, "Invalid payload: {msg}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ProtocolError {}
