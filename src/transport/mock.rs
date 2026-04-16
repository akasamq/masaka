use core::sync::atomic::{AtomicBool, Ordering};

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use crate::error::TransportError;

use super::{AsyncReadWrite, MqttTransport};

/// A mock transport for testing MQTT protocol implementations.
#[derive(Debug)]
pub struct MockTransport {
    /// Data to be returned by read operations
    incoming_data: VecDeque<u8>,
    /// Data written to the transport (captured for verification)
    outgoing_data: Vec<u8>,
    /// Whether the transport is connected
    connected: AtomicBool,
    /// Simulated read errors to return
    read_errors: VecDeque<TransportError>,
    /// Simulated write errors to return
    write_errors: VecDeque<TransportError>,
    /// Local address for testing
    local_addr: Option<String>,
    /// Remote address for testing
    remote_addr: Option<String>,
    /// Whether to simulate slow reads (for timeout testing)
    simulate_slow_read: bool,
    /// Whether to simulate connection loss
    simulate_connection_loss: bool,
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl MockTransport {
    /// Creates a new MockTransport in connected state
    pub fn new() -> Self {
        Self {
            incoming_data: VecDeque::new(),
            outgoing_data: Vec::new(),
            connected: AtomicBool::new(true),
            read_errors: VecDeque::new(),
            write_errors: VecDeque::new(),
            local_addr: None,
            remote_addr: None,
            simulate_slow_read: false,
            simulate_connection_loss: false,
        }
    }

    /// Creates a new MockTransport in disconnected state
    pub fn new_disconnected() -> Self {
        let transport = Self::new();
        transport.connected.store(false, Ordering::Relaxed);
        transport
    }

    /// Adds data that will be returned by subsequent read operations
    pub fn add_incoming_data(&mut self, data: &[u8]) {
        self.incoming_data.extend(data.iter().copied());
    }

    /// Adds a complete MQTT packet to incoming data
    pub fn add_incoming_packet(&mut self, packet_data: &[u8]) {
        self.add_incoming_data(packet_data);
    }

    /// Returns all data that has been written to the transport
    pub fn get_outgoing_data(&self) -> &[u8] {
        &self.outgoing_data
    }

    /// Clears the captured outgoing data
    pub fn clear_outgoing_data(&mut self) {
        self.outgoing_data.clear();
    }

    /// Returns the last N bytes written to the transport
    pub fn get_last_outgoing_bytes(&self, n: usize) -> &[u8] {
        let start = self.outgoing_data.len().saturating_sub(n);
        &self.outgoing_data[start..]
    }

    /// Checks if the outgoing data contains the expected bytes
    pub fn outgoing_contains(&self, expected: &[u8]) -> bool {
        self.outgoing_data
            .windows(expected.len())
            .any(|window| window == expected)
    }

    /// Checks if the outgoing data ends with the expected bytes
    pub fn outgoing_ends_with(&self, expected: &[u8]) -> bool {
        self.outgoing_data.ends_with(expected)
    }

    /// Adds an error that will be returned by the next read operation
    pub fn add_read_error(&mut self, error: TransportError) {
        self.read_errors.push_back(error);
    }

    /// Adds an error that will be returned by the next write operation
    pub fn add_write_error(&mut self, error: TransportError) {
        self.write_errors.push_back(error);
    }

    /// Sets the connection state
    pub fn set_connected(&mut self, connected: bool) {
        self.connected.store(connected, Ordering::Relaxed);
    }

    /// Simulates a connection loss
    pub fn disconnect(&mut self) {
        self.set_connected(false);
    }

    /// Simulates reconnection
    pub fn reconnect(&mut self) {
        self.set_connected(true);
    }

    /// Sets local address for testing
    pub fn set_local_addr(&mut self, addr: String) {
        self.local_addr = Some(addr);
    }

    /// Sets remote address for testing
    pub fn set_remote_addr(&mut self, addr: String) {
        self.remote_addr = Some(addr);
    }

    /// Enables simulation of slow reads (useful for timeout testing)
    pub fn set_simulate_slow_read(&mut self, enable: bool) {
        self.simulate_slow_read = enable;
    }

    /// Enables simulation of connection loss during operations
    pub fn set_simulate_connection_loss(&mut self, enable: bool) {
        self.simulate_connection_loss = enable;
    }

    /// Returns the number of bytes available for reading
    pub fn incoming_data_len(&self) -> usize {
        self.incoming_data.len()
    }

    /// Returns the number of bytes written
    pub fn outgoing_data_len(&self) -> usize {
        self.outgoing_data.len()
    }

    /// Clears all incoming data
    pub fn clear_incoming_data(&mut self) {
        self.incoming_data.clear();
    }

    /// Clears all errors
    pub fn clear_errors(&mut self) {
        self.read_errors.clear();
        self.write_errors.clear();
    }

    /// Resets the transport to initial state
    pub fn reset(&mut self) {
        self.incoming_data.clear();
        self.outgoing_data.clear();
        self.read_errors.clear();
        self.write_errors.clear();
        self.connected.store(true, Ordering::Relaxed);
        self.simulate_slow_read = false;
        self.simulate_connection_loss = false;
    }
}

impl MqttTransport for MockTransport {
    async fn close(&mut self) -> Result<(), TransportError> {
        self.connected.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    fn local_addr(&self) -> Option<&str> {
        self.local_addr.as_deref()
    }

    fn remote_addr(&self) -> Option<&str> {
        self.remote_addr.as_deref()
    }
}

impl MockTransport {
    fn do_read(&mut self, buf: &mut [u8]) -> Result<usize, TransportError> {
        // Check for simulated connection loss
        if self.simulate_connection_loss {
            self.connected.store(false, Ordering::Relaxed);
            return Err(TransportError::ConnectionLost);
        }

        // Check if we're connected
        if !self.is_connected() {
            return Err(TransportError::ConnectionLost);
        }

        // Return any queued read errors first
        if let Some(error) = self.read_errors.pop_front() {
            return Err(error);
        }

        // Simulate slow read if enabled
        if self.simulate_slow_read {
            // In a real async environment, this would be a delay
            // For testing, we just return WouldBlock to simulate timeout
            return Err(TransportError::Timeout);
        }

        // Read available data
        let bytes_to_read = core::cmp::min(buf.len(), self.incoming_data.len());
        if bytes_to_read == 0 {
            return Ok(0); // No data available
        }

        for i in 0..bytes_to_read {
            buf[i] = self.incoming_data.pop_front().unwrap();
        }

        Ok(bytes_to_read)
    }

