#![warn(missing_docs)]

use std::{collections::HashMap, io::Write, sync::Arc};

use arbiter_core::database::fork::*;
use ethers::{
  providers::{Http, Provider},
  types::{Address, BlockId, BlockNumber, U256},
  utils::{hex, keccak256},
};
use revm::{
  db::{ethersdb::EthersDB, in_memory_db::CacheDB, EmptyDB, EmptyDBTyped},
  Database,
};
use serde::Serialize;

use super::*;

pub(crate) mod digest;
#[cfg(test)] mod tests;

/// A `ForkConfig` is a d
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ForkConfig {
  output_directory:          Option<String>,
  output_filename:           Option<String>,
  provider:                  String,
  block_number:              u64,
  #[serde(rename = "contracts")]
  contracts_meta:            HashMap<String, ContractMetadata>,
  externally_owned_accounts: HashMap<String, Address>,
}

impl ForkConfig {
  pub(crate) fn new(fork_config_path: &str) -> Result<Self, ConfigError> {
    let mut cwd = env::current_dir().unwrap();
    cwd.push(fork_config_path);
    println!("Reading config from: {:?}", cwd.to_str().unwrap());
    let config = Config::builder()
      .add_source(config::File::with_name(
        cwd.to_str().ok_or(ConfigError::NotFound("File not found!".to_owned()))?,
      ))
      .build()?;
    let mut fork_config: ForkConfig = config.try_deserialize()?;

    if fork_config.output_directory.is_none() {
      println!("No output path specified. Defaulting to current directory.");
      fork_config.output_directory = Some("./".to_string());
    }
    if fork_config.output_filename.is_none() {
      println!("No output filename specified. Defaulting to `output.json.`");
      fork_config.output_filename = Some("output.json".to_string());
    }

    Ok(fork_config)
  }

  /// Digests the config file and takes in an `EthersDB` so that the data can
  /// be fetched from the blockchain.
  /// Once all the `AccountInfo` for the contracts are fetched, we digest the
  /// contract artifacts to get the storage layout.
  pub(crate) fn digest_config(&self) -> Result<CacheDB<EmptyDB>, ArbiterError> {
    // Spawn the `EthersDB` and the `CacheDB` we will write to.
    let ethers_db = &mut self.spawn_ethers_db()?;
    let mut db = CacheDB::new(EmptyDBTyped::default());
    for contract_data in self.contracts_meta.values() {
      let address = contract_data.address;
      let info = ethers_db
        .basic(address.to_fixed_bytes().into())
        .map_err(|_| {
          ArbiterError::DBError(
            "Failed to fetch account info with
                EthersDB."
              .to_string(),
          )
        })?
        .ok_or(ArbiterError::DBError("Failed to fetch account info with EthersDB.".to_string()))?;

      db.insert_account_info(address.to_fixed_bytes().into(), info);
      let artifacts = digest::digest_artifacts(contract_data.artifacts_path.as_str())?;
      let storage_layout = artifacts.storage_layout;

      digest::create_storage_layout(contract_data, storage_layout, &mut db, ethers_db)?;

      for eoa in self.externally_owned_accounts.values() {
        let info = ethers_db
          .basic(eoa.to_fixed_bytes().into())
          .map_err(|_| {
            ArbiterError::DBError(
              "Failed to fetch account info with
                EthersDB."
                .to_string(),
            )
          })?
          .ok_or(ArbiterError::DBError(
            "Failed to fetch account info with EthersDB.".to_string(),
          ))?;
        db.insert_account_info(eoa.to_fixed_bytes().into(), info);
      }
    }
    Ok(db)
  }

  pub(crate) fn into_fork(self) -> Result<Fork, ArbiterError> {
    // Digest all of the contracts and their storage data listed in the fork config.
    let db = self.digest_config()?;
    Ok(Fork {
      db,
      contracts_meta: self.contracts_meta.clone(),
      eoa: self.externally_owned_accounts.clone(),
    })
  }

  pub(crate) fn write_to_disk(self, overwrite: &bool) -> Result<(), ArbiterError> {
    // The unwraps that appear here should not fail.

    // Check if a file at the output path already exists.
    let dir = self.output_directory.clone().unwrap();
    let file_path = Path::new(&self.output_directory.clone().unwrap())
      .join(self.output_filename.clone().unwrap());
    if file_path.try_exists().unwrap() && file_path.is_file() {
      if !overwrite {
        // TODO: We should allow for an overwrite flag here.
        panic!(
          "File already exists at output path. Please use the `--overwrite` flag, delete it, or \
           change the output path."
        );
      } else {
        // weirdly sometimes fails here with message: "No such file or directory"
        fs::remove_file(&file_path).unwrap();
      }
    }
    let fork = self.into_fork()?;
    let mut raw = HashMap::new();
    for (address, db_account) in fork.db.accounts {
      let info = db_account.info;
      let mut storage = HashMap::new();
      for key in db_account.storage.keys() {
        let recast_key = key.to_string();
        let recast_value = db_account.storage.get(key).unwrap().to_string();
        storage.insert(recast_key, recast_value);
      }
      raw.insert(Address::from(address.into_array()), (info, storage));
    }
    let disk_data =
      DiskData { meta: fork.contracts_meta, raw, externally_owned_accounts: fork.eoa };

    let json_data = serde_json::to_string(&disk_data)?;

    fs::create_dir_all(dir)?;
    let mut file = fs::File::create(file_path)?;
    file.write_all(json_data.as_bytes()).unwrap();
    println!("Wrote fork data to disk.");
    Ok(())
  }

  fn spawn_ethers_db(&self) -> Result<EthersDB<Provider<Http>>, ArbiterError> {
    let ethers_db = EthersDB::new(
      Arc::new(
        Provider::<Http>::try_from(self.provider.clone())
          .expect("could not instantiate HTTP Provider"),
      ),
      Some(BlockId::Number(BlockNumber::Number(self.block_number.into()))),
    )
    .unwrap();
    Ok(ethers_db)
  }
}
