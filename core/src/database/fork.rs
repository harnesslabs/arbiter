//! This module contains the [`Fork`] struct which is used to store the data
//! that will be loaded into an [`Environment`] and be used in `arbiter-core`.
//! [`Fork`] contains a [`CacheDB`] and [`ContractMetadata`] so
//! that the [`Environment`] can be initialized with a forked database and the
//! end-user still has access to the relevant metadata.

use std::{env, fs};

use super::*;

/// A [`ContractMetadata`] is used to store the metadata of a contract that will
/// be loaded into a [`Fork`].
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ContractMetadata {
  /// The address of the contract.
  pub address: eAddress,

  /// The path to the contract artifacts.
  pub artifacts_path: String,

  /// The mappings that are part of the contract's storage.
  pub mappings: HashMap<String, Vec<String>>,
}

/// A [`Fork`] is used to store the data that will be loaded into an
/// [`Environment`] and be used in `arbiter-core`. It is a wrapper around a
/// [`CacheDB`] and a [`HashMap`] of [`ContractMetadata`] so that the
/// [`environment::Environment`] can be initialized with the data and the
/// end-user still has access to the relevant metadata.
#[derive(Clone, Debug)]
pub struct Fork {
  /// The [`CacheDB`] that will be loaded into the [`Environment`].
  pub db: CacheDB<EmptyDB>,

  /// The [`HashMap`] of [`ContractMetadata`] that will be used by the
  /// end-user.
  pub contracts_meta: HashMap<String, ContractMetadata>,
  /// The [`HashMap`] of [`Address`] that will be used by the end-user.
  pub eoa:            HashMap<String, eAddress>,
}

impl Fork {
  /// Creates a new [`Fork`] from serialized [`DiskData`] stored on disk.
  pub fn from_disk(path: &str) -> Result<Self, ArbiterCoreError> {
    // Read the file
    let mut cwd = env::current_dir().unwrap();
    cwd.push(path);
    print!("Reading db from: {:?}", cwd);
    let data = fs::read_to_string(cwd).unwrap();

    // Deserialize the JSON data to your OutputData type
    let disk_data: DiskData = serde_json::from_str(&data).unwrap();

    // Create a CacheDB instance
    let mut db = CacheDB::new(EmptyDB::default());

    // Populate the CacheDB from the OutputData
    for (address, (info, storage_map)) in disk_data.raw {
      // Convert the string address back to its original type
      let address = address.as_fixed_bytes().into(); // You'd need to define this

      // Insert account info into the DB
      db.insert_account_info(address, info);

      // Insert storage data into the DB
      for (key_str, value_str) in storage_map {
        let key = U256::from_str_radix(&key_str, 10).unwrap();
        let value = U256::from_str_radix(&value_str, 10).unwrap();

        db.insert_account_storage(address, key, value).unwrap();
      }
    }

    Ok(Self { db, contracts_meta: disk_data.meta, eoa: disk_data.externally_owned_accounts })
  }
}

impl From<Fork> for CacheDB<EmptyDB> {
  fn from(val: Fork) -> Self { val.db }
}

type Storage = HashMap<String, String>;

/// This is the data that will be written to and loaded from disk to generate a
/// [`Fork`].
#[derive(Debug, Serialize, Deserialize)]
pub struct DiskData {
  /// This is the metadata for the contracts that will be loaded into the
  /// [`Fork`].
  pub meta: HashMap<String, ContractMetadata>,

  /// This is the raw data that will be loaded into the [`Fork`].
  pub raw: HashMap<eAddress, (AccountInfo, Storage)>,

  /// This is the eoa data that will be loaded into the [`Fork`].
  pub externally_owned_accounts: HashMap<String, eAddress>,
}
