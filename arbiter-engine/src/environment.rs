use super::*;

// TODO: This is a bit of a dummy implementation for testing.
#[derive(Debug)]
pub struct Environment<K, V> {
  inner:       HashMap<K, V>,
  tx_sender:   mpsc::Sender<Vec<u8>>,
  tx_receiver: Option<mpsc::Receiver<Vec<u8>>>,
  broadcast:   broadcast::Sender<Vec<u8>>,
}

impl<K, V> Environment<K, V>
where
  K: Clone + Eq + Hash + Send + Sync + 'static + DeserializeOwned,
  V: Clone + Send + Sync + 'static + DeserializeOwned,
{
  pub fn new() -> Result<Self, ArbiterEngineError> {
    let (tx_sender, tx_receiver) = mpsc::channel(1000);
    let (broadcast, _) = broadcast::channel(1000);
    Ok(Self { inner: HashMap::new(), tx_sender, tx_receiver: Some(tx_receiver), broadcast })
  }

  pub fn run(mut self) -> Result<task::JoinHandle<()>, ArbiterEngineError> {
    let mut tx_receiver = self.tx_receiver.take().unwrap();
    Ok(tokio::spawn(async move {
      while let Some(tx) = tx_receiver.recv().await {
        self.handle_transaction(&tx).unwrap();
      }
    }))
  }

  pub fn middleware(&self) -> Middleware {
    Middleware { sender: self.tx_sender.clone(), receiver: Some(self.broadcast.subscribe()) }
  }

  fn handle_transaction(&mut self, tx: &[u8]) -> Result<(), ArbiterEngineError> {
    let (key, value): (K, V) = serde_json::from_slice(tx).map_err(|e| {
      ArbiterEngineError::DatabaseError(format!("Failed to deserialize transaction: {e}"))
    })?;
    self.set(key, value)?;
    Ok(())
  }

  fn set(&mut self, key: K, value: V) -> Result<(), ArbiterEngineError> {
    self.inner.insert(key, value);
    Ok(())
  }
}

#[derive(Debug)]
pub struct Middleware {
  pub sender:   mpsc::Sender<Vec<u8>>,
  pub receiver: Option<broadcast::Receiver<Vec<u8>>>,
}

impl Clone for Middleware {
  fn clone(&self) -> Self {
    Self {
      sender:   self.sender.clone(),
      receiver: Some(self.receiver.as_ref().unwrap().resubscribe()),
    }
  }
}

impl Middleware {
  pub async fn send(&self, tx: Vec<u8>) -> Result<(), ArbiterEngineError> {
    self
      .sender
      .send(tx)
      .await
      .map_err(|e| ArbiterEngineError::DatabaseError(format!("Failed to send transaction: {e}")))
  }

  pub fn stream(&mut self) -> Result<impl Stream<Item = Vec<u8>>, ArbiterEngineError> {
    let mut broadcast_receiver = self.receiver.take().unwrap();
    Ok(async_stream::stream! {
      while let Ok(tx) = broadcast_receiver.recv().await {
        yield tx;
      }
    })
  }
}

// Helper functions for creating transactions
pub fn create_transaction<K, V>(key: &K, value: &V) -> Result<Vec<u8>, ArbiterEngineError>
where
  K: Serialize,
  V: Serialize, {
  serde_json::to_vec(&(key, value))
    .map_err(|e| ArbiterEngineError::DatabaseError(format!("Failed to serialize transaction: {e}")))
}

// TODO: Below is a trait interface we could use later.
// pub trait Environment: Send + Sync + Sized {
//   type Query: Send + Sync;
//   type Response: Send + Sync;
//   type Sender;

//   fn new() -> Result<Self, ArbiterEngineError>;
//   fn run(&self) -> Result<JoinHandle<()>, ArbiterEngineError>;
//   fn get(&self, key: &Self::Query) -> Result<Option<Self::Response>, ArbiterEngineError>;
//   fn set(&mut self, key: Self::Query, value: Self::Response) -> Result<(), ArbiterEngineError>;
//   fn sender(&self) -> Self::Sender;
// }

// pub trait Middleware: Send + Sync {
//   fn send(&self, tx: Vec<u8>) -> Result<(), ArbiterEngineError>;
//   fn stream(&self) -> Result<impl Stream<Item = Vec<u8>>, ArbiterEngineError>;
// }
