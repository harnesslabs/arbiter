#![warn(missing_docs)]

use super::*;

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Artifacts {
  #[serde(rename = "storageLayout")]
  pub(crate) storage_layout: StorageLayout,
  // TODO: Add more here if we need them.
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct StorageLayout {
  pub(crate) storage: Vec<StorageItem>,
  pub(crate) types:   HashMap<String, StorageType>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct StorageItem {
  #[serde(rename = "astId")]
  ast_id:   usize,
  contract: String,
  label:    String,
  offset:   usize,
  slot:     String,
  #[serde(rename = "type")]
  type_:    String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum StorageType {
  // mapping has to come first so we attempt to deserialize that way first.
  Mapping {
    encoding:        String,
    key:             String,
    value:           String,
    label:           Option<String>,
    #[serde(rename = "numberOfBytes")]
    number_of_bytes: Option<String>,
  },
  Simple {
    encoding:        String,
    label:           String,
    #[serde(rename = "numberOfBytes")]
    number_of_bytes: String,
  },
}

pub(crate) fn digest_artifacts(path: &str) -> Result<Artifacts, ArbiterError> {
  // Read the file to a string
  let mut cwd = env::current_dir().unwrap();
  cwd.push(path);
  println!("Reading artifacts from: {:?}", cwd);
  let data = fs::read_to_string(path)?;
  let json_data = serde_json::from_str(&data)?;

  Ok(json_data)
}

pub(crate) fn create_storage_layout(
  contract_data: &ContractMetadata,
  storage_layout: StorageLayout,
  db: &mut CacheDB<EmptyDB>,
  ethers_db: &mut EthersDB<Provider<Http>>,
) -> Result<(), ArbiterError> {
  for storage_item in storage_layout.storage {
    // The unwraps here should not fail.
    let label = storage_item.label;
    let slot = storage_item.slot;
    let slot_bytes =
      revm::primitives::U256::from_limbs(U256::from_str_radix(slot.as_str(), 10).unwrap().0);
    let storage =
      ethers_db.storage(contract_data.address.to_fixed_bytes().into(), slot_bytes).unwrap();
    db.insert_account_storage(contract_data.address.to_fixed_bytes().into(), slot_bytes, storage)
      .unwrap();
    match storage_layout.types.get(&storage_item.type_).unwrap() {
      StorageType::Simple { encoding: _, label: _, number_of_bytes: _ } => {
        // Already got the storage slot above, so continue.
        continue;
      },
      StorageType::Mapping {
        encoding: _encoding,
        key,
        value,
        label: _label,
        number_of_bytes: _number_of_bytes,
      } => {
        // Catch the case where the value of a map is a map. We don't handle that yet.
        if let StorageType::Mapping { .. } = storage_layout.types.get(&value.to_string()).unwrap() {
          println!("Only handling one map deep for now. A map of a map was found and ignored.");
          continue;
        }
        // We got a one-deep mapping, so we need to get the keys and values from the
        // config and properly pad everything to get the storage slot.
        let key_bytes = match storage_layout.types.get(&key.to_string()).unwrap() {
          StorageType::Simple { encoding: _, label: _, number_of_bytes } =>
            number_of_bytes.parse::<usize>().unwrap(),
          StorageType::Mapping { .. } => {
            println!("Only handling one map deep for now. A map of a map was found and ignored.");
            continue;
          },
        };
        if let StorageType::Mapping { .. } = storage_layout.types.get(&value.to_string()).unwrap() {
          println!("Only handling one map deep for now. A map of a map was found and ignored.");
          continue;
        }

        if let Some(keys) = contract_data.mappings.get(&label) {
          for key in keys {
            let mut padded_key_bytes = vec![0; 32 - key_bytes];
            let key_bytes = hex::decode(key).unwrap();
            padded_key_bytes.extend(key_bytes.clone());
            let to_hash: Vec<u8> =
              padded_key_bytes.into_iter().chain(slot_bytes.to_be_bytes_vec()).collect();
            let slot_to_get = keccak256(to_hash);
            let storage = ethers_db
              .storage(
                contract_data.address.to_fixed_bytes().into(),
                revm::primitives::U256::from_be_bytes(slot_to_get),
              )
              .unwrap();
            db.insert_account_storage(
              contract_data.address.to_fixed_bytes().into(),
              revm::primitives::U256::from_be_bytes(slot_to_get),
              storage,
            )
            .unwrap();
          }
        }
      },
    }
  }
  Ok(())
}
