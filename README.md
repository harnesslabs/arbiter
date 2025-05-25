<img width="529" alt="arbiter" src="https://user-images.githubusercontent.com/20118821/236929861-2a1fe071-0053-453c-ac86-224b32febcd6.png">

> A Rust-based multi-agent framework.

![Visitors badge](https://visitor-badge.laobi.icu/badge?page_id=arbiter)
[![Twitter Badge](https://badgen.net/badge/icon/twitter?icon=twitter&label)](https://x.com/harnesslabs)

## Overview
> **Arbiter** is a multi-agent framework that lets developers orchestrate event-driven simulations.


---

## NOTE
Arbiter is under active development to enter into a new phase.
We are currently working on the following:
- [ ] `arbiter`: crate should be a lib to re-export our sub crates. The bin never caught on anyway.
- [ ] `arbiter-core` becomes `arbiter-ethereum` or `arbiter-evm`.
- [ ] `arbiter-engine` overhauled to handle more generic agent setups. Maintains the structure of being the actual underlying move forward / event handling mechanism.
- [ ] `arbiter-core` should then house abstractions for different types of environments and other DES type items to be used with the engine.

---
## Below is the original README.md for Arbiter

Arbiter allows for fine-grained control over a (Rust) Ethereum Virtual Machine (EVM) to provide stateful Ethereum smart-contract interactions and the creation of behaviors that can be coalesced into complex scenarios or automation. As such, Arbiter enables robust and granular simulation to be conducted.
We use an [`ethers-rs`](https://github.com/gakonst/ethers-rs) middleware on top of [revm](https://github.com/bluealloy/revm), which is used in ETH clients such as [`reth`](https://github.com/paradigmxyz/reth) as well as [`foundry`](https://github.com/foundry-rs/foundry).
This enables speed, configurability, and modularity that feels like a lightweight custom Ethereum node. 

The primary use of Arbiter is to probe the mechanism security of smart contracts, and perform advanced economic modelling.
If mechanism security interests you, please see the [Vulnerability Corpus](https://anthias-labs.github.io/arbiter/vulnerability_corpus.html).

---

The Arbiter workspace has five crates:
- `arbiter`: The bin that exposes a command line interface for forking and binding contracts.
- `arbiter-core`: A lib containing the core logic for the Arbiter framework, including the `ArbiterMiddleware` discussed before, and the `Environment`, our sandbox.
- `arbiter-engine`: A lib that provides abstractions for building simulations, agents, and behaviors.
- `arbiter-macros`: A lib crate that contains the macros used to simplify development with Arbiter.
- `arbiter-bindings`: A lib crate containing bindings for utility smart contracts used for testing and development.


## Book
Here you can find the [Arbiter Documentation](https://anthias-labs.github.io/arbiter/).
This book highlights some key Arbiter usage patterns and explains all key functionality.

## Motivation 
Arbiter was built to allow you to work with smart contracts in a stateful sandbox and thus design agents that can be used alongside the contracts.
This gives you many capabilities.
For instance, smart contract engineers must test their contracts against various potentially adversarial environments and parameters instead of relying on static and stateless testing.

In Decentralized Finance (DeFi), a wide array of complex decentralized applications can use the testing described above. Still, implicit financial strategies also encompass many agents and parameterizations. 
A financial engineer may want to test their strategies against thousands of market conditions, contract settings, shocks, and autonomous or random AI agents while ensuring their approach isn't vulnerable to bytecode-level exploits.
Likewise, the same engineer may want to develop searcher agents, solver agents, or other autonomous agents that can be run on the blockchain.

## Working with the Arbiter Framework
To work with Arbiter, you must have Rust installed on your machine. 
You can install Rust by following the instructions [here](https://www.rust-lang.org/tools/install). 
It will also be helpful to get the `cargo-generate` package, which you can install by doing:
```bash
cargo install cargo-generate
```

### Examples
We have an example that will run what we have set up in a template.
To run this, you can clone the repository and update the submodules:
```bash
git clone https://github.com/anthias-labs/arbiter.git
cd arbiter
git submodule update --init --recursive
```
From here, you can now try running the following from the clone's root directory:
```bash
cargo run --example template 
```
This command will enter the template CLI and show you the commands and flags.

To run the `ModifiedCounter.sol` example and see some logging, try:
```bash
cargo run --example template simulate examples/template/configs/example.toml -vvv
```
This sets the log level to `debug` so you can see what's happening internally.

### Initialization
To create your own Arbiter project off of our template [arbiter-template](https://github.com/anthias-labs/arbiter-template), you can run the following:
```bash
cd <your/chosen/directory>
cargo generate https://github.com/anthias-labs/arbiter-template.git
```
You'll be prompted to provide a project name, and the rest will be set up for you!

### Binary
To install the Arbiter binary, run:
```bash
cargo install arbiter
```
This will install the Arbiter binary on your machine. You can then run `arbiter --help` to see that Arbiter was correctly installed and see the help menu.

### Bindings
You can load or write your own smart contracts in the `contracts/` directory of your templated project and begin writing your own simulations. 
Arbiter treats Rust smart-contract bindings as first-class citizens. 
The contract bindings are generated via Foundry's `forge` command. 
`arbiter bind` wraps `forge` with convenience features that generate all your bindings to `src/bindings` as a Rust module. 
[Foundry](https://github.com/foundry-rs/foundry) power-users can use `forge` directly.

### Forking
To fork a state of an EVM network, you must first create a fork config file.
An example is provided in the `examples/fork` directory.
Essentially, you provide your storage location for the data, the network you want, the block number you want, and metadata about the contracts you want to fork.

```bash
arbiter fork <fork_config.toml>
```
This will create a fork of the network you specified in the config file and store it in your specified location.
It can then be loaded into an `arbiter-core` `Environment` using the `Fork::from_disk()` method.

Forking is done this way to ensure that all emulation does not require a constant connection to an RPC endpoint.
You may find that [Anvil](https://book.getfoundry.sh/anvil/) has a more accessible forking interface. However, an online forking mechanism makes RPC calls to update the state as necessary.
Arbiter `Environment` forking is for creating a state, storing it locally, and being able to initialize a simulation from that state when desired.
We plan to allow `arbiter-engine` to integrate with other network types, such as Anvil, in the future!

**Optional Arguments** 
You can run `arbiter fork <fork_config.toml> --overwrite` to overwrite the fork if it already exists.

## Cargo Docs

To see the Cargo docs for the Arbiter crates, please visit the following:
- [`arbiter`](https://docs.rs/crate/arbiter/)
- [`arbiter-bindings`](https://docs.rs/crate/arbiter-bindings/)
- [`arbiter-core`](https://docs.rs/arbiter-core/)
- [`arbiter-macros`](https://docs.rs/arbiter-macros/)
- [`arbiter-engine`](https://docs.rs/arbiter-engine/)

You will find each of these on crates.io.

## Benchmarks
In `arbiter-core`, we have a a small benchmarking suite that compares the `ArbiterMiddleware` implementation over the `Environment` to the [Anvil](https://github.com/foundry-rs/foundry/tree/master/crates/anvil) local testnet chain implementation.
The biggest reasons we chose to build Arbiter was to gain more control over the EVM environment and to have a more robust simulation framework. Still, we also wanted to gain speed, so we chose to build our own interface over `revm` instead of using Anvil (which uses `revm` under the hood). 
For the following, Anvil was set to mine blocks for each transaction instead of setting an enforced block time. The `Environment` was configured with a block rate of 10.0.
Preliminary benchmarks of the `ArbiterMiddleware` interface over `revm` against Anvil are given in the following table.

To run the benchmarking code yourself, you can run:
```bash
cargo bench --package arbiter-core
```

| Operation       |  ArbiterMiddleware |    Anvil     | Relative Difference |
|-----------------|-----------------|--------------|---------------------|
| Deploy          | 238.975µs       | 7712.436µs   | ~32.2729x           |
| Lookup          | 565.617µs       | 17880.124µs  | ~31.6117x           |
| Stateless Call  | 1402.524µs      | 10397.55µs   | ~7.413456x          |
| Stateful Call   | 2043.88µs       | 154553.225µs | ~75.61756x          |


The above can be described by:
- Deploy: Deploying a contract to the EVM. 
In this method, we deployed both `ArbiterToken` and `ArbiterMath`, so you can divide the time by two to estimate the time it takes to deploy a single contract.

- Lookup: Look up the `balanceOf` for a client's address for `ArbiterToken`.
In this method, we called `ArbiterToken`'s `balanceOf` function 100 times.
Divide by 100 to get the time to look up a single balance.

- Stateless Call: Calling a contract that does not mutate state. 
In this method, we called `ArbiterMath`'s `cdf` function 100 times.
Divide by 100 to get the time to call a single stateless function.

- Stateful Call: Calling a contract that mutates state. 
In this call, we called `ArbiterToken`'s `mint` function 100 times.
Divide by 100 to get the time to call a single stateful function.

The benchmarking code can be found in the `arbiter-core/benches/` directory, and these specific times were achieved over a 1000 run average. 
The above was achieved by running `cargo bench --package arbiter-core`, which will automatically run with the release profile.
Times were achieved on an Apple Macbook Pro M1 Max with 8 performance and 2 efficiency cores and 32GB of RAM.

Of course, the use cases of Anvil and the `ArbiterMiddleware` can be different. 
Anvil represents a more realistic environment with networking and mining. At the same time, the `ArbiterMiddleware` is a simpler environment with the bare essentials to running stateful simulations.
Anvil also mines blocks for each transaction, while the `ArbiterMiddleware` does not.

Please let us know if you need any help with these benchmarks or suggestions for improving them!

## Testing

If you contribute, please write tests for any new code you write. To run the tests, you can run the following:

```bash
cargo test --all --all-features
```

## Contributing

See our [Contributing Guidelines](https://github.com/anthias-labs/arbiter/blob/main/.github/CONTRIBUTING.md)
