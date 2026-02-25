//! Public façade crate for Arbiter.
//!
//! The project currently keeps most implementation work inside `arbiter-core` and re-exports a
//! stable surface from `arbiter` while the ecosystem evolves.

pub use arbiter_core as core;
#[allow(unused_imports)]
pub use arbiter_core::*;

pub use arbiter_core::prelude;

pub mod agent {
  pub use arbiter_core::agent::*;
}

pub mod handler {
  pub use arbiter_core::handler::*;
}

pub mod network {
  pub use arbiter_core::network::*;
}

pub mod observe {
  pub use arbiter_core::observe::*;
}

pub mod protocol {
  pub use arbiter_core::protocol::*;
}

#[cfg(feature = "replay")]
pub mod replay {
  pub use arbiter_core::replay::*;
}

pub mod runtime {
  pub use arbiter_core::runtime::*;
}
