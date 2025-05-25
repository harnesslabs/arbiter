use std::{collections::HashMap, hash::Hash, sync::Arc};

use futures::Stream;
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;

use crate::errors::ArbiterEngineError;

pub trait Database {
  type Key;
  type Value;
  type TransactionLayer: TransactionLayer;
  fn get(&self, key: &Self::Key) -> Result<Self::Value, ArbiterEngineError>;
  fn set(&mut self, key: Self::Key, value: Self::Value) -> Result<(), ArbiterEngineError>;
}

pub trait TransactionLayer {
  async fn send(&self, tx: &[u8]) -> Result<(), ArbiterEngineError>;
  async fn stream(&self) -> Result<impl Stream<Item = &[u8]>, ArbiterEngineError>;
}

// TODO: This is a testing implementation basically
impl<K, V> Database for HashMap<K, V>
where
  K: Eq + Hash,
  V: Clone,
{
  type Key = K;
  type TransactionLayer = HashMapTransactionLayer<K, V>;
  type Value = V;

  fn get(&self, key: &Self::Key) -> Result<Self::Value, ArbiterEngineError> {
    Ok(self.get(key).unwrap().clone())
  }

  fn set(&mut self, key: Self::Key, value: Self::Value) -> Result<(), ArbiterEngineError> {
    self.insert(key, value);
    Ok(())
  }
}

struct HashMapTransactionLayer<K, V> {
  inner: Arc<Mutex<HashMap<K, V>>>,
}

impl<K, V> TransactionLayer for HashMapTransactionLayer<K, V>
where
  K: DeserializeOwned + Eq + Hash,
  V: DeserializeOwned + Clone,
{
  async fn send(&self, tx: &[u8]) -> Result<(), ArbiterEngineError> {
    let (key, value): (K, V) = serde_json::from_slice(tx).unwrap();
    self.inner.lock().await.set(key, value);
    Ok(())
  }

  // TODO: Want to stream any new changes to the database
  async fn stream(&self) -> Result<impl Stream<Item = &[u8]>, ArbiterEngineError> {
    Ok(self.iter().map(|(k, v)| (k, v)).collect())
  }
}
