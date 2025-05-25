pub mod token_admin;
pub mod token_requester;

use std::sync::Arc;

use anyhow::Result;
use arbiter_bindings::bindings::arbiter_token::ArbiterToken;
use arbiter_core::middleware::ArbiterMiddleware;
use arbiter_engine::{
  machine::{Behavior, ControlFlow, CreateStateMachine, Engine, EventStream, StateMachine},
  messager::{Message, Messager, To},
};
use arbiter_macros::Behaviors;
use ethers::types::{Address as eAddress, U256 as eU256};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, trace, warn};

#[derive(Behaviors, Debug, Clone, Serialize, Deserialize)]
pub enum Behaviors {
  TokenAdmin(token_admin::TokenAdmin),
  TokenRequester(token_requester::TokenRequester),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenData {
  pub name:     String,
  pub symbol:   String,
  pub decimals: u8,
  pub address:  Option<eAddress>,
}
