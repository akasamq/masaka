use alloc::vec::Vec;

use bytes::Bytes;
use hashbrown::HashMap;
use mqtt_proto::{
    v5::{
        self, Connect, ConnectProperties, Disconnect, DisconnectReasonCode, Header, LastWill,
        Packet, PubackReasonCode, PubcompReasonCode, Publish, PublishProperties, PubrecReasonCode,
        PubrelReasonCode, Subscribe, SubscribeProperties, SubscriptionOptions, Unsubscribe,
        UnsubscribeProperties, WillProperties,
    },
    Error as MqttProtoError, Pid, Protocol, QoS, QosPid, TopicFilter, TopicName, VarBytes,
};

use crate::config::{V5ConnectConfig, V5PublishConfig, V5SubscribeConfig};
use crate::protocol::{MqttProtocolHandler, PacketAction};

/// Enhanced MQTT v5.0 protocol handler with full feature support.
#[derive(Debug)]
pub struct V5Handler {
    /// Default connect configuration
    pub connect_config: V5ConnectConfig,
    /// Topic alias mapping for outbound messages
    topic_aliases: HashMap<TopicName, u16>,
    /// Next available topic alias
    next_topic_alias: u16,
}

impl V5Handler {
    /// Creates a new MQTT v5.0 handler with default configuration.
    pub fn new() -> Self {
        Self {
            connect_config: V5ConnectConfig::default(),
            topic_aliases: HashMap::new(),
            next_topic_alias: 1,
        }
    }

    /// Creates a new handler with custom connect configuration.
    pub fn with_config(config: V5ConnectConfig) -> Self {
        Self {
            connect_config: config,
            topic_aliases: HashMap::new(),
            next_topic_alias: 1,
        }
    }

    /// Sets the connect configuration.
    pub fn set_connect_config(&mut self, config: V5ConnectConfig) {
        self.connect_config = config;
    }

    /// Creates a PUBLISH packet with enhanced v5.0 properties.
    pub fn create_publish_with_config(
        &mut self,
        topic: &TopicName,
        qos: QoS,
        retain: bool,
        payload: &[u8],
        pid: Option<Pid>,
        dup: bool,
        config: V5PublishConfig,
    ) -> Result<Packet, v5::ErrorV5> {
        let qos_pid = match qos {
            QoS::Level0 => {
                if pid.is_some() {
                    return Err(MqttProtoError::ZeroPid.into());
                }
                QosPid::Level0
            }
            QoS::Level1 => QosPid::Level1(pid.ok_or(MqttProtoError::ZeroPid)?),
            QoS::Level2 => QosPid::Level2(pid.ok_or(MqttProtoError::ZeroPid)?),
        };

        let mut properties = PublishProperties {
            payload_is_utf8: config.payload_format_indicator,
            message_expiry_interval: config.message_expiry_interval,
            topic_alias: config.topic_alias,
            response_topic: config.response_topic,
            correlation_data: config.correlation_data,
            subscription_id: config.subscription_identifiers.first().copied(),
            content_type: config.content_type,
            user_properties: config.user_properties,
        };

        // Handle topic alias optimization
        if let Some(alias) = self.topic_aliases.get(topic) {
            properties.topic_alias = Some(*alias);
        } else if properties.topic_alias.is_none() {
            // Assign new topic alias
            let alias = self.next_topic_alias;
            self.topic_aliases.insert(topic.clone(), alias);
            properties.topic_alias = Some(alias);
            self.next_topic_alias += 1;
        }

        let publish = Publish {
            dup,
            qos_pid,
            retain,
            topic_name: topic.clone(),
            payload: Bytes::from(payload.to_vec()),
            properties,
        };

        Ok(Packet::Publish(publish))
    }

