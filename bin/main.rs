#![warn(missing_docs)]

//! `Arbiter` CLI Tool
//!
//! The Arbiter command-line interface provides minimum utilities for the
//! utilization of the arbiter-core crate. It is designed to be a simple and
//! versatile.
//!
//!
//! Key Features:
//! - Simulation Initialization: Allow users to kickstart new data analysis simulations.
//! - Contract Bindings: Generate necessary bindings for interfacing with different contracts.
//!
//!
//! This CLI leverages the power of Rust's type system to
//! offer fast and reliable operations, ensuring data integrity and ease of use.

use std::{env, fs, path::Path};

use clap::{command, CommandFactory, Parser, Subcommand};
use config::{Config, ConfigError};
use serde::Deserialize;
use thiserror::Error;

use crate::fork::ForkConfig;

mod bind;
mod fork;

/// Represents command-line arguments passed to the `Arbiter` tool.
#[derive(Parser)]
#[command(name = "Arbiter")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Ethereum Virtual Machine Logic Simulator", long_about = None)]
#[command(author)]
struct Args {
  /// Defines the subcommand to execute.
  #[command(subcommand)]
  command: Option<Commands>,
}

/// `ConfigurationError` enumeration type for errors parsing a `.toml`
/// configuration file.
#[derive(Error, Debug)]
pub enum ArbiterError {
  /// Indicates an error occurred during the parsing of the configuration
  /// file.
  #[error("Error with config parsing: {0}")]
  ConfigError(#[from] config::ConfigError),
  /// Indicates that the configuration file could not be read from the given
  /// path.
  #[error("Error with file IO: {0}")]
  IOError(#[from] std::io::Error),

  /// Indicates an error occurred during the deserialization of the `.toml`
  /// file.
  #[error("Error with toml deserialization: {0}")]
  TomlError(#[from] toml::de::Error),

  /// Indicates an error occurred during processing of a JSON file.
  #[error("Error with serde_json: {0}")]
  JsonError(#[from] serde_json::Error),

  /// Indicates an error occurred with a database.
  #[error("Error with DB: {0}")]
  DBError(String),
}

/// Defines available subcommands for the `Arbiter` tool.
#[derive(Subcommand)]
enum Commands {
  /// Represents the `Bind` subcommand.
  Bind,
  /// Represents the `Fork` subcommand.
  Fork {
    /// The name of the config file used to configure the fork.
    #[clap(index = 1)]
    fork_config_path: String,
    #[clap(long)]
    overwrite:        bool,
  },
}

/// The main entry point for the `Arbiter` tool.
///
/// This function parses command line arguments, and based on the provided
/// subcommand, either initializes a new simulation or generates bindings.
///
/// # Returns
///
/// * A `Result` which is either an empty tuple for successful execution or a dynamic error.
fn main() -> Result<(), ArbiterError> {
  let args = Args::parse();

  match &args.command {
    Some(Commands::Bind) => {
      println!("Generating bindings...");
      bind::forge_bind()?;
    },
    Some(Commands::Fork { fork_config_path, overwrite }) => {
      println!("Forking...");
      let fork_config = ForkConfig::new(fork_config_path)?;
      fork_config.write_to_disk(overwrite)?;
    },
    None => Args::command().print_long_help()?,
  }

  Ok(())
}
