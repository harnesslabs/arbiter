use thiserror::Error;

#[cfg(not(all(feature = "wasm", target_arch = "wasm32")))]
use tokio::task::JoinError as TaskJoinError;
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
use tokio_with_wasm::task::JoinError as TaskJoinError;

/// The primary error type used throughout `arbiter-core`.
#[derive(Debug, Error)]
pub enum ArbiterError {
  /// The underlying communication channel was closed unexpectedly.
  #[error("channel closed unexpectedly")]
  ChannelClosed,

  /// A spawned actor task panicked during execution.
  #[error("spawned task panicked")]
  TaskPanicked(#[from] TaskJoinError),

  /// The snapshot stream for the actor was already taken and cannot be taken again.
  #[error("snapshot stream already taken")]
  StreamAlreadyTaken,
}

/// A specialized [`Result`] type for Arbiter operations.
pub type Result<T> = std::result::Result<T, ArbiterError>;