    /// Creates a SUBSCRIBE packet with enhanced v5.0 properties.
    pub fn create_subscribe_with_config(
        &self,
        subscriptions: &[(TopicFilter, QoS)],
        pid: Pid,
        config: V5SubscribeConfig,
    ) -> Result<Packet, v5::ErrorV5> {
        if subscriptions.is_empty() {
            return Err(MqttProtoError::EmptySubscription.into());
        }

        let subscribe = Subscribe {
            pid,
            topics: subscriptions
                .iter()
                .map(|(filter, qos)| (filter.clone(), SubscriptionOptions::new(*qos)))
                .collect(),
            properties: SubscribeProperties {
                subscription_id: config.subscription_identifier,
                user_properties: config.user_properties,
            },
        };

        Ok(Packet::Subscribe(subscribe))
    }

    /// Helper to create a will message with v5.0 properties.
    fn create_will_message(
        topic: &TopicName,
        message: &[u8],
        qos: QoS,
        retain: bool,
    ) -> Result<LastWill, v5::ErrorV5> {
        Ok(LastWill {
            qos,
            retain,
            topic_name: topic.clone(),
            payload: Bytes::from(message.to_vec()),
            properties: WillProperties::default(), // Can be extended for will properties
        })
    }
}

impl Default for V5Handler {
    fn default() -> Self {
        Self::new()
    }
}

impl MqttProtocolHandler for V5Handler {
    type Packet = Packet;
    type Error = v5::ErrorV5;
    type Header = Header;

