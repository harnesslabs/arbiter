//! # Leader-Follower Demo Server
//!
//! A simple web server that serves the interactive Leader-Follower demo.
//!
//! ## Usage
//! ```bash
//! cargo run --bin leader
//! ```
//! Then open <http://localhost:3030>

#[cfg(not(target_arch = "wasm32"))]
use warp::Filter;

const HTML_CONTENT: &str = include_str!("../index.html");

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() {
  println!("🦀 Starting Leader-Follower Demo Server...");

  // Build the WASM module automatically
  let manifest_dir = env!("CARGO_MANIFEST_DIR");
  println!("📦 Building WebAssembly module...");
  let status = std::process::Command::new("wasm-pack")
    .arg("build")
    .arg("--target")
    .arg("web")
    .current_dir(manifest_dir)
    .status()
    .expect("Failed to execute wasm-pack. Is it installed?");

  if !status.success() {
    eprintln!("❌ Failed to build WebAssembly module.");
    std::process::exit(1);
  }
  println!("✅ WebAssembly build complete.");

  // Serve the main HTML page
  let index = warp::path::end()
    .map(|| warp::reply::html(HTML_CONTENT))
    .with(warp::reply::with::header("Cache-Control", "no-cache, no-store, must-revalidate"));

  // Serve WASM files from pkg directory
  let pkg_dir = format!("{}/pkg", manifest_dir);
  let wasm_files = warp::path("pkg")
    .and(warp::fs::dir(pkg_dir))
    .with(warp::reply::with::header("Cache-Control", "no-cache, no-store, must-revalidate"));

  // Combine routes with CORS and logging
  let routes = index.or(wasm_files).with(warp::cors().allow_any_origin()).with(warp::log("leader"));

  println!("🌐 Demo available at: http://localhost:3030");
  println!("📖 Click to add points, right-click to remove, adjust epsilon slider!");
  println!("🛑 Press Ctrl+C to stop the server");

  warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
  panic!("This is a server");
}
