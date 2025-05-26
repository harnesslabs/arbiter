//! The [`Coprocessor`] is used to process calls and can access read-only from
//! the [`Environment`]'s database while staying up to date with the
//! latest state of the [`Environment`]'s database.

use std::convert::Infallible;

use revm_primitives::{EVMError, ResultAndState};

use super::*;
use crate::environment::Environment;

/// A [`Coprocessor`] is used to process calls and can access read-only from the
/// [`Environment`]'s database. This can eventually be used for things like
/// parallelized compute for agents that are not currently sending transactions
/// that need to be processed by the [`Environment`], but are instead using the
/// current state to make decisions.
pub struct Coprocessor<'a> {
  evm: Evm<'a, (), ArbiterDB>,
}

impl<'a> Coprocessor<'a> {
  /// Create a new `Coprocessor` with the given `Environment`.
  pub fn new(environment: &Environment) -> Self {
    let db = environment.db.clone();
    let evm = Evm::builder().with_db(db).build();
    Self { evm }
  }

  // TODO: Should probably take in a TxEnv or something.
  /// Used as an entrypoint to process a call with the `Coprocessor`.
  pub fn transact(&mut self) -> Result<ResultAndState, EVMError<Infallible>> { self.evm.transact() }
}

#[cfg(test)]
mod tests {
  use revm_primitives::{InvalidTransaction, U256};

  use super::*;

  #[test]
  fn coprocessor() {
    let environment = Environment::builder().build();
    let mut coprocessor = Coprocessor::new(&environment);
    coprocessor.evm.tx_mut().value = U256::from(100);
    let outcome = coprocessor.transact();
    if let Err(EVMError::Transaction(InvalidTransaction::LackOfFundForMaxFee { fee, balance })) =
      outcome
    {
      assert_eq!(*fee, U256::from(100));
      assert_eq!(*balance, U256::from(0));
    }
  }
}
