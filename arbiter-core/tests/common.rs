use std::sync::Arc;

use arbiter_bindings::bindings::{
  arbiter_math::ArbiterMath, arbiter_token::ArbiterToken, liquid_exchange::LiquidExchange,
};
use arbiter_core::{environment::Environment, middleware::ArbiterMiddleware};
use ethers::utils::parse_ether;

pub const TEST_ARG_NAME: &str = "ArbiterToken";
pub const TEST_ARG_SYMBOL: &str = "ARBT";
pub const TEST_ARG_DECIMALS: u8 = 18;

pub const TEST_MINT_AMOUNT: u128 = 69;
pub const TEST_MINT_TO: &str = "0xf7e93cc543d97af6632c9b8864417379dba4bf15";

pub const TEST_APPROVAL_AMOUNT: u128 = 420;

pub const TEST_SIGNER_SEED_AND_LABEL: &str = "test_seed_and_label";

pub const ARBITER_TOKEN_X_NAME: &str = "Arbiter Token X";
pub const ARBITER_TOKEN_X_SYMBOL: &str = "ARBX";
pub const ARBITER_TOKEN_X_DECIMALS: u8 = 18;

pub const ARBITER_TOKEN_Y_NAME: &str = "Arbiter Token Y";
pub const ARBITER_TOKEN_Y_SYMBOL: &str = "ARBY";
pub const ARBITER_TOKEN_Y_DECIMALS: u8 = 18;

pub const LIQUID_EXCHANGE_PRICE: f64 = 420.69;

pub fn log() {
  std::env::set_var("RUST_LOG", "trace");
  tracing_subscriber::fmt::init();
}

pub fn startup() -> (Environment, Arc<ArbiterMiddleware>) {
  let env = Environment::builder().build();
  let client = ArbiterMiddleware::new(&env, Some(TEST_SIGNER_SEED_AND_LABEL)).unwrap();
  (env, client)
}

pub async fn deploy_arbx(client: Arc<ArbiterMiddleware>) -> ArbiterToken<ArbiterMiddleware> {
  ArbiterToken::deploy(
    client,
    (
      ARBITER_TOKEN_X_NAME.to_string(),
      ARBITER_TOKEN_X_SYMBOL.to_string(),
      ARBITER_TOKEN_X_DECIMALS,
    ),
  )
  .unwrap()
  .send()
  .await
  .unwrap()
}

pub async fn deploy_arby(client: Arc<ArbiterMiddleware>) -> ArbiterToken<ArbiterMiddleware> {
  ArbiterToken::deploy(
    client,
    (
      ARBITER_TOKEN_Y_NAME.to_string(),
      ARBITER_TOKEN_Y_SYMBOL.to_string(),
      ARBITER_TOKEN_Y_DECIMALS,
    ),
  )
  .unwrap()
  .send()
  .await
  .unwrap()
}

pub async fn deploy_liquid_exchange(
  client: Arc<ArbiterMiddleware>,
) -> (
  ArbiterToken<ArbiterMiddleware>,
  ArbiterToken<ArbiterMiddleware>,
  LiquidExchange<ArbiterMiddleware>,
) {
  let arbx = deploy_arbx(client.clone()).await;
  let arby = deploy_arby(client.clone()).await;
  let price = parse_ether(LIQUID_EXCHANGE_PRICE).unwrap();
  let liquid_exchange = LiquidExchange::deploy(client, (arbx.address(), arby.address(), price))
    .unwrap()
    .send()
    .await
    .unwrap();
  (arbx, arby, liquid_exchange)
}

pub async fn deploy_arbiter_math(client: Arc<ArbiterMiddleware>) -> ArbiterMath<ArbiterMiddleware> {
  ArbiterMath::deploy(client, ()).unwrap().send().await.unwrap()
}
