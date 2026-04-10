use alloc::string::String;
use alloc::vec::Vec;

use mqtt_proto::{QoS, TopicName};

use crate::error::ConfigError;

mod common;
mod protocol;

pub use common::*;
pub use protocol::*;

/// An MQTT Will message.
#[derive(Debug, Clone)]
pub struct WillMessage {
    /// Topic of the will message.
    pub topic: TopicName,
    /// Payload of the will message.
    pub payload: Vec<u8>,
    /// QoS level of the will message.
    pub qos: QoS,
    /// Retain the will message.
    pub retain: bool,
}

impl WillMessage {
    /// Constructs a new `WillMessage`.
    pub fn new(topic: TopicName, payload: Vec<u8>, qos: QoS, retain: bool) -> Self {
        Self {
            topic,
            payload,
            qos,
            retain,
        }
    }

    /// Creates a will message from a topic string.
    pub fn with_topic_str(
        topic: &str,
        payload: Vec<u8>,
        qos: QoS,
        retain: bool,
    ) -> Result<Self, mqtt_proto::Error> {
        let topic = TopicName::try_from(topic)?;
        Ok(Self::new(topic, payload, qos, retain))
    }

    /// Creates a simple text will message.
    pub fn text(topic: &str, message: &str, qos: QoS) -> Result<Self, mqtt_proto::Error> {
        Self::with_topic_str(topic, message.as_bytes().to_vec(), qos, false)
    }
}

/// Configuration for publishing messages.
#[derive(Debug, Clone)]
pub struct PublishConfig {
    /// Publish QoS level.
    pub qos: QoS,
    /// Retain this message.
    pub retain: bool,
    /// Timeout for QoS > 0 messages (ms).
    pub timeout_ms: Option<u32>,
    /// Enable DUP flag on re-transmission.
    pub enable_dup_flag: bool,
}

impl Default for PublishConfig {
    fn default() -> Self {
        Self::qos0()
    }
}

impl PublishConfig {
    /// Creates a QoS 0 publish configuration.
    pub fn qos0() -> Self {
        Self {
            qos: QoS::Level0,
            retain: false,
            timeout_ms: None,
            enable_dup_flag: false,
        }
    }

    /// Creates a QoS 1 publish configuration.
    pub fn qos1() -> Self {
        Self {
            qos: QoS::Level1,
            retain: false,
            timeout_ms: Some(5_000),
            enable_dup_flag: true,
        }
    }

    /// Creates a QoS 2 publish configuration.
    pub fn qos2() -> Self {
        Self {
            qos: QoS::Level2,
            retain: false,
            timeout_ms: Some(10_000),
            enable_dup_flag: true,
        }
    }

    /// Creates a configuration for a retained message.
    pub fn retained(qos: QoS) -> Self {
        Self {
            qos,
            retain: true,
            timeout_ms: if matches!(qos, QoS::Level0) {
                None
            } else {
                Some(5_000)
            },
            enable_dup_flag: !matches!(qos, QoS::Level0),
        }
    }

    /// Sets the QoS level.
    pub fn with_qos(mut self, qos: QoS) -> Self {
        self.qos = qos;
        self
    }

    /// Sets the retain flag.
    pub fn with_retain(mut self, retain: bool) -> Self {
        self.retain = retain;
        self
    }

    /// Sets the timeout for QoS > 0 messages.
    pub fn with_timeout(mut self, timeout_ms: u32) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Enables or disables the DUP flag.
    pub fn with_dup_flag(mut self, enable: bool) -> Self {
        self.enable_dup_flag = enable;
        self
    }
}

/// MQTT client configuration.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Client ID, must be unique per broker.
    pub client_id: String,
    /// Username for authentication.
    pub username: Option<String>,
    /// Password for authentication.
    pub password: Option<Vec<u8>>,
    /// Keep-alive interval (seconds). `0` disables it.
    pub keep_alive: u16,
    /// Use a clean session.
    pub clean_session: bool,
    /// Will message to be sent on unexpected disconnect.
    pub will_message: Option<WillMessage>,
    /// Connection timeout (ms).
    pub connect_timeout_ms: u32,
    /// Automatic reconnect configuration.
    pub reconnect: ReconnectConfig,
    /// Transport layer configuration.
    pub transport: TransportConfig,
    /// Maximum number of active subscriptions.
    pub max_subscriptions: usize,
    /// Maximum number of in-flight messages.
    pub max_inflight_messages: usize,
    /// Enable automatic ping responses.
    pub enable_auto_ping: bool,
    /// Maximum packet size (bytes).
    pub max_packet_size: u32,
    /// Enable packet ID validation.
    pub enable_packet_id_validation: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            client_id: "masaka_client".into(),
            username: None,
            password: None,
            keep_alive: 60,
            clean_session: true,
            will_message: None,
            connect_timeout_ms: 30_000,
            reconnect: ReconnectConfig::default(),
            transport: TransportConfig::default(),
            max_subscriptions: 32,
            max_inflight_messages: 64,
            enable_auto_ping: true,
            max_packet_size: 268_435_456, // 256 MB
            enable_packet_id_validation: true,
        }
    }
}