    fn create_connect_packet(
        &self,
        client_id: &str,
        username: Option<&str>,
        password: Option<&[u8]>,
        keep_alive: u16,
        clean_session: bool,
    ) -> Result<Self::Packet, Self::Error> {
        let properties = ConnectProperties {
            session_expiry_interval: self.connect_config.session_expiry_interval,
            receive_max: self.connect_config.receive_maximum,
            max_packet_size: self.connect_config.maximum_packet_size,
            topic_alias_max: self.connect_config.topic_alias_maximum,
            request_response_info: self.connect_config.request_response_information,
            request_problem_info: self.connect_config.request_problem_information,
            auth_method: self.connect_config.authentication_method.clone(),
            auth_data: self.connect_config.authentication_data.clone(),
            user_properties: self.connect_config.user_properties.clone(),
        };

        let connect = Connect {
            protocol: Protocol::V500,
            clean_start: clean_session,
            keep_alive,
            client_id: client_id.into(),
            last_will: None,
            username: username.map(|s| s.into()),
            password: password.map(|p| Bytes::from(p.to_vec())),
            properties,
        };

        Ok(Packet::Connect(connect))
    }

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
    ) -> Result<Self::Packet, Self::Error> {
        let last_will = Some(Self::create_will_message(
            will_topic,
            will_message,
            will_qos,
            will_retain,
        )?);

        let properties = ConnectProperties {
            session_expiry_interval: self.connect_config.session_expiry_interval,
            receive_max: self.connect_config.receive_maximum,
            max_packet_size: self.connect_config.maximum_packet_size,
            topic_alias_max: self.connect_config.topic_alias_maximum,
            request_response_info: self.connect_config.request_response_information,
            request_problem_info: self.connect_config.request_problem_information,
            auth_method: self.connect_config.authentication_method.clone(),
            auth_data: self.connect_config.authentication_data.clone(),
            user_properties: self.connect_config.user_properties.clone(),
        };

        let connect = Connect {
            protocol: Protocol::V500,
            clean_start: clean_session,
            keep_alive,
            client_id: client_id.into(),
            last_will,
            username: username.map(|s| s.into()),
            password: password.map(|p| Bytes::from(p.to_vec())),
            properties,
        };

        Ok(Packet::Connect(connect))
    }

    fn create_publish_packet(
        &self,
        topic: &TopicName,
        qos: QoS,
        retain: bool,
        payload: &[u8],
        pid: Option<Pid>,
        dup: bool,
    ) -> Result<Self::Packet, Self::Error> {
        let qos_pid = match qos {
            QoS::Level0 => {
                if pid.is_some() {
                    return Err(MqttProtoError::ZeroPid.into());
                }
                QosPid::Level0
            }
            QoS::Level1 => QosPid::Level1(pid.ok_or(MqttProtoError::ZeroPid)?),
            QoS::Level2 => QosPid::Level2(pid.ok_or(MqttProtoError::ZeroPid)?),
        };

        let publish = Publish {
            dup,
            qos_pid,
            retain,
            topic_name: topic.clone(),
            payload: Bytes::from(payload.to_vec()),
            properties: PublishProperties::default(),
        };

        Ok(Packet::Publish(publish))
    }

    fn create_subscribe_packet(
        &self,
        subscriptions: &[(TopicFilter, QoS)],
        pid: Pid,
    ) -> Result<Self::Packet, Self::Error> {
        if subscriptions.is_empty() {
            return Err(MqttProtoError::EmptySubscription.into());
        }

        let subscribe = Subscribe {
            pid,
            topics: subscriptions
                .iter()
                .map(|(filter, qos)| (filter.clone(), SubscriptionOptions::new(*qos)))
                .collect(),
            properties: SubscribeProperties::default(),
        };

        Ok(Packet::Subscribe(subscribe))
    }

    fn create_unsubscribe_packet(
        &self,
        topics: &[TopicFilter],
        pid: Pid,
    ) -> Result<Self::Packet, Self::Error> {
        if topics.is_empty() {
            return Err(MqttProtoError::EmptySubscription.into());
        }

        let unsubscribe = Unsubscribe {
            pid,
            topics: topics.iter().cloned().collect(),
            properties: UnsubscribeProperties::default(),
        };

        Ok(Packet::Unsubscribe(unsubscribe))
    }

    fn create_puback_packet(&self, pid: Pid) -> Result<Self::Packet, Self::Error> {
        Ok(Packet::Puback(v5::Puback::new(
            pid,
            PubackReasonCode::Success,
        )))
    }

    fn create_pubrec_packet(&self, pid: Pid) -> Result<Self::Packet, Self::Error> {
        Ok(Packet::Pubrec(v5::Pubrec::new(
            pid,
            PubrecReasonCode::Success,
        )))
    }

    fn create_pubrel_packet(&self, pid: Pid) -> Result<Self::Packet, Self::Error> {
        Ok(Packet::Pubrel(v5::Pubrel::new(
            pid,
            PubrelReasonCode::Success,
        )))
    }

    fn create_pubcomp_packet(&self, pid: Pid) -> Result<Self::Packet, Self::Error> {
        Ok(Packet::Pubcomp(v5::Pubcomp::new(
            pid,
            PubcompReasonCode::Success,
        )))
    }

    fn create_pingreq_packet(&self) -> Self::Packet {
        Packet::Pingreq
    }

    fn create_disconnect_packet(&self) -> Self::Packet {
        Packet::Disconnect(Disconnect::new(DisconnectReasonCode::NormalDisconnect))
    }

    fn encode_packet(&self, packet: &Self::Packet) -> Result<VarBytes, Self::Error> {
        packet.encode().map_err(Into::into)
    }

    fn handle_packet(&mut self, packet: Self::Packet) -> Result<PacketAction, Self::Error> {
        use v5::Packet as P;

        match packet {
            P::Connack(connack) => {
                // Extract enhanced information from v5.0 CONNACK
                let session_present = connack.session_present;
                let return_code = connack.reason_code as u8;

                // Log important v5.0 properties
                if let Some(max_qos) = connack.properties.max_qos {
                    log::debug!("Broker maximum QoS: {max_qos:?}");
                }
                if let Some(retain_available) = connack.properties.retain_available {
                    log::debug!("Broker retain available: {retain_available}");
                }
                if let Some(server_keep_alive) = connack.properties.server_keep_alive {
                    log::debug!("Server keep alive override: {server_keep_alive}");
                }

                Ok(PacketAction::ConnectAck {
                    session_present,
                    return_code,
                })
            }

            P::Publish(publish) => {
                let (qos, pid) = match publish.qos_pid {
                    QosPid::Level0 => (QoS::Level0, None),
                    QosPid::Level1(pid) => (QoS::Level1, Some(pid)),
                    QosPid::Level2(pid) => (QoS::Level2, Some(pid)),
                };

                // Log v5.0 specific properties for debugging
                if let Some(alias) = publish.properties.topic_alias {
                    log::trace!("Received message with topic alias: {alias}");
                }
                if let Some(expiry) = publish.properties.message_expiry_interval {
                    log::trace!("Message expiry interval: {expiry}s");
                }

                Ok(PacketAction::PublishReceived {
                    topic: publish.topic_name,
                    qos,
                    retain: publish.retain,
                    payload: publish.payload.to_vec(),
                    pid,
                })
            }

            P::Puback(ack) => {
                if ack.reason_code != PubackReasonCode::Success {
                    log::warn!("PUBACK with non-success reason: {:?}", ack.reason_code);
                }
                Ok(PacketAction::PublishAck { pid: ack.pid })
            }

            P::Pubrec(ack) => {
                if ack.reason_code != PubrecReasonCode::Success {
                    log::warn!("PUBREC with non-success reason: {:?}", ack.reason_code);
                }
                Ok(PacketAction::PublishRec { pid: ack.pid })
            }

            P::Pubrel(ack) => {
                if ack.reason_code != PubrelReasonCode::Success {
                    log::warn!("PUBREL with non-success reason: {:?}", ack.reason_code);
                }
                Ok(PacketAction::PublishRelease { pid: ack.pid })
            }

            P::Pubcomp(ack) => {
                if ack.reason_code != PubcompReasonCode::Success {
                    log::warn!("PUBCOMP with non-success reason: {:?}", ack.reason_code);
                }
                Ok(PacketAction::PublishComplete { pid: ack.pid })
            }

            P::Suback(suback) => {
                let return_codes = suback
                    .topics
                    .iter()
                    .map(|code| *code as u8)
                    .collect::<Vec<u8>>();

                // Log subscription failures
                for (i, &code) in return_codes.iter().enumerate() {
                    if code >= 0x80 {
                        log::warn!("Subscription {i} failed with reason code: 0x{code:02X}");
                    }
                }

                Ok(PacketAction::SubscribeAck {
                    pid: suback.pid,
                    return_codes,
                })
            }

            P::Unsuback(unsuback) => {
                // Enhanced unsubscribe handling with reason codes
                for (i, code) in unsuback.topics.iter().enumerate() {
                    match code {
                        mqtt_proto::v5::UnsubscribeReasonCode::Success => {
                            log::debug!("Unsubscription {i} successful");
                        }
                        mqtt_proto::v5::UnsubscribeReasonCode::NoSubscriptionExisted => {
                            log::warn!("Unsubscription {i}: no subscription existed");
                        }
                        _ => {
                            log::error!("Unsubscription {i} failed: {code:?}");
                        }
                    }
                }

                Ok(PacketAction::UnsubscribeAck { pid: unsuback.pid })
            }

            P::Pingresp => Ok(PacketAction::PingResponse),

            P::Disconnect(disconnect) => {
                log::info!("Server disconnect: {:?}", disconnect.reason_code);
                if let Some(reason) = disconnect.properties.reason_string.as_ref() {
                    log::info!("Disconnect reason: {reason}");
                }
                Ok(PacketAction::None)
            }

            // These packets should not be received by a client
            P::Connect(_) | P::Subscribe(_) | P::Unsubscribe(_) | P::Pingreq | P::Auth(_) => {
                Err(v5::ErrorV5::from(MqttProtoError::InvalidHeader))
            }
        }
    }
}
