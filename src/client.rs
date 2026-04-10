use core::time::Duration;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use mqtt_proto::{QoS, TopicFilter, TopicName};

use crate::config::{
    ClientConfig, PublishConfig, V5ConnectConfig, V5PublishConfig, V5SubscribeConfig,
};
use crate::error::{ConfigError, MqttError, ProtocolError, TransportError};
use crate::protocol::{MqttProtocolEngine, MqttProtocolHandler, PacketAction, V5Handler};
use crate::session::{InMemorySession, InflightMessage, SessionState};
use crate::state::{ClientState, ClientStats, ConnectionState, ReceivedMessage};
use crate::time::{DefaultTimeProvider, TimeProvider};
use crate::transport::MqttTransport;

/// High-level MQTT client with comprehensive feature support.
pub struct MqttClient<T, H, S = InMemorySession, TP = DefaultTimeProvider>
where
    T: MqttTransport + Unpin,
    H: MqttProtocolHandler,
    S: SessionState,
    TP: TimeProvider,
{
    config: ClientConfig,
    protocol_engine: MqttProtocolEngine<T, H, TP>,
    state: ClientState,
    session_state: S,
    time_provider: TP,
    reconnect_attempt: u32,
    reconnect_interval_ms: u32,
}

impl<T, H> MqttClient<T, H, InMemorySession, DefaultTimeProvider>
where
    T: MqttTransport + Unpin,
    H: MqttProtocolHandler,
    H::Error: From<TransportError>,
{
    /// Creates a new MQTT client with default session and time provider.
    pub fn new(transport: T, protocol_handler: H, config: ClientConfig) -> Self {
        let time_provider = DefaultTimeProvider::default();
        let session_state = InMemorySession::new(config.max_subscriptions);

        Self::with_session_state(
            transport,
            protocol_handler,
            config,
            session_state,
            time_provider,
        )
    }
}

