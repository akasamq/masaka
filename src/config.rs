use alloc::string::String;
use alloc::vec::Vec;

use mqtt_proto::{QoS, TopicName};

use crate::error::ConfigError;

#[derive(Debug, Clone)]
pub struct WillMessage {
    /// The topic of the will message.
    pub topic: TopicName,
    /// The payload of the will message.
    pub payload: Vec<u8>,
    /// The QoS level of the will message.
    pub qos: QoS,
    /// Indicates if the will message should be retained.
    pub retain: bool,
}

impl WillMessage {
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
        let topic = TopicName::try_from(topic.to_string())?;
        Ok(Self::new(topic, payload, qos, retain))
    }
}

#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Connection timeout in milliseconds.
    pub connect_timeout_ms: u32,
    /// Read timeout in milliseconds.
    pub read_timeout_ms: u32,
    /// Write timeout in milliseconds.
    pub write_timeout_ms: u32,
    /// Enables or disables TCP Keep-Alive.
    pub tcp_keepalive: bool,
    /// TCP Keep-Alive interval in seconds.
    pub tcp_keepalive_interval_secs: u16,
    /// TCP Keep-Alive probe count.
    pub tcp_keepalive_probes: u8,
    /// Read buffer size in bytes.
    pub read_buffer_size: usize,
    /// Write buffer size in bytes.
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

#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// The client ID, must be unique per broker.
    pub client_id: String,
    /// The username for authentication.
    pub username: Option<String>,
    /// The password for authentication.
    pub password: Option<Vec<u8>>,
    /// The keep-alive interval in seconds. `0` disables it.
    pub keep_alive: u16,
    /// Indicates if a clean session should be used.
    pub clean_session: bool,
    /// The will message configuration.
    pub will_message: Option<WillMessage>,
    /// The connection timeout in milliseconds.
    pub connect_timeout_ms: u32,
    /// The auto-reconnect configuration.
    pub reconnect: ReconnectConfig,
    /// The transport layer configuration.
    pub transport: TransportConfig,
    /// The maximum number of active subscriptions.
    pub max_subscriptions: usize,
    /// The maximum number of in-flight messages.
    pub max_inflight_messages: usize,
    /// Enables or disables automatic ping responses.
    pub enable_auto_ping: bool,
    /// The maximum packet size in bytes.
    pub max_packet_size: u32,
    /// Enables or disables packet ID validation.
    pub enable_packet_id_validation: bool,
}

// TODO: Add missing MQTT configuration options for complete implementation:
// 1. Session expiry interval (MQTT v5.0)
// 2. Receive maximum (flow control)
// 3. Topic alias maximum (MQTT v5.0)
// 4. Request response information (MQTT v5.0)
// 5. Request problem information (MQTT v5.0)
// 6. User properties (MQTT v5.0)
// 7. Authentication method and data (MQTT v5.0)
// 8. Server keep alive override
// 9. Assigned client identifier handling
// 10. Reason string handling (MQTT v5.0)

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
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            ..Default::default()
        }
    }

    /// Sets the username and password for authentication.
    pub fn with_credentials(
        mut self,
        username: impl Into<String>,
        password: impl Into<Vec<u8>>,
    ) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
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

    /// Validates the client configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Client ID validation
        if self.client_id.is_empty() {
            return Err(ConfigError::InvalidClientId(
                "Client ID cannot be empty".into(),
            ));
        }

        if self.client_id.len() > 23 {
            return Err(ConfigError::InvalidClientId(
                "Client ID cannot exceed 23 characters for MQTT v3.1.1".into(),
            ));
        }

        // Keep alive validation
        if self.keep_alive > 65535 {
            return Err(ConfigError::InvalidKeepAlive(
                "Keep alive must be <= 65535 seconds".into(),
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

        // Subscription limits validation
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

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Enables or disables auto-reconnect.
    pub enabled: bool,
    /// Maximum reconnect attempts. `0` means unlimited.
    pub max_attempts: u32,
    /// The initial reconnect interval in milliseconds.
    pub initial_interval_ms: u32,
    /// The maximum reconnect interval in milliseconds.
    pub max_interval_ms: u32,
    /// The multiplier for exponential backoff.
    pub backoff_multiplier: f32,
    /// Enables or disables jitter to avoid thundering herd.
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
    /// Disables auto-reconnect.
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

#[derive(Debug, Clone)]
pub struct PublishConfig {
    /// The QoS level for the publish.
    pub qos: QoS,
    /// Indicates if this is a retained message.
    pub retain: bool,
    /// Timeout in milliseconds for publishes with QoS > 0.
    pub timeout_ms: Option<u32>,
    /// Enables or disables the DUP flag for retransmissions.
    pub enable_dup_flag: bool,
}

impl Default for PublishConfig {
    fn default() -> Self {
        Self {
            qos: QoS::Level0,
            retain: false,
            timeout_ms: Some(5_000),
            enable_dup_flag: true,
        }
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
}
