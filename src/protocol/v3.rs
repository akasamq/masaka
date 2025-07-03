use alloc::sync::Arc;

use bytes::Bytes;
use mqtt_proto::{
    v3::{self, Connect, Header, Packet, Publish, Subscribe, Unsubscribe},
    Error as MqttProtoError, Pid, Protocol, QoS, QosPid, TopicFilter, TopicName, VarBytes,
};

use crate::protocol::{MqttProtocolHandler, PacketAction};

/// MQTT v3.1.1 Protocol Handler
#[derive(Debug, Default)]
pub struct V3Handler;

impl V3Handler {
    /// Creates a new V3 protocol handler
    pub fn new() -> Self {
        Self
    }
}

impl MqttProtocolHandler for V3Handler {
    type Packet = Packet;
    type Error = MqttProtoError;
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
            protocol: Protocol::V311,
            keep_alive,
            client_id: Arc::new(client_id.to_string()),
            clean_session,
            last_will: None, // TODO: Will be handled by create_connect_with_will_packet
            username: username.map(|s| Arc::new(s.to_string())),
            password: password.map(|p| Bytes::from(p.to_vec())),
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
                    return Err(MqttProtoError::ZeroPid);
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
        };

        Ok(Packet::Publish(publish))
    }

    fn create_subscribe_packet(
        &self,
        subscriptions: &[(TopicFilter, QoS)],
        pid: Pid,
    ) -> Result<Self::Packet, Self::Error> {
        if subscriptions.is_empty() {
            return Err(MqttProtoError::EmptySubscription);
        }

        let subscribe = Subscribe {
            pid,
            topics: subscriptions.iter().cloned().collect(),
        };

        Ok(Packet::Subscribe(subscribe))
    }

    fn create_unsubscribe_packet(
        &self,
        topics: &[TopicFilter],
        pid: Pid,
    ) -> Result<Self::Packet, Self::Error> {
        if topics.is_empty() {
            return Err(MqttProtoError::EmptySubscription);
        }

        let unsubscribe = Unsubscribe {
            pid,
            topics: topics.iter().cloned().collect(),
        };

        Ok(Packet::Unsubscribe(unsubscribe))
    }

    fn create_puback_packet(&self, pid: Pid) -> Result<Self::Packet, Self::Error> {
        Ok(Packet::Puback(pid))
    }

    fn create_pubrec_packet(&self, pid: Pid) -> Result<Self::Packet, Self::Error> {
        Ok(Packet::Pubrec(pid))
    }

    fn create_pubrel_packet(&self, pid: Pid) -> Result<Self::Packet, Self::Error> {
        Ok(Packet::Pubrel(pid))
    }

    fn create_pubcomp_packet(&self, pid: Pid) -> Result<Self::Packet, Self::Error> {
        Ok(Packet::Pubcomp(pid))
    }

    fn create_pingreq_packet(&self) -> Self::Packet {
        Packet::Pingreq
    }

    fn create_disconnect_packet(&self) -> Self::Packet {
        Packet::Disconnect
    }

    fn encode_packet(&self, packet: &Self::Packet) -> Result<VarBytes, Self::Error> {
        packet.encode()
    }

    fn handle_packet(&mut self, packet: Self::Packet) -> Result<PacketAction, Self::Error> {
        use v3::Packet as P;

        match packet {
            P::Connack(connack) => Ok(PacketAction::ConnectAck {
                session_present: connack.session_present,
                return_code: connack.code as u8,
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

            P::Puback(pid) => Ok(PacketAction::PublishAck { pid }),

            P::Pubrec(pid) => Ok(PacketAction::PublishRec { pid }),

            P::Pubrel(pid) => Ok(PacketAction::PublishRelease { pid }),

            P::Pubcomp(pid) => Ok(PacketAction::PublishComplete { pid }),

            P::Suback(suback) => Ok(PacketAction::SubscribeAck {
                pid: suback.pid,
                return_codes: suback.topics.iter().map(|code| *code as u8).collect(),
            }),

            P::Unsuback(pid) => Ok(PacketAction::UnsubscribeAck { pid }),

            P::Pingresp => Ok(PacketAction::PingResponse),

            // These packets should not be received by client
            P::Connect(_) | P::Subscribe(_) | P::Unsubscribe(_) | P::Pingreq | P::Disconnect => {
                Err(MqttProtoError::InvalidHeader)
            }
        }
    }
}

impl V3Handler {
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
    ) -> Result<Packet, MqttProtoError> {
        let last_will = Some(v3::LastWill {
            topic_name: will_topic.clone(),
            message: Bytes::from(will_message.to_vec()),
            qos: will_qos,
            retain: will_retain,
        });

        let connect = Connect {
            protocol: Protocol::V311,
            keep_alive,
            client_id: Arc::new(client_id.to_string()),
            clean_session,
            last_will,
            username: username.map(|s| Arc::new(s.to_string())),
            password: password.map(|p| Bytes::from(p.to_vec())),
        };

        Ok(Packet::Connect(connect))
    }
}
