//! The `data_collection` module provides the `EventLogger` struct for logging
//! events from the Ethereum network.
//!
//! The `EventLogger` struct contains a BTreeMap of events, where each event is
//! represented by a string key and a vector of `Event` instances.
//! It also optionally contains a path where the event logs will be stored.
//!
//! This module also provides the implementation of the `EventLogger` struct,
//! including methods for constructing a new `EventLogger`, adding an event to
//! the `EventLogger`, and writing the event logs to a file.
//!
//! # Type Parameters
//!
//! * `M` - Middleware that implements the `Middleware` trait, `std::borrow::Borrow<D>`, and has a
//!   static lifetime.
//! * `D` - Middleware that implements the `Middleware` trait, `Debug`, `Send`, `Sync`, and has a
//!   static lifetime.
//! * `E` - Type that implements the `EthLogDecode`, `Debug`, `Serialize` traits, and has a static
//!   lifetime.

use std::{io::BufWriter, marker::PhantomData, mem::transmute, pin::Pin};

use ethers::{
  abi::RawLog,
  contract::{builders::Event, EthLogDecode},
  core::k256::sha2::{Digest, Sha256},
  providers::Middleware,
  types::{Filter, FilteredParams},
};
use futures_util::Stream;
use polars::{
  io::parquet::ParquetWriter,
  prelude::{CsvWriter, DataFrame, NamedFrom, SerWriter},
  series::Series,
};
use serde::Serialize;
use serde_json::Value;
use tokio::{sync::broadcast::Receiver as BroadcastReceiver, task::JoinHandle};

use super::*;
use crate::middleware::{connection::revm_logs_to_ethers_logs, ArbiterMiddleware};

pub(crate) type FilterDecoder =
  BTreeMap<String, (FilteredParams, Box<dyn Fn(&RawLog) -> String + Send + Sync>)>;
/// `EventLogger` is a struct that logs events from the Ethereum network.
///
/// It contains a BTreeMap of events, where each event is represented by a
/// string key and a vector of `Event` instances. It also optionally contains a
/// path where the event logs will be stored.
///
/// # Type Parameters
///
/// * `M` - Middleware that implements the `Middleware` trait, `std::borrow::Borrow<D>`, and has a
///   static lifetime.
/// * `D` - Middleware that implements the `Middleware` trait, `Debug`, `Send`, `Sync`, and has a
///   static lifetime.
/// * `E` - Type that implements the `EthLogDecode`, `Debug`, `Serialize` traits, and has a static
///   lifetime.
pub struct Logger {
  decoder:          FilterDecoder,
  receiver:         Option<BroadcastReceiver<Broadcast>>,
  output_file_type: Option<OutputFileType>,
  directory:        Option<String>,
  file_name:        Option<String>,
  metadata:         Option<Value>,
}

impl Debug for Logger {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("EventLogger")
      .field("receiver", &self.receiver)
      .field("output_file_type", &self.output_file_type)
      .field("directory", &self.directory)
      .field("file_name", &self.file_name)
      .field("metadata", &self.metadata)
      .finish()
  }
}

/// `OutputFileType` is an enumeration that represents the different types of
/// file formats that the `EventLogger` can output to.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum OutputFileType {
  /// * `JSON` - Represents the JSON file format. When this variant is used, the `EventLogger` will
  ///   output the logged events to a JSON file.
  JSON,
  /// * `CSV` - Represents the CSV (Comma Separated Values) file format. When this variant is used,
  ///   the `EventLogger` will output the logged events to a CSV file.
  CSV,
  /// * `Parquet` - Represents the Parquet file format. When this variant is used, the
  ///   `EventLogger` will output the logged events to a Parquet file. Parquet is a columnar
  ///   storage file format that is optimized for use with big data processing frameworks.
  Parquet,
}

impl Logger {
  /// Constructs a new `EventLogger`.
  ///
  /// # Returns
  ///
  /// A fresh `EventLogger` instance with an uninitialized events BTreeMap and
  /// no specified path.
  pub fn builder() -> Self {
    debug!("`EventLogger` initialized");
    Self {
      directory:        None,
      file_name:        None,
      decoder:          BTreeMap::new(),
      receiver:         None,
      // shutdown_sender: None,
      output_file_type: None,
      metadata:         None,
    }
  }

