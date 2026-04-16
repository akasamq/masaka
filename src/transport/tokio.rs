use core::net::SocketAddr;
use core::pin::Pin;
use core::task::{Context, Poll};
use core::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;

use crate::error::TransportError;
use crate::transport::MqttTransport;

pub trait AsyncReadWrite: tokio::io::AsyncRead + tokio::io::AsyncWrite {}

impl AsyncReadWrite for TcpTransport {}

/// A transport implementation for MQTT over a Tokio-based TCP stream.
pub struct TcpTransport {
    stream: Option<TcpStream>,
}

impl TcpTransport {
    /// Creates a new `TcpTransport` and connects to the given address.
    pub async fn new(addr: SocketAddr) -> Result<Self, TransportError> {
        let stream = TcpStream::connect(addr)
            .await
            .map_err(TransportError::from)?;
        Ok(Self {
            stream: Some(stream),
        })
    }

    /// Creates a `TcpTransport` with a connection timeout.
    pub async fn new_with_timeout(
        addr: SocketAddr,
        connect_timeout: Duration,
    ) -> Result<Self, TransportError> {
        let connect_future = TcpStream::connect(addr);
        let stream = tokio::time::timeout(connect_timeout, connect_future)
            .await
            .map_err(|_| TransportError::Timeout)? // Outer error is from timeout
            .map_err(TransportError::from)?; // Inner error is from connect

        Ok(Self {
            stream: Some(stream),
        })
    }

    fn get_stream_mut(&mut self) -> Result<&mut TcpStream, TransportError> {
        self.stream.as_mut().ok_or(TransportError::ConnectionLost)
    }
}

impl MqttTransport for TcpTransport {
    async fn close(&mut self) -> Result<(), TransportError> {
        if let Some(mut stream) = self.stream.take() {
            stream.shutdown().await.map_err(TransportError::from)?;
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.stream.is_some()
    }
}

impl AsyncRead for TcpTransport {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.stream.as_mut() {
            Some(stream) => Pin::new(stream).poll_read(cx, buf),
            None => Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                TransportError::ConnectionLost,
            ))),
        }
    }
}

impl AsyncWrite for TcpTransport {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.stream.as_mut() {
            Some(stream) => Pin::new(stream).poll_write(cx, buf),
            None => Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                TransportError::ConnectionLost,
            ))),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.stream.as_mut() {
            Some(stream) => Pin::new(stream).poll_flush(cx),
            None => Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                TransportError::ConnectionLost,
            ))),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.stream.as_mut() {
            Some(stream) => Pin::new(stream).poll_shutdown(cx),
            None => Poll::Ready(Ok(())),
        }
    }
}
