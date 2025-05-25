use std::path::Path;

use arbiter_core::{
  errors::ArbiterCoreError,
  events::{Logger, OutputFileType},
};
use ethers::types::U256 as eU256;
use serde::Serialize;
include!("common.rs");

#[derive(Serialize, Clone)]
struct MockMetadata {
  pub name: String,
}

async fn generate_events(
  arbx: ArbiterToken<ArbiterMiddleware>,
  arby: ArbiterToken<ArbiterMiddleware>,
  lex: LiquidExchange<ArbiterMiddleware>,
  client: Arc<ArbiterMiddleware>,
) -> Result<(), ArbiterCoreError> {
  for _ in 0..2 {
    arbx.approve(client.address(), eU256::from(1)).send().await.unwrap().await?;
    arby.approve(client.address(), eU256::from(1)).send().await.unwrap().await?;
    lex.set_price(eU256::from(10u128.pow(18))).send().await.unwrap().await?;
  }
  Ok(())
}

#[tokio::test]
async fn data_capture() {
  //
  let (env, client) = startup();
  let (arbx, arby, lex) = deploy_liquid_exchange(client.clone()).await;
  println!("Deployed contracts");

  // default_listener
  let logger_task = Logger::builder()
    .with_event(arbx.events(), "arbx")
    .with_event(arby.events(), "arby")
    .with_event(lex.events(), "lex")
    .run()
    .unwrap();

  let metadata = MockMetadata { name: "test".to_string() };

  Logger::builder()
    .with_event(arbx.events(), "arbx")
    .with_event(arby.events(), "arby")
    .with_event(lex.events(), "lex")
    .metadata(metadata)
    .unwrap()
    .run()
    .unwrap();

  Logger::builder()
    .with_event(arbx.events(), "arbx")
    .with_event(arby.events(), "arby")
    .with_event(lex.events(), "lex")
    .file_type(OutputFileType::CSV)
    .run()
    .unwrap();

  Logger::builder()
    .with_event(arbx.events(), "arbx")
    .with_event(arby.events(), "arby")
    .with_event(lex.events(), "lex")
    .file_type(OutputFileType::Parquet)
    .run()
    .unwrap();

  generate_events(arbx, arby, lex, client.clone()).await.unwrap_or_else(|e| {
    panic!("Error generating events: {}", e);
  });

  let _ = env.stop();

  logger_task.await.unwrap();
  std::thread::sleep(std::time::Duration::from_secs(1));
  assert!(Path::new("./data/output.csv").exists());
  assert!(Path::new("./data/output.parquet").exists());
  assert!(Path::new("./data/output.json").exists());
  std::fs::remove_dir_all("./data").unwrap();
}
