#![cfg(all(feature = "tokio", not(feature = "embassy")))]

mod common;

use masaka::{ClientEvent, V5ConnectConfig};
use tokio::time::timeout;

use common::TIMEOUT;

#[tokio::test]
async fn v5_connect_and_disconnect() {
    let addr = common::server().await;
    let mut client = common::client_v5(addr, "v5-conn-disc").await;

    timeout(TIMEOUT, client.disconnect())
        .await
        .unwrap()
        .unwrap();
    assert!(!client.is_connected());
}

#[tokio::test]
async fn v5_connect_clean_session() {
    let addr = common::server().await;
    let config = common::base_config("v5-clean").with_clean_session(true);
    let client = common::client_v5_config(addr, config, V5ConnectConfig::new()).await;
    assert!(client.is_connected());
}

#[tokio::test]
async fn v5_connect_with_session_expiry() {
    let addr = common::server().await;
    let v5_config = V5ConnectConfig::new().with_session_expiry(30);
    let client = common::client_v5_config(addr, common::base_config("v5-expiry"), v5_config).await;
    assert!(client.is_connected());
}

#[tokio::test]
async fn v5_ping_pong() {
    let addr = common::server().await;
    let mut client = common::client_v5(addr, "v5-ping").await;

    timeout(TIMEOUT, client.ping()).await.unwrap().unwrap();

    let event = timeout(TIMEOUT, client.poll()).await.unwrap().unwrap();
    assert!(
        matches!(event, Some(ClientEvent::PingResponse)),
        "expected PingResponse, got {:?}",
        event
    );
}

#[tokio::test]
async fn v5_connect_duplicate_client_id_takeover() {
    let addr = common::server().await;

    let mut client_a = common::client_v5(addr, "v5-dup").await;
    let mut client_b = common::client_v5(addr, "v5-dup").await;
    assert!(client_b.is_connected());

    // Cleanly disconnect client_b; don't care about client_a's state.
    timeout(TIMEOUT, client_b.disconnect())
        .await
        .unwrap()
        .unwrap();
    let _ = timeout(TIMEOUT, client_a.disconnect()).await;
}
