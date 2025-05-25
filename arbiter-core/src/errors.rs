//! Errors that can occur when managing or interfacing with Arbiter's sandboxed
//! Ethereum environment.

use std::sync::{PoisonError, RwLockWriteGuard};

// use crossbeam_channel::SendError;
use crossbeam_channel::{RecvError, SendError};
use ethers::{
  providers::{MiddlewareError, ProviderError},
  signers::WalletError,
};
use revm_primitives::{EVMError, HaltReason};
use thiserror::Error;

use self::environment::instruction::{Instruction, Outcome};
use super::*;

/// The error type for `arbiter-core`.
#[derive(Error, Debug)]
pub enum ArbiterCoreError {
  /// Tried to create an account that already exists.
  #[error("Account already exists!")]
  AccountCreationError,

  /// Tried to access an account that doesn't exist.
  #[error("Account doesn't exist!")]
  AccountDoesNotExistError,

  /// Tried to sign with forked EOA.
  #[error("Can't sign with a forked EOA!")]
  ForkedEOASignError,

  /// Failed to upgrade instruction sender in middleware.
  #[error("Failed to upgrade sender to a strong reference!")]
  UpgradeSenderError,

  /// Data missing when calling a transaction.
  #[error("Data missing when calling a transaction!")]
  MissingDataError,

  /// Invalid data used for a query request.
  #[error("Invalid data used for a query request!")]
  InvalidQueryError,

  /// Failed to join environment thread on stop.
  #[error("Failed to join environment thread on stop!")]
  JoinError,

  /// Reverted execution.
  #[error("Execution failed with revert: {gas_used:?} gas used, {output:?}")]
  ExecutionRevert {
    /// The amount of gas used.
    gas_used: u64,
    /// The output bytes of the execution.
    output:   Vec<u8>,
  },

  /// Halted execution.
  #[error("Execution failed with halt: {reason:?}, {gas_used:?} gas used")]
  ExecutionHalt {
    /// The halt reason.
    reason:   HaltReason,
    /// The amount of gas used.
    gas_used: u64,
  },

  /// Failed to parse integer.
  #[error(transparent)]
  ParseIntError(#[from] std::num::ParseIntError),

  /// Evm had a runtime error.
  #[error(transparent)]
  EVMError(#[from] EVMError<Infallible>),

  /// Provider error.
  #[error(transparent)]
  ProviderError(#[from] ProviderError),

  /// Wallet error.
  #[error(transparent)]
  WalletError(#[from] WalletError),

  /// Send error.
  #[error(transparent)]
  SendError(
    #[from]
    #[allow(private_interfaces)]
    SendError<Instruction>,
  ),

  /// Recv error.
  #[error(transparent)]
  RecvError(#[from] RecvError),

  /// Failed to parse integer from string.
  #[error(transparent)]
  FromStrRadixError(#[from] uint::FromStrRadixErr),

  /// Failed to handle json.
  #[error(transparent)]
  SerdeJsonError(#[from] serde_json::Error),

  /// Failed to reply to instruction.
  #[error("{0}")]
  ReplyError(String),

  /// Failed to grab a lock.
  #[error("{0}")]
  RwLockError(String),
}

impl From<SendError<Result<Outcome, ArbiterCoreError>>> for ArbiterCoreError {
  fn from(e: SendError<Result<Outcome, ArbiterCoreError>>) -> Self {
    ArbiterCoreError::ReplyError(e.to_string())
  }
}

impl<T> From<PoisonError<RwLockWriteGuard<'_, T>>> for ArbiterCoreError {
  fn from(e: PoisonError<RwLockWriteGuard<'_, T>>) -> Self {
    ArbiterCoreError::RwLockError(e.to_string())
  }
}

impl MiddlewareError for ArbiterCoreError {
  type Inner = ProviderError;

  fn from_err(e: Self::Inner) -> Self { ArbiterCoreError::from(e) }

  fn as_inner(&self) -> Option<&Self::Inner> { None }
}
