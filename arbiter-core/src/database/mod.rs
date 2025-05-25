//! The [`ArbiterDB`] is a wrapper around a `CacheDB` that is used to provide
//! access to the `Environment`'s database to multiple `Coprocessors`.
//! It is also used to be able to write out the `Environment` database to a
//! file.
//!
//! Further, it gives the ability to be generated from a [`fork::Fork`] so that
//! you can preload an [`environment::Environment`] with a specific state.

use std::{
  fs,
  io::{self, Read, Write},
};

use revm::{
  primitives::{db::DatabaseRef, keccak256, Bytecode, B256},
  DatabaseCommit,
};
use serde_json;

use super::*;
pub mod fork;
pub mod inspector;

/// A [`ArbiterDB`] is contains both a [`CacheDB`] that is used to provide
/// state for the [`environment::Environment`]'s as well as for multiple
/// [`coprocessor::Coprocessor`]s.
/// The `logs` field is a [`HashMap`] to store [`ethers::types::Log`]s that can
/// be queried from at any point.
#[derive(Debug, Serialize, Deserialize)]
pub struct ArbiterDB {
  /// The state of the `ArbiterDB`. This is a `CacheDB` that is used to
  /// provide a db for the `Environment` to use.
  pub state: Arc<RwLock<CacheDB<EmptyDB>>>,

  /// The logs of the `ArbiterDB`. This is a `HashMap` that is used to store
  /// logs that can be queried from at any point.
  pub logs: Arc<RwLock<HashMap<U256, Vec<eLog>>>>,
}

// Implement `Clone` by hand so we utilize the `Arc`'s `Clone` implementation.
impl Clone for ArbiterDB {
  fn clone(&self) -> Self { Self { state: self.state.clone(), logs: self.logs.clone() } }
}

impl ArbiterDB {
  /// Create a new `ArbiterDB`.
  pub fn new() -> Self {
    Self {
      state: Arc::new(RwLock::new(CacheDB::new(EmptyDB::new()))),
      logs:  Arc::new(RwLock::new(HashMap::new())),
    }
  }

  /// Write the `ArbiterDB` to a file at the given path.``
  pub fn write_to_file(&self, path: &str) -> io::Result<()> {
    // Serialize the ArbiterDB
    let serialized = serde_json::to_string(self)?;
    // Write to file
    let mut file = fs::File::create(path)?;
    file.write_all(serialized.as_bytes())?;
    Ok(())
  }

  /// Read the `ArbiterDB` from a file at the given path.
  pub fn read_from_file(path: &str) -> io::Result<Self> {
    // Read the file content
    let mut file = fs::File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    // Deserialize the content into ArbiterDB
    #[derive(Deserialize)]
    struct TempDB {
      state: Option<CacheDB<EmptyDB>>,
      logs:  Option<HashMap<U256, Vec<eLog>>>,
    }
    let temp_db: TempDB = serde_json::from_str(&contents)?;
    Ok(Self {
      state: Arc::new(RwLock::new(temp_db.state.unwrap_or_default())),
      logs:  Arc::new(RwLock::new(temp_db.logs.unwrap_or_default())),
    })
  }
}

impl Default for ArbiterDB {
  fn default() -> Self { Self::new() }
}

// TODO: This is a BAD implementation of PartialEq, but it works for now as we
// do not ever really need to compare DBs directly at the moment.
// This is only used in the `Outcome` enum for `instruction.rs`.
impl PartialEq for ArbiterDB {
  fn eq(&self, _other: &Self) -> bool { true }
}

impl Database for ArbiterDB {
  type Error = Infallible;

  // TODO: Not sure we want this, but it works for now.

  fn basic(
    &mut self,
    address: revm::primitives::Address,
  ) -> Result<Option<AccountInfo>, Self::Error> {
    self.state.write().unwrap().basic(address)
  }

  fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
    self.state.write().unwrap().code_by_hash(code_hash)
  }

  fn storage(
    &mut self,
    address: revm::primitives::Address,
    index: U256,
  ) -> Result<U256, Self::Error> {
    self.state.write().unwrap().storage(address, index)
  }

  fn block_hash(&mut self, number: U256) -> Result<B256, Self::Error> {
    self.state.write().unwrap().block_hash(number)
  }
}

impl DatabaseRef for ArbiterDB {
  type Error = Infallible;

  // TODO: Not sure we want this, but it works for now.

  fn basic_ref(
    &self,
    address: revm::primitives::Address,
  ) -> Result<Option<AccountInfo>, Self::Error> {
    self.state.read().unwrap().basic_ref(address)
  }

  fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
    self.state.read().unwrap().code_by_hash_ref(code_hash)
  }

  fn storage_ref(
    &self,
    address: revm::primitives::Address,
    index: U256,
  ) -> Result<U256, Self::Error> {
    self.state.read().unwrap().storage_ref(address, index)
  }

  fn block_hash_ref(&self, number: U256) -> Result<B256, Self::Error> {
    self.state.read().unwrap().block_hash_ref(number)
  }
}

impl DatabaseCommit for ArbiterDB {
  fn commit(
    &mut self,
    changes: revm_primitives::HashMap<revm::primitives::Address, revm::primitives::Account>,
  ) {
    self.state.write().unwrap().commit(changes)
  }
}

/// [AnvilDump] models the schema of an [anvil](https://github.com/foundry-rs/foundry) state dump.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnvilDump {
  /// Mapping of account addresses to [AccountRecord]s stored in the dump
  /// file.
  pub accounts: BTreeMap<Address, AccountRecord>,
}

/// [AccountRecord] describes metadata about an account within the state trie.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountRecord {
  /// The nonce of the account.
  pub nonce:   u64,
  /// The balance of the account.
  pub balance: U256,
  /// The bytecode of the account. If empty, the account is an EOA.
  pub code:    Bytes,
  /// The storage mapping of the account.
  pub storage: revm_primitives::HashMap<U256, U256>,
}

impl TryFrom<AnvilDump> for CacheDB<EmptyDB> {
  type Error = <CacheDB<EmptyDB> as Database>::Error;

  fn try_from(dump: AnvilDump) -> Result<Self, Self::Error> {
    let mut db = CacheDB::default();

    dump.accounts.into_iter().try_for_each(|(address, account_record)| {
      db.insert_account_info(address, AccountInfo {
        balance:   account_record.balance,
        nonce:     account_record.nonce,
        code_hash: keccak256(account_record.code.as_ref()),
        code:      (!account_record.code.is_empty())
          .then(|| Bytecode::new_raw(account_record.code)),
      });
      db.replace_account_storage(address, account_record.storage)
    })?;

    Ok(db)
  }
}

#[cfg(test)]
mod tests {
  use revm_primitives::{address, bytes};

  use super::*;

  #[test]
  fn read_write_to_file() {
    let db = ArbiterDB::new();
    db.write_to_file("test.json").unwrap();
    let db = ArbiterDB::read_from_file("test.json").unwrap();
    assert_eq!(db, ArbiterDB::new());
    fs::remove_file("test.json").unwrap();
  }

  #[test]
  fn load_anvil_dump_cachedb() {
    const RAW_DUMP: &str = r#"
        {
            "accounts": {
                "0x0000000000000000000000000000000000000000": {
                    "nonce": 1234,
                    "balance": "0xfacade",
                    "code": "0x",
                    "storage": {}
                },
                "0x0000000000000000000000000000000000000001": {
                    "nonce": 555,
                    "balance": "0xc0ffee",
                    "code": "0xbadc0de0",
                    "storage": {
                        "0x0000000000000000000000000000000000000000000000000000000000000000": "0x000000000000000000000000000000000000000000000000000000000000deAD",
                        "0x0000000000000000000000000000000000000000000000000000000000000001": "0x000000000000000000000000000000000000000000000000000000000000babe"
                    }
                }
            }
        }
        "#;

    let dump: AnvilDump = serde_json::from_str(RAW_DUMP).unwrap();
    let mut db: CacheDB<EmptyDB> = dump.try_into().unwrap();

    let account_a = db.load_account(address!("0000000000000000000000000000000000000000")).unwrap();
    assert_eq!(account_a.info.nonce, 1234);
    assert_eq!(account_a.info.balance, U256::from(0xfacade));
    assert_eq!(account_a.info.code, None);
    assert_eq!(account_a.info.code_hash, keccak256([]));

    let account_b = db.load_account(address!("0000000000000000000000000000000000000001")).unwrap();
    let b_bytecode = bytes!("badc0de0");
    assert_eq!(account_b.info.nonce, 555);
    assert_eq!(account_b.info.balance, U256::from(0xc0ffee));
    assert_eq!(account_b.info.code_hash, keccak256(b_bytecode.as_ref()));
    assert_eq!(account_b.info.code, Some(Bytecode::new_raw(b_bytecode)));
    assert_eq!(account_b.storage.get(&U256::ZERO), Some(&U256::from(0xdead)));
    assert_eq!(account_b.storage.get(&U256::from(1)), Some(&U256::from(0xbabe)));
  }
}
