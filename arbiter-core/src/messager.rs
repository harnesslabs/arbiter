//! The messager module contains the core messager layer for the Arbiter Engine.

use super::*;

/// A message that can be sent between agents.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message {
  /// The sender of the message.
  pub from: String,

  /// The recipient of the message.
  pub to: To,

  /// The data of the message.
  /// This can be a struct serialized into JSON.
  pub data: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MessageTo {
  pub to:   To,
  pub data: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MessageFrom {
  pub from: String,
  pub data: String,
}
/// The recipient of the message.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum To {
  /// Send the message to all agents who are listening for broadcasts.
  All,

  /// Send the message to a specific agent.
  Agent(String),
}

/// A messager that can be used to send messages between agents.
#[derive(Debug)]
pub struct Messager {
  /// The identifier of the entity that is using the messager.
  pub id: Option<String>,

  pub(crate) broadcast_sender: broadcast::Sender<Message>,

  pub(crate) broadcast_receiver: broadcast::Receiver<Message>,
}

impl Messager {
  /// Creates a new messager with the given capacity.
  #[allow(clippy::new_without_default)]
  pub fn new() -> Self {
    let (broadcast_sender, broadcast_receiver) = broadcast::channel(512);
    Self { broadcast_sender, broadcast_receiver, id: None }
  }

  /// Returns a [`Messager`] interface connected to the same instance but with
  /// the `id` provided.
  pub fn for_agent(&self, id: &str) -> Self {
    Self {
      broadcast_sender:   self.broadcast_sender.clone(),
      broadcast_receiver: self.broadcast_sender.subscribe(),
      id:                 Some(id.to_owned()),
    }
  }
}
