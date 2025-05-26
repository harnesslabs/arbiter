use std::{
  collections::HashMap,
  convert::TryFrom,
  sync::Arc,
  time::{Duration, Instant},
};

use arbiter_bindings::bindings::{
  arbiter_math::ArbiterMath,
  arbiter_token::{self, ArbiterToken},
};
use arbiter_core::{environment::Environment, middleware::ArbiterMiddleware};
use ethers::{
  core::{k256::ecdsa::SigningKey, utils::Anvil},
  middleware::SignerMiddleware,
  providers::{Http, Middleware, Provider},
  signers::{LocalWallet, Signer, Wallet},
  types::{Address, I256, U256},
  utils::AnvilInstance,
};
use polars::{
  prelude::{DataFrame, NamedFrom},
  series::Series,
};
use tracing::info;

const NUM_BENCH_ITERATIONS: usize = 100;
const NUM_LOOP_STEPS: usize = 10;

#[derive(Debug)]
struct BenchDurations {
  deploy:         Duration,
  lookup:         Duration,
  stateless_call: Duration,
  stateful_call:  Duration,
}

#[tokio::main]
async fn main() {
  // Choose the benchmark group items by label.
  let group = ["anvil", "arbiter"];
  let mut results: HashMap<&str, HashMap<&str, Duration>> = HashMap::new();

  // Set up for showing percentage done.
  let ten_percent = NUM_BENCH_ITERATIONS / 10;

  for item in group {
    let mut item_results = HashMap::new();
    // Count up total durations for each part of the benchmark.
    let mut durations = Vec::with_capacity(NUM_BENCH_ITERATIONS);
    println!("Running {item} benchmark");

    for index in 0..NUM_BENCH_ITERATIONS {
      durations.push(match item {
        label @ "anvil" => {
          let (client, _anvil_instance) = anvil_startup().await;
          let duration = bencher(client, label).await;
          drop(_anvil_instance);
          duration
        },
        label @ "arbiter" => {
          let (_environment, client) = arbiter_startup();
          bencher(client, label).await
        },
        _ => panic!("Invalid argument"),
      });
      if index % ten_percent == 0 {
        println!("{index} out of {NUM_BENCH_ITERATIONS} complete");
      }
    }
    let sum_durations = durations.iter().fold(
      BenchDurations {
        deploy:         Duration::default(),
        lookup:         Duration::default(),
        stateless_call: Duration::default(),
        stateful_call:  Duration::default(),
      },
      |acc, duration| BenchDurations {
        deploy:         acc.deploy + duration.deploy,
        lookup:         acc.lookup + duration.lookup,
        stateless_call: acc.stateless_call + duration.stateless_call,
        stateful_call:  acc.stateful_call + duration.stateful_call,
      },
    );

    let average_durations = BenchDurations {
      deploy:         sum_durations.deploy / NUM_BENCH_ITERATIONS as u32,
      lookup:         sum_durations.lookup / NUM_BENCH_ITERATIONS as u32,
      stateless_call: sum_durations.stateless_call / NUM_BENCH_ITERATIONS as u32,
      stateful_call:  sum_durations.stateful_call / NUM_BENCH_ITERATIONS as u32,
    };

    item_results.insert("Deploy", average_durations.deploy);
    item_results.insert("Lookup", average_durations.lookup);
    item_results.insert("Stateless Call", average_durations.stateless_call);
    item_results.insert("Stateful Call", average_durations.stateful_call);

    results.insert(item, item_results);
  }

  let df = create_dataframe(&results, &group);

  match get_version_of("arbiter-core") {
    Some(version) => println!("arbiter-core version: {}", version),
    None => println!("Could not find version for arbiter-core"),
  }

  match get_version_of("ethers") {
    Some(version) => println!("ethers-core anvil version: {}", version),
    None => println!("Could not find version for anvil"),
  }
  println!("Date: {}", chrono::Local::now().format("%Y-%m-%d"));
  println!("{}", df);
}

async fn bencher<M: Middleware + 'static>(client: Arc<M>, label: &str) -> BenchDurations {
  // Track the duration for each part of the benchmark.
  let mut total_deploy_duration = 0;
  let mut total_lookup_duration = 0;
  let mut total_stateless_call_duration = 0;
  let mut total_stateful_call_duration = 0;

  // Deploy `ArbiterMath` and `ArbiterToken` contracts and tally up how long this
  // takes.
  let (arbiter_math, arbiter_token, deploy_duration) = deployments(client.clone(), label).await;
  total_deploy_duration += deploy_duration.as_micros();

  // Call `balance_of` `NUM_LOOP_STEPS` times on `ArbiterToken` and tally up how
  // long basic lookups take.
  let lookup_duration = lookup(arbiter_token.clone(), label).await;
  total_lookup_duration += lookup_duration.as_micros();

  // Call `cdf` `NUM_LOOP_STEPS` times on `ArbiterMath` and tally up how long this
  // takes.
  let stateless_call_duration = stateless_call_loop(arbiter_math, label).await;
  total_stateless_call_duration += stateless_call_duration.as_micros();

  // Call `mint` `NUM_LOOP_STEPS` times on `ArbiterToken` and tally up how long
  // this takes.
  let statefull_call_duration =
    stateful_call_loop(arbiter_token, client.default_sender().unwrap(), label).await;
  total_stateful_call_duration += statefull_call_duration.as_micros();

  BenchDurations {
    deploy:         Duration::from_micros(total_deploy_duration as u64),
    lookup:         Duration::from_micros(total_lookup_duration as u64),
    stateless_call: Duration::from_micros(total_stateless_call_duration as u64),
    stateful_call:  Duration::from_micros(total_stateful_call_duration as u64),
  }
}

