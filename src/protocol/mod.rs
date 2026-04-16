use core::marker::PhantomData;

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use mqtt_proto::{
    Error as MqttProtoError, GenericPollPacket, GenericPollPacketState, Pid, PollHeader, QoS,
    TopicFilter, TopicName, VarBytes,
};
#[cfg(feature = "tokio")]
use tokio::io::AsyncWriteExt;

use crate::error::{MqttError, ProtocolError, TransportError};
use crate::session::{InflightMessage, SessionState};
use crate::time::{DefaultTimeProvider, TimeProvider};
use crate::transport::MqttTransport;

mod v3;
mod v5;

pub use v3::V3Handler;
pub use v5::V5Handler;

/// Handles MQTT protocol-level logic for a specific version.
pub trait MqttProtocolHandler {
    /// The packet type for this protocol version.
    type Packet: Send + Unpin;
    /// The error type for this protocol version.
    #[cfg(feature = "tokio")]
    type Error: Into<MqttError>
        + From<MqttProtoError>
        + From<TransportError>
        + From<std::io::Error>
        + Send
        + core::fmt::Debug;
    #[cfg(not(feature = "tokio"))]
    type Error: Into<MqttError>
        + From<MqttProtoError>
        + From<TransportError>
        + Send
        + core::fmt::Debug;
    /// The header type for this protocol version.
    type Header: PollHeader<Packet = Self::Packet, Error = Self::Error> + Copy + Unpin + Send;

    /// Creates a `CONNECT` packet.
    fn create_connect_packet(
        &self,
        client_id: &str,
        username: Option<&str>,
        password: Option<&[u8]>,
        keep_alive: u16,
        clean_session: bool,
    ) -> Result<Self::Packet, Self::Error>;

    /// Creates a `CONNECT` packet with a will message.
    fn create_connect_with_will_packet(
        &self,
        client_id: &str,
        username: Option<&str>,
        password: Option<&[u8]>,
        keep_alive: u16,
        clean_session: bool,
        will_topic: &TopicName,
        will_message: &[u8],
        will_qos: QoS,
        will_retain: bool,
    ) -> Result<Self::Packet, Self::Error>;

    /// Creates a `PUBLISH` packet.
    fn create_publish_packet(
        &self,
        topic: &TopicName,
        qos: QoS,
        retain: bool,
        payload: &[u8],
        pid: Option<Pid>,
        dup: bool,
    ) -> Result<Self::Packet, Self::Error>;

    /// Creates a `SUBSCRIBE` packet.
    fn create_subscribe_packet(
        &self,
        subscriptions: &[(TopicFilter, QoS)],
        pid: Pid,
    ) -> Result<Self::Packet, Self::Error>;

    /// Creates an `UNSUBSCRIBE` packet.
    fn create_unsubscribe_packet(
        &self,
        topics: &[TopicFilter],
        pid: Pid,
    ) -> Result<Self::Packet, Self::Error>;

    /// Creates a `PUBACK` packet.
    fn create_puback_packet(&self, pid: Pid) -> Result<Self::Packet, Self::Error>;

    /// Creates a `PUBREC` packet.
    fn create_pubrec_packet(&self, pid: Pid) -> Result<Self::Packet, Self::Error>;

    /// Creates a `PUBREL` packet.
    fn create_pubrel_packet(&self, pid: Pid) -> Result<Self::Packet, Self::Error>;

    /// Creates a `PUBCOMP` packet.
    fn create_pubcomp_packet(&self, pid: Pid) -> Result<Self::Packet, Self::Error>;

    /// Creates a `PINGREQ` packet.
    fn create_pingreq_packet(&self) -> Self::Packet;

    /// Creates a `DISCONNECT` packet.
    fn create_disconnect_packet(&self) -> Self::Packet;

    /// Encodes a packet into a byte buffer.
    fn encode_packet(&self, packet: &Self::Packet) -> Result<VarBytes, Self::Error>;

    /// Handles a received packet and returns a corresponding action.
    fn handle_packet(&mut self, packet: Self::Packet) -> Result<PacketAction, Self::Error>;
}

