use core::fmt;

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
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

    /// The client identifier.
    client_id: String,

    /// Indicates if this is a clean session.
    clean_session: bool,

    /// The keep-alive interval in seconds.
    keep_alive: u16,

    /// A map of active subscriptions, keyed by packet ID.
    subscriptions: BTreeMap<Pid, SubscriptionInfo>,

    /// A set of confirmed subscription topics.
    active_topics: BTreeSet<TopicFilter>,

    /// A queue of received messages.
    received_messages: VecDeque<ReceivedMessage>,

    /// Client statistics.
    stats: ClientStats,

    /// The maximum number of subscriptions.
    sub_capacity: usize,
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

const DEFAULT_SUB_CAPACITY: usize = 32;
const DEFAULT_MSG_CAPACITY: usize = 64;

impl ClientState {
    /// Creates a new `ClientState`.
    pub fn new(client_id: String, sub_capacity: usize, msg_capacity: usize) -> Self {
        Self {
            connection_state: ConnectionState::Disconnected,
            client_id,
            clean_session: true,
            keep_alive: 60,
            subscriptions: BTreeMap::new(),
            active_topics: BTreeSet::new(),
            received_messages: VecDeque::with_capacity(msg_capacity),
            stats: ClientStats::default(),
            sub_capacity,
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

                // Clean up state (if Clean Session)
                if self.clean_session {
                    self.clear_session_state();
                }
            }
            (_, ConnectionState::Error) => {
                self.stats.error_count += 1;
            }
            _ => {}
        }

        self.connection_state = state;
    }

    /// Returns the client ID.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Sets session parameters like clean session and keep-alive.
    pub fn set_session_params(&mut self, clean_session: bool, keep_alive: u16) {
        self.clean_session = clean_session;
        self.keep_alive = keep_alive;
    }

    /// Adds a new pending subscription.
    pub fn add_subscription(
        &mut self,
        pid: Pid,
        topic_filter: TopicFilter,
        qos: QoS,
    ) -> Result<(), ()> {
        if self.subscriptions.len() >= self.sub_capacity {
            return Err(());
        }

        let subscription = SubscriptionInfo {
            topic_filter: topic_filter.clone(),
            qos,
            state: SubscriptionState::Pending,
            retry_count: 0,
        };

        self.subscriptions.insert(pid, subscription);
        log::debug!(
            "Added subscription: {} (PID: {})",
            topic_filter,
            pid.value()
        );
        Ok(())
    }

    /// Confirms a subscription based on the `SUBACK` response.
    pub fn confirm_subscription(&mut self, pid: Pid, return_codes: &[u8]) -> Result<(), ()> {
        if let Some(subscription) = self.subscriptions.get_mut(&pid) {
            // Check return codes (simplified handling, only checks the first one)
            if let Some(&return_code) = return_codes.first() {
                if return_code <= 2 {
                    // QoS 0, 1, 2
                    subscription.state = SubscriptionState::Active;
                    if self.active_topics.len() < self.sub_capacity {
                        self.active_topics.insert(subscription.topic_filter.clone());
                    } else {
                        return Err(());
                    }
                    log::info!(
                        "Subscription confirmed: {} (QoS: {})",
                        subscription.topic_filter,
                        return_code
                    );
                } else {
                    subscription.state = SubscriptionState::Failed;
                    log::warn!(
                        "Subscription failed: {} (return code: {})",
                        subscription.topic_filter,
                        return_code
                    );
                }
            }
        }
        Ok(())
    }

    /// Removes a subscription by its packet ID.
    pub fn remove_subscription(&mut self, pid: Pid) -> Option<SubscriptionInfo> {
        if let Some(subscription) = self.subscriptions.remove(&pid) {
            self.active_topics.remove(&subscription.topic_filter);
            log::debug!(
                "Removed subscription: {} (PID: {})",
                subscription.topic_filter,
                pid.value()
            );
            Some(subscription)
        } else {
            None
        }
    }

    /// Returns an iterator over active subscriptions.
    pub fn active_subscriptions(&self) -> impl Iterator<Item = &SubscriptionInfo> {
        self.subscriptions
            .values()
            .filter(|sub| sub.state == SubscriptionState::Active)
    }

    /// Checks if the client is subscribed to a specific topic filter.
    pub fn is_subscribed(&self, topic_filter: &TopicFilter) -> bool {
        self.active_topics.contains(topic_filter)
    }

    /// Adds a received message to the queue.
    pub fn add_received_message(&mut self, message: ReceivedMessage) {
        if self.received_messages.len() >= self.msg_capacity {
            // Queue is full, remove the oldest message
            self.received_messages.pop_front();
        }

        self.received_messages.push_back(message);
        self.stats.messages_received += 1;
        self.stats.publishes_received += 1;
    }

    /// Pops the next received message from the queue.
    pub fn pop_received_message(&mut self) -> Option<ReceivedMessage> {
        self.received_messages.pop_front()
    }

    /// Returns the number of messages in the received queue.
    pub fn received_message_count(&self) -> usize {
        self.received_messages.len()
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

    /// Clears the session state if clean session is enabled.
    fn clear_session_state(&mut self) {
        self.subscriptions.clear();
        self.active_topics.clear();
        self.received_messages.clear();
        log::debug!("Session state cleared (Clean Session)");
    }

    /// Resets all statistics to zero.
    pub fn reset_stats(&mut self) {
        self.stats = ClientStats::default();
    }
}

impl Default for ClientState {
    fn default() -> Self {
        Self::new(
            "default_client".into(),
            DEFAULT_SUB_CAPACITY,
            DEFAULT_MSG_CAPACITY,
        )
    }
}