impl<T, H, S, TP> MqttClient<T, H, S, TP>
where
    T: MqttTransport + Unpin,
    H: MqttProtocolHandler,
    S: SessionState,
    H::Error: From<TransportError>,
    TP: TimeProvider + Clone,
{
    /// Creates a new MQTT client with custom session state and time provider.
    pub fn with_session_state(
        transport: T,
        protocol_handler: H,
        config: ClientConfig,
        session_state: S,
        time_provider: TP,
    ) -> Self {
        let state = ClientState::new(
            config.client_id.clone(),
            config.max_inflight_messages,
            config.clean_session,
        );
        let protocol_engine = MqttProtocolEngine::with_time_provider(
            transport,
            protocol_handler,
            time_provider.clone(),
        );

        Self {
            config,
            protocol_engine,
            state,
            session_state,
            time_provider,
            reconnect_attempt: 0,
            reconnect_interval_ms: 0,
        }
    }

    /// Connects to the MQTT broker.
    pub async fn connect(&mut self) -> Result<(), MqttError> {
        self.config.validate()?;
        self.state.set_connection_state(ConnectionState::Connecting);

        if self.config.clean_session {
            self.session_state.clear();
        }

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
                self.reset_reconnect_state();

                if session_present && !self.config.clean_session {
                    log::info!("Session present, resuming and resending pending messages...");
                    self.resend_pending_messages().await?;
                }

                log::info!(
                    "Connected to MQTT broker (session_present: {}, will_message: {})",
                    session_present,
                    self.config.will_message.is_some()
                );
                Ok(())
            }
            Ok(PacketAction::ConnectAck { return_code, .. }) => {
                self.state.set_connection_state(ConnectionState::Error);
                log::error!("Connection failed with return code: {return_code}");
                Err(MqttError::AuthenticationFailed)
            }
            Err(err) => {
                self.state.set_connection_state(ConnectionState::Error);
                log::error!("Connection failed: {err}");
                Err(err)
            }
            _ => {
                self.state.set_connection_state(ConnectionState::Error);
                Err(MqttError::Protocol(ProtocolError::InvalidHeader))
            }
        }
    }

    /// Resends pending messages after reconnection.
    async fn resend_pending_messages(&mut self) -> Result<(), MqttError> {
        let mut to_resend = Vec::new();
        for (pid, msg) in self.session_state.pending_outgoing_publishes() {
            to_resend.push((pid, msg.clone()));
        }

        for (pid, msg) in to_resend {
            log::debug!(
                "Resending message to topic: {} (PID: {})",
                msg.topic,
                pid.value()
            );
            let packet = self
                .protocol_engine
                .handler
                .create_publish_packet(
                    &msg.topic,
                    msg.qos,
                    msg.retain,
                    &msg.payload,
                    Some(pid),
                    true, // Set DUP flag
                )
                .map_err(Into::into)?;
            self.protocol_engine
                .send_packet_from_client(&packet)
                .await?;
        }
        Ok(())
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

        // Flow control check
        if self.session_state.pending_outgoing_publishes().count()
            >= self.config.max_inflight_messages
        {
            log::warn!("Max inflight messages reached, publish rejected");
            return Err(MqttError::PublishFailed);
        }

        let result = self
            .protocol_engine
            .publish(
                topic,
                config.qos,
                config.retain,
                payload,
                &mut self.session_state,
            )
            .await;

        match result {
            Ok(_pid) => {
                self.state.record_message_sent();
                log::debug!("Published message to topic: {topic}");
                Ok(())
            }
            Err(err) => {
                log::error!("Failed to publish message: {err}");
                Err(err)
            }
        }
    }

    /// Publishes a text message for convenience.
    pub async fn publish_text(
        &mut self,
        topic: &TopicName,
        text: &str,
        config: PublishConfig,
    ) -> Result<(), MqttError> {
        self.publish(topic, text.as_bytes(), config).await
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
        let pid = self
            .protocol_engine
            .subscribe(&subscriptions, &mut self.session_state)
            .await?;

        self.session_state
            .add_subscription(pid, topic_filter.clone(), qos)?;

        log::debug!("Subscribed to topic: {topic_filter} (QoS: {qos:?})");
        Ok(())
    }

    /// Subscribes to multiple topic filters in a single request.
    pub async fn subscribe_multiple(
        &mut self,
        subscriptions: &[(TopicFilter, QoS)],
    ) -> Result<(), MqttError> {
        if self.state.connection_state() != ConnectionState::Connected {
            return Err(MqttError::InvalidState);
        }

        if subscriptions.is_empty() {
            return Ok(());
        }

        let pid = self
            .protocol_engine
            .subscribe(subscriptions, &mut self.session_state)
            .await?;

        // Add all subscriptions to session state
        for (topic_filter, qos) in subscriptions {
            self.session_state
                .add_subscription(pid, topic_filter.clone(), *qos)?;
        }

        log::debug!("Subscribed to {} topics", subscriptions.len());
        Ok(())
    }

    /// Unsubscribes from a topic filter.
    pub async fn unsubscribe(&mut self, topic_filter: &TopicFilter) -> Result<(), MqttError> {
        if self.state.connection_state() != ConnectionState::Connected {
            return Err(MqttError::InvalidState);
        }

        let topics = [topic_filter.clone()];
        let _pid = self
            .protocol_engine
            .unsubscribe(&topics, &mut self.session_state)
            .await?;

        log::debug!("Unsubscribed from topic: {topic_filter}");
        Ok(())
    }

    /// Unsubscribes from multiple topic filters.
    pub async fn unsubscribe_multiple(
        &mut self,
        topic_filters: &[TopicFilter],
    ) -> Result<(), MqttError> {
        if self.state.connection_state() != ConnectionState::Connected {
            return Err(MqttError::InvalidState);
        }

        if topic_filters.is_empty() {
            return Ok(());
        }

        let _pid = self
            .protocol_engine
            .unsubscribe(topic_filters, &mut self.session_state)
            .await?;

        log::debug!("Unsubscribed from {} topics", topic_filters.len());
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
    pub async fn poll(&mut self) -> Result<Option<ClientEvent>, MqttError> {
        if !self.protocol_engine.is_connected() {
            if self.state.connection_state() == ConnectionState::Connected
                && self.config.reconnect.enabled
            {
                log::warn!("Connection lost, attempting to reconnect...");
                if self.reconnect().await.is_err() {
                    log::error!("Reconnect failed, client will remain disconnected.");
                    self.state
                        .set_connection_state(ConnectionState::Disconnected);
                    return Ok(Some(ClientEvent::Disconnected));
                }
            } else if self.state.connection_state() != ConnectionState::Disconnected {
                self.state
                    .set_connection_state(ConnectionState::Disconnected);
                return Ok(Some(ClientEvent::Disconnected));
            } else {
                return Ok(None);
            }
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
                log::error!("Failed to send keep-alive: {err}");
                return Err(err);
            }
        }

        // Handle retries
        if let Err(err) = self
            .protocol_engine
            .handle_retries(current_time, &mut self.session_state)
            .await
        {
            log::error!("Error handling retries: {err}");
            return Err(err);
        }

        // Process incoming packets
        match self
            .protocol_engine
            .receive_packet(&mut self.session_state)
            .await
        {
            Ok(action) => {
                let event = self.handle_packet_action(action).await?;
                Ok(event)
            }
            Err(err) => {
                log::error!("Error receiving packet: {err}");
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
    pub fn stats(&self) -> &ClientStats {
        self.state.stats()
    }

    /// Returns the client configuration.
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Returns a mutable reference to the session state.
    pub fn session_mut(&mut self) -> &mut S {
        &mut self.session_state
    }

    /// Returns a reference to the session state.
    pub fn session(&self) -> &S {
        &self.session_state
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
                    topic,
                    qos,
                    retain,
                    payload,
                    packet_id: pid,
                    timestamp: self.time_provider.current_timestamp_ms(),
                };
                self.state.add_received_message(message.clone());
                Ok(Some(ClientEvent::MessageReceived(message)))
            }
            PacketAction::SubscribeAck { pid, return_codes } => {
                self.session_state
                    .confirm_subscription(pid, &return_codes)?;
                Ok(Some(ClientEvent::SubscriptionConfirmed(pid)))
            }
            PacketAction::UnsubscribeAck { pid } => {
                self.session_state.remove_subscription(pid);
                Ok(Some(ClientEvent::UnsubscriptionConfirmed(pid)))
            }
            PacketAction::PingResponse => Ok(Some(ClientEvent::PingResponse)),
            PacketAction::PublishAck { pid } => Ok(Some(ClientEvent::MessageAcknowledged(pid))),
            PacketAction::PublishComplete { pid } => {
                Ok(Some(ClientEvent::MessageAcknowledged(pid)))
            }
            _ => Ok(None),
        }
    }

    /// Resets the reconnect attempt counter.
    fn reset_reconnect_state(&mut self) {
        self.reconnect_attempt = 0;
        self.reconnect_interval_ms = 0;
    }
}

