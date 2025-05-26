use thiserror::Error;

use super::*;

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

  /// Error occurred with the database.
  #[error("DatabaseError: {0}")]
  DatabaseError(String),

  /// Error occurred in joining a task.
  #[error(transparent)]
  JoinError(#[from] task::JoinError),

  /// Error occurred in sending a message.
  #[error(transparent)]
  SendError(#[from] broadcast::error::SendError<crate::messager::Message>),
}
