# Masaka

Modular, `no_std`-oriented async MQTT client for Rust, built on **mqtt-proto** (v3.1.1 / v5). Transport and time sources are pluggable (Tokio TCP vs Embassy TCP).

## Features

| Feature | Meaning |
|---------|---------|
| `std` | Standard library–oriented deps (see `Cargo.toml`). |
| `embassy` | Embassy-net TCP `TcpTransport` + Embassy time default. |
| `tokio` | Tokio TCP `TcpTransport` + Tokio time default (implies `std`). |
| `tls` | Reserved; not wired up. |

**Do not enable `embassy` and `tokio` together**

## Quick start (Tokio, host)

```toml
[dependencies]
masaka = { version = "*", default-features = false, features = ["tokio"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread", "net", "time"] }
```

```rust
use masaka::{
    protocol::V3Handler, ClientConfig, MqttClient, PublishConfig, QoS,
    transport::TcpTransport,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transport = TcpTransport::new("127.0.0.1:1883").await?;
    let config = ClientConfig::new("masaka_demo").with_keep_alive(60);
    let mut client = MqttClient::new(transport, V3Handler::new(), config);

    client.connect().await?;

    let topic = masaka::topic!("demo/topic");
    client
        .publish(&topic, b"hello", PublishConfig::qos0())
        .await?;

    Ok(())
}
```

## Building & tests

```bash
cargo check
cargo test --no-default-features --features tokio
```