/// Events emitted by the MQTT client.
#[derive(Debug)]
pub enum ClientEvent {
    /// The client has successfully connected.
    Connected,
    /// The client has been disconnected.
    Disconnected,
    /// A new message has been received.
    MessageReceived(ReceivedMessage),
    /// A QoS > 0 message has been acknowledged by the broker.
    MessageAcknowledged(mqtt_proto::Pid),
    /// A subscription has been confirmed by the broker.
    SubscriptionConfirmed(mqtt_proto::Pid),
    /// An unsubscription has been confirmed by the broker.
    UnsubscriptionConfirmed(mqtt_proto::Pid),
    /// A `PINGRESP` has been received from the broker.
    PingResponse,
    /// A connection error occurred.
    Error(String),
}

// MQTT v5.0 specific client extensions
impl<T> MqttClient<T, V5Handler, InMemorySession, DefaultTimeProvider>
where
    T: MqttTransport + Unpin,
{
    /// Creates a new MQTT v5.0 client with enhanced configuration.
    pub fn new_v5(transport: T, config: ClientConfig, v5_config: V5ConnectConfig) -> Self {
        let handler = V5Handler::with_config(v5_config);
        let max_subs = config.max_subscriptions;
        Self::with_session_state(
            transport,
            handler,
            config,
            InMemorySession::new(max_subs),
            DefaultTimeProvider::default(),
        )
    }
}

