//! The messager module contains the core messager layer for the Arbiter Engine.

use tokio::sync::broadcast::{channel, Receiver, Sender};

use super::*;
use crate::machine::EventStream;

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

  pub(crate) broadcast_sender: Sender<Message>,

  broadcast_receiver: Option<Receiver<Message>>,
}

impl Clone for Messager {
  fn clone(&self) -> Self {
    Self {
      broadcast_sender:   self.broadcast_sender.clone(),
      broadcast_receiver: Some(self.broadcast_sender.subscribe()),
      id:                 self.id.clone(),
    }
  }
}

impl Messager {
  /// Creates a new messager with the given capacity.
  #[allow(clippy::new_without_default)]
  pub fn new() -> Self {
    let (broadcast_sender, broadcast_receiver) = channel(512);
    Self { broadcast_sender, broadcast_receiver: Some(broadcast_receiver), id: None }
  }

  /// Returns a [`Messager`] interface connected to the same instance but with
  /// the `id` provided.
  pub fn for_agent(&self, id: &str) -> Self {
    Self {
      broadcast_sender:   self.broadcast_sender.clone(),
      broadcast_receiver: Some(self.broadcast_sender.subscribe()),
      id:                 Some(id.to_owned()),
    }
  }

  /// utility function for getting the next value from the broadcast_receiver
  /// without streaming
  pub async fn get_next(&mut self) -> Result<Message, ArbiterEngineError> {
    let mut receiver = match self.broadcast_receiver.take() {
      Some(receiver) => receiver,
      None =>
        return Err(ArbiterEngineError::MessagerError(
          "Receiver has been taken! Are you already streaming on this messager?".to_owned(),
        )),
    };
    while let Ok(message) = receiver.recv().await {
      match &message.to {
        To::All => {
          return Ok(message);
        },
        To::Agent(id) => {
          if let Some(self_id) = &self.id {
            if id == self_id {
              return Ok(message);
            }
          }
          continue;
        },
      }
    }
    unreachable!()
  }

  /// Returns a stream of messages that are either sent to [`To::All`] or to
  /// the agent via [`To::Agent(id)`].
  pub fn stream(mut self) -> Result<EventStream<Message>, ArbiterEngineError> {
    let mut receiver = match self.broadcast_receiver.take() {
      Some(receiver) => receiver,
      None =>
        return Err(ArbiterEngineError::MessagerError(
          "Receiver has been taken! Are you already streaming on this messager?".to_owned(),
        )),
    };
    Ok(Box::pin(async_stream::stream! {
        while let Ok(message) = receiver.recv().await {
            match &message.to {
                To::All => {
                    yield message;
                }
                To::Agent(id) => {
                    if let Some(self_id) = &self.id {
                        if id == self_id {
                            yield message;
                        }
                    }
                }
            }
        }
    }))
  }

  /// Asynchronously sends a message to a specified recipient.
  ///
  /// This method constructs a message with the provided data and sends it to
  /// the specified recipient. The recipient can either be a single agent
  /// or all agents, depending on the `to` parameter. The data is
  /// serialized into a JSON string before being sent.
  ///
  /// # Type Parameters
  ///
  /// - `T`: The type that can be converted into a recipient specification (`To`).
  /// - `S`: The type of the data being sent. Must implement `Serialize`.
  ///
  /// # Parameters
  ///
  /// - `to`: The recipient of the message. Can be an individual agent's ID or a broadcast to all
  ///   agents.
  /// - `data`: The data to be sent in the message. This data is serialized into JSON format.
  pub async fn send<S: Serialize>(&self, to: To, data: S) -> Result<(), ArbiterEngineError> {
    trace!("Sending message via messager.");
    if let Some(id) = &self.id {
      let message = Message { from: id.clone(), to, data: serde_json::to_string(&data)? };
      self.broadcast_sender.send(message)?;
      Ok(())
    } else {
      Err(ArbiterEngineError::MessagerError(
        "Messager has no ID! You must have an ID to send messages!".to_owned(),
      ))
    }
  }
}
