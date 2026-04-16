use alloc::vec::Vec;

use mqtt_proto::{Pid, QoS, TopicFilter, TopicName};

use crate::error::MqttError;
use crate::state::SubscriptionInfo;

mod in_memory;

pub use in_memory::InMemorySession;

// This was PendingPublish in protocol/mod.rs. Renaming for clarity.
#[derive(Debug, Clone)]
pub struct InflightMessage {
    pub topic: TopicName,
    pub qos: QoS,
    pub retain: bool,
    pub payload: Vec<u8>,
    pub retry_count: u8,
    pub timestamp: u64,
}

/// A trait for managing the persistent state of an MQTT session.
/// This allows for different storage backends (in-memory, flash, etc.).
#[allow(async_fn_in_trait)]
pub trait SessionState {
    /// Retrieves the next available Packet ID.
    /// This must be managed as part of the persistent state.
    fn next_pid(&mut self) -> Pid;

    /// Stores an outgoing QoS 1 or QoS 2 message that is awaiting acknowledgment.
    fn store_outgoing_publish(
        &mut self,
        pid: Pid,
        message: InflightMessage,
    ) -> Result<(), MqttError>;

    /// Fetches a mutable reference to a pending outgoing message.
    fn get_outgoing_publish_mut(&mut self, pid: Pid) -> Option<&mut InflightMessage>;

    /// Removes an outgoing message from storage after receiving acknowledgment (e.g., PUBACK, PUBCOMP).
    fn complete_outgoing_publish(&mut self, pid: Pid) -> Option<InflightMessage>;

    /// Retrieves all pending outgoing messages that need to be re-sent upon reconnection.
    fn pending_outgoing_publishes(&self) -> impl Iterator<Item = (Pid, &InflightMessage)>;

    // QoS 2: Handling for PUBREL messages
    // This is for when we receive a PUBREC and need to send a PUBREL.
    // We need to track that we are awaiting a PUBCOMP.
    // TODO: For robust QoS 2 PUBREL retries, the session state should store not just the PID,
    // but also a timestamp and retry count for each pending PUBREL.
    // `pending_outgoing_pubrels` would then return an iterator over a struct containing this information.

    /// Stores a `PUBREL` message that is awaiting a `PUBCOMP`.
    fn store_outgoing_pubrel(&mut self, pid: Pid) -> Result<(), MqttError>;

    /// Removes a `PUBREL` message from storage after receiving `PUBCOMP`.
    fn complete_outgoing_pubrel(&mut self, pid: Pid) -> Option<Pid>;

    /// Retrieves all pending outgoing `PUBREL` messages that need to be re-sent.
    fn pending_outgoing_pubrels(&self) -> impl Iterator<Item = &Pid>;

    /// Adds a new pending subscription.
    fn add_subscription(
        &mut self,
        pid: Pid,
        topic_filter: TopicFilter,
        qos: QoS,
    ) -> Result<(), MqttError>;

    /// Confirms a subscription based on the `SUBACK` response.
    fn confirm_subscription(&mut self, pid: Pid, return_codes: &[u8]) -> Result<(), MqttError>;

    /// Removes a subscription by its topic filter.
    fn remove_subscription(&mut self, topic_filter: &TopicFilter) -> Option<SubscriptionInfo>;

    /// Clears all session state. Called when a clean session starts.
    fn clear(&mut self);
}
