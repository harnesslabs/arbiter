use super::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message {
  pub from: String,
  pub to:   To,
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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum To {
  All,
  Agent(String),
}

#[derive(Debug)]
pub struct Messager {
  pub id: Option<String>,
  pub(crate) broadcast_sender: broadcast::Sender<Message>,
  pub(crate) broadcast_receiver: broadcast::Receiver<Message>,
}

impl Messager {
  #[allow(clippy::new_without_default)]
  pub fn new() -> Self {
    let (broadcast_sender, broadcast_receiver) = broadcast::channel(512);
    Self { broadcast_sender, broadcast_receiver, id: None }
  }

  pub fn for_agent(&self, id: &str) -> Self {
    Self {
      broadcast_sender:   self.broadcast_sender.clone(),
      broadcast_receiver: self.broadcast_sender.subscribe(),
      id:                 Some(id.to_owned()),
    }
  }
}
