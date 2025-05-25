#![warn(missing_docs)]
use std::{
  fs::{write, File},
  io,
  io::{BufRead, BufReader, ErrorKind},
  path::PathBuf,
  process::Command,
};

use super::*;

pub(crate) mod digest;
#[cfg(test)] mod tests;
use foundry_config::Config as FoundryConfig;
use inflector::Inflector;
use proc_macro2::{Ident, Span};
use serde::Deserialize;

use self::digest::ArbiterConfig;

/// Runs the `forge` command-line tool to generate bindings.
///
/// This function attempts to execute the external command `forge` with the
/// provided arguments to generate necessary bindings. The bindings are stored
/// in the `arbiter/src/bindings/` directory, and existing bindings will be
/// overwritten. The function wraps the forge command to generate bindings as a
/// module to a specific destination.
///
/// # Returns
///
/// * `Ok(())` if the `forge` command successfully generates the bindings.
/// * `Err(std::io::Error)` if the command execution fails or if there's an error in generating the
///   bindings. This can also include if the `forge` tool is not installed.

pub(crate) fn forge_bind() -> std::io::Result<()> {
  let foundry_config = FoundryConfig::load();
  let arbiter_config = ArbiterConfig::new().unwrap_or_default();
  // let project_bidnings_output_path = arbiter_config.bindings_path;
  let output = Command::new("forge")
    .arg("bind")
    .arg("--revert-strings")
    .arg("debug")
    .arg("-b")
    .arg(arbiter_config.bindings_path.clone())
    .arg("--module")
    .arg("--overwrite")
    .arg("--force")
    .output()?;
  let project_contracts = collect_contract_list(&foundry_config.src, &arbiter_config)?;
  if output.status.success() {
    let output_str = String::from_utf8_lossy(&output.stdout);
    println!("Command output: {}", output_str);
  } else {
    let err_str = String::from_utf8_lossy(&output.stderr);
    println!("Command failed, error: {}, is forge installed?", err_str);
    return Err(std::io::Error::new(std::io::ErrorKind::Other, "Command failed"));
  }
  remove_unneeded_contracts(&arbiter_config.bindings_path, project_contracts.0)?;
  if arbiter_config.submodules {
    for lib_dir in &foundry_config.libs {
      for_each_submodule(arbiter_config.clone(), lib_dir)?;
    }
  }
  Ok(())
}
/// This function is used to generate bindings for each submodule in the library
/// directory. It takes in an ArbiterConfig and a reference to the library
/// directory path.
///
/// # Arguments
///
/// * `arbiter_config` - An instance of ArbiterConfig that holds the configuration for the Arbiter.
/// * `lib_dir` - A reference to a Path that holds the path to the library directory.
///
/// # Returns
///
/// This function returns an io::Result. If the function executes successfully,
/// it returns Ok(()). If there is an error during execution, it returns
/// Err(std::io::Error).

fn for_each_submodule(arbiter_config: ArbiterConfig, lib_dir: &Path) -> std::io::Result<()> {
  if lib_dir.is_dir() {
    for entry in fs::read_dir(lib_dir)? {
      let entry = entry?;
      let path = entry.path();
      // Check if the directory is a submodule
      if path.is_dir() && path.join(".git").exists() {
        println!("Generating bindings for library: {:?}", path);
        let (output_path, sub_module_contracts) = bindings_for_submodules(&path, &arbiter_config)?;
        if output_path.is_none() {
          continue;
        }
        remove_unneeded_contracts(&output_path.unwrap(), sub_module_contracts)?;
      }
    }
  }
  Ok(())
}

/// Generates bindings for submodules.
///
/// This function takes a path to a library directory and a reference to an
/// ArbiterConfig. It returns a Result containing a tuple of a PathBuf and a
/// Vector of Strings. The PathBuf is the output path for the bindings, and the
/// Vector of Strings is a list of contracts for which bindings are generated.
///
/// # Arguments
///
/// * `libdir` - A reference to a Path that holds the path to the library directory.
/// * `config` - A reference to an ArbiterConfig that holds the configuration for the Arbiter.
///
/// # Returns
///
/// This function returns an io::Result containing a tuple of a PathBuf and a
/// Vector of Strings. The PathBuf is the output path for the bindings, and the
/// Vector of Strings is a list of contracts for which bindings are generated.
///
/// # Errors
///
/// This function will return an error if there is a problem reading the
/// directory or running the forge command.

