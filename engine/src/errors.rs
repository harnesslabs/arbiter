//! Error types for the arbiter engine.

use thiserror::Error;

use super::*;

/// Errors that can occur in the arbiter engine.
#[derive(Debug, Error)]
pub enum ArbiterEngineError {
  /// Error occurred with the [`Messager`].
  #[error("MessagerError: {0}")]
  MessagerError(String),

  /// Error occurred with the [`crate::agent::Agent`].
  #[error("AgentBuildError: {0}")]
  AgentBuildError(String),

  /// Error occurred with the [`crate::world::World`].
  #[error("WorldError: {0}")]
  WorldError(String),

  /// Error occurred with the [`crate::universe::Universe`].
  #[error("UniverseError: {0}")]
  UniverseError(String),

  /// Error occurred in joining a task.
  #[error(transparent)]
  JoinError(#[from] tokio::task::JoinError),

  /// Error occurred in sending a message.
  #[error(transparent)]
  SendError(#[from] tokio::sync::broadcast::error::SendError<crate::messager::Message>),

  /// Error occurred in deserializing json.
  #[error(transparent)]
  SerdeJsonError(#[from] serde_json::Error),

  /// Error occurred in reading in a file.
  #[error(transparent)]
  IoError(#[from] std::io::Error),

  /// Error occurred in deserializing toml.
  #[error(transparent)]
  TomlError(#[from] toml::de::Error),

  /// Error occurred within [`arbiter_core`].
  #[error(transparent)]
  ArbiterCoreError(#[from] arbiter_core::errors::ArbiterCoreError),
}
