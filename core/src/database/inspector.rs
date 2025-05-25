//! This module contains an extensible [`Inspector`] called
//! [`ArbiterInspector`]. It is currently configurable in order to allow
//! for users to set configuration to see logs generated in Solidity contracts
//! and or enforce gas payment.

use revm::{
  inspectors::GasInspector,
  interpreter::{CreateInputs, CreateOutcome, Interpreter},
};

use super::*;
use crate::console::ConsoleLogs;

/// An configurable [`Inspector`] that collects information about the
/// execution of the [`Interpreter`]. Depending on whether which or both
/// features are enabled, it collects information about the gas used by each
/// opcode and the `console2.log`s emitted during execution. It ensures gas
/// payments are made when `gas` is enabled.
#[derive(Debug, Clone)]
pub struct ArbiterInspector {
  /// Whether to collect `console2.log`s.
  pub console_log: Option<ConsoleLogs>,

  /// Whether to collect gas usage information.
  pub gas: Option<GasInspector>,
}

impl ArbiterInspector {
  /// Create a new [`ArbiterInspector`] with the given configuration.
  pub fn new(console_log: bool, gas: bool) -> Self {
    let console_log = if console_log { Some(ConsoleLogs::default()) } else { None };
    let gas = if gas { Some(GasInspector::default()) } else { None };
    Self { console_log, gas }
  }
}

impl Inspector<ArbiterDB> for ArbiterInspector {
  #[inline]
  fn initialize_interp(&mut self, interp: &mut Interpreter, context: &mut EvmContext<ArbiterDB>) {
    if let Some(gas) = &mut self.gas {
      gas.initialize_interp(interp, context);
    }
  }

  #[inline]
  fn step_end(&mut self, interp: &mut Interpreter, context: &mut EvmContext<ArbiterDB>) {
    if let Some(gas) = &mut self.gas {
      gas.step_end(interp, context);
    }
  }

  #[inline]
  fn call(
    &mut self,
    context: &mut EvmContext<ArbiterDB>,
    inputs: &mut CallInputs,
  ) -> Option<CallOutcome> {
    if let Some(console_log) = &mut self.console_log {
      console_log.call(context, inputs)
    } else {
      None
    }
  }

  #[inline]
  fn call_end(
    &mut self,
    context: &mut EvmContext<ArbiterDB>,
    inputs: &CallInputs,
    outcome: CallOutcome,
  ) -> CallOutcome {
    if let Some(gas) = &mut self.gas {
      gas.call_end(context, inputs, outcome)
    } else {
      outcome
    }
  }

  #[inline]
  fn create_end(
    &mut self,
    _context: &mut EvmContext<ArbiterDB>,
    _inputs: &CreateInputs,
    outcome: CreateOutcome,
  ) -> CreateOutcome {
    outcome
  }
}
