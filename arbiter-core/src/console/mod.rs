//! This module contains the backend for the `console2.log` Solidity function so
//! that these logs can be read in Arbiter.

use revm_primitives::address;

use super::*;

const CONSOLE_ADDRESS: Address = address!("000000000000000000636F6e736F6c652e6c6f67");

#[allow(clippy::all)]
#[rustfmt::skip]
#[allow(missing_docs)]
pub(crate) mod abi;

/// An inspector that collects `console2.log`s during execution.
#[derive(Debug, Clone, Default)]
pub struct ConsoleLogs(pub Vec<Bytes>);

impl<DB: Database> Inspector<DB> for ConsoleLogs {
  #[inline]
  fn call(&mut self, _context: &mut EvmContext<DB>, call: &mut CallInputs) -> Option<CallOutcome> {
    if call.contract == CONSOLE_ADDRESS {
      self.0.push(call.input.clone());
    }
    None
  }
}
