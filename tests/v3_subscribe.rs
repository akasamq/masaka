#![cfg(all(feature = "tokio", not(feature = "embassy")))]

mod common;

use std::time::Duration;

use masaka::{
    ClientEvent, MqttClient, PublishConfig, QoS, TopicFilter, protocol::V3Handler,
    state::ReceivedMessage, transport::TcpTransport,
};
use tokio::time::timeout;

use common::TIMEOUT;

async fn drain_sub_confirmed(client: &mut MqttClient<TcpTransport, V3Handler>) {
    loop {
        let ev = timeout(TIMEOUT, client.poll()).await.unwrap().unwrap();
        if matches!(ev, Some(ClientEvent::SubscriptionConfirmed(_))) {
            return;
        }
    }
}

async fn recv_message(client: &mut MqttClient<TcpTransport, V3Handler>) -> ReceivedMessage {
    loop {
        let ev = timeout(TIMEOUT, client.poll()).await.unwrap().unwrap();
        if let Some(ClientEvent::MessageReceived(msg)) = ev {
            let _ = client.next_message();
            return msg;
        }
    }
}

// Subscribe & receive

#[tokio::test]
async fn v3_subscribe_exact_topic() {
    let addr = common::server().await;
    let mut sub = common::client_v3(addr, "v3-sub-exact-sub").await;
    let mut pub_c = common::client_v3(addr, "v3-sub-exact-pub").await;

    let filter = TopicFilter::try_from("sensor/temp").unwrap();
    timeout(TIMEOUT, sub.subscribe(&filter, QoS::Level0))
        .await
        .unwrap()
        .unwrap();
    drain_sub_confirmed(&mut sub).await;

    let topic = masaka::topic!("sensor/temp");
    timeout(TIMEOUT, pub_c.publish(&topic, b"42", PublishConfig::qos0()))
        .await
        .unwrap()
        .unwrap();

    let msg = recv_message(&mut sub).await;
    assert_eq!(msg.topic.to_string(), "sensor/temp");
    assert_eq!(msg.payload, b"42");
}

#[tokio::test]
async fn v3_subscribe_single_level_wildcard() {
    let addr = common::server().await;
    let mut sub = common::client_v3(addr, "v3-sub-plus-sub").await;
    let mut pub_c = common::client_v3(addr, "v3-sub-plus-pub").await;

    let filter = TopicFilter::try_from("sensor/+").unwrap();
    timeout(TIMEOUT, sub.subscribe(&filter, QoS::Level0))
        .await
        .unwrap()
        .unwrap();
    drain_sub_confirmed(&mut sub).await;

    for topic_str in &["sensor/temp", "sensor/humidity"] {
        let topic = masaka::TopicName::try_from(*topic_str).unwrap();
        timeout(
            TIMEOUT,
            pub_c.publish(&topic, topic_str.as_bytes(), PublishConfig::qos0()),
        )
        .await
        .unwrap()
        .unwrap();
    }

    for expected in &["sensor/temp", "sensor/humidity"] {
        let msg = recv_message(&mut sub).await;
        assert_eq!(msg.topic.to_string(), *expected);
    }
}

#[tokio::test]
async fn v3_subscribe_multi_level_wildcard() {
    let addr = common::server().await;
    let mut sub = common::client_v3(addr, "v3-sub-hash-sub").await;
    let mut pub_c = common::client_v3(addr, "v3-sub-hash-pub").await;

    let filter = TopicFilter::try_from("home/#").unwrap();
    timeout(TIMEOUT, sub.subscribe(&filter, QoS::Level0))
        .await
        .unwrap()
        .unwrap();
    drain_sub_confirmed(&mut sub).await;

    let topics = ["home/living/temp", "home/kitchen/light", "home/bed/door"];
    for t in &topics {
        let topic = masaka::TopicName::try_from(*t).unwrap();
        timeout(
            TIMEOUT,
            pub_c.publish(&topic, t.as_bytes(), PublishConfig::qos0()),
        )
        .await
        .unwrap()
        .unwrap();
    }

    for expected in &topics {
        let msg = recv_message(&mut sub).await;
        assert_eq!(msg.topic.to_string(), *expected);
    }
}

// Unsubscribe

#[tokio::test]
async fn v3_unsubscribe_stops_delivery() {
    let addr = common::server().await;
    let mut sub = common::client_v3(addr, "v3-unsub-sub").await;
    let mut pub_c = common::client_v3(addr, "v3-unsub-pub").await;

    let filter = TopicFilter::try_from("unsub/test").unwrap();
    timeout(TIMEOUT, sub.subscribe(&filter, QoS::Level0))
        .await
        .unwrap()
        .unwrap();
    drain_sub_confirmed(&mut sub).await;

    // Verify delivery works before unsub.
    let topic = masaka::topic!("unsub/test");
    timeout(
        TIMEOUT,
        pub_c.publish(&topic, b"before", PublishConfig::qos0()),
    )
    .await
    .unwrap()
    .unwrap();
    let msg = recv_message(&mut sub).await;
    assert_eq!(msg.payload, b"before");

    // Unsubscribe.
    timeout(TIMEOUT, sub.unsubscribe(&filter))
        .await
        .unwrap()
        .unwrap();

    // Drain the UnsubscriptionConfirmed event.
    loop {
        let ev = timeout(TIMEOUT, sub.poll()).await.unwrap().unwrap();
        if matches!(ev, Some(ClientEvent::UnsubscriptionConfirmed(_))) {
            break;
        }
    }

    // Publish again — subscriber should not receive it.
    timeout(
        TIMEOUT,
        pub_c.publish(&topic, b"after", PublishConfig::qos0()),
    )
    .await
    .unwrap()
    .unwrap();

    // Give the server a moment to route the message, then verify no delivery.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        sub.next_message().is_none(),
        "subscriber should not receive message after unsubscribe"
    );
}

// Subscribe with QoS downgrade

#[tokio::test]
async fn v3_subscribe_qos1_receives_qos1_message() {
    let addr = common::server().await;
    let mut sub = common::client_v3(addr, "v3-sub-q1-sub").await;
    let mut pub_c = common::client_v3(addr, "v3-sub-q1-pub").await;

    let filter = TopicFilter::try_from("qos1/topic").unwrap();
    timeout(TIMEOUT, sub.subscribe(&filter, QoS::Level1))
        .await
        .unwrap()
        .unwrap();
    drain_sub_confirmed(&mut sub).await;

    let topic = masaka::topic!("qos1/topic");
    timeout(
        TIMEOUT,
        pub_c.publish(&topic, b"qos1 payload", PublishConfig::qos1()),
    )
    .await
    .unwrap()
    .unwrap();

    // Drive pub_client PUBACK exchange.
    loop {
        let ev = timeout(TIMEOUT, pub_c.poll()).await.unwrap().unwrap();
        if matches!(ev, Some(ClientEvent::MessageAcknowledged(_))) {
            break;
        }
    }

    let msg = recv_message(&mut sub).await;
    assert_eq!(msg.payload, b"qos1 payload");
    assert_eq!(msg.qos, QoS::Level1);
}
