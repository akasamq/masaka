#![cfg(all(feature = "tokio", not(feature = "embassy")))]

mod common;

use masaka::{
    ClientEvent, MqttClient, PublishConfig, QoS, TopicFilter, protocol::V3Handler,
    transport::TcpTransport,
};
use tokio::time::timeout;

use common::TIMEOUT;

/// Drive `poll()` until a `MessageReceived` event arrives or the timeout fires.
async fn recv_message(client: &mut MqttClient<TcpTransport, V3Handler>) -> ClientEvent {
    loop {
        let event = timeout(TIMEOUT, client.poll())
            .await
            .expect("poll timed out")
            .expect("poll error");
        if let Some(e @ ClientEvent::MessageReceived(_)) = event {
            return e;
        }
    }
}

/// Drive `poll()` until a `MessageAcknowledged` event arrives.
async fn recv_ack(client: &mut MqttClient<TcpTransport, V3Handler>) -> ClientEvent {
    loop {
        let event = timeout(TIMEOUT, client.poll())
            .await
            .expect("poll timed out")
            .expect("poll error");
        if let Some(e @ ClientEvent::MessageAcknowledged(_)) = event {
            return e;
        }
    }
}

// QoS 0

#[tokio::test]
async fn v3_publish_qos0_loopback() {
    let addr = common::server().await;
    let mut client = common::client_v3(addr, "v3-pub-q0").await;

    let filter = TopicFilter::try_from("test/qos0").unwrap();
    timeout(TIMEOUT, client.subscribe(&filter, QoS::Level0))
        .await
        .unwrap()
        .unwrap();

    // Consume the SubscriptionConfirmed event.
    loop {
        let ev = timeout(TIMEOUT, client.poll()).await.unwrap().unwrap();
        if matches!(ev, Some(ClientEvent::SubscriptionConfirmed(_))) {
            break;
        }
    }

    let topic = masaka::topic!("test/qos0");
    timeout(
        TIMEOUT,
        client.publish(&topic, b"hello qos0", PublishConfig::qos0()),
    )
    .await
    .unwrap()
    .unwrap();

    let ClientEvent::MessageReceived(msg) = recv_message(&mut client).await else {
        panic!("expected MessageReceived");
    };
    assert_eq!(msg.topic.to_string(), "test/qos0");
    assert_eq!(msg.payload, b"hello qos0");
    assert_eq!(msg.qos, QoS::Level0);
}

// QoS 1

#[tokio::test]
async fn v3_publish_qos1_loopback() {
    let addr = common::server().await;
    let mut pub_client = common::client_v3(addr, "v3-pub-q1-pub").await;
    let mut sub_client = common::client_v3(addr, "v3-pub-q1-sub").await;

    let filter = TopicFilter::try_from("test/qos1").unwrap();
    timeout(TIMEOUT, sub_client.subscribe(&filter, QoS::Level1))
        .await
        .unwrap()
        .unwrap();

    // Drain subscription confirmation.
    loop {
        let ev = timeout(TIMEOUT, sub_client.poll()).await.unwrap().unwrap();
        if matches!(ev, Some(ClientEvent::SubscriptionConfirmed(_))) {
            break;
        }
    }

    let topic = masaka::topic!("test/qos1");
    timeout(
        TIMEOUT,
        pub_client.publish(&topic, b"hello qos1", PublishConfig::qos1()),
    )
    .await
    .unwrap()
    .unwrap();

    // pub_client should receive PUBACK.
    recv_ack(&mut pub_client).await;

    // sub_client should receive the message.
    let ClientEvent::MessageReceived(msg) = recv_message(&mut sub_client).await else {
        panic!("expected MessageReceived");
    };
    assert_eq!(msg.topic.to_string(), "test/qos1");
    assert_eq!(msg.payload, b"hello qos1");
    assert_eq!(msg.qos, QoS::Level1);
}

// QoS 2

#[tokio::test]
async fn v3_publish_qos2_loopback() {
    let addr = common::server().await;
    let mut pub_client = common::client_v3(addr, "v3-pub-q2-pub").await;
    let mut sub_client = common::client_v3(addr, "v3-pub-q2-sub").await;

    let filter = TopicFilter::try_from("test/qos2").unwrap();
    timeout(TIMEOUT, sub_client.subscribe(&filter, QoS::Level2))
        .await
        .unwrap()
        .unwrap();

    loop {
        let ev = timeout(TIMEOUT, sub_client.poll()).await.unwrap().unwrap();
        if matches!(ev, Some(ClientEvent::SubscriptionConfirmed(_))) {
            break;
        }
    }

    let topic = masaka::topic!("test/qos2");
    timeout(
        TIMEOUT,
        pub_client.publish(&topic, b"hello qos2", PublishConfig::qos2()),
    )
    .await
    .unwrap()
    .unwrap();

    // Publisher needs to drive the QoS-2 handshake (PUBREC → PUBREL → PUBCOMP).
    recv_ack(&mut pub_client).await;

    // Subscriber should eventually receive the message.
    let ClientEvent::MessageReceived(msg) = recv_message(&mut sub_client).await else {
        panic!("expected MessageReceived");
    };
    assert_eq!(msg.topic.to_string(), "test/qos2");
    assert_eq!(msg.payload, b"hello qos2");
    assert_eq!(msg.qos, QoS::Level2);
}

// Multiple messages / retained check

#[tokio::test]
async fn v3_publish_multiple_qos0() {
    let addr = common::server().await;
    let mut client = common::client_v3(addr, "v3-multi").await;

    let filter = TopicFilter::try_from("test/multi").unwrap();
    timeout(TIMEOUT, client.subscribe(&filter, QoS::Level0))
        .await
        .unwrap()
        .unwrap();

    loop {
        let ev = timeout(TIMEOUT, client.poll()).await.unwrap().unwrap();
        if matches!(ev, Some(ClientEvent::SubscriptionConfirmed(_))) {
            break;
        }
    }

    let topic = masaka::topic!("test/multi");
    for i in 0u8..5 {
        timeout(TIMEOUT, client.publish(&topic, &[i], PublishConfig::qos0()))
            .await
            .unwrap()
            .unwrap();
    }

    for i in 0u8..5 {
        let ClientEvent::MessageReceived(msg) = recv_message(&mut client).await else {
            panic!("expected MessageReceived");
        };
        assert_eq!(msg.payload, &[i]);
    }
}