  /// Adds an event to the `EventLogger`.
  ///
  /// # Arguments
  ///
  /// * `event` - The event to be added.
  /// * `name` - The name of the event.
  ///
  /// # Returns
  ///
  /// The `EventLogger` instance with the added event.
  pub fn with_event<S: Into<String>, D: EthLogDecode + Debug + Serialize + 'static>(
    mut self,
    event: Event<Arc<ArbiterMiddleware>, ArbiterMiddleware, D>,
    name: S,
  ) -> Self {
    let name = name.into();
    // Grab the connection from the client and add a new event sender so that we
    // have a distinct channel to now receive events over
    let event_transmuted: EventTransmuted<Arc<ArbiterMiddleware>, ArbiterMiddleware, D> =
      unsafe { transmute(event) };
    let middleware = event_transmuted.provider.clone();
    let decoder = |x: &_| serde_json::to_string(&D::decode_log(x).unwrap()).unwrap();
    let filter = event_transmuted.filter.clone();
    self.decoder.insert(name.clone(), (FilteredParams::new(Some(filter)), Box::new(decoder)));
    let connection = middleware.provider().as_ref();
    if self.receiver.is_none() {
      self.receiver = Some(connection.event_sender.subscribe());
    }
    debug!("`EventLogger` now provided with event labeled: {:?}", name);
    self
  }

  /// Sets the directory for the `EventLogger`.
  ///
  /// # Arguments
  ///
  /// * `directory` - The directory where the event logs will be stored.
  ///
  /// # Returns
  ///
  /// The `EventLogger` instance with the specified directory.
  pub fn directory<S: Into<String>>(mut self, path: S) -> Self {
    let cwd = std::env::current_dir().unwrap();
    let full_path = cwd.join(path.into());
    self.directory = Some(full_path.to_str().unwrap().to_owned());
    debug!("`EventLogger` output directory set to: {:?}", full_path);
    self
  }

  /// Sets the output file name for the `EventLogger`.
  ///
  /// # Arguments
  ///
  /// * `file_name` - The file where the event logs will be stored.
  ///
  /// # Returns
  ///
  /// The `EventLogger` instance with the specified file.
  pub fn file_name<S: Into<String>>(mut self, path: S) -> Self {
    let path = path.into();
    self.file_name = Some(path.clone());
    debug!("`EventLogger` output file name set to: {:?}", path);
    self
  }

  /// Sets the output file type for the `EventLogger`.
  /// The default file type is JSON.
  /// # Arguments
  ///
  /// * `file_type` - The file type that the event logs will be stored in.
  ///
  /// # Returns
  ///
  /// The `EventLogger` instance with the specified file type.
  pub fn file_type(mut self, file_type: OutputFileType) -> Self {
    self.output_file_type = Some(file_type);
    self
  }

  /// Sets the metadata for the `EventLogger`.
  ///
  /// # Arguments
  ///
  /// * `metadata` - The metadata to be stored with the event logs which must implement the
  ///   `Serialize` trait.
  ///
  /// # Returns
  ///
  /// The `EventLogger` instance with the specified metadata.
  pub fn metadata(mut self, metadata: impl Serialize) -> Result<Self, serde_json::Error> {
    let metadata = serde_json::to_value(metadata)?;
    self.metadata = Some(metadata);
    debug!("`EventLogger` metadata provided");
    Ok(self)
  }

  /// Executes the `EventLogger`.
  ///
  /// This function starts the event logging process. It first deletes the
  /// existing events directory, then creates a new directory for each
  /// event. For each event, it creates a new CSV file and writes
  /// the event data into the file. If the file already exists, it appends the
  /// new data to the file.
  ///
  /// # Returns
  ///
  /// A `Result` which is:
  ///
  /// * `Ok(())` if the `EventLogger` ran successfully.
  /// * `Err(RevmMiddlewareError)` if there was an error running the `EventLogger`.
  ///
  /// # Errors
  ///
  /// This function will return an error if there is a problem creating the
  /// directories or files, or writing to the files.
  pub fn run(self) -> Result<JoinHandle<()>, ArbiterCoreError> {
    let mut receiver = self.receiver.unwrap();
    let dir = self.directory.unwrap_or("./data".into());
    let file_name = self.file_name.unwrap_or("output".into());
    let file_type = self.output_file_type.unwrap_or(OutputFileType::JSON);
    let metadata = self.metadata.clone();
    let task = tokio::spawn(async move {
      let mut events: BTreeMap<String, BTreeMap<String, Vec<Value>>> = BTreeMap::new();
      while let Ok(broadcast) = receiver.recv().await {
        match broadcast {
          Broadcast::StopSignal => {
            debug!("`EventLogger` has seen a stop signal");
            // create new directory with path
            let output_dir = std::env::current_dir().unwrap().join(dir);
            std::fs::create_dir_all(&output_dir).unwrap();
            let file_path = output_dir.join(format!("{}.json", file_name));
            debug!(
              "`EventLogger` dumping event data into: {:?}",
              file_path.to_str().unwrap().to_owned()
            );
            // match the file output type and write to correct file using the right file
            // type
            match file_type {
              OutputFileType::JSON => {
                let file_path = output_dir.join(format!("{}.json", file_name));
                let file = std::fs::File::create(file_path).unwrap();
                let writer = BufWriter::new(file);

                #[derive(Serialize, Clone)]
                struct OutputData<T> {
                  events:   BTreeMap<String, BTreeMap<String, Vec<Value>>>,
                  metadata: Option<T>,
                }
                let data = OutputData { events, metadata };
                serde_json::to_writer(writer, &data).expect("Unable to write data");
              },
              OutputFileType::CSV => {
                // Write the DataFrame to a CSV file
                let mut df = flatten_to_data_frame(events);
                let file_path = output_dir.join(format!("{}.csv", file_name));
                let file = std::fs::File::create(file_path).unwrap_or_else(|_| {
                  panic!("Error creating csv file");
                });
                let mut writer = CsvWriter::new(file);
                writer.finish(&mut df).unwrap_or_else(|_| {
                  panic!("Error writing to csv file");
                });
              },
              OutputFileType::Parquet => {
                // Write the DataFrame to a parquet file
                let mut df = flatten_to_data_frame(events);
                let file_path = output_dir.join(format!("{}.parquet", file_name));
                let file = std::fs::File::create(file_path).unwrap_or_else(|_| {
                  panic!("Error creating parquet file");
                });
                let writer = ParquetWriter::new(file);
                writer.finish(&mut df).unwrap_or_else(|_| {
                  panic!("Error writing to parquet file");
                });
              },
            }
            break;
          },
          Broadcast::Event(event, receipt_data) => {
            trace!("`EventLogger` received an event");
            let ethers_logs = revm_logs_to_ethers_logs(event, &receipt_data);
            for log in ethers_logs {
              for (contract_name, (filter, decoder)) in self.decoder.iter() {
                if filter.filter_address(&log) && filter.filter_topics(&log) {
                  let cloned_logs = log.clone();
                  let event_as_value =
                    serde_json::from_str::<Value>(&decoder(&cloned_logs.into())).unwrap();
                  let event_as_object = event_as_value.as_object().unwrap();

                  let contract = events.get(contract_name);
                  if contract.is_none() {
                    events.insert(contract_name.clone(), BTreeMap::new());
                  }
                  let contract = events.get_mut(contract_name).unwrap();

                  let event_name =
                    event_as_object.clone().keys().collect::<Vec<&String>>()[0].clone();

                  let event = contract.get_mut(&event_name);
                  if event.is_none() {
                    contract.insert(event_name.to_string(), vec![]);
                  }
                  let event = contract.get_mut(&event_name).unwrap();

                  for (_key, value) in event_as_object {
                    event.push(value.clone());
                  }
                  trace!("`EventLogger` successfully filtered and logged the event")
                }
              }
            }
          },
        }
      }
    });
    Ok(task)
  }
}

