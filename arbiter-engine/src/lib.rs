#![warn(missing_docs)]

//! Arbiter Engine is a library for creating and running automations or
//! simulations of multi-agent systems. It is designed to be used in a
//! distributed fashion where each agent is running in its own process and
//! communicating with other agents via a messaging layer.

use std::{collections::HashMap, fmt::Debug, sync::Arc};

use futures_util::future::join_all;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::task::{spawn, JoinError};
use tracing::{debug, info, trace, warn};

use crate::messager::Messager;

pub mod agent;
pub mod environment;
pub mod error;
pub mod machine;
pub mod messager;
// pub mod universe; // TODO: Unneeded for now really
pub mod world;