    fn do_write(&mut self, buf: &[u8]) -> Result<usize, TransportError> {
        // Check for simulated connection loss
        if self.simulate_connection_loss {
            self.connected.store(false, Ordering::Relaxed);
            return Err(TransportError::ConnectionLost);
        }

        // Check if we're connected
        if !self.is_connected() {
            return Err(TransportError::ConnectionLost);
        }

        // Return any queued write errors first
        if let Some(error) = self.write_errors.pop_front() {
            return Err(error);
        }

        // Capture the written data
        self.outgoing_data.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn do_flush(&mut self) -> Result<(), TransportError> {
        // Check if we're connected
        if !self.is_connected() {
            return Err(TransportError::ConnectionLost);
        }

        // Mock flush always succeeds if connected
        Ok(())
    }
}

impl AsyncReadWrite for MockTransport {}

#[cfg(not(feature = "tokio"))]
impl embedded_io::ErrorType for MockTransport {
    type Error = TransportError;
}

#[cfg(not(feature = "tokio"))]
impl embedded_io_async::Read for MockTransport {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.do_read(buf)
    }
}

#[cfg(not(feature = "tokio"))]
impl embedded_io_async::Write for MockTransport {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.do_write(buf)
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.do_flush()
    }
}

#[cfg(feature = "tokio")]
impl tokio::io::AsyncRead for MockTransport {
    fn poll_read(
        mut self: core::pin::Pin<&mut Self>,
        _cx: &mut core::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> core::task::Poll<std::io::Result<()>> {
        let dst = buf.initialize_unfilled();
        match self.do_read(dst) {
            Ok(n) => {
                buf.advance(n);
                core::task::Poll::Ready(Ok(()))
            }
            Err(e) => core::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))),
        }
    }
}

#[cfg(feature = "tokio")]
impl tokio::io::AsyncWrite for MockTransport {
    fn poll_write(
        mut self: core::pin::Pin<&mut Self>,
        _cx: &mut core::task::Context<'_>,
        buf: &[u8],
    ) -> core::task::Poll<std::io::Result<usize>> {
        core::task::Poll::Ready(
            self.do_write(buf)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string())),
        )
    }

    fn poll_flush(
        mut self: core::pin::Pin<&mut Self>,
        _cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<std::io::Result<()>> {
        core::task::Poll::Ready(
            self.do_flush()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string())),
        )
    }

    fn poll_shutdown(
        self: core::pin::Pin<&mut Self>,
        _cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<std::io::Result<()>> {
        core::task::Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_transport_basic_operations() {
        let transport = MockTransport::new();

        // Test initial state
        assert!(transport.is_connected());
        assert_eq!(transport.incoming_data_len(), 0);
        assert_eq!(transport.outgoing_data_len(), 0);
    }

    #[test]
    fn test_mock_transport_data_capture() {
        let mut transport = MockTransport::new();

        // Add incoming data
        transport.add_incoming_data(b"hello");
        assert_eq!(transport.incoming_data_len(), 5);

        // Test outgoing data capture would require async test
        // This is just testing the setup
        assert_eq!(transport.outgoing_data_len(), 0);
    }

    #[test]
    fn test_mock_transport_connection_state() {
        let mut transport = MockTransport::new();

        assert!(transport.is_connected());

        transport.disconnect();
        assert!(!transport.is_connected());

        transport.reconnect();
        assert!(transport.is_connected());
    }

    #[test]
    fn test_mock_transport_addresses() {
        let mut transport = MockTransport::new();

        assert!(transport.local_addr().is_none());
        assert!(transport.remote_addr().is_none());

        transport.set_local_addr("127.0.0.1:1234".to_string());
        transport.set_remote_addr("broker.example.com:1883".to_string());

        assert_eq!(transport.local_addr(), Some("127.0.0.1:1234"));
        assert_eq!(transport.remote_addr(), Some("broker.example.com:1883"));
    }

    #[test]
    fn test_mock_transport_reset() {
        let mut transport = MockTransport::new();

        transport.add_incoming_data(b"test");
        transport.add_read_error(TransportError::Timeout);
        transport.disconnect();

        transport.reset();

        assert!(transport.is_connected());
        assert_eq!(transport.incoming_data_len(), 0);
        assert_eq!(transport.read_errors.len(), 0);
    }
}
