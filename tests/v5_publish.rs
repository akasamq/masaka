mod common;

use masaka::{
    protocol::V5Handler, transport::TcpTransport, ClientEvent, MqttClient, PublishConfig, QoS,
    TopicFilter, V5PublishConfig,
};
use tokio::time::timeout;

use common::TIMEOUT;

async fn drain_sub_confirmed(client: &mut MqttClient<TcpTransport, V5Handler>) {
    loop {
        let ev = timeout(TIMEOUT, client.poll()).await.unwrap().unwrap();
        if matches!(ev, Some(ClientEvent::SubscriptionConfirmed(_))) {
            return;
        }
    }
}

async fn recv_message(client: &mut MqttClient<TcpTransport, V5Handler>) -> ClientEvent {
    loop {
        let ev = timeout(TIMEOUT, client.poll()).await.unwrap().unwrap();
        if let Some(e @ ClientEvent::MessageReceived(_)) = ev {
            return e;
        }
    }
}

async fn recv_ack(client: &mut MqttClient<TcpTransport, V5Handler>) {
    loop {
        let ev = timeout(TIMEOUT, client.poll()).await.unwrap().unwrap();
        if matches!(ev, Some(ClientEvent::MessageAcknowledged(_))) {
            return;
        }
    }
}

// QoS 0

#[tokio::test]
async fn v5_publish_qos0_loopback() {
    let addr = common::server().await;
    let mut client = common::client_v5(addr, "v5-pub-q0").await;

    let filter = TopicFilter::try_from("v5/qos0").unwrap();
    timeout(TIMEOUT, client.subscribe(&filter, QoS::Level0))
        .await
        .unwrap()
        .unwrap();
    drain_sub_confirmed(&mut client).await;

    let topic = masaka::topic!("v5/qos0");
    timeout(
        TIMEOUT,
        client.publish(&topic, b"v5 hello qos0", PublishConfig::qos0()),
    )
    .await
    .unwrap()
    .unwrap();

    let ClientEvent::MessageReceived(msg) = recv_message(&mut client).await else {
        panic!("expected MessageReceived");
    };
    assert_eq!(msg.topic.to_string(), "v5/qos0");
    assert_eq!(msg.payload, b"v5 hello qos0");
    assert_eq!(msg.qos, QoS::Level0);
}

// QoS 1

#[tokio::test]
async fn v5_publish_qos1_loopback() {
    let addr = common::server().await;
    let mut pub_c = common::client_v5(addr, "v5-pub-q1-pub").await;
    let mut sub_c = common::client_v5(addr, "v5-pub-q1-sub").await;

    let filter = TopicFilter::try_from("v5/qos1").unwrap();
    timeout(TIMEOUT, sub_c.subscribe(&filter, QoS::Level1))
        .await
        .unwrap()
        .unwrap();
    drain_sub_confirmed(&mut sub_c).await;

    let topic = masaka::topic!("v5/qos1");
    timeout(
        TIMEOUT,
        pub_c.publish(&topic, b"v5 hello qos1", PublishConfig::qos1()),
    )
    .await
    .unwrap()
    .unwrap();

    recv_ack(&mut pub_c).await;

    let ClientEvent::MessageReceived(msg) = recv_message(&mut sub_c).await else {
        panic!("expected MessageReceived");
    };
    assert_eq!(msg.payload, b"v5 hello qos1");
    assert_eq!(msg.qos, QoS::Level1);
}

// QoS 2

#[tokio::test]
async fn v5_publish_qos2_loopback() {
    let addr = common::server().await;
    let mut pub_c = common::client_v5(addr, "v5-pub-q2-pub").await;
    let mut sub_c = common::client_v5(addr, "v5-pub-q2-sub").await;

    let filter = TopicFilter::try_from("v5/qos2").unwrap();
    timeout(TIMEOUT, sub_c.subscribe(&filter, QoS::Level2))
        .await
        .unwrap()
        .unwrap();
    drain_sub_confirmed(&mut sub_c).await;

    let topic = masaka::topic!("v5/qos2");
    timeout(
        TIMEOUT,
        pub_c.publish(&topic, b"v5 hello qos2", PublishConfig::qos2()),
    )
    .await
    .unwrap()
    .unwrap();

    recv_ack(&mut pub_c).await;

    let ClientEvent::MessageReceived(msg) = recv_message(&mut sub_c).await else {
        panic!("expected MessageReceived");
    };
    assert_eq!(msg.payload, b"v5 hello qos2");
    assert_eq!(msg.qos, QoS::Level2);
}

// v5-specific: publish with user properties via publish_v5

#[tokio::test]
async fn v5_publish_with_content_type() {
    let addr = common::server().await;
    let mut client = common::client_v5(addr, "v5-content-type").await;

    let filter = TopicFilter::try_from("v5/content").unwrap();
    timeout(TIMEOUT, client.subscribe(&filter, QoS::Level0))
        .await
        .unwrap()
        .unwrap();
    drain_sub_confirmed(&mut client).await;

    let topic = masaka::topic!("v5/content");
    let v5_pub = V5PublishConfig::new().with_content_type("application/json");
    timeout(
        TIMEOUT,
        client.publish_v5(
            &topic,
            b"{\"key\":\"value\"}",
            PublishConfig::qos0(),
            v5_pub,
        ),
    )
    .await
    .unwrap()
    .unwrap();

    let ClientEvent::MessageReceived(msg) = recv_message(&mut client).await else {
        panic!("expected MessageReceived");
    };
    assert_eq!(msg.payload, b"{\"key\":\"value\"}");
}

// v5-specific: subscribe_v5 with subscription identifier

#[tokio::test]
async fn v5_subscribe_with_id() {
    let addr = common::server().await;
    let mut sub_c = common::client_v5(addr, "v5-subid-sub").await;
    let mut pub_c = common::client_v5(addr, "v5-subid-pub").await;

    let filter = TopicFilter::try_from("v5/subid").unwrap();
    timeout(TIMEOUT, sub_c.subscribe_with_id(&filter, QoS::Level0, 42))
        .await
        .unwrap()
        .unwrap();
    drain_sub_confirmed(&mut sub_c).await;

    let topic = masaka::topic!("v5/subid");
    timeout(
        TIMEOUT,
        pub_c.publish(&topic, b"subid payload", PublishConfig::qos0()),
    )
    .await
    .unwrap()
    .unwrap();

    let ClientEvent::MessageReceived(msg) = recv_message(&mut sub_c).await else {
        panic!("expected MessageReceived");
    };
    assert_eq!(msg.payload, b"subid payload");
}
