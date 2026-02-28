use thiserror::Error;

/// The primary error type used throughout `arbiter-core`.
#[derive(Debug, Error)]
pub enum ArbiterError {
  /// The underlying communication channel was closed unexpectedly.
  #[error("channel closed unexpectedly")]
  ChannelClosed,

  /// A spawned actor task panicked during execution.
  #[error("spawned task panicked")]
  TaskPanicked(#[from] tokio::task::JoinError),

  /// The snapshot stream for the actor was already taken and cannot be taken again.
  #[error("snapshot stream already taken")]
  StreamAlreadyTaken,
}

/// A specialized [`Result`] type for Arbiter operations.
pub type Result<T> = std::result::Result<T, ArbiterError>;
