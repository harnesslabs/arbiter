//! Type registry for network serialization.
//!
//! Provides a mapping between string type names and `TypeId`s since `TypeId`
//! is not stable across compilation boundaries or network boundaries.

use std::{
  any::{TypeId, type_name},
  collections::HashMap,
  sync::{Arc, OnceLock, RwLock},
};

/// A thread-safe registry mapping `String` type names to `TypeId`s.
#[derive(Default, Clone)]
pub struct TypeRegistry {
  name_to_id: Arc<RwLock<HashMap<String, TypeId>>>,
}

impl TypeRegistry {
  /// Registers a message type `M` in the registry.
  pub fn register<M: 'static>(&self) {
    let name = type_name::<M>().to_string();
    let id = TypeId::of::<M>();
    let mut map = self.name_to_id.write().unwrap();
    map.insert(name, id);
  }

  /// Retrieves the `TypeId` associated with a type name.
  pub fn get_id(&self, name: &str) -> Option<TypeId> {
    let map = self.name_to_id.read().unwrap();
    map.get(name).copied()
  }
}

/// Returns the global type registry.
pub fn global_registry() -> &'static TypeRegistry {
  static REGISTRY: OnceLock<TypeRegistry> = OnceLock::new();
  REGISTRY.get_or_init(TypeRegistry::default)
}