fn flatten_to_data_frame(events: BTreeMap<String, BTreeMap<String, Vec<Value>>>) -> DataFrame {
  // 1. Flatten the BTreeMap
  let mut contract_names = Vec::new();
  let mut event_names = Vec::new();
  let mut event_values = Vec::new();

  for (contract, events) in &events {
    for (event, values) in events {
      for value in values {
        contract_names.push(contract.clone());
        event_names.push(event.clone());
        event_values.push(value.to_string());
      }
    }
  }

  // 2. Convert the vectors into a DataFrame
  DataFrame::new(vec![
    Series::new("contract_name", contract_names),
    Series::new("event_name", event_names),
    Series::new("event_value", event_values),
  ])
  .unwrap()
}
pub(crate) struct EventTransmuted<B, M, D> {
  /// The event filter's state
  pub filter:          Filter,
  pub(crate) provider: B,
  /// Stores the event datatype
  pub(crate) datatype: PhantomData<D>,
  pub(crate) _m:       PhantomData<M>,
}

/// Adds an event to the `EventLogger` and generates a unique ID for the
/// event since we don't need to name events that are solely streamed and
/// not stored.
pub fn stream_event<D: EthLogDecode + Debug + Serialize + 'static>(
  event: Event<Arc<ArbiterMiddleware>, ArbiterMiddleware, D>,
) -> Pin<Box<dyn Stream<Item = D> + Send + Sync>> {
  let mut hasher = Sha256::new();
  hasher.update(serde_json::to_string(&event.filter).unwrap());
  let hash = hasher.finalize();
  let id = hex::encode(hash);
  let mut logger = Logger::builder().with_event(event, id);

  if let Some(mut receiver) = logger.receiver.take() {
    let stream = async_stream::stream! {
        while let Ok(broadcast) = receiver.recv().await {
            match broadcast {
                Broadcast::StopSignal => {
                    trace!("`EventLogger` has seen a stop signal");
                    break;
                }
                Broadcast::Event(event, receipt_data) => {
                    trace!("`EventLogger` received an event");
                    let ethers_logs = revm_logs_to_ethers_logs(event, &receipt_data);
                    for log in &ethers_logs {
                        for (_id, (filter, _)) in logger.decoder.iter() {
                            if filter.filter_address(log) && filter.filter_topics(log) {
                                let raw_log = RawLog::from(log.clone());
                                yield D::decode_log(&raw_log).unwrap();
                            }
                        }
                    }
                }
            }
        }
    };
    Box::pin(stream)
  } else {
    unreachable!()
  }
}