fn bindings_for_submodules(
  libdir: &Path,
  config: &ArbiterConfig,
) -> io::Result<(Option<PathBuf>, Vec<String>)> {
  let mut contracts_to_generate = Vec::new(); // to keep track of contracts we're generating bindings for
  let mut last_output_path = config.bindings_path.clone();
  if let Some(parent_path) = last_output_path.parent() {
    last_output_path = parent_path.to_path_buf();
  }
  let target;
  if libdir.is_dir()
    && libdir.join(".git").exists()
    && libdir.file_name().unwrap_or_default() != "forge-std"
  {
    (contracts_to_generate, target) = collect_contract_list(libdir, config)?;
    if contracts_to_generate.len() <= 1 {
      return Ok((None, contracts_to_generate));
    }
    let submodule_name = libdir.file_name().unwrap().to_str().unwrap().replace('-', "_");
    println!("submodule name: {:?}", submodule_name);

    let output_path = last_output_path
            .clone() // Get the bindings path from config
            .join(format!("{}_bindings", submodule_name));

    println!("output path: for submodule {:?} is {:?}", submodule_name, &output_path);
    let output = Command::new("forge")
      .arg("bind")
      .arg("--revert-strings")
      .arg("debug")
      .arg("-b")
      .arg(output_path.clone())
      .arg("-C")
      .arg(target)
      .arg("--module")
      .arg("--overwrite")
      .arg("--force")
      .output()?;

    if output.status.success() {
      let output_str = String::from_utf8_lossy(&output.stdout);
      println!("Command output: {}", output_str);
    } else {
      let err_str = String::from_utf8_lossy(&output.stderr);
      println!("Command failed, error: {}", err_str);
      return Err(std::io::Error::new(std::io::ErrorKind::Other, "Command failed"));
    }
    last_output_path = output_path;
  }
  Ok((Some(last_output_path), contracts_to_generate))
}

/// This function collects a list of contract names from a given directory and
/// its subdirectories.
///
/// # Arguments
///
/// * `dir` - A reference to a Path that represents the directory to search.
/// * `settings` - A reference to an ArbiterConfig that holds the configuration for the Arbiter.
///
/// # Returns
///
/// This function returns an io::Result that contains a Vector of Strings. Each
/// String in the Vector is a contract name. If the function executes
/// successfully, it returns Ok(Vec<String>). If there is an error during
/// execution, it returns Err(std::io::Error).
fn collect_contract_list(
  dir: &Path,
  settings: &ArbiterConfig,
) -> io::Result<(Vec<String>, PathBuf)> {
  let mut contract_list = Vec::new();
  let mut target_dir = dir.to_path_buf();
  contract_list.push("shared_types".to_string());
  if dir.is_dir() {
    let dir_name = dir.file_name().unwrap().to_str().unwrap(); // Assuming file_name() is not None and is valid UTF-8

    target_dir = if dir_name == "src" || dir_name == "contracts" {
      dir.to_path_buf()
    } else {
      // Look inside the directory for a directory named "src" or "contracts"
      let potential_src = dir.join("src");
      let potential_contracts = dir.join("contracts");
      if potential_src.is_dir() {
        potential_src
      } else if potential_contracts.is_dir() {
        potential_contracts
      } else {
        dir.to_path_buf()
      }
    };

    for entry in fs::read_dir(target_dir.clone())? {
      let entry = entry?;
      let path = entry.path();
      if path.is_file() {
        let is_test = is_test(path.to_str().unwrap());
        let filename = path.file_stem().unwrap().to_str().unwrap();
        let valid_name = Ident::new(filename, proc_macro2::Span::call_site());
        let safe_filename = safe_module_name(&valid_name.to_string());
        if !settings.ignore_interfaces || !safe_filename.starts_with('i') && !is_test {
          contract_list.push(safe_filename);
        }
      }
    }
  }
  Ok((contract_list, target_dir))
}

fn is_test(file_path: &str) -> bool {
  let path = Path::new(file_path);
  match path.extension() {
    Some(ext) => ext != "t",
    None => true,
  }
}

/// Removes unneeded contracts from the bindings path.
///
/// This function takes a reference to a PathBuf that represents the bindings
/// path and a Vector of Strings that represents the needed contracts.
/// It returns an io::Result. If the function executes successfully, it returns
/// Ok(()). If there is an error during execution, it returns
/// Err(std::io::Error).
///
/// # Arguments
///
/// * `bindings_path` - A reference to a PathBuf that represents the bindings path.
/// * `needed_contracts` - A Vector of Strings that represents the needed contracts.
///
/// # Returns
///
/// This function returns an io::Result. If the function executes successfully,
/// it returns Ok(()). If there is an error during execution, it returns
/// Err(std::io::Error).
fn remove_unneeded_contracts(
  bindings_path: &PathBuf,
  needed_contracts: Vec<String>,
) -> io::Result<()> {
  if needed_contracts.is_empty() {
    return Ok(());
  }
  if bindings_path.is_dir() {
    let entries = fs::read_dir(bindings_path).map_err(|e| {
      std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to read directory: {}", e))
    })?;

    for entry in entries {
      let entry = entry.map_err(|e| {
        std::io::Error::new(
          std::io::ErrorKind::Other,
          format!("Failed to read directory entry: {}", e),
        )
      })?;
      let path = entry.path();

      if path.is_file() {
        let filename = path
          .file_stem()
          .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "Failed to get file stem"))?
          .to_str()
          .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to convert OsStr to str")
          })?
          .to_lowercase();

        // Skip if the file is `mod.rs`
        if filename == "mod" || filename == "settings" {
          continue;
        }
        let needed_contracts: Vec<String> =
          needed_contracts.clone().into_iter().map(|contract| contract.to_lowercase()).collect();
        if !needed_contracts.contains(&filename) {
          fs::remove_file(&path).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to remove file: {}", e))
          })?;
        }
      }
    }
  }

  // update_mod_file(bindings_path, needed_contracts).unwrap();
  update_mod_file(bindings_path, needed_contracts).map_err(|e| {
    std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to update mod file: {}", e))
  })?;

  Ok(())
}