#[derive(Debug, Clone)]
pub enum PacketAction {
    /// A `CONNACK` packet was received.
    ConnectAck {
        session_present: bool,
        return_code: u8,
    },
    /// A `PUBACK` packet was received.
    PublishAck { pid: Pid },
    /// A `PUBREC` packet was received.
    PublishRec { pid: Pid },
    /// A `PUBREL` packet was received.
    PublishRelease { pid: Pid },
    /// A `PUBCOMP` packet was received.
    PublishComplete { pid: Pid },
    /// A `PUBLISH` message was received.
    PublishReceived {
        topic: TopicName,
        qos: QoS,
        retain: bool,
        payload: Vec<u8>,
        pid: Option<Pid>,
    },
    /// A `SUBACK` packet was received.
    SubscribeAck { pid: Pid, return_codes: Vec<u8> },
    /// An `UNSUBACK` packet was received.
    UnsubscribeAck { pid: Pid },
    /// A `PINGRESP` packet was received.
    PingResponse,
    /// No specific action is required.
    None,
}

pub struct MqttProtocolEngine<T, H, TP = DefaultTimeProvider>
where
    T: MqttTransport + Unpin,
    H: MqttProtocolHandler,
    TP: TimeProvider,
{
    transport: T,
    pub handler: H,
    time_provider: TP,

    // Decoding state
    packet_state: GenericPollPacketState<H::Header>,

    // Write queue
    write_queue: VecDeque<VarBytes>,

    // Keep-alive management
    last_packet_sent_time: u64,
    last_packet_received_time: u64,
    keep_alive_interval_ms: u64,

    // Connection state
    connected: bool,

    _phantom: PhantomData<H>,
}

impl<T, H> MqttProtocolEngine<T, H, DefaultTimeProvider>
where
    T: MqttTransport + Unpin,
    H: MqttProtocolHandler,
{
    /// Creates a new protocol engine with the default time provider.
    pub fn new(transport: T, handler: H) -> Self {
        Self::with_time_provider(transport, handler, DefaultTimeProvider::default())
    }
}

