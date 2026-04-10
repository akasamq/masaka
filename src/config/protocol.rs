use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use bytes::Bytes;
use hashbrown::HashMap;
use mqtt_proto::v5::{UserProperty, VarByteInt};
use mqtt_proto::TopicName;

/// A trait for protocol-specific configurations.
pub trait MqttProtocolConfig: core::fmt::Debug + Clone {
    /// Returns the protocol version identifier.
    fn protocol_version(&self) -> &'static str;

    /// Validates the configuration.
    fn validate(&self) -> Result<(), crate::error::ConfigError>;

    /// Returns protocol-specific connection properties.
    fn connection_properties(&self) -> ProtocolProperties;
}

/// Protocol-specific connection properties.
#[derive(Debug, Clone, Default)]
pub struct ProtocolProperties {
    /// Session expiry interval in seconds.
    pub session_expiry_interval: Option<u32>,
    /// Receive maximum value.
    pub receive_maximum: Option<u16>,
    /// Maximum packet size.
    pub maximum_packet_size: Option<u32>,
    /// Topic alias maximum value.
    pub topic_alias_maximum: Option<u16>,
    /// Extended properties.
    pub extended_properties: HashMap<String, Vec<u8>>,
}

/// MQTT v3.1.1 protocol configuration.
#[derive(Debug, Clone)]
pub struct V3Config {
    /// Protocol name.
    pub protocol_name: String,
    /// Protocol version.
    pub protocol_version: u8,
    /// Enable strict protocol validation.
    pub strict_protocol_validation: bool,
}

impl Default for V3Config {
    fn default() -> Self {
        Self {
            protocol_name: "MQTT".to_string(),
            protocol_version: 4, // MQTT v3.1.1
            strict_protocol_validation: true,
        }
    }
}

impl V3Config {
    /// Creates a new `V3Config`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether to use strict protocol validation.
    pub fn with_strict_validation(mut self, strict: bool) -> Self {
        self.strict_protocol_validation = strict;
        self
    }
}

impl MqttProtocolConfig for V3Config {
    fn protocol_version(&self) -> &'static str {
        "3.1.1"
    }

    fn validate(&self) -> Result<(), crate::error::ConfigError> {
        if self.protocol_name != "MQTT" && self.strict_protocol_validation {
            return Err(crate::error::ConfigError::InvalidProtocol(
                "MQTT v3.1.1 protocol name must be 'MQTT'".into(),
            ));
        }

        if self.protocol_version != 4 && self.strict_protocol_validation {
            return Err(crate::error::ConfigError::InvalidProtocol(
                "MQTT v3.1.1 protocol version must be 4".into(),
            ));
        }

        Ok(())
    }

    fn connection_properties(&self) -> ProtocolProperties {
        // v3.1.1 has no special connection properties
        ProtocolProperties::default()
    }
}

/// MQTT v5.0 connect configuration.
#[derive(Debug, Clone, Default)]
pub struct V5ConnectConfig {
    /// Session expiry interval in seconds.
    pub session_expiry_interval: Option<u32>,
    /// Receive maximum for flow control.
    pub receive_maximum: Option<u16>,
    /// Maximum packet size.
    pub maximum_packet_size: Option<u32>,
    /// Topic alias maximum.
    pub topic_alias_maximum: Option<u16>,
    /// Request response information.
    pub request_response_information: Option<bool>,
    /// Request problem information.
    pub request_problem_information: Option<bool>,
    /// Extended authentication method.
    pub authentication_method: Option<Arc<str>>,
    /// Extended authentication data.
    pub authentication_data: Option<Bytes>,
    /// User properties.
    pub user_properties: Vec<UserProperty>,
}

impl V5ConnectConfig {
    /// Creates a new v5.0 connection configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the session expiry interval.
    pub fn with_session_expiry(mut self, interval_secs: u32) -> Self {
        self.session_expiry_interval = Some(interval_secs);
        self
    }

    /// Sets the receive maximum value.
    pub fn with_receive_maximum(mut self, max: u16) -> Self {
        self.receive_maximum = Some(max);
        self
    }

    /// Sets the maximum packet size.
    pub fn with_maximum_packet_size(mut self, size: u32) -> Self {
        self.maximum_packet_size = Some(size);
        self
    }

    /// Sets the topic alias maximum value.
    pub fn with_topic_alias_maximum(mut self, max: u16) -> Self {
        self.topic_alias_maximum = Some(max);
        self
    }

    /// Adds a user property.
    pub fn with_user_property(mut self, key: &str, value: &str) -> Self {
        self.user_properties.push(UserProperty {
            name: key.into(),
            value: value.into(),
        });
        self
    }

    /// Sets extended authentication.
    pub fn with_extended_auth(mut self, method: &str, data: impl Into<Bytes>) -> Self {
        self.authentication_method = Some(method.into());
        self.authentication_data = Some(data.into());
        self
    }
}

