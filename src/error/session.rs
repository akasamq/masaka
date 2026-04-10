use core::fmt;

use alloc::string::String;

/// Session state errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// Session storage error.
    StorageError(String),
    /// Session capacity exceeded.
    CapacityExceeded,
    /// Subscription limit exceeded.
    SubscriptionLimitExceeded,
    /// In-flight message limit exceeded.
    InflightLimitExceeded,
    /// Invalid packet ID.
    InvalidPacketId(u16),
    /// Duplicate packet ID.
    DuplicatePacketId(u16),
    /// Packet IDs are exhausted.
    PacketIdExhausted,
    /// Corrupted session state.
    CorruptedState(String),
    /// Persistence failed.
    PersistenceError(String),
    /// Session restoration failed.
    RestoreError(String),
    /// Session cleanup failed.
    CleanupError(String),
    /// Out of memory.
    OutOfMemory,
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StorageError(msg) => write!(f, "Session storage error: {msg}"),
            Self::CapacityExceeded => write!(f, "Session capacity exceeded"),
            Self::SubscriptionLimitExceeded => write!(f, "Subscription limit exceeded"),
            Self::InflightLimitExceeded => write!(f, "In-flight message limit exceeded"),
            Self::InvalidPacketId(id) => write!(f, "Invalid packet ID: {id}"),
            Self::DuplicatePacketId(id) => write!(f, "Duplicate packet ID: {id}"),
            Self::PacketIdExhausted => write!(f, "Packet IDs exhausted"),
            Self::CorruptedState(msg) => write!(f, "Corrupted session state: {msg}"),
            Self::PersistenceError(msg) => write!(f, "Persistence failed: {msg}"),
            Self::RestoreError(msg) => write!(f, "Session restoration failed: {msg}"),
            Self::CleanupError(msg) => write!(f, "Session cleanup failed: {msg}"),
            Self::OutOfMemory => write!(f, "Out of memory"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SessionError {}
