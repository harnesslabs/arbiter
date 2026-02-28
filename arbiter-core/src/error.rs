use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArbiterError {
  #[error("channel closed unexpectedly")]
  ChannelClosed,

  #[error("spawned task panicked")]
  TaskPanicked(#[from] tokio::task::JoinError),

  #[error("snapshot stream already taken")]
  StreamAlreadyTaken,

  #[error("serialization failed: {0}")]
  Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, ArbiterError>;
