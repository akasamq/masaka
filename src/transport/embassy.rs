use embassy_net::tcp::TcpSocket;
use embassy_net::IpEndpoint;
use embedded_io::ErrorType;
use embedded_io_async::{Read, Write};

use crate::error::TransportError;
use crate::transport::MqttTransport;

/// An transport implementation for MQTT over an embassy-net TCP stream.
pub struct TcpTransport<'a> {
    socket: TcpSocket<'a>,
}

impl<'a> ErrorType for TcpTransport<'a> {
    type Error = TransportError;
}

impl<'a> TcpTransport<'a> {
    /// Creates a new `TcpTransport` and connects to the given address.
    pub async fn new(
        mut socket: TcpSocket<'a>,
        remote_endpoint: IpEndpoint,
    ) -> Result<Self, TransportError> {
        socket
            .connect(remote_endpoint)
            .await
            .map_err(TransportError::from)?;
        Ok(Self { socket })
    }
}

impl<'a> MqttTransport for TcpTransport<'a> {
    async fn close(&mut self) -> Result<(), TransportError> {
        self.socket.close();
        // In embassy-net, close is not async and does not return an error.
        // It signals the connection to start closing. For a graceful shutdown,
        // one might want to wait for the socket state to become `Closed`.
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.socket.state() == embassy_net::tcp::State::Established
    }
}

impl<'a> Read for TcpTransport<'a> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.socket.read(buf).await.map_err(TransportError::from)
    }
}

impl<'a> Write for TcpTransport<'a> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.socket.write(buf).await.map_err(TransportError::from)
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.socket.flush().await.map_err(TransportError::from)
    }
}