impl ClientConfig {
    /// Creates a new `ClientConfig`.
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            ..Default::default()
        }
    }

    /// Sets authentication credentials.
    pub fn with_credentials(
        mut self,
        username: impl Into<String>,
        password: impl Into<Vec<u8>>,
    ) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }

    /// Sets a string password (convenience method).
    pub fn with_string_password(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into().into_bytes());
        self
    }

    /// Sets the keep-alive interval.
    pub fn with_keep_alive(mut self, keep_alive: u16) -> Self {
        self.keep_alive = keep_alive;
        self
    }

    /// Sets the clean session flag.
    pub fn with_clean_session(mut self, clean_session: bool) -> Self {
        self.clean_session = clean_session;
        self
    }

    /// Sets the will message.
    pub fn with_will_message(mut self, will_message: WillMessage) -> Self {
        self.will_message = Some(will_message);
        self
    }

    /// Sets a simple text will message.
    pub fn with_will_text(
        mut self,
        topic: &str,
        message: &str,
        qos: QoS,
    ) -> Result<Self, mqtt_proto::Error> {
        self.will_message = Some(WillMessage::text(topic, message, qos)?);
        Ok(self)
    }

    /// Sets the connection timeout.
    pub fn with_connect_timeout(mut self, timeout_ms: u32) -> Self {
        self.connect_timeout_ms = timeout_ms;
        self
    }

    /// Sets the reconnect configuration.
    pub fn with_reconnect(mut self, reconnect: ReconnectConfig) -> Self {
        self.reconnect = reconnect;
        self
    }

    /// Sets the transport configuration.
    pub fn with_transport(mut self, transport: TransportConfig) -> Self {
        self.transport = transport;
        self
    }

    /// Sets the maximum number of subscriptions.
    pub fn with_max_subscriptions(mut self, max_subscriptions: usize) -> Self {
        self.max_subscriptions = max_subscriptions;
        self
    }

    /// Sets the maximum number of in-flight messages.
    pub fn with_max_inflight_messages(mut self, max_inflight_messages: usize) -> Self {
        self.max_inflight_messages = max_inflight_messages;
        self
    }

    /// Enables or disables automatic ping responses.
    pub fn with_auto_ping(mut self, enable: bool) -> Self {
        self.enable_auto_ping = enable;
        self
    }

    /// Sets the maximum packet size.
    pub fn with_max_packet_size(mut self, max_packet_size: u32) -> Self {
        self.max_packet_size = max_packet_size;
        self
    }

    /// Enables or disables packet ID validation.
    pub fn with_packet_id_validation(mut self, enable: bool) -> Self {
        self.enable_packet_id_validation = enable;
        self
    }

    /// Creates a configuration optimized for IoT devices.
    pub fn iot_device(client_id: impl Into<String>) -> Self {
        Self::new(client_id)
            .with_keep_alive(30)
            .with_max_subscriptions(8)
            .with_max_inflight_messages(16)
            .with_transport(TransportConfig::low_latency())
            .with_reconnect(ReconnectConfig::exponential_backoff(500, 30_000, 2.0, 10))
    }

    /// Creates a configuration optimized for server-side applications.
    pub fn server_application(client_id: impl Into<String>) -> Self {
        Self::new(client_id)
            .with_keep_alive(120)
            .with_max_subscriptions(256)
            .with_max_inflight_messages(1024)
            .with_transport(TransportConfig::high_throughput())
            .with_reconnect(ReconnectConfig::exponential_backoff(1000, 60_000, 1.5, 0))
    }

    /// Validates the configuration, returning an error if it's invalid.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Client ID validation
        if self.client_id.is_empty() {
            return Err(ConfigError::InvalidClientId(
                "Client ID cannot be empty".into(),
            ));
        }

        if self.client_id.len() > 23 {
            return Err(ConfigError::InvalidClientId(
                "MQTT v3.1.1 client ID cannot exceed 23 characters".into(),
            ));
        }

        // Will message validation
        if let Some(will) = &self.will_message {
            if will.payload.len() > self.max_packet_size as usize {
                return Err(ConfigError::InvalidWillMessage(
                    "Will message payload exceeds maximum packet size".into(),
                ));
            }
        }

        // Subscription limit validation
        if self.max_subscriptions == 0 {
            return Err(ConfigError::InvalidLimit(
                "Maximum subscriptions must be > 0".into(),
            ));
        }

        if self.max_inflight_messages == 0 {
            return Err(ConfigError::InvalidLimit(
                "Maximum in-flight messages must be > 0".into(),
            ));
        }

        // Transport timeout validation
        if self.transport.connect_timeout_ms == 0 {
            return Err(ConfigError::InvalidTimeout(
                "Connection timeout must be > 0".into(),
            ));
        }

        Ok(())
    }
}