async fn anvil_startup(
) -> (Arc<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>, AnvilInstance) {
  // Create an Anvil instance
  // No blocktime mines a new block for each tx, which is fastest.
  let anvil = Anvil::new().spawn();

  // Create a client
  let provider = Provider::<Http>::try_from(anvil.endpoint()).unwrap().interval(Duration::ZERO);

  let wallet: LocalWallet = anvil.keys()[0].clone().into();
  let client = Arc::new(SignerMiddleware::new(provider, wallet.with_chain_id(anvil.chain_id())));

  (client, anvil)
}

fn arbiter_startup() -> (Environment, Arc<ArbiterMiddleware>) {
  let environment = Environment::builder().build();

  let client = ArbiterMiddleware::new(&environment, Some("name")).unwrap();
  (environment, client)
}

async fn deployments<M: Middleware + 'static>(
  client: Arc<M>,
  label: &str,
) -> (ArbiterMath<M>, ArbiterToken<M>, Duration) {
  let start = Instant::now();
  let arbiter_math = ArbiterMath::deploy(client.clone(), ()).unwrap().send().await.unwrap();
  let arbiter_token = arbiter_token::ArbiterToken::deploy(
    client.clone(),
    ("Bench Token".to_string(), "BNCH".to_string(), 18_u8),
  )
  .unwrap()
  .send()
  .await
  .unwrap();
  let duration = start.elapsed();
  info!("Time elapsed in {} deployment is: {:?}", label, duration);

  (arbiter_math, arbiter_token, duration)
}

async fn lookup<M: Middleware + 'static>(arbiter_token: ArbiterToken<M>, label: &str) -> Duration {
  let address = arbiter_token.client().default_sender().unwrap();
  let start = Instant::now();
  for _ in 0..NUM_LOOP_STEPS {
    arbiter_token.balance_of(address).call().await.unwrap();
  }
  let duration = start.elapsed();
  info!("Time elapsed in {} cdf loop is: {:?}", label, duration);

  duration
}

async fn stateless_call_loop<M: Middleware + 'static>(
  arbiter_math: ArbiterMath<M>,
  label: &str,
) -> Duration {
  let iwad = I256::from(10_u128.pow(18));
  let start = Instant::now();
  for _ in 0..NUM_LOOP_STEPS {
    arbiter_math.cdf(iwad).call().await.unwrap();
  }
  let duration = start.elapsed();
  info!("Time elapsed in {} cdf loop is: {:?}", label, duration);

  duration
}

async fn stateful_call_loop<M: Middleware + 'static>(
  arbiter_token: arbiter_token::ArbiterToken<M>,
  mint_address: Address,
  label: &str,
) -> Duration {
  let wad = U256::from(10_u128.pow(18));
  let start = Instant::now();
  for _ in 0..NUM_LOOP_STEPS {
    arbiter_token.mint(mint_address, wad).send().await.unwrap().await.unwrap();
  }
  let duration = start.elapsed();
  info!("Time elapsed in {} mint loop is: {:?}", label, duration);

  duration
}

fn create_dataframe(results: &HashMap<&str, HashMap<&str, Duration>>, group: &[&str]) -> DataFrame {
  let operations = ["Deploy", "Lookup", "Stateless Call", "Stateful Call"];
  let mut df = DataFrame::new(vec![
    Series::new("Operation", operations.to_vec()),
    Series::new(
      &format!("{} (μs)", group[0]),
      operations
        .iter()
        .map(|&op| results.get(group[0]).unwrap().get(op).unwrap().as_micros() as f64)
        .collect::<Vec<_>>(),
    ),
    Series::new(
      &format!("{} (μs)", group[1]),
      operations
        .iter()
        .map(|&op| results.get(group[1]).unwrap().get(op).unwrap().as_micros() as f64)
        .collect::<Vec<_>>(),
    ),
  ])
  .unwrap();

  let s0 = df.column(&format!("{} (μs)", group[0])).unwrap().to_owned();
  let s1 = df.column(&format!("{} (μs)", group[1])).unwrap().to_owned();
  let mut relative_difference = s0.divide(&s1).unwrap();

  df.with_column::<Series>(relative_difference.rename("Relative Speedup").clone()).unwrap().clone()
}

fn get_version_of(crate_name: &str) -> Option<String> {
  let metadata = cargo_metadata::MetadataCommand::new().exec().unwrap();

  for package in metadata.packages {
    if package.name == crate_name {
      return Some(package.version.to_string());
    }
  }

  None
}
