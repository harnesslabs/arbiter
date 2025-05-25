#[allow(unused_imports)] use rayon::result;
use tempfile::tempdir;

use super::*;

#[test]
fn test_safe_module_name() {
  assert_eq!(safe_module_name("Valid"), "valid");
  assert_eq!(safe_module_name("Enum"), "enum_");
  assert_eq!(safe_module_name("Mod"), "mod_");
  assert_eq!(safe_module_name("2Two"), "_2_two");
}

#[test]
fn test_collect_contract_list_from_contracts() {
  // Create a temporary directory
  let dir = tempfile::tempdir().expect("Failed to create temporary directory");
  let setting = ArbiterConfig::_new_mock_config();
  // Create nested directories "src" and "contracts"
  let contracts_dir = dir.path().join("contracts");
  fs::create_dir(&contracts_dir).expect("Failed to create contracts directory");

  // Create files in the contracts directory
  fs::write(contracts_dir.join("ExampleContract.sol"), "").expect("Failed to write file");
  fs::write(contracts_dir.join("AnotherTest.sol"), "").expect("Failed to write file");
  fs::write(contracts_dir.join("ITestInterface.sol"), "").expect("Failed to write file"); // This should be ignored
  fs::write(contracts_dir.join("G3M.sol"), "").expect("Failed to write file");
  fs::write(contracts_dir.join("SD59x18Math.sol"), "").expect("Failed to write file");

  // Call the function
  let (mut contracts, _) =
    collect_contract_list(dir.path(), &setting).expect("Failed to collect contracts");
  contracts.sort();
  // Assert the results
  let mut expected = vec![
    "shared_types",
    "example_contract",
    "sd5_9x_18_math",
    "g3m",
    "another_test",
    "i_test_interface",
  ];
  expected.sort();
  assert_eq!(contracts, expected);

  // Temp dir will be automatically cleaned up after going out of scope.
}

#[test]
fn test_collect_contract_list_from_src() {
  // Create a temporary directory
  let dir = tempfile::tempdir().expect("Failed to create temporary directory");
  let config = ArbiterConfig::_new_mock_config();

  // Create a nested directory "src"
  let src_dir = dir.path().join("src");
  fs::create_dir(&src_dir).expect("Failed to create src directory");

  // Create files in the src directory
  fs::write(src_dir.join("ExampleOne.sol"), "").expect("Failed to write file");
  fs::write(src_dir.join("TestTwo.sol"), "").expect("Failed to write file");
  fs::write(src_dir.join("ITestInterface.sol"), "").expect("Failed to write file"); // This should be ignored
  fs::write(src_dir.join("G3M.sol"), "").expect("Failed to write file"); // This should be ignored
  fs::write(src_dir.join("SD59x18Math.sol"), "").expect("Failed to write file"); // This should be ignored

  // Call the function
  let (mut contracts, _) =
    collect_contract_list(dir.path(), &config).expect("Failed to collect contracts");
  contracts.sort();
  // Assert the results
  let mut expected =
    vec!["shared_types", "sd5_9x_18_math", "example_one", "test_two", "g3m", "i_test_interface"];
  expected.sort();
  assert_eq!(contracts, expected);

  // Temp dir will be automatically cleaned up after going out of scope.
}
#[test]
fn test_update_mod_file() {
  // Create a temporary directory
  let dir = tempfile::tempdir().expect("Failed to create temporary directory");

  // Mock a mod.rs file with some content
  let mocked_mod_path = dir.path().join("mod.rs");
  let content = "
    // Some comments
    pub mod example_contract;
    pub mod test_contract;
    ";
  fs::write(&mocked_mod_path, content).expect("Failed to write mock mod.rs file");

  // Call the function
  let contracts_to_keep = vec!["example_contract".to_owned()];
  update_mod_file(mocked_mod_path.parent().unwrap(), contracts_to_keep)
    .expect("Failed to update mod file");

  // Open the mocked mod.rs file and check its content
  let updated_content = fs::read_to_string(&mocked_mod_path).unwrap();
  assert!(updated_content.contains("pub mod example_contract;"));
  assert!(!updated_content.contains("pub mod test_contract;"));
}

#[test]
fn test_for_each_submodule() {
  // Create a mock ArbiterConfig with submodules set to true
  let arbiter_config = ArbiterConfig::_new_mock_config_with_submodules();

  // Create a temporary directory
  let dir = tempdir().expect("Failed to create temporary directory");

  // Create a mock project directory inside the temporary directory
  let project_dir = dir.path().join("mock_project");
  fs::create_dir_all(&project_dir).expect("Failed to create mock project directory");

  // Create a mock lib directory inside the project directory
  let lib_dir = project_dir.join("lib");
  fs::create_dir_all(&lib_dir).expect("Failed to create mock lib directory");

  // Create subdirectories inside the lib directory
  let subdirs = ["foo", "Bar", "duck"];
  for subdir in &subdirs {
    let subdir_path = lib_dir.join(subdir);
    fs::create_dir_all(&subdir_path).expect("Failed to create subdirectory");
  }

  // Add the mock lib directory to the FoundryConfig
  let foundry_config = FoundryConfig {
    libs: vec![lib_dir.to_str().unwrap().into()],
    ..Default::default() // Fill other fields with default values
  };

  // Call the function with the mock configs
  for lib_dir in &foundry_config.libs {
    let result = for_each_submodule(arbiter_config.clone(), lib_dir);
    assert!(result.is_ok());
  }
}

#[test]
fn test_submodule_bindings() {
  // Create a temporary directory
  let dir = tempdir().unwrap();
  let path = dir.path();

  // Create some subdirectories
  fs::create_dir(path.join("foo")).unwrap();
  fs::create_dir(path.join("bar")).unwrap();

  // Create some empty files in these subdirectories
  File::create(path.join("foo").join("file.txt")).unwrap();
  File::create(path.join("bar").join("file.txt")).unwrap();
  // Create a mock ArbiterConfig
  let config = ArbiterConfig::_new_mock_config_with_submodules();

  // Call the function
  let result = bindings_for_submodules(path, &config);

  assert!(result.is_ok());
  let (_output_path, _contract_list) = result.unwrap();

  println!("output_path: {:?}", _output_path);
  println!("contract_list: {:?}", _contract_list);
}

#[test]
fn test_forge_bind() {
  // Call the function
  let result = forge_bind();
  assert!(result.is_err());
}
