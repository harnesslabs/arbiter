//! Messengers/connections to the underlying EVM in the environment.
use std::sync::Weak;

use super::*;
use crate::environment::{InstructionSender, OutcomeReceiver, OutcomeSender};

/// Represents a connection to the EVM contained in the corresponding
/// [`Environment`].
#[derive(Debug)]
pub struct Connection {
  /// Used to send calls and transactions to the [`Environment`] to be
  /// executed by `revm`.
  pub(crate) instruction_sender: Weak<InstructionSender>,

  /// Used to send results back to a client that made a call/transaction with
  /// the [`Environment`]. This [`ResultSender`] is passed along with a
  /// call/transaction so the [`Environment`] can reply back with the
  /// [`ExecutionResult`].
  pub(crate) outcome_sender: OutcomeSender,

  /// Used to receive the [`ExecutionResult`] from the [`Environment`] upon
  /// call/transact.
  pub(crate) outcome_receiver: OutcomeReceiver,

  pub(crate) event_sender: BroadcastSender<Broadcast>,

  /// A collection of `FilterReceiver`s that will receive outgoing logs
  /// generated by `revm` and output by the [`Environment`].
  pub(crate) filter_receivers: Arc<Mutex<HashMap<ethers::types::U256, FilterReceiver>>>,
}

impl From<&Environment> for Connection {
  fn from(environment: &Environment) -> Self {
    let instruction_sender = &Arc::clone(&environment.socket.instruction_sender);
    let (outcome_sender, outcome_receiver) = crossbeam_channel::unbounded();
    Self {
      instruction_sender: Arc::downgrade(instruction_sender),
      outcome_sender,
      outcome_receiver,
      event_sender: environment.socket.event_broadcaster.clone(),
      filter_receivers: Arc::new(Mutex::new(HashMap::new())),
    }
  }
}

#[async_trait::async_trait]
impl JsonRpcClient for Connection {
  type Error = ProviderError;

  /// Processes a JSON-RPC request and returns the response.
  /// Currently only handles the `eth_getFilterChanges` call since this is
  /// used for polling events emitted from the [`Environment`].
  async fn request<T: Serialize + Send + Sync, R: DeserializeOwned>(
    &self,
    method: &str,
    params: T,
  ) -> Result<R, ProviderError> {
    match method {
      "eth_getFilterChanges" => {
        // TODO: The extra json serialization/deserialization can probably be avoided
        // somehow

        trace!("Getting filter changes...");
        // Get the `Filter` ID from the params `T`
        // First convert it into a JSON `Value`
        let value = serde_json::to_value(&params)?;

        // Take this value as an array then cast it to a string
        let str = value.as_array().ok_or(ProviderError::CustomError(
          "The params value passed to the `Connection` via a `request` was empty. 
                    This is likely due to not specifying a specific `Filter` ID!"
            .to_string(),
        ))?[0]
          .as_str()
          .ok_or(ProviderError::CustomError(
            "The params value passed to the `Connection` via a `request` could not be later cast \
             to `str`!"
              .to_string(),
          ))?;

        // Now get the `U256` ID via the string decoded from hex radix.
        let id = ethers::types::U256::from_str_radix(str, 16).map_err(|e| {
          ProviderError::CustomError(format!(
            "The `str` representation of the filter ID could not be cast into `U256` due to: {:?}!",
            e
          ))
        })?;

        // Get the corresponding `filter_receiver` and await for logs to appear.
        let mut filter_receivers = self.filter_receivers.lock().unwrap();
        let filter_receiver = filter_receivers.get_mut(&id).ok_or(ProviderError::CustomError(
          "The filter ID does not seem to match any that this client owns!".to_string(),
        ))?;
        let mut logs = vec![];
        let filtered_params = FilteredParams::new(Some(filter_receiver.filter.clone()));
        if let Some(receiver) = filter_receiver.receiver.as_mut() {
          if let Ok(broadcast) = receiver.try_recv() {
            match broadcast {
              Broadcast::Event(received_logs, receipt_data) => {
                let ethers_logs = revm_logs_to_ethers_logs(received_logs, &receipt_data);
                for log in ethers_logs {
                  if filtered_params.filter_address(&log) && filtered_params.filter_topics(&log) {
                    logs.push(log);
                  }
                }
              },
              Broadcast::StopSignal => {
                return Err(ProviderError::CustomError(
                  "The `EventBroadcaster` has stopped!".to_string(),
                ));
              },
            }
          }
        }
        // Take the logs and Stringify then JSONify to cast into `R`.
        let logs_str = serde_json::to_string(&logs)?;
        let logs_deserializeowned: R = serde_json::from_str(&logs_str)?;
        Ok(logs_deserializeowned)
      },
      val => Err(ProviderError::CustomError(format!(
        "The method `{}` is not supported by the `Connection`!",
        val
      ))),
    }
  }
}

