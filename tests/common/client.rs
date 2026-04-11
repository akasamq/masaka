use std::net::SocketAddr;
use std::time::Duration;

use masaka::{
    protocol::V3Handler, protocol::V5Handler, transport::TcpTransport, ClientConfig, MqttClient,
    ReconnectConfig, V5ConnectConfig,
};
use tokio::time::timeout;

pub const TIMEOUT: Duration = Duration::from_secs(5);

pub fn base_config(id: &str) -> ClientConfig {
    ClientConfig::new(id)
        .with_keep_alive(60)
        .with_reconnect(ReconnectConfig::disabled())
}

/// Create and connect an MQTT v3 client with the default config.
pub async fn client_v3(addr: SocketAddr, id: &str) -> MqttClient<TcpTransport, V3Handler> {
    client_v3_config(addr, base_config(id)).await
}

/// Create and connect an MQTT v3 client with a custom `ClientConfig`.
pub async fn client_v3_config(
    addr: SocketAddr,
    config: ClientConfig,
) -> MqttClient<TcpTransport, V3Handler> {
    let transport = TcpTransport::new(addr).await.unwrap();
    let mut client = MqttClient::new(transport, V3Handler::new(), config);
    timeout(TIMEOUT, client.connect()).await.unwrap().unwrap();
    assert!(client.is_connected());
    client
}

/// Create and connect an MQTT v5 client with default configs.
pub async fn client_v5(addr: SocketAddr, id: &str) -> MqttClient<TcpTransport, V5Handler> {
    client_v5_full(addr, base_config(id), V5ConnectConfig::new()).await
}

/// Create and connect an MQTT v5 client with custom configs.
/// Pass `V5ConnectConfig::new()` for the default v5 connect options.
pub async fn client_v5_full(
    addr: SocketAddr,
    config: ClientConfig,
    v5_config: V5ConnectConfig,
) -> MqttClient<TcpTransport, V5Handler> {
    let transport = TcpTransport::new(addr).await.unwrap();
    let mut client = MqttClient::new_v5(transport, config, v5_config);
    timeout(TIMEOUT, client.connect()).await.unwrap().unwrap();
    assert!(client.is_connected());
    client
}