impl<T, H, TP> MqttProtocolEngine<T, H, TP>
where
    T: MqttTransport + Unpin,
    H: MqttProtocolHandler,
    TP: TimeProvider,
{
    /// Creates a new protocol engine with a custom time provider.
    pub fn with_time_provider(transport: T, handler: H, time_provider: TP) -> Self {
        Self {
            transport,
            handler,
            time_provider,
            packet_state: GenericPollPacketState::default(),
            write_queue: VecDeque::new(),
            last_packet_sent_time: 0,
            last_packet_received_time: 0,
            keep_alive_interval_ms: 60_000, // Default 60 seconds
            connected: false,
            _phantom: PhantomData,
        }
    }

    /// Sets the keep-alive interval.
    pub fn set_keep_alive_interval(&mut self, keep_alive_secs: u16) {
        self.keep_alive_interval_ms = (keep_alive_secs as u64) * 1000;
    }

    /// Updates the timestamp of the last sent packet.
    fn update_last_sent_time(&mut self, timestamp: u64) {
        self.last_packet_sent_time = timestamp;
    }

    /// Updates the timestamp of the last received packet.
    fn update_last_received_time(&mut self, timestamp: u64) {
        self.last_packet_received_time = timestamp;
    }

    /// Checks if a `PINGREQ` should be sent.
    pub fn should_send_keep_alive(&self, current_time: u64) -> bool {
        if self.keep_alive_interval_ms == 0 {
            return false;
        }

        let elapsed = current_time.saturating_sub(self.last_packet_sent_time);
        elapsed >= self.keep_alive_interval_ms
    }

    /// Checks if the connection has timed out.
    pub fn is_connection_timeout(&self, current_time: u64) -> bool {
        if self.keep_alive_interval_ms == 0 {
            return false;
        }

        // Connection timeout is 1.5 times the Keep-Alive interval
        let timeout_ms = self.keep_alive_interval_ms + (self.keep_alive_interval_ms / 2);
        let elapsed = current_time.saturating_sub(self.last_packet_received_time);
        elapsed >= timeout_ms
    }

    /// Connects to the MQTT broker.
    pub async fn connect(
        &mut self,
        client_id: &str,
        username: Option<&str>,
        password: Option<&[u8]>,
        keep_alive: u16,
        clean_session: bool,
    ) -> Result<PacketAction, MqttError> {
        self.connect_with_will(
            client_id,
            username,
            password,
            keep_alive,
            clean_session,
            None,
            None,
            None,
            false,
        )
        .await
    }

    /// Connects to the MQTT broker with a will message.
    pub async fn connect_with_will(
        &mut self,
        client_id: &str,
        username: Option<&str>,
        password: Option<&[u8]>,
        keep_alive: u16,
        clean_session: bool,
        will_topic: Option<&TopicName>,
        will_message: Option<&[u8]>,
        will_qos: Option<QoS>,
        will_retain: bool,
    ) -> Result<PacketAction, MqttError> {
        if self.connected {
            return Err(MqttError::InvalidState);
        }

        self.set_keep_alive_interval(keep_alive);

        // Create CONNECT packet
        let connect_packet = match (will_topic, will_message, will_qos) {
            (Some(topic), Some(message), Some(qos)) => self
                .handler
                .create_connect_with_will_packet(
                    client_id,
                    username,
                    password,
                    keep_alive,
                    clean_session,
                    topic,
                    message,
                    qos,
                    will_retain,
                )
                .map_err(Into::into)?,
            _ => self
                .handler
                .create_connect_packet(client_id, username, password, keep_alive, clean_session)
                .map_err(Into::into)?,
        };

        // Send CONNECT packet
        self.send_packet(&connect_packet).await?;

        // Wait for CONNACK
        let action = self.receive_packet_unlogged().await?;

        match action {
            PacketAction::ConnectAck { return_code: 0, .. } => {
                self.connected = true;
                Ok(action)
            }
            PacketAction::ConnectAck { .. } => Err(MqttError::AuthenticationFailed),
            _ => Err(MqttError::Protocol(ProtocolError::InvalidHeader)),
        }
    }

    /// Helper to create a connect packet with a will message.
    fn create_connect_with_will(
        &self,
        client_id: &str,
        username: Option<&str>,
        password: Option<&[u8]>,
        keep_alive: u16,
        clean_session: bool,
        will_topic: Option<&TopicName>,
        will_message: Option<&[u8]>,
        _will_qos: Option<QoS>,
        _will_retain: bool,
    ) -> Result<H::Packet, MqttError> {
        // TODO: Fix will message support properly
        // For now, just log and create basic connect packet
        if will_topic.is_some() && will_message.is_some() {
            log::warn!("Will message configured but not yet supported in generic handler");
        }

        self.handler
            .create_connect_packet(client_id, username, password, keep_alive, clean_session)
            .map_err(Into::into)
    }

    /// Publishes a message.
    pub async fn publish<S: SessionState>(
        &mut self,
        topic: &TopicName,
        qos: QoS,
        retain: bool,
        payload: &[u8],
        session: &mut S,
    ) -> Result<Option<Pid>, MqttError> {
        if !self.connected {
            return Err(MqttError::NotConnected);
        }

        let pid = match qos {
            QoS::Level0 => None,
            QoS::Level1 | QoS::Level2 => Some(session.next_pid()),
        };

        let publish_packet = self
            .handler
            .create_publish_packet(topic, qos, retain, payload, pid, false)
            .map_err(Into::into)?;

        // For QoS > 0, we need to record the message pending acknowledgment
        if let Some(pid) = pid {
            session.store_outgoing_publish(
                pid,
                InflightMessage {
                    topic: topic.clone(),
                    qos,
                    retain,
                    payload: payload.to_vec(),
                    retry_count: 0,
                    timestamp: self.time_provider.current_timestamp_ms(),
                },
            )?;
        }

        self.send_packet(&publish_packet).await?;
        Ok(pid)
    }

    /// Subscribes to topics.
    pub async fn subscribe<S: SessionState>(
        &mut self,
        subscriptions: &[(TopicFilter, QoS)],
        session: &mut S,
    ) -> Result<Pid, MqttError> {
        if !self.connected {
            return Err(MqttError::NotConnected);
        }

        let pid = session.next_pid();
        let subscribe_packet = self
            .handler
            .create_subscribe_packet(subscriptions, pid)
            .map_err(Into::into)?;

        self.send_packet(&subscribe_packet).await?;
        Ok(pid)
    }

    /// Unsubscribes from topics.
    pub async fn unsubscribe<S: SessionState>(
        &mut self,
        topics: &[TopicFilter],
        session: &mut S,
    ) -> Result<Pid, MqttError> {
        if !self.connected {
            return Err(MqttError::NotConnected);
        }

        let pid = session.next_pid();
        let unsubscribe_packet = self
            .handler
            .create_unsubscribe_packet(topics, pid)
            .map_err(Into::into)?;

        self.send_packet(&unsubscribe_packet).await?;
        Ok(pid)
    }

    /// Sends a `PINGREQ` packet.
    pub async fn ping(&mut self) -> Result<(), MqttError> {
        if !self.connected {
            return Err(MqttError::NotConnected);
        }

        let ping_packet = self.handler.create_pingreq_packet();
        self.send_packet(&ping_packet).await
    }

    /// Disconnects from the broker.
    pub async fn disconnect(&mut self) -> Result<(), MqttError> {
        if !self.connected {
            return Ok(());
        }

        let disconnect_packet = self.handler.create_disconnect_packet();
        self.send_packet(&disconnect_packet).await?;
        self.connected = false;
        self.transport.close().await.map_err(MqttError::Transport)?;
        Ok(())
    }

    /// Receives and processes the next MQTT packet from the transport.
    pub async fn receive_packet<S: SessionState>(
        &mut self,
        session: &mut S,
    ) -> Result<PacketAction, MqttError> {
        let packet = self.read_from_transport().await?;
        let current_time = self.time_provider.current_timestamp_ms();
        self.update_last_received_time(current_time);

        let action = self.handler.handle_packet(packet).map_err(Into::into)?;

        // Handle QoS acknowledgment logic
        match &action {
            PacketAction::PublishAck { pid } => {
                session.complete_outgoing_publish(*pid);
            }
            PacketAction::PublishComplete { pid } => {
                session.complete_outgoing_publish(*pid);
                session.complete_outgoing_pubrel(*pid);
            }
            PacketAction::PublishRec { pid } => {
                // QoS 2: Received PUBREC, send PUBREL
                if let Some(_pending) = session.complete_outgoing_publish(*pid) {
                    let pubrel_packet = self
                        .handler
                        .create_pubrel_packet(*pid)
                        .map_err(Into::into)?;
                    self.send_packet(&pubrel_packet).await?;
                    session.store_outgoing_pubrel(*pid)?;
                }
            }
            PacketAction::PublishRelease { pid } => {
                // QoS 2: Received PUBREL, send PUBCOMP
                let pubcomp_packet = self
                    .handler
                    .create_pubcomp_packet(*pid)
                    .map_err(Into::into)?;
                self.send_packet(&pubcomp_packet).await?;
            }
            PacketAction::PublishReceived {
                pid: Some(pid),
                qos: QoS::Level1,
                ..
            } => {
                // QoS 1: Send PUBACK
                let puback_packet = self
                    .handler
                    .create_puback_packet(*pid)
                    .map_err(Into::into)?;
                self.send_packet(&puback_packet).await?;
            }
            PacketAction::PublishReceived {
                pid: Some(pid),
                qos: QoS::Level2,
                ..
            } => {
                // QoS 2: Send PUBREC
                let pubrec_packet = self
                    .handler
                    .create_pubrec_packet(*pid)
                    .map_err(Into::into)?;
                self.send_packet(&pubrec_packet).await?;
            }
            _ => {}
        }

        Ok(action)
    }

    /// A version of receive_packet that doesn't interact with SessionState.
    /// Used during initial connection before session is established.
    async fn receive_packet_unlogged(&mut self) -> Result<PacketAction, MqttError> {
        let packet = self.read_from_transport().await?;
        let current_time = self.time_provider.current_timestamp_ms();
        self.update_last_received_time(current_time);
        self.handler.handle_packet(packet).map_err(Into::into)
    }

    /// Reads a complete MQTT packet from the transport.
    async fn read_from_transport(&mut self) -> Result<H::Packet, MqttError> {
        let poll_packet = GenericPollPacket::new(&mut self.packet_state, &mut self.transport);
        let (_encode_len, _body, packet) = poll_packet.await.map_err(Into::into)?;
        Ok(packet)
    }

    /// A version of send_packet that can be called from the client layer directly.
    pub async fn send_packet_from_client(&mut self, packet: &H::Packet) -> Result<(), MqttError> {
        self.send_packet(packet).await
    }

    /// Encodes and sends a packet through the transport.
    async fn send_packet(&mut self, packet: &H::Packet) -> Result<(), MqttError> {
        let encoded = self.handler.encode_packet(packet).map_err(Into::into)?;
        self.write_queue.push_back(encoded);
        self.flush_write_queue().await?;

        let current_time = self.time_provider.current_timestamp_ms();
        self.update_last_sent_time(current_time);
        Ok(())
    }

    /// Flushes the write queue to the transport.
    async fn flush_write_queue(&mut self) -> Result<(), MqttError> {
        while let Some(data) = self.write_queue.pop_front() {
            let mut written = 0;
            let bytes = data.as_ref();

            while written < bytes.len() {
                let n = self
                    .transport
                    .write(&bytes[written..])
                    .await
                    .map_err(|e| MqttError::Transport(e.into()))?;
                if n == 0 {
                    return Err(MqttError::Transport(TransportError::ConnectionLost));
                }
                written += n;
            }
        }

        self.transport
            .flush()
            .await
            .map_err(|e| MqttError::Transport(e.into()))?;
        Ok(())
    }

    /// Returns `true` if the engine is connected.
    pub fn is_connected(&self) -> bool {
        self.connected && self.transport.is_connected()
    }

    /// Handles retry logic for unacknowledged packets.
    pub async fn handle_retries<S: SessionState>(
        &mut self,
        current_time: u64,
        session: &mut S,
    ) -> Result<(), MqttError> {
        const RETRY_TIMEOUT_MS: u64 = 5000; // 5 seconds
        const MAX_RETRY_COUNT: u8 = 3;

        // Handle PUBLISH retries
        let mut to_retry = Vec::new();
        let mut to_drop = Vec::new();

        for (pid, pending) in session.pending_outgoing_publishes() {
            let elapsed = current_time.saturating_sub(pending.timestamp);
            if elapsed >= RETRY_TIMEOUT_MS && pending.retry_count < MAX_RETRY_COUNT {
                to_retry.push(pid);
            } else if pending.retry_count >= MAX_RETRY_COUNT {
                to_drop.push(pid);
            }
        }

        // Drop messages that exceeded max retry count
        for pid in to_drop {
            log::warn!(
                "PUBLISH with PID {} exceeded max retry count, dropping",
                pid.value()
            );
            session.complete_outgoing_publish(pid);
        }

        for pid in to_retry {
            if let Some(pending) = session.get_outgoing_publish_mut(pid) {
                pending.retry_count += 1;
                pending.timestamp = current_time;

                let publish_packet = self
                    .handler
                    .create_publish_packet(
                        &pending.topic,
                        pending.qos,
                        pending.retain,
                        &pending.payload,
                        Some(pid),
                        true, // DUP flag
                    )
                    .map_err(Into::into)?;

                log::debug!(
                    "Retrying PUBLISH for PID: {} (attempt {})",
                    pid.value(),
                    pending.retry_count
                );
                self.send_packet(&publish_packet).await?;
            }
        }

        // Handle PUBREL retries - simplified approach for now
        // TODO: This should be improved to track timestamps and retry counts per PUBREL
        // For now, we limit to a basic approach to avoid infinite retries
        let pubrels_to_retry: Vec<_> = session.pending_outgoing_pubrels().take(10).collect();

        for pid in pubrels_to_retry {
            let pubrel_packet = self
                .handler
                .create_pubrel_packet(*pid)
                .map_err(Into::into)?;

            // Send PUBREL but limit frequency
            if self.last_packet_sent_time.saturating_add(1000) < current_time {
                self.send_packet(&pubrel_packet).await?;
                log::debug!("Retrying PUBREL for PID: {}", pid.value());
            }
        }

        Ok(())
    }
}