impl<T, S, TP> MqttClient<T, V5Handler, S, TP>
where
    T: MqttTransport + Unpin,
    S: SessionState,
    TP: TimeProvider + Clone,
{
    /// Publishes a message with full MQTT v5.0 properties support.
    pub async fn publish_v5(
        &mut self,
        topic: &TopicName,
        payload: &[u8],
        config: PublishConfig,
        v5_config: V5PublishConfig,
    ) -> Result<(), MqttError> {
        if self.state.connection_state() != ConnectionState::Connected {
            return Err(MqttError::InvalidState);
        }

        // Flow control check
        if self.session_state.pending_outgoing_publishes().count()
            >= self.config.max_inflight_messages
        {
            log::warn!("Max inflight messages reached, publish rejected");
            return Err(MqttError::PublishFailed);
        }

        let pid = match config.qos {
            QoS::Level0 => None,
            QoS::Level1 | QoS::Level2 => Some(self.session_state.next_pid()),
        };

        // Create packet using enhanced v5 handler
        let packet = self
            .protocol_engine
            .handler
            .create_publish_with_config(
                topic,
                config.qos,
                config.retain,
                payload,
                pid,
                false, // DUP flag
                v5_config,
            )
            .map_err(|e| MqttError::Protocol(ProtocolError::V5Specific(e.to_string())))?;

        // Store for QoS > 0
        if let Some(pid) = pid {
            let message = InflightMessage {
                topic: topic.clone(),
                qos: config.qos,
                retain: config.retain,
                payload: payload.to_vec(),
                retry_count: 0,
                timestamp: self.time_provider.current_timestamp_ms(),
            };
            self.session_state.store_outgoing_publish(pid, message)?;
        }

        // Send packet
        self.protocol_engine
            .send_packet_from_client(&packet)
            .await?;
        self.state.record_message_sent();

        log::debug!("Published v5.0 message to topic: {topic}");
        Ok(())
    }

    /// Subscribes with full MQTT v5.0 properties support.
    pub async fn subscribe_v5(
        &mut self,
        subscriptions: &[(TopicFilter, QoS)],
        v5_config: V5SubscribeConfig,
    ) -> Result<(), MqttError> {
        if self.state.connection_state() != ConnectionState::Connected {
            return Err(MqttError::InvalidState);
        }

        if subscriptions.is_empty() {
            return Ok(());
        }

        let pid = self.session_state.next_pid();

        // Create packet using enhanced v5 handler
        let packet = self
            .protocol_engine
            .handler
            .create_subscribe_with_config(subscriptions, pid, v5_config)
            .map_err(|e| MqttError::Protocol(ProtocolError::V5Specific(e.to_string())))?;

        // Send packet
        self.protocol_engine
            .send_packet_from_client(&packet)
            .await?;

        // Add all subscriptions to session state
        for (topic_filter, qos) in subscriptions {
            self.session_state
                .add_subscription(pid, topic_filter.clone(), *qos)?;
        }

        log::debug!(
            "Subscribed to {} topics with v5.0 properties",
            subscriptions.len()
        );
        Ok(())
    }

    /// Publishes a request message and returns a future for the response.
    pub async fn publish_request(
        &mut self,
        request_topic: &TopicName,
        response_topic: &TopicName,
        payload: &[u8],
        config: PublishConfig,
        correlation_data: Vec<u8>,
    ) -> Result<(), MqttError> {
        let v5_config = V5PublishConfig {
            response_topic: Some(response_topic.clone()),
            correlation_data: Some(bytes::Bytes::from(correlation_data)),
            ..Default::default()
        };

        self.publish_v5(request_topic, payload, config, v5_config)
            .await
    }

    /// Publishes a response message with correlation data.
    pub async fn publish_response(
        &mut self,
        response_topic: &TopicName,
        payload: &[u8],
        config: PublishConfig,
        correlation_data: Vec<u8>,
    ) -> Result<(), MqttError> {
        let v5_config = V5PublishConfig {
            correlation_data: Some(bytes::Bytes::from(correlation_data)),
            ..Default::default()
        };

        self.publish_v5(response_topic, payload, config, v5_config)
            .await
    }

    /// Sets up a subscription with a specific subscription identifier.
    pub async fn subscribe_with_id(
        &mut self,
        topic_filter: &TopicFilter,
        qos: QoS,
        subscription_id: u32,
    ) -> Result<(), MqttError> {
        let subscriptions = [(topic_filter.clone(), qos)];
        let v5_config = V5SubscribeConfig {
            subscription_identifier: Some(
                mqtt_proto::v5::VarByteInt::try_from(subscription_id)
                    .map_err(|_| MqttError::Protocol(ProtocolError::InvalidPacketId))?,
            ),
            ..Default::default()
        };

        self.subscribe_v5(&subscriptions, v5_config).await
    }

    /// Updates the v5.0 connection configuration.
    pub fn set_v5_config(&mut self, config: V5ConnectConfig) {
        self.protocol_engine.handler.set_connect_config(config);
    }

    /// Gets a reference to the current v5.0 configuration.
    pub fn v5_config(&self) -> &V5ConnectConfig {
        &self.protocol_engine.handler.connect_config
    }
}
