#![cfg(all(feature = "tokio", not(feature = "embassy")))]

mod common;

use tokio::time::timeout;

use common::TIMEOUT;

#[tokio::test]
async fn v3_connect_and_disconnect() {
    let addr = common::server().await;
    let mut client = common::client_v3(addr, "v3-conn-disc").await;

    timeout(TIMEOUT, client.disconnect())
        .await
        .unwrap()
        .unwrap();
    assert!(!client.is_connected());
}

#[tokio::test]
async fn v3_connect_clean_session_true() {
    let addr = common::server().await;
    let config = common::base_config("v3-clean").with_clean_session(true);
    let client = common::client_v3_config(addr, config).await;
    assert!(client.is_connected());
}

#[tokio::test]
async fn v3_connect_clean_session_false() {
    let addr = common::server().await;
    let config = common::base_config("v3-persist").with_clean_session(false);
    let client = common::client_v3_config(addr, config).await;
    assert!(client.is_connected());
}

#[tokio::test]
async fn v3_connect_duplicate_client_id_takeover() {
    let addr = common::server().await;

    let mut client_a = common::client_v3(addr, "v3-dup").await;

    // Second client with same ID — broker accepts and takes over.
    let client_b = common::client_v3(addr, "v3-dup").await;
    assert!(client_b.is_connected());

    // client_a will notice the connection was taken over when it polls.
    // We just verify client_b is healthy; no assertion on client_a's state
    // here since that requires a poll() round-trip after broker disconnect.
    let _ = timeout(TIMEOUT, client_a.disconnect()).await;
}

#[tokio::test]
async fn v3_ping_pong() {
    let addr = common::server().await;
    let mut client = common::client_v3(addr, "v3-ping").await;

    timeout(TIMEOUT, client.ping()).await.unwrap().unwrap();

    // After ping, the next poll() should return a PingResponse event.
    let event = timeout(TIMEOUT, client.poll()).await.unwrap().unwrap();
    assert!(
        matches!(event, Some(masaka::ClientEvent::PingResponse)),
        "expected PingResponse, got {:?}",
        event
    );
}
