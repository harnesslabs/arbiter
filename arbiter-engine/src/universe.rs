//! The [`universe`] module contains the [`Universe`] struct which is the
//! primary interface for creating and running many `World`s in parallel.

use super::*;
use crate::world::World;

/// The [`Universe`] struct is the primary interface for creating and running
/// many `World`s in parallel. At the moment, is a wrapper around a
/// [`HashMap`] of [`World`]s, but can be extended to handle generics inside of
/// [`World`]s and crosstalk between [`World`]s.
#[derive(Debug, Default)]
pub struct Universe {
    worlds: Option<HashMap<String, World>>,
    world_tasks: Option<Vec<Result<World, JoinError>>>,
}

impl Universe {
    /// Creates a new [`Universe`].
    pub fn new() -> Self {
        Self {
            worlds: Some(HashMap::new()),
            world_tasks: None,
        }
    }

    /// Adds a [`World`] to the [`Universe`].
    pub fn add_world(&mut self, world: World) {
        if let Some(worlds) = self.worlds.as_mut() {
            worlds.insert(world.id.clone(), world);
        }
    }

    /// Runs all of the [`World`]s in the [`Universe`] in parallel.
    pub async fn run_worlds(&mut self) -> Result<(), ArbiterEngineError> {
        if self.is_online() {
            return Err(ArbiterEngineError::UniverseError(
                "Universe is already running.".to_owned(),
            ));
        }
        let mut tasks = Vec::new();
        // NOTE: Unwrap is safe because we checked if the universe is online.
        for (_, mut world) in self.worlds.take().unwrap().drain() {
            tasks.push(spawn(async move {
                world.run().await.unwrap();
                world
            }));
        }
        self.world_tasks = Some(join_all(tasks.into_iter()).await);
        Ok(())
    }

    /// Returns `true` if the [`Universe`] is running.
    pub fn is_online(&self) -> bool {
        self.world_tasks.is_some()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn run_universe() {
        let mut universe = Universe::new();
        let world = World::new("test");
        universe.add_world(world);
        universe.run_worlds().await.unwrap();
        universe.world_tasks.unwrap().remove(0).unwrap();
    }

    #[tokio::test]
    #[should_panic(expected = "Universe is already running.")]
    async fn cant_run_twice() {
        let mut universe = Universe::new();
        let world1 = World::new("test");
        universe.add_world(world1);
        universe.run_worlds().await.unwrap();
        universe.run_worlds().await.unwrap();
    }
}
