//! ```text
//!      _     _____  ____ _____ _______ ______ _____  
//!     / \   |  __ \|  _ \_   _|__   __|  ____|  __ \
//!    /   \  | |__) | |_) || |    | |  | |__  | |__) |
//!   / / \ \ |  _  /|  _ < | |    | |  |  __| |  _  /
//!  / _____ \| | \ \| |_) || |_   | |  | |____| | \ \
//! /_/     \_\_|  \_\____/_____|  |_|  |______|_|  \_\
//! ```
//!                                              
//! `arbiter-core` is designed to facilitate agent-based simulations of Ethereum
//! smart contracts in a local environment.
//!
//! With a primary emphasis on ease of use and performance, it employs the
//! [`revm`](https://crates.io/crates/revm) (Rust EVM) to provide a local
//! execution environment that closely simulates the Ethereum blockchain but
//! without associated overheads like networking latency.
//!
//! Key Features:
//! - **Environment Handling**: Detailed setup and control mechanisms for running the Ethereum-like
//!   blockchain environment.
//! - **Middleware Implementation**: Customized middleware to reduce overhead and provide optimal
//!   performance.
//!
//! For a detailed guide on getting started, check out the
//! [Arbiter Github page](https://github.com/amthias-labs/arbiter/).
//!
//! For specific module-level information and examples, navigate to the
//! respective module documentation below.

#![warn(missing_docs)]

pub mod console;
pub mod coprocessor;
pub mod database;
pub mod environment;
pub mod errors;
pub mod events;
pub mod middleware;

use std::{
  collections::{BTreeMap, HashMap},
  convert::Infallible,
  fmt::Debug,
  sync::{Arc, RwLock},
};

use async_trait::async_trait;
use ethers::types::{Address as eAddress, Filter, Log as eLog, H256, U256 as eU256, U64};
use revm::{
  db::{CacheDB, EmptyDB},
  interpreter::{CallInputs, CallOutcome},
  primitives::{AccountInfo, Address, Bytes, ExecutionResult, Log, TxEnv, U256},
  Database, Evm, EvmContext, Inspector,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::{Receiver as BroadcastReceiver, Sender as BroadcastSender};
use tracing::{debug, error, info, trace, warn};

use crate::{database::ArbiterDB, environment::Broadcast, errors::ArbiterCoreError};
