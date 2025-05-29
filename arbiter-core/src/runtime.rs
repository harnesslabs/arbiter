use std::sync::{Arc, Mutex};

use super::*;

pub struct Runtime {
  pub agents: HashMap<String, Arc<Mutex<dyn Agent>>>,
}

impl Runtime {
  pub fn new() -> Self { Self { agents: Vec::new() } }
}
