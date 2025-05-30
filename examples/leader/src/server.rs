//! # Vietoris-Rips Demo Server
//!
//! A simple web server that serves the interactive Vietoris-Rips complex demo.
//!
//! ## Usage
//! ```bash
//! cargo run --bin server
//! ```
//! Then open http://localhost:3030

#[cfg(not(target_arch = "wasm32"))] use warp::Filter;

const HTML_CONTENT: &str = include_str!("../index.html");

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() {
  println!("🦀 Starting Leader-Follower Demo Server...");

  // Serve the main HTML page
  let index = warp::path::end().map(|| warp::reply::html(HTML_CONTENT));

  // Serve WASM files from pkg directory
  let wasm_files = warp::path("pkg").and(warp::fs::dir("pkg"));

  // Combine routes with CORS
  let routes = index.or(wasm_files).with(warp::cors().allow_any_origin());

  println!("🌐 Demo available at: http://localhost:3030");
  println!("📖 Click to add points, right-click to remove, adjust epsilon slider!");
  println!("🛑 Press Ctrl+C to stop the server");

  warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
  panic!("This is a server");
}
