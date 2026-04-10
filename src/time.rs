use core::time::Duration;

/// An abstraction for time-related operations.
#[allow(async_fn_in_trait)]
pub trait TimeProvider {
    /// Returns the current unix timestamp in milliseconds.
    fn current_timestamp_ms(&self) -> u64;

    /// Asynchronously waits for a specified duration.
    async fn delay(&self, duration: Duration);
}

/// A `tokio` runtime time provider.
#[cfg(feature = "tokio")]
#[derive(Debug, Clone, Default)]
pub struct TokioTimeProvider;

#[cfg(feature = "tokio")]
impl TimeProvider for TokioTimeProvider {
    fn current_timestamp_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    async fn delay(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

/// A `embassy` runtime time provider.
#[cfg(feature = "embassy")]
#[derive(Debug, Clone, Default)]
pub struct EmbassyTimeProvider;

#[cfg(feature = "embassy")]
impl TimeProvider for EmbassyTimeProvider {
    fn current_timestamp_ms(&self) -> u64 {
        embassy_time::Instant::now().as_millis()
    }

    async fn delay(&self, duration: Duration) {
        embassy_time::Timer::after(embassy_time::Duration::from_millis(
            duration.as_millis() as u64
        ))
        .await;
    }
}

#[cfg(feature = "tokio")]
#[doc(hidden)]
pub type DefaultTimeProvider = TokioTimeProvider;

#[cfg(feature = "embassy")]
#[doc(hidden)]
pub type DefaultTimeProvider = EmbassyTimeProvider;
