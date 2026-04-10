#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(not(any(feature = "tokio", feature = "embassy")))]
compile_error!("A runtime feature must be enabled. Available: 'tokio', 'embassy'. 'embassy' is enabled by default.");

pub mod client;
pub mod config;
pub mod error;
pub mod protocol;
pub mod session;
pub mod state;
pub mod time;
pub mod transport;

// Re-export core types from mqtt-proto
pub use mqtt_proto::{Pid, Protocol, QoS, TopicFilter, TopicName};

// Re-export main client types
pub use client::{ClientEvent, MqttClient};

// Re-export configuration types
pub use config::{
    ClientConfig, ProtocolConfig, PublishConfig, ReconnectConfig, TransportConfig, V5ConnectConfig,
    V5PublishConfig, V5SubscribeConfig, WillMessage,
};

// Re-export error types
pub use error::{ConfigError, MqttError, ProtocolError, SessionError, TransportError};

// Re-export protocol handlers and types
pub use protocol::{MqttProtocolHandler, PacketAction, V3Handler, V5Handler};

// Re-export time provider
pub use time::TimeProvider;

// Re-export transport trait
pub use transport::MqttTransport;

// Convenience macros
/// Create a `TopicName` from a string literal.
///
/// # Example
///
/// ```rust
/// use masaka::topic;
/// let topic = topic!("sensors/temperature");
/// ```
#[macro_export]
macro_rules! topic {
    ($topic:expr) => {
        $crate::TopicName::try_from($topic).expect("Invalid topic name")
    };
}

/// Create a `TopicFilter` from a string literal.
///
/// # Example
///
/// ```rust
/// use masaka::topic_filter;
/// let filter = topic_filter!("sensors/+/temperature");
/// ```
#[macro_export]
macro_rules! topic_filter {
    ($filter:expr) => {
        $crate::TopicFilter::try_from($filter).expect("Invalid topic filter")
    };
}