impl MqttProtocolConfig for V5ConnectConfig {
    fn protocol_version(&self) -> &'static str {
        "5.0"
    }

    fn validate(&self) -> Result<(), crate::error::ConfigError> {
        if let Some(receive_max) = self.receive_maximum {
            if receive_max == 0 {
                return Err(crate::error::ConfigError::InvalidLimit(
                    "Receive maximum cannot be 0".into(),
                ));
            }
        }

        if let Some(packet_size) = self.maximum_packet_size {
            if packet_size == 0 {
                return Err(crate::error::ConfigError::InvalidLimit(
                    "Maximum packet size cannot be 0".into(),
                ));
            }
        }

        Ok(())
    }

    fn connection_properties(&self) -> ProtocolProperties {
        ProtocolProperties {
            session_expiry_interval: self.session_expiry_interval,
            receive_maximum: self.receive_maximum,
            maximum_packet_size: self.maximum_packet_size,
            topic_alias_maximum: self.topic_alias_maximum,
            extended_properties: HashMap::new(), // User properties can be extended here
        }
    }
}

/// MQTT v5.0 publish properties.
#[derive(Debug, Clone, Default)]
pub struct V5PublishConfig {
    /// Payload format indicator (0 = bytes, 1 = UTF-8 string).
    pub payload_format_indicator: Option<bool>,
    /// Message expiry interval in seconds.
    pub message_expiry_interval: Option<u32>,
    /// Topic alias (used to reduce packet size).
    pub topic_alias: Option<u16>,
    /// Response topic (for request/response pattern).
    pub response_topic: Option<TopicName>,
    /// Correlation data (for request/response pattern).
    pub correlation_data: Option<Bytes>,
    /// Subscription identifiers.
    pub subscription_identifiers: Vec<VarByteInt>,
    /// Content type.
    pub content_type: Option<Arc<str>>,
    /// User properties.
    pub user_properties: Vec<UserProperty>,
}

impl V5PublishConfig {
    /// Creates a new v5.0 publish configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the payload format indicator.
    pub fn with_payload_format_indicator(mut self, is_utf8: bool) -> Self {
        self.payload_format_indicator = Some(is_utf8);
        self
    }

    /// Sets the message expiry interval.
    pub fn with_message_expiry(mut self, interval_secs: u32) -> Self {
        self.message_expiry_interval = Some(interval_secs);
        self
    }

    /// Sets the response topic.
    pub fn with_response_topic(mut self, topic: TopicName) -> Self {
        self.response_topic = Some(topic);
        self
    }

    /// Sets the correlation data.
    pub fn with_correlation_data(mut self, data: impl Into<Bytes>) -> Self {
        self.correlation_data = Some(data.into());
        self
    }

    /// Sets the content type.
    pub fn with_content_type(mut self, content_type: &str) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// Adds a user property.
    pub fn with_user_property(mut self, key: &str, value: &str) -> Self {
        self.user_properties.push(UserProperty {
            name: key.into(),
            value: value.into(),
        });
        self
    }
}

/// MQTT v5.0 subscribe properties.
#[derive(Debug, Clone, Default)]
pub struct V5SubscribeConfig {
    /// Subscription identifier (for tracking).
    pub subscription_identifier: Option<VarByteInt>,
    /// User properties.
    pub user_properties: Vec<UserProperty>,
}

impl V5SubscribeConfig {
    /// Creates a new v5.0 subscribe configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the subscription identifier.
    pub fn with_subscription_identifier(mut self, id: u32) -> Self {
        self.subscription_identifier = Some(VarByteInt::try_from(id).unwrap_or_default());
        self
    }

    /// Adds a user property.
    pub fn with_user_property(mut self, key: &str, value: &str) -> Self {
        self.user_properties.push(UserProperty {
            name: key.into(),
            value: value.into(),
        });
        self
    }
}

/// MQTT protocol version configuration.
#[derive(Debug, Clone)]
pub enum ProtocolConfig {
    /// MQTT v3.1.1 configuration.
    V3(V3Config),
    /// MQTT v5.0 configuration.
    V5(V5ConnectConfig),
}

impl ProtocolConfig {
    /// Creates a new MQTT v3.1.1 configuration.
    pub fn v3() -> Self {
        Self::V3(V3Config::new())
    }

    /// Creates a new MQTT v5.0 configuration.
    pub fn v5() -> Self {
        Self::V5(V5ConnectConfig::new())
    }

    /// Returns the protocol version identifier.
    pub fn protocol_version(&self) -> &'static str {
        match self {
            Self::V3(config) => config.protocol_version(),
            Self::V5(config) => config.protocol_version(),
        }
    }

    /// Validates the protocol-specific configuration.
    pub fn validate(&self) -> Result<(), crate::error::ConfigError> {
        match self {
            Self::V3(config) => config.validate(),
            Self::V5(config) => config.validate(),
        }
    }

    /// Returns the protocol-specific connection properties.
    pub fn connection_properties(&self) -> ProtocolProperties {
        match self {
            Self::V3(config) => config.connection_properties(),
            Self::V5(config) => config.connection_properties(),
        }
    }
}
