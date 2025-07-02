use core::time::Duration;

use alloc::string::String;

use mqtt_proto::{Error as MqttProtoError, QoS, TopicFilter, TopicName};

use crate::config::{ClientConfig, PublishConfig};
use crate::error::{ConfigError, MqttError, TransportError};
use crate::protocol::{MqttProtocolEngine, MqttProtocolHandler, PacketAction};
use crate::time::{DefaultTimeProvider, TimeProvider};
use crate::transport::MqttTransport;

pub mod state;

pub use state::*;

pub struct MqttClient<T, H, TP = DefaultTimeProvider>
where
    T: MqttTransport + Unpin,
    H: MqttProtocolHandler,
    TP: TimeProvider,
{
    config: ClientConfig,
    protocol_engine: MqttProtocolEngine<T, H>,
    state: ClientState,
    time_provider: TP,
    reconnect_attempt: u32,
    reconnect_interval_ms: u32,
}

impl<T, H> MqttClient<T, H, DefaultTimeProvider>
where
    T: MqttTransport + Unpin,
    H: MqttProtocolHandler,
    H::Error: From<TransportError>,
{
    pub fn new(transport: T, protocol_handler: H, config: ClientConfig) -> Self {
        let time_provider = DefaultTimeProvider::default();

        Self::with_time_provider(transport, protocol_handler, config, time_provider)
    }
}

