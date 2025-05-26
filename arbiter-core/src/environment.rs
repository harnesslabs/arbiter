use std::{collections::HashMap, hash::Hash};

use futures::{pin_mut, stream, Stream, StreamExt};
use tokio::task;

use super::*;

pub trait StateDB {
  type Location;
  type State;
  type Error;
  fn get(&self, location: Self::Location) -> Result<&Self::State, Self::Error>;
  fn set(&mut self, location: Self::Location, state: Self::State) -> Result<(), Self::Error>;
}

impl<K, V> StateDB for HashMap<K, V>
where K: Eq + Hash
{
  type Error = Box<dyn std::error::Error>;
  type Location = K;
  type State = V;

  fn get(&self, location: Self::Location) -> Result<&Self::State, Self::Error> {
    Ok(self.get(&location).unwrap())
  }

  fn set(&mut self, location: Self::Location, state: Self::State) -> Result<(), Self::Error> {
    self.insert(location, state);
    Ok(())
  }
}

pub struct Environment<S: StateDB> {
  inner:       S,
  tx_sender:   mpsc::Sender<(S::Location, S::State)>,
  tx_receiver: mpsc::Receiver<(S::Location, S::State)>,
  broadcast:   broadcast::Sender<(S::Location, S::State)>,
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

  pub fn run(mut self) -> Result<task::JoinHandle<()>, <HashMap<K, V> as StateDB>::Error>
  where
    K: Send + Sync + 'static,
    V: Send + Sync + 'static, {
    let mut tx_receiver = self.tx_receiver;
    Ok(tokio::spawn(async move {
      while let Some((k, v)) = tx_receiver.recv().await {
        self.inner.set(k, v).unwrap();
      }
    }))
  }

  pub fn middleware(&self) -> Middleware<HashMap<K, V>> {
    Middleware { sender: self.tx_sender.clone(), receiver: self.broadcast.subscribe() }
  }
}

pub struct Middleware<S: StateDB> {
  pub sender:   mpsc::Sender<(S::Location, S::State)>,
  pub receiver: broadcast::Receiver<(S::Location, S::State)>,
}

impl<S: StateDB> Middleware<S>
where S::Error: From<mpsc::error::SendError<(S::Location, S::State)>>
{
  pub async fn send(&self, location: S::Location, state: S::State) -> Result<(), S::Error> {
    self.sender.send((location, state)).await.map_err(S::Error::from)?;
    Ok(())
  }

  pub fn stream(&self) -> impl Stream<Item = (S::Location, S::State)>
  where
    S::Location: Clone,
    S::State: Clone, {
    let stream = stream::unfold(self.receiver.resubscribe(), |mut receiver| async move {
      loop {
        match receiver.recv().await {
          Ok(request) => return Some((request, receiver)),
          Err(broadcast::error::RecvError::Closed) => return None,
          Err(broadcast::error::RecvError::Lagged(_)) => {},
        }
      }
    });
    pin_mut!(stream);
    stream
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_middleware() {
    // Build environment
    let environment = Environment::<HashMap<String, String>>::new(10).unwrap();

    // Get middleware and start streaming events
    let middleware = environment.middleware();
    let stream = middleware.stream();

    // Start environment
    let handle = environment.run().unwrap();

    // Send event
    middleware.send("test_location".to_string(), "test_state".to_string()).await.unwrap();

    let next = stream.next().await.unwrap();
    assert_eq!(next, ("test_location".to_string(), "test_state".to_string()));
  }
}
