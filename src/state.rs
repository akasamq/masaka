use core::fmt;

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;

use mqtt_proto::{Pid, QoS, TopicFilter, TopicName};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// The client is disconnected.
    Disconnected,
    /// The client is attempting to connect.
    Connecting,
    /// The client is connected.
    Connected,
    /// The client is disconnecting.
    Disconnecting,
    /// The client has encountered an error state.
    Error,
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => write!(f, "Disconnected"),
            Self::Connecting => write!(f, "Connecting"),
            Self::Connected => write!(f, "Connected"),
            Self::Disconnecting => write!(f, "Disconnecting"),
            Self::Error => write!(f, "Error"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubscriptionInfo {
    /// The topic filter for the subscription.
    pub topic_filter: TopicFilter,
    /// The QoS level of the subscription.
    pub qos: QoS,
    /// The current state of the subscription.
    pub state: SubscriptionState,
    /// The number of retries for this subscription.
    pub retry_count: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionState {
    /// The subscription is pending acknowledgment.
    Pending,
    /// The subscription is active.
    Active,
    /// The client is unsubscribing from this topic.
    Unsubscribing,
    /// The subscription has failed.
    Failed,
}

#[derive(Debug, Clone)]
pub struct ReceivedMessage {
    /// The topic of the received message.
    pub topic: TopicName,
    /// The QoS level of the received message.
    pub qos: QoS,
    /// Indicates if the received message is a retained message.
    pub retain: bool,
    /// The payload of the received message.
    pub payload: Vec<u8>,
    /// The packet ID of the received message (if QoS > 0).
    pub packet_id: Option<Pid>,
    /// The timestamp of when the message was received (in milliseconds).
    pub timestamp: u64,
}

#[derive(Debug)]
pub struct ClientState {
    /// The current state of the connection.
    connection_state: ConnectionState,

    /// A queue of received messages.
    received_messages: VecDeque<ReceivedMessage>,

    /// Client statistics.
    stats: ClientStats,

    /// Indicates if the session was unexpectedly disconnected.
    clean_session: bool,

    /// The maximum number of messages in the receive queue.
    msg_capacity: usize,
}

#[derive(Debug, Default, Clone)]
pub struct ClientStats {
    /// The total number of packets sent.
    pub messages_sent: u32,
    /// The total number of packets received.
    pub messages_received: u32,
    /// The number of `PUBLISH` packets sent.
    pub publishes_sent: u32,
    /// The number of `PUBLISH` packets received.
    pub publishes_received: u32,
    /// The number of successful connections.
    pub connect_count: u32,
    /// The number of disconnections.
    pub disconnect_count: u32,
    /// The number of reconnections.
    pub reconnect_count: u32,
    /// The number of errors encountered.
    pub error_count: u32,
}

const DEFAULT_MSG_CAPACITY: usize = 64;

impl ClientState {
    /// Creates a new `ClientState`.
    pub fn new(_client_id: String, msg_capacity: usize, clean_session: bool) -> Self {
        Self {
            connection_state: ConnectionState::Disconnected,
            received_messages: VecDeque::with_capacity(msg_capacity),
            stats: ClientStats::default(),
            clean_session,
            msg_capacity,
        }
    }

    /// Returns the current connection state.
    pub fn connection_state(&self) -> ConnectionState {
        self.connection_state
    }

    /// Sets the connection state.
    pub fn set_connection_state(&mut self, state: ConnectionState) {
        log::debug!(
            "Connection state changed: {} -> {}",
            self.connection_state,
            state
        );

        match (self.connection_state, state) {
            (ConnectionState::Disconnected, ConnectionState::Connecting) => {
                // Start connecting
            }
            (ConnectionState::Connecting, ConnectionState::Connected) => {
                self.stats.connect_count += 1;
            }
            (ConnectionState::Connected, ConnectionState::Disconnecting) => {
                // Start disconnecting
            }
            (_, ConnectionState::Disconnected) => {
                self.stats.disconnect_count += 1;
                if self.connection_state == ConnectionState::Connected {
                    // Unexpected disconnect, increment reconnect count
                    self.stats.reconnect_count += 1;
                }
            }
            (_, ConnectionState::Error) => {
                self.stats.error_count += 1;
            }
            _ => {}
        }

        self.connection_state = state;
    }

    /// Adds a received message to the queue.
    pub fn add_received_message(&mut self, message: ReceivedMessage) {
        if self.received_messages.len() >= self.msg_capacity {
            // Queue is full, remove the oldest message
            self.received_messages.pop_front();
            log::warn!("Received message queue is full, oldest message dropped.");
        }

        self.received_messages.push_back(message);
        self.stats.messages_received += 1;
        self.stats.publishes_received += 1;
    }

    /// Pops the next received message from the queue.
    pub fn pop_received_message(&mut self) -> Option<ReceivedMessage> {
        self.received_messages.pop_front()
    }

    /// Increments counters for a sent message.
    pub fn record_message_sent(&mut self) {
        self.stats.messages_sent += 1;
        self.stats.publishes_sent += 1;
    }

    /// Returns a reference to the client statistics.
    pub fn stats(&self) -> &ClientStats {
        &self.stats
    }

    /// Clears the volatile state.
    pub fn clear_volatile_state(&mut self) {
        self.received_messages.clear();
        log::debug!("Volatile state cleared");
    }

    /// Resets all statistics to zero.
    pub fn reset_stats(&mut self) {
        self.stats = ClientStats::default();
    }
}

impl Default for ClientState {
    fn default() -> Self {
        Self::new("default_client".into(), DEFAULT_MSG_CAPACITY, true)
    }
}
