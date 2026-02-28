# Getting Started

To get started with Arbiter, you will need to add it to your `Cargo.toml`.

## Installation

Add the following to your `Cargo.toml` dependencies:

```toml
[dependencies]
arbiter = "0.5.0"
```

## First Steps

Once you have Arbiter installed, the typical flow for setting up a simulation is:

1. **Initialize the Runtime**: The runtime is the execution environment for your actors.
2. **Define an Actor**: Create a struct for your agent and implement the `LifeCycle` and `Handler` traits.
3. **Spawn the Actor**: Use the runtime to spawn your actor into the environment.
4. **Send Messages**: Interact with your actors by sending them messages through their addresses.

Let's explore these concepts in more detail in the next chapter.
