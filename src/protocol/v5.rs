use alloc::sync::Arc;
use alloc::vec::Vec;

use bytes::Bytes;
use mqtt_proto::{
    v5::{
        self, Connect, ConnectProperties, Disconnect, DisconnectReasonCode, Header, Packet,
        PubackReasonCode, PubcompReasonCode, Publish, PublishProperties, PubrecReasonCode,
        PubrelReasonCode, Subscribe, SubscribeProperties, SubscriptionOptions, Unsubscribe,
        UnsubscribeProperties, UnsubscribeReasonCode,
    },
    Error as MqttProtoError, Pid, Protocol, QoS, QosPid, TopicFilter, TopicName, VarBytes,
};

use crate::protocol::{MqttProtocolHandler, PacketAction};

/// MQTT v5.0 Protocol Handler
#[derive(Debug, Default)]
pub struct V5Handler;

impl V5Handler {
    /// Creates a new V5 protocol handler
    pub fn new() -> Self {
        Self
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
        let connect = Connect {
            protocol: Protocol::V500,
            keep_alive,
            client_id: Arc::new(client_id.to_string()),
            clean_start: clean_session,
            last_will: None, // TODO: Will be handled by create_connect_with_will_packet
            username: username.map(|s| Arc::new(s.to_string())),
            password: password.map(|p| Bytes::from(p.to_vec())),
            properties: ConnectProperties::default(), // TODO: Add property support
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
        let last_will = Some(v5::LastWill {
            topic_name: will_topic.clone(),
            payload: Bytes::from(will_message.to_vec()),
            qos: will_qos,
            retain: will_retain,
            properties: v5::WillProperties::default(), // TODO: Add will properties support
        });

        let connect = Connect {
            protocol: Protocol::V500,
            keep_alive,
            client_id: Arc::new(client_id.to_string()),
            clean_start: clean_session,
            last_will,
            username: username.map(|s| Arc::new(s.to_string())),
            password: password.map(|p| Bytes::from(p.to_vec())),
            properties: ConnectProperties::default(),
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
            dup: false, // Will be set to true on retransmission
            qos_pid,
            retain,
            topic_name: topic.clone(),
            payload: Bytes::from(payload.to_vec()),
            properties: PublishProperties::default(), // TODO: Add property support
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
            properties: SubscribeProperties::default(), // TODO: Add property support
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
            properties: UnsubscribeProperties::default(), // TODO: Add property support
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
            P::Connack(connack) => Ok(PacketAction::ConnectAck {
                session_present: connack.session_present,
                return_code: connack.reason_code as u8,
            }),

            P::Publish(publish) => {
                let (qos, pid) = match publish.qos_pid {
                    QosPid::Level0 => (QoS::Level0, None),
                    QosPid::Level1(pid) => (QoS::Level1, Some(pid)),
                    QosPid::Level2(pid) => (QoS::Level2, Some(pid)),
                };

                Ok(PacketAction::PublishReceived {
                    topic: publish.topic_name,
                    qos,
                    retain: publish.retain,
                    payload: publish.payload.to_vec(),
                    pid,
                })
            }

            P::Puback(ack) => Ok(PacketAction::PublishAck { pid: ack.pid }),
            P::Pubrec(ack) => Ok(PacketAction::PublishRec { pid: ack.pid }),
            P::Pubrel(ack) => Ok(PacketAction::PublishRelease { pid: ack.pid }),
            P::Pubcomp(ack) => Ok(PacketAction::PublishComplete { pid: ack.pid }),

            P::Suback(suback) => Ok(PacketAction::SubscribeAck {
                pid: suback.pid,
                return_codes: suback
                    .topics
                    .iter()
                    .map(|code| *code as u8)
                    .collect::<Vec<u8>>(),
            }),

            P::Unsuback(unsuback) => {
                // TODO: Handle multiple reason codes properly
                // For now, we just check the first one for simplicity
                if let Some(code) = unsuback.topics.first() {
                    if *code != UnsubscribeReasonCode::Success {
                        log::warn!("Unsubscribe failed with code: {:?}", code);
                    }
                }
                Ok(PacketAction::UnsubscribeAck { pid: unsuback.pid })
            }

            P::Pingresp => Ok(PacketAction::PingResponse),

            P::Disconnect(d) => {
                log::info!("Received disconnect packet: {:?}", d.reason_code);
                // The protocol engine should handle the disconnection state.
                // This action indicates that the server initiated the disconnection.
                Ok(PacketAction::None)
            }

            // These packets should not be received by client
            P::Connect(_) | P::Subscribe(_) | P::Unsubscribe(_) | P::Pingreq | P::Auth(_) => {
                Err(v5::ErrorV5::from(MqttProtoError::InvalidHeader))
            }
        }
    }
}

impl V5Handler {
    /// Creates a CONNECT packet with will message support
    pub fn create_connect_with_will_packet(
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
    ) -> Result<Packet, v5::ErrorV5> {
        let last_will = Some(v5::LastWill {
            topic_name: will_topic.clone(),
            payload: Bytes::from(will_message.to_vec()),
            qos: will_qos,
            retain: will_retain,
            properties: v5::WillProperties::default(), // TODO: Add will properties support
        });

        let connect = Connect {
            protocol: Protocol::V500,
            keep_alive,
            client_id: Arc::new(client_id.to_string()),
            clean_start: clean_session,
            last_will,
            username: username.map(|s| Arc::new(s.to_string())),
            password: password.map(|p| Bytes::from(p.to_vec())),
            properties: ConnectProperties::default(),
        };

        Ok(Packet::Connect(connect))
    }
}
