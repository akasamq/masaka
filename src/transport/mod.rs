use embedded_io_async::{Read, Write};

use crate::error::TransportError;

/// An abstract transport for MQTT connections.
///
/// This allows the MQTT client to be generic over different network stacks.
#[allow(async_fn_in_trait)]
pub trait MqttTransport: Read<Error = TransportError> + Write<Error = TransportError> {
    /// Closes the connection gracefully.
    ///
    /// Should be safe to call multiple times.
    async fn close(&mut self) -> Result<(), TransportError>;

    /// Checks if the connection is active.
    ///
    /// Returns `true` if the transport can send and receive data.
    fn is_connected(&self) -> bool;

    /// Returns the local address of the connection, if available.
    fn local_addr(&self) -> Option<&str> {
        None
    }

    /// Returns the remote address of the connection, if available.
    fn remote_addr(&self) -> Option<&str> {
        None
    }
}

// TODO: Implement concrete transport implementations
// The following transport implementations are missing but essential for a complete MQTT client:
//
// 1. TcpTransport: Basic TCP socket transport
//    - Should support both std::net and embassy-net
//    - Handle connection timeouts and keepalive
//
// 2. TlsTransport: TLS-secured TCP transport
//    - Support both rustls and native-tls backends
//    - Certificate validation and custom CA support
//
// 3. WebSocketTransport: WebSocket transport for browser environments
//    - MQTT over WebSocket protocol support
//    - Compatible with web assemblies
//
// 4. UnixSocketTransport: Unix domain socket for local IPC
//    - Useful for edge computing and local services
//
// 5. SerialTransport: Serial port transport for embedded devices
//    - Support embedded-hal serial traits
//    - Handle framing for MQTT over serial
//
// These should be implemented in separate modules: tcp.rs, tls.rs, websocket.rs, etc.