/// Updates the `mod.rs` file in the bindings directory.
///
/// This function takes a path to the bindings directory and a vector of
/// contract names. It reads the existing `mod.rs` file, filters out any module
/// declarations that are not in the list of contracts to keep, and writes the
/// filtered lines back to the `mod.rs` file.
///
/// # Arguments
///
/// * `bindings_path` - A reference to a Path that represents the bindings directory.
/// * `contracts_to_keep` - A vector of contract names for which bindings should be kept.
///
/// # Returns
///
/// This function returns an io::Result. If the function executes successfully,
/// it returns Ok(()). If there is an error during execution, it returns
/// Err(std::io::Error).

fn update_mod_file(bindings_path: &Path, contracts_to_keep: Vec<String>) -> io::Result<()> {
  let mod_path = bindings_path.join("mod.rs");

  // Open the file and read its contents
  let file = match File::open(&mod_path) {
    Ok(file) => file,
    Err(e) =>
      if e.kind() == ErrorKind::NotFound {
        File::create(&mod_path)?
      } else {
        return Err(std::io::Error::new(
          std::io::ErrorKind::Other,
          format!("Failed to open file: {}", e),
        ));
      },
  };
  let reader = BufReader::new(file);

  let lines: Result<Vec<String>, std::io::Error> = reader
    .lines()
    .map(|line_result| {
      line_result.map(|line| {
        // Keep the line if it's a comment
        if line.trim().starts_with("//") || line.trim().starts_with('#') {
          return line;
        }

        // Check if the line is a module declaration and if it's one of the contracts we
        // want to keep
        if let Some(contract_name) =
          line.trim().strip_prefix("pub mod ").and_then(|s| s.strip_suffix(';'))
        {
          if contracts_to_keep.contains(&contract_name.to_string()) {
            return line;
          }
        }

        String::new() // return an empty string if the line is not
                      // needed
      })
    })
    .collect();

  let lines = match lines {
    Ok(lines) => lines,
    Err(e) =>
      return Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!("Failed to read lines from file: {}", e),
      )),
  };
  let lines: Vec<String> = lines.into_iter().filter(|line| !line.is_empty()).collect();

  // Write the new lines back to the mod.rs
  match write(&mod_path, lines.join("\n")) {
    Ok(_) => Ok(()),
    Err(e) =>
      Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to write to file: {}", e))),
  }
}

/// The following methods were picked out of https://github.com/gakonst/ethers-rs/blob/9d01a9810940d3acd7c78bf2b2f2ca85a74f73eb/ethers-contract/ethers-contract-abigen/src/lib.rs#L393
/// Expands an identifier string into a token and appending `_` if the
/// identifier is for a reserved keyword.
///
/// Parsing keywords like `self` can fail, in this case we add an underscore.
fn safe_ident(name: &str) -> Ident {
  syn::parse_str::<Ident>(name).unwrap_or_else(|_| ident(&format!("{name}_")))
}
/// Creates a new Ident with the given string at [`Span::call_site`].
///
/// # Panics
///
/// If the input string is neither a keyword nor a legal variable name.
fn ident(name: &str) -> Ident { Ident::new(name, Span::call_site()) }
/// converts invalid rust module names to valid ones
fn safe_module_name(name: &str) -> String {
  // handle reserve words used in contracts (eg Enum is a gnosis contract)
  safe_ident(&safe_snake_case(name)).to_string()
}

///  Converts a `&str` to `snake_case` `String` while respecting identifier
/// rules
fn safe_snake_case(ident: &str) -> String { safe_identifier_name(ident.to_snake_case()) }

/// respects identifier rules, such as, an identifier must not start with a
/// numeric char
fn safe_identifier_name(name: String) -> String {
  if name.starts_with(char::is_numeric) {
    format!("_{name}")
  } else {
    name
  }
}
