use core::net::SocketAddr;
use core::time::Duration;

use masaka::{
    ClientConfig, MqttClient, ReconnectConfig, V5ConnectConfig, protocol::V3Handler,
    protocol::V5Handler, transport::TcpTransport,
};
use tokio::time::timeout;

pub const TIMEOUT: Duration = Duration::from_secs(5);

pub fn base_config(id: &str) -> ClientConfig {
    ClientConfig::new(id)
        .with_keep_alive(60)
        .with_reconnect(ReconnectConfig::disabled())
}

pub async fn client_v3(addr: SocketAddr, id: &str) -> MqttClient<TcpTransport, V3Handler> {
    client_v3_config(addr, base_config(id)).await
}

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

pub async fn client_v5(addr: SocketAddr, id: &str) -> MqttClient<TcpTransport, V5Handler> {
    client_v5_config(addr, base_config(id), V5ConnectConfig::new()).await
}

pub async fn client_v5_config(
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
