use std::{collections::HashMap, hash::Hash};

use futures::{stream, Stream};

use super::*;

pub trait StateDB {
  type Request;
  type State;
  type Error;
  fn get(&self, request: Self::Request) -> Result<&Self::State, Self::Error>;
  fn set(&mut self, request: Self::Request, state: Self::State) -> Result<(), Self::Error>;
}

impl<K, V> StateDB for HashMap<K, V>
where K: Eq + Hash
{
  type Error = Box<dyn std::error::Error>;
  type Request = K;
  type State = V;

  fn get(&self, request: Self::Request) -> Result<&Self::State, Self::Error> {
    Ok(self.get(&request).unwrap())
  }

  fn set(&mut self, request: Self::Request, state: Self::State) -> Result<(), Self::Error> {
    self.insert(request, state);
    Ok(())
  }
}

pub struct Environment<S: StateDB> {
  inner:       S,
  tx_sender:   mpsc::Sender<S::Request>,
  tx_receiver: mpsc::Receiver<S::Request>,
  broadcast:   broadcast::Sender<S::Request>,
}

impl<K, V> Environment<HashMap<K, V>>
where K: Eq + Hash
{
  pub fn new(capacity: usize) -> Result<Self, <HashMap<K, V> as StateDB>::Error> {
    let (tx_sender, tx_receiver) = mpsc::channel(capacity);
    Ok(Self {
      inner: HashMap::new(),
      tx_sender,
      tx_receiver,
      broadcast: broadcast::Sender::new(capacity),
    })
  }

  pub fn middleware(&self) -> Middleware<HashMap<K, V>> {
    Middleware { sender: self.tx_sender.clone(), receiver: self.broadcast.subscribe() }
  }
}

pub struct Middleware<S: StateDB> {
  pub sender:   mpsc::Sender<S::Request>,
  pub receiver: broadcast::Receiver<S::Request>,
}

impl<S: StateDB> Middleware<S>
where
  S::Error: From<mpsc::error::SendError<S::Request>>,
  S::Request: Clone,
{
  pub async fn send(&self, request: S::Request) -> Result<(), S::Error> {
    self.sender.send(request).await.map_err(S::Error::from)?;
    Ok(())
  }

  pub fn stream(mut self) -> impl Stream<Item = S::Request> {
    stream::unfold(self.receiver, |mut receiver| async move {
      loop {
        match receiver.recv().await {
          Ok(request) => return Some((request, receiver)),
          Err(broadcast::error::RecvError::Closed) => return None,
          Err(broadcast::error::RecvError::Lagged(_)) => {},
        }
      }
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_middleware() { let environment = Environment::new(10).unwrap(); }
}
