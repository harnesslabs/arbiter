use std::{
  collections::HashMap,
  sync::{Arc, Mutex},
};

use super::*;

// TODO: This should likely hold a registry of agents and their handlers.
pub struct Runtime {
  pub agents: HashMap<String, String>,
}

impl Runtime {
  pub fn new() -> Self { Self { agents: HashMap::new() } }
}
