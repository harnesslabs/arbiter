use std::collections::HashMap;

use super::*;
use crate::machine::{Event, EventStream};

pub trait Database: Sized + Send {
  type Location: Clone;
  type State: Clone;
  type Error: Debug;
  fn new() -> Result<Self, Self::Error>;
  fn get(&self, location: Self::Location) -> Result<&Self::State, Self::Error>;
  fn set(&mut self, location: Self::Location, state: Self::State) -> Result<(), Self::Error>;
}

pub trait Environment<DB: Database>: Sized {
  fn new() -> Result<Self, DB::Error>;
  fn middleware(&self) -> Middleware<DB>;
}

impl<K, V> Database for HashMap<K, V>
where
  K: Eq + Hash + Clone + Send,
  V: Clone + Send,
{
  type Error = Box<dyn std::error::Error>;
  type Location = K;
  type State = V;

  fn new() -> Result<Self, Self::Error> { Ok(HashMap::new()) }

  fn get(&self, location: Self::Location) -> Result<&Self::State, Self::Error> {
    Ok(self.get(&location).unwrap())
  }

  fn set(&mut self, location: Self::Location, state: Self::State) -> Result<(), Self::Error> {
    self.insert(location, state);
    Ok(())
  }
}

impl Database for () {
  type Error = ();
  type Location = ();
  type State = ();

  fn new() -> Result<Self, Self::Error> { Ok(()) }

  fn get(&self, _location: Self::Location) -> Result<&Self::State, Self::Error> { Ok(&()) }

  fn set(&mut self, _location: Self::Location, _state: Self::State) -> Result<(), Self::Error> {
    Ok(())
  }
}

// TODO: Could have a `State` trait so we can replace the inner with a JoinHandle<DB> when we run.

#[derive(Debug)]
pub struct InMemoryEnvironment<DB: Database> {
  inner:       DB,
  tx_sender:   mpsc::Sender<(DB::Location, DB::State)>,
  tx_receiver: mpsc::Receiver<(DB::Location, DB::State)>,
  broadcast:   broadcast::Sender<(DB::Location, DB::State)>,
}

impl<DB: Database> InMemoryEnvironment<DB> {
  pub fn new(capacity: usize) -> Result<Self, DB::Error> {
    let (tx_sender, tx_receiver) = mpsc::channel(capacity);
    Ok(Self {
      inner: DB::new()?,
      tx_sender,
      tx_receiver,
      broadcast: broadcast::Sender::new(capacity),
    })
  }

  pub fn with_database(database: DB, capacity: usize) -> Self {
    let (tx_sender, tx_receiver) = mpsc::channel(capacity);
    Self { inner: database, tx_sender, tx_receiver, broadcast: broadcast::Sender::new(capacity) }
  }

  pub fn run(mut self) -> Result<task::JoinHandle<DB>, DB::Error>
  where
    DB: 'static,
    DB::Location: Send + Sync + 'static,
    DB::State: Send + Sync + 'static, {
    let task = tokio::spawn(async move {
      while let Some((k, v)) = self.tx_receiver.recv().await {
        self.inner.set(k.clone(), v.clone()).unwrap();
        let _ = self.broadcast.send((k, v));
      }
      self.inner
    });
    Ok(task)
  }

  pub fn middleware(&self) -> Middleware<DB> {
    Middleware { sender: self.tx_sender.clone(), receiver: self.broadcast.subscribe() }
  }

  pub fn database(&self) -> &DB { &self.inner }
}

#[derive(Debug)]
pub struct Middleware<DB: Database> {
  pub sender:   mpsc::Sender<(DB::Location, DB::State)>,
  pub receiver: broadcast::Receiver<(DB::Location, DB::State)>,
}

impl<DB: Database> Clone for Middleware<DB> {
  fn clone(&self) -> Self {
    Self { sender: self.sender.clone(), receiver: self.receiver.resubscribe() }
  }
}

impl<DB: Database> Middleware<DB>
where DB::Error: From<mpsc::error::SendError<(DB::Location, DB::State)>>
{
  pub async fn send(&self, location: DB::Location, state: DB::State) -> Result<(), DB::Error> {
    self.sender.send((location, state)).await.map_err(DB::Error::from)?;
    Ok(())
  }

  pub fn into_sender_and_stream(
    self,
  ) -> (mpsc::Sender<(DB::Location, DB::State)>, impl Stream<Item = Event<DB>> + Unpin) {
    (
      self.sender,
      Box::pin(stream::unfold(self.receiver, |mut receiver| async move {
        loop {
          match receiver.recv().await {
            Ok((location, state)) => return Some((Event::StateChange(location, state), receiver)),
            Err(broadcast::error::RecvError::Closed) => return None,
            Err(broadcast::error::RecvError::Lagged(_)) => {},
          }
        }
      })),
    )
  }
}

#[cfg(test)]
mod tests {
  use futures::StreamExt;

  use super::*;

  #[tokio::test]
  async fn test_middleware() {
    // Build environment
    let environment = InMemoryEnvironment::<HashMap<String, String>>::new(10).unwrap();

    // Get middleware and start streaming events
    let middleware = environment.middleware();
    let (sender, mut stream) = middleware.into_sender_and_stream();

    // Start environment
    let handle = environment.run().unwrap();

    // Send event
    sender.send((String::from("test_location"), String::from("test_state"))).await.unwrap();

    let next = stream.next().await.unwrap();
    assert_eq!(next, Event::StateChange(String::from("test_location"), String::from("test_state")));
  }
}
