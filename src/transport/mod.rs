#[cfg(feature = "tokio")]
mod tokio;
#[cfg(feature = "tokio")]
pub use tokio::{AsyncReadWrite, TcpTransport};

#[cfg(feature = "embassy")]
mod embassy;
#[cfg(feature = "embassy")]
pub use embassy::{AsyncReadWrite, TcpTransport};

#[cfg(test)]
mod mock;
#[cfg(test)]
pub use mock::MockTransport;

use crate::error::TransportError;

/// An abstract transport for MQTT connections.
///
/// This allows the MQTT client to be generic over different network stacks.
#[allow(async_fn_in_trait)]
pub trait MqttTransport: AsyncReadWrite {
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