impl<T, H, TP> MqttClient<T, H, TP>
where
    T: MqttTransport + Unpin,
    H: MqttProtocolHandler,
    H::Error: From<TransportError>,
    TP: TimeProvider,
{
    /// Creates a new MQTT client with a custom time provider.
    pub fn with_time_provider(
        transport: T,
        protocol_handler: H,
        config: ClientConfig,
        time_provider: TP,
    ) -> Self {
        let state = ClientState::new(
            config.client_id.clone(),
            config.max_subscriptions,
            config.max_inflight_messages,
        );
        let protocol_engine = MqttProtocolEngine::new(transport, protocol_handler);

        Self {
            config,
            protocol_engine,
            state,
            time_provider,
            reconnect_attempt: 0,
            reconnect_interval_ms: 0,
        }
    }

    /// Connects to the MQTT broker.
    pub async fn connect(&mut self) -> Result<(), MqttError> {
        // Validate configuration
        self.config.validate()?;

        self.state.set_connection_state(ConnectionState::Connecting);

        // Prepare will message parameters if configured
        let (will_topic, will_message, will_qos, will_retain) =
            if let Some(will) = &self.config.will_message {
                (
                    Some(&will.topic),
                    Some(will.payload.as_slice()),
                    Some(will.qos),
                    will.retain,
                )
            } else {
                (None, None, None, false)
            };

        // Connect with or without will message
        let result = self
            .protocol_engine
            .connect_with_will(
                &self.config.client_id,
                self.config.username.as_deref(),
                self.config.password.as_deref(),
                self.config.keep_alive,
                self.config.clean_session,
                will_topic,
                will_message,
                will_qos,
                will_retain,
            )
            .await;

        match result {
            Ok(PacketAction::ConnectAck {
                session_present,
                return_code: 0,
            }) => {
                self.state.set_connection_state(ConnectionState::Connected);
                self.state
                    .set_session_params(self.config.clean_session, self.config.keep_alive);
                self.reset_reconnect_state();

                // TODO: Implement session resumption logic
                // If `session_present` is true and clean_session is false, the client should:
                // 1. Resend any pending QoS 1/2 messages that were not acknowledged
                // 2. Restore subscription state from previous session  
                // 3. Handle any messages queued by the broker during disconnection
                // This is critical for reliable message delivery in persistent sessions

                // TODO: Implement message deduplication
                // Need to track message IDs to prevent duplicate processing
                // Essential for QoS > 0 message reliability

                log::info!(
                    "Connected to MQTT broker (session_present: {}, will_message: {})",
                    session_present,
                    self.config.will_message.is_some()
                );
                Ok(())
            }
            Ok(PacketAction::ConnectAck { return_code, .. }) => {
                self.state.set_connection_state(ConnectionState::Error);
                log::error!("Connection failed with return code: {}", return_code);
                Err(MqttError::AuthenticationFailed)
            }
            Err(err) => {
                self.state.set_connection_state(ConnectionState::Error);
                log::error!("Connection failed: {}", err);
                Err(err)
            }
            _ => {
                self.state.set_connection_state(ConnectionState::Error);
                Err(MqttError::Protocol(MqttProtoError::InvalidHeader))
            }
        }
    }

    /// Attempts to reconnect to the broker using the configured strategy.
    pub async fn reconnect(&mut self) -> Result<(), MqttError> {
        if !self.config.reconnect.enabled {
            return Err(ConfigError::ReconnectDisabled.into());
        }

        if self.config.reconnect.max_attempts > 0
            && self.reconnect_attempt >= self.config.reconnect.max_attempts
        {
            log::error!(
                "Maximum reconnect attempts ({}) exceeded",
                self.config.reconnect.max_attempts
            );
            return Err(MqttError::Internal);
        }

        self.reconnect_attempt += 1;

        // Calculate reconnect interval (exponential backoff)
        if self.reconnect_interval_ms == 0 {
            self.reconnect_interval_ms = self.config.reconnect.initial_interval_ms;
        } else {
            let new_interval = (self.reconnect_interval_ms as f32
                * self.config.reconnect.backoff_multiplier) as u32;
            self.reconnect_interval_ms = new_interval.min(self.config.reconnect.max_interval_ms);
        }

        log::info!(
            "Attempting reconnect #{} after {}ms",
            self.reconnect_attempt,
            self.reconnect_interval_ms
        );

        // Wait for the reconnect interval
        self.time_provider
            .delay(Duration::from_millis(self.reconnect_interval_ms as u64))
            .await;

        // Attempt to reconnect
        self.connect().await
    }

    /// Publishes a message to a topic.
    pub async fn publish(
        &mut self,
        topic: &TopicName,
        payload: &[u8],
        config: PublishConfig,
    ) -> Result<(), MqttError> {
        if self.state.connection_state() != ConnectionState::Connected {
            return Err(MqttError::InvalidState);
        }

        let result = self
            .protocol_engine
            .publish(topic, config.qos, config.retain, payload)
            .await;

        match result {
            Ok(_pid) => {
                self.state.record_message_sent();
                log::debug!("Published message to topic: {}", topic);
                Ok(())
            }
            Err(err) => {
                log::error!("Failed to publish message: {}", err);
                Err(err)
            }
        }
    }

    /// Subscribes to a topic filter.
    pub async fn subscribe(
        &mut self,
        topic_filter: &TopicFilter,
        qos: QoS,
    ) -> Result<(), MqttError> {
        if self.state.connection_state() != ConnectionState::Connected {
            return Err(MqttError::InvalidState);
        }

        let subscriptions = [(topic_filter.clone(), qos)];
        let pid = self.protocol_engine.subscribe(&subscriptions).await?;

        self.state
            .add_subscription(pid, topic_filter.clone(), qos)
            .map_err(|_| MqttError::Internal)?;

        log::debug!("Subscribed to topic: {} (QoS: {:?})", topic_filter, qos);
        Ok(())
    }

    /// Unsubscribes from a topic filter.
    pub async fn unsubscribe(&mut self, topic_filter: &TopicFilter) -> Result<(), MqttError> {
        if self.state.connection_state() != ConnectionState::Connected {
            return Err(MqttError::InvalidState);
        }

        let topics = [topic_filter.clone()];
        let _pid = self.protocol_engine.unsubscribe(&topics).await?;

        log::debug!("Unsubscribed from topic: {}", topic_filter);
        Ok(())
    }

    /// Sends a `PINGREQ` to the broker.
    pub async fn ping(&mut self) -> Result<(), MqttError> {
        if self.state.connection_state() != ConnectionState::Connected {
            return Err(MqttError::InvalidState);
        }

        self.protocol_engine.ping().await
    }

    /// Disconnects from the broker.
    pub async fn disconnect(&mut self) -> Result<(), MqttError> {
        if self.state.connection_state() == ConnectionState::Disconnected {
            return Ok(());
        }

        self.state
            .set_connection_state(ConnectionState::Disconnecting);

        let result = self.protocol_engine.disconnect().await;
        self.state
            .set_connection_state(ConnectionState::Disconnected);

        result
    }

    /// Polls the client for new events and messages.
    ///
    /// This method should be called repeatedly in a loop to drive the client's state machine,
    /// handle keep-alives, and process incoming data.
    pub async fn poll(&mut self) -> Result<Option<ClientEvent>, MqttError> {
        // TODO: Implement automatic reconnection on connection loss
        // Currently only checks if connected but doesn't attempt reconnect
        if !self.protocol_engine.is_connected() {
            self.state
                .set_connection_state(ConnectionState::Disconnected);
            return Ok(Some(ClientEvent::Disconnected));
        }

        let current_time = self.time_provider.current_timestamp_ms();

        // Check for connection timeout
        if self.protocol_engine.is_connection_timeout(current_time) {
            log::warn!("Connection timeout detected");
            self.state.set_connection_state(ConnectionState::Error);
            return Err(MqttError::Transport(TransportError::Timeout));
        }

        // Check if we need to send keep-alive
        if self.protocol_engine.should_send_keep_alive(current_time) {
            log::debug!("Sending keep-alive ping");
            if let Err(err) = self.ping().await {
                log::error!("Failed to send keep-alive: {}", err);
                return Err(err);
            }
        }

        // TODO: Implement QoS 1/2 message acknowledgment timeout handling
        // Need to track and retry unacknowledged messages based on configured timeouts
        // Handle retries
        if let Err(err) = self.protocol_engine.handle_retries(current_time).await {
            log::error!("Error handling retries: {}", err);
            return Err(err);
        }

        // TODO: Implement flow control for QoS > 0 messages
        // Need to respect max_inflight_messages limit to prevent overwhelming broker

        // Process incoming packets
        match self.protocol_engine.receive_packet().await {
            Ok(action) => {
                let event = self.handle_packet_action(action).await?;
                Ok(event)
            }
            Err(err) => {
                log::error!("Error receiving packet: {}", err);
                self.state.set_connection_state(ConnectionState::Error);
                Err(err)
            }
        }
    }

    /// Returns the next message from the receive queue, if any.
    pub fn next_message(&mut self) -> Option<ReceivedMessage> {
        self.state.pop_received_message()
    }

    /// Returns the current connection state.
    pub fn connection_state(&self) -> ConnectionState {
        self.state.connection_state()
    }

    /// Returns `true` if the client is connected.
    pub fn is_connected(&self) -> bool {
        self.state.connection_state() == ConnectionState::Connected
            && self.protocol_engine.is_connected()
    }

    /// Returns a reference to the client's statistics.
    pub fn stats(&self) -> &state::ClientStats {
        self.state.stats()
    }

    /// Handles a `PacketAction` from the protocol engine.
    async fn handle_packet_action(
        &mut self,
        action: PacketAction,
    ) -> Result<Option<ClientEvent>, MqttError> {
        match action {
            PacketAction::PublishReceived {
                topic,
                qos,
                retain,
                payload,
                pid,
            } => {
                let message = ReceivedMessage {
                    topic: topic.clone(),
                    qos,
                    retain,
                    payload: payload.clone(),
                    packet_id: pid,
                    timestamp: self.time_provider.current_timestamp_ms(),
                };

                self.state.add_received_message(message.clone());

                // TODO: Implement proper QoS handling here
                // For QoS 1: Should send PUBACK automatically  
                // For QoS 2: Should send PUBREC and manage PUBREL/PUBCOMP flow
                // Current implementation delegates this to protocol engine but
                // client should maintain QoS state for reliability

                Ok(Some(ClientEvent::MessageReceived(message)))
            }
            PacketAction::SubscribeAck { pid, return_codes } => {
                self.state
                    .confirm_subscription(pid, &return_codes)
                    .map_err(|_| MqttError::Internal)?;
                Ok(Some(ClientEvent::SubscriptionConfirmed(pid)))
            }
            PacketAction::UnsubscribeAck { pid } => {
                self.state.remove_subscription(pid);
                Ok(Some(ClientEvent::UnsubscriptionConfirmed(pid)))
            }
            PacketAction::PingResponse => Ok(Some(ClientEvent::PingResponse)),
            // TODO: Handle additional PacketActions for complete QoS state machine:
            // - PublishAck: Confirm QoS 1 message delivery
            // - PublishRec: Handle QoS 2 PUBREC
            // - PublishRelease: Handle QoS 2 PUBREL  
            // - PublishComplete: Confirm QoS 2 completion
            // - ConnectAck: Should be handled in connect() method
            _ => Ok(None),
        }
    }

    /// Resets the reconnect attempt counter.
    fn reset_reconnect_state(&mut self) {
        self.reconnect_attempt = 0;
        self.reconnect_interval_ms = 0;
    }
}

#[derive(Debug)]
pub enum ClientEvent {
    /// The client has successfully connected.
    Connected,
    /// The client has been disconnected.
    Disconnected,
    /// A new message has been received.
    MessageReceived(ReceivedMessage),
    /// A subscription has been confirmed by the broker.
    SubscriptionConfirmed(mqtt_proto::Pid),
    /// An unsubscription has been confirmed by the broker.
    UnsubscriptionConfirmed(mqtt_proto::Pid),
    /// A `PINGRESP` has been received from the broker.
    PingResponse,
    /// A connection error occurred.
    Error(String),
}

/// Convenience macro: create TopicName
#[macro_export]
macro_rules! topic {
    ($topic:expr) => {
        mqtt_proto::TopicName::try_from($topic.to_string()).unwrap()
    };
}

/// Convenience macro: create TopicFilter
#[macro_export]
macro_rules! topic_filter {
    ($filter:expr) => {
        mqtt_proto::TopicFilter::try_from($filter.to_string()).unwrap()
    };
}
