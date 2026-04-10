use hashbrown::{HashMap, HashSet};
use mqtt_proto::{Pid, QoS, TopicFilter};

use crate::error::MqttError;
use crate::state::{SubscriptionInfo, SubscriptionState};

use super::{InflightMessage, SessionState};

const DEFAULT_SUB_CAPACITY: usize = 32;

#[derive(Debug)]
pub struct InMemorySession {
    next_pid: Pid,
    pending_publishes: HashMap<Pid, InflightMessage>,
    pending_pubrels: HashSet<Pid>,
    subscriptions: HashMap<Pid, SubscriptionInfo>,
    active_topics: HashSet<TopicFilter>,
    sub_capacity: usize,
    // TODO: For complete QoS 2 support, pending PUBREL messages need to be tracked with more context.
    // This should include a timestamp and retry count to allow for proper retransmission logic
    // without overwhelming the network. A new struct might be needed to hold this state.
    // See `protocol/mod.rs` `handle_retries` for where this would be used.
}

impl InMemorySession {
    pub fn new(sub_capacity: usize) -> Self {
        Self {
            sub_capacity,
            ..Default::default()
        }
    }
}

impl Default for InMemorySession {
    fn default() -> Self {
        Self {
            next_pid: Pid::default(),
            pending_publishes: HashMap::new(),
            pending_pubrels: HashSet::new(),
            subscriptions: HashMap::new(),
            active_topics: HashSet::new(),
            sub_capacity: DEFAULT_SUB_CAPACITY,
        }
    }
}

impl SessionState for InMemorySession {
    fn next_pid(&mut self) -> Pid {
        let pid = self.next_pid;
        self.next_pid += 1;
        pid
    }

    fn store_outgoing_publish(
        &mut self,
        pid: Pid,
        message: InflightMessage,
    ) -> Result<(), MqttError> {
        // TODO: check for capacity if we want to limit inflight messages at this level.
        self.pending_publishes.insert(pid, message);
        Ok(())
    }

    fn get_outgoing_publish_mut(&mut self, pid: Pid) -> Option<&mut InflightMessage> {
        self.pending_publishes.get_mut(&pid)
    }

    fn complete_outgoing_publish(&mut self, pid: Pid) -> Option<InflightMessage> {
        self.pending_publishes.remove(&pid)
    }

    fn pending_outgoing_publishes(&self) -> impl Iterator<Item = (Pid, &InflightMessage)> {
        self.pending_publishes.iter().map(|(p, m)| (*p, m))
    }

    fn store_outgoing_pubrel(&mut self, pid: Pid) -> Result<(), MqttError> {
        self.pending_pubrels.insert(pid);
        Ok(())
    }

    fn complete_outgoing_pubrel(&mut self, pid: Pid) -> Option<Pid> {
        if self.pending_pubrels.remove(&pid) {
            Some(pid)
        } else {
            None
        }
    }

    fn pending_outgoing_pubrels(&self) -> impl Iterator<Item = &Pid> {
        self.pending_pubrels.iter()
    }

    fn add_subscription(
        &mut self,
        pid: Pid,
        topic_filter: TopicFilter,
        qos: QoS,
    ) -> Result<(), MqttError> {
        if self.subscriptions.len() >= self.sub_capacity {
            // TODO: Define a `MqttError::SubscriptionLimitExceeded` variant for more precise error handling.
            // This would allow the caller to distinguish between different internal limits.
            return Err(MqttError::Internal);
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

    fn confirm_subscription(&mut self, pid: Pid, return_codes: &[u8]) -> Result<(), MqttError> {
        if let Some(subscription) = self.subscriptions.get_mut(&pid) {
            // TODO: Enhance subscription confirmation logic to handle multiple topics in a single SUBACK.
            // A single `SUBSCRIBE` packet can contain multiple topic filters. The `SUBACK` contains a list
            // of return codes, one for each filter. The current implementation only handles one filter per
            // subscribe request and thus only checks the first return code.
            // A complete implementation should:
            // 1. Associate a single PID with a list of topic filters sent in one packet.
            // 2. Iterate through the `return_codes` and update the state of each topic filter individually.
            // This is crucial for robustly handling partial subscription successes/failures.
            if let Some(&return_code) = return_codes.first() {
                if return_code <= 2 {
                    // QoS 0, 1, 2
                    subscription.state = SubscriptionState::Active;
                    if self.active_topics.len() < self.sub_capacity {
                        self.active_topics.insert(subscription.topic_filter.clone());
                    } else {
                        // TODO: Define and use `MqttError::SubscriptionLimitExceeded` here as well.
                        // This indicates that the session cannot store more active topic filters.
                        return Err(MqttError::Internal);
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

    fn remove_subscription(&mut self, pid: Pid) -> Option<SubscriptionInfo> {
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

    fn clear(&mut self) {
        self.next_pid = Pid::default();
        self.pending_publishes.clear();
        self.subscriptions.clear();
        self.active_topics.clear();
        self.pending_pubrels.clear();
    }
}
