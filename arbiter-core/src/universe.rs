use super::*;
use crate::{environment::Database, world::World};

#[derive(Default)]
pub struct Universe<DB: Database> {
  worlds:      Option<HashMap<String, World<DB>>>,
  world_tasks: Option<Vec<Result<(), task::JoinError>>>,
}

impl<DB: Database> Universe<DB>
where
  DB: Database + Send + 'static,
  DB::Location: Send + Sync + 'static,
  DB::State: Send + Sync + 'static,
{
  pub fn new() -> Self { Self { worlds: Some(HashMap::new()), world_tasks: None } }

  pub fn add_world(&mut self, world: World<DB>) {
    if let Some(worlds) = self.worlds.as_mut() {
      worlds.insert(world.id.clone(), world);
    }
  }

  pub async fn run_worlds(&mut self) -> Result<(), ArbiterCoreError> {
    if self.is_online() {
      return Err(ArbiterCoreError::UniverseError("Universe is already running.".to_owned()));
    }
    let mut tasks = Vec::new();
    // NOTE: Unwrap is safe because we checked if the universe is online.
    for (_, world) in self.worlds.take().unwrap().drain() {
      tasks.push(task::spawn(async move {
        world.run().await.unwrap();
      }));
    }
    self.world_tasks = Some(join_all(tasks.into_iter()).await);
    Ok(())
  }

  /// Returns `true` if the [`Universe`] is running.
  pub fn is_online(&self) -> bool { self.world_tasks.is_some() }
}

#[cfg(test)]
mod tests {

  use super::*;

  #[tokio::test]
  async fn run_universe() {
    let mut universe = Universe::<HashMap<String, String>>::new();
    let world = World::new("test");
    universe.add_world(world);
    universe.run_worlds().await.unwrap();
    universe.world_tasks.unwrap().remove(0).unwrap();
  }

  #[tokio::test]
  #[should_panic(expected = "Universe is already running.")]
  async fn cant_run_twice() {
    let mut universe = Universe::<HashMap<String, String>>::new();
    let world1 = World::new("test");
    universe.add_world(world1);
    universe.run_worlds().await.unwrap();
    universe.run_worlds().await.unwrap();
  }
}