impl PubsubClient for Connection {
  type NotificationStream = Pin<Box<dyn Stream<Item = Box<RawValue>> + Send>>;

  fn subscribe<T: Into<ethers::types::U256>>(
    &self,
    id: T,
  ) -> Result<Self::NotificationStream, Self::Error> {
    let id = id.into();
    debug!("Subscribing to filter with ID: {:?}", id);

    let mut filter_receiver = self.filter_receivers.lock().unwrap().remove(&id).take().unwrap();

    let mut receiver = filter_receiver.receiver.take().unwrap();
    let stream = async_stream::stream! {
                while let Ok(broadcast) = receiver.recv().await {
                    match broadcast {
                        Broadcast::StopSignal => {
                            break;
                        }
                    Broadcast::Event(logs, receipt_data) => {
                        let filtered_params =
                            FilteredParams::new(Some(filter_receiver.filter.clone()));
                        let ethers_logs = revm_logs_to_ethers_logs(logs, &receipt_data);
                        // Return the first log that matches the filter, if any
                        for log in ethers_logs {
                            if filtered_params.filter_address(&log)
                                && filtered_params.filter_topics(&log)
                            {
                                let raw_log = match serde_json::to_string(&log) {
                                    Ok(log) => log,
                                    Err(e) => {
                                        eprintln!("Error serializing log: {}", e);
                                        continue;
                                    }
                                };
                                let raw_log = match RawValue::from_string(raw_log) {
                                    Ok(log) => log,
                                    Err(e) => {
                                        eprintln!("Error creating RawValue: {}", e);
                                        continue;
                                    }
                                };
                                yield raw_log;
                            }
                        }

                    }
            }
        }
    };

    Ok(Box::pin(stream))
  }

  // TODO: At the moment, this won't actually drop the stream.
  fn unsubscribe<T: Into<ethers::types::U256>>(&self, id: T) -> Result<(), Self::Error> {
    let id = id.into();
    debug!("Unsubscribing from filter with ID: {:?}", id);
    if self.filter_receivers.lock().unwrap().remove(&id).is_some() {
      Ok(())
    } else {
      Err(ProviderError::CustomError(
        "The filter ID does not seem to match any that this client owns!".to_string(),
      ))
    }
  }
}

/// Packages together a [`crossbeam_channel::Receiver<Vec<Log>>`] along with a
/// [`Filter`] for events. Allows the client to have a stream of filtered
/// events.
#[derive(Debug)]
pub(crate) struct FilterReceiver {
  /// The filter definition used for this receiver.
  /// Comes from the `ethers-rs` crate.
  pub(crate) filter: Filter,

  /// The receiver for the channel that receives logs from the broadcaster.
  /// These are filtered upon reception.
  pub(crate) receiver: Option<BroadcastReceiver<Broadcast>>,
}

// TODO: The logs below could have the block number, transaction index, and
// maybe other fields populated. Right now, some are defaulted and are not
// correct!

/// Converts logs from the Revm format to the Ethers format.
///
/// This function iterates over a list of logs as they appear in the `revm` and
/// converts each log entry to the corresponding format used by the `ethers-rs`
/// library.
#[inline]
pub fn revm_logs_to_ethers_logs(revm_logs: Vec<Log>, receipt_data: &ReceiptData) -> Vec<eLog> {
  let mut logs: Vec<eLog> = vec![];
  for revm_log in revm_logs {
    let topics = revm_log.topics().iter().map(recast_b256).collect();
    let data = eBytes::from(revm_log.data.data.0);
    let log = eLog {
      address: eAddress::from(revm_log.address.into_array()),
      topics,
      data,
      block_hash: Some(H256::default()),
      block_number: Some(receipt_data.block_number),
      transaction_hash: Some(H256::default()),
      transaction_index: Some(receipt_data.transaction_index),
      log_index: Some(eU256::from(0)),
      transaction_log_index: None,
      log_type: None,
      removed: None,
    };
    logs.push(log);
  }
  logs
}

/// Recast a B256 into an H256 type
/// # Arguments
/// * `input` - B256 to recast. (B256)
/// # Returns
/// * `H256` - Recasted H256.
#[inline]
pub fn recast_b256(input: &revm::primitives::B256) -> ethers::types::H256 {
  ethers::types::H256::from(input.0)
}
