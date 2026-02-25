---
date: 2026-02-25
commit: 6ae6046a
status: completed
tags:
  - milestone
  - phase4
  - docs
  - ci
  - facade
  - transport
components:
  - arbiter/Cargo.toml
  - arbiter/src/lib.rs
  - arbiter-core/Cargo.toml
  - arbiter-core/src/network/tcp.rs
  - arbiter-core/src/runtime/broker.rs
  - justfile
  - docs/src
---

# Phase 4 Ecosystem Polish + Crate Extraction Review

## Why

- The roadmap reached the point where infrastructure existed but adoption/documentation/CI ergonomics were lagging.
- The façade crate (`arbiter`) needed explicit feature passthroughs and a more intentional re-export surface so users can depend on `arbiter` instead of chasing internals.
- CI parity needed a concrete, repeatable command (`just ci-pr`) and explicit example validation to reduce drift.

## What Shipped

- `arbiter` façade crate refresh:
  - feature passthroughs for `in-memory`, `tcp`, `replay`, `fixtures`
  - curated module re-exports (`agent`, `handler`, `network`, `observe`, `protocol`, `runtime`, `replay`)
  - compatibility-preserving glob re-export retained
- Docs refresh:
  - `Local Simulation Guide`
  - `LAN Runtime Guide`
  - `Replay Guide`
  - `Compatibility & Features` reference (protocol version, feature flags, crate split triggers/current decision)
  - `SUMMARY.md` navigation cleanup and `contributing.md` command updates
- CI parity / validation improvements:
  - `just check-examples`
  - `just ci-pr`
  - Taplo checks scoped to tracked manifests (avoid `target/semver-checks` noise)
- Core stability fixes discovered during Phase 4 validation:
  - `arbiter-core` Tokio `net`/`io-util` features are now gated behind the `tcp` feature (restores wasm example compatibility for non-TCP builds)
  - broker connection I/O now uses split framed read/write halves with dedicated tasks to avoid cancel-safety frame desync when using `tokio::select!`

## Blockers & Solutions

### Cargo manifest inheritance blocked façade feature passthrough

- **Failure:** `arbiter/Cargo.toml` could not override `default-features` for `arbiter-core` when inheriting from `[workspace.dependencies]`.
- **Solution:** Switched `arbiter` to a direct path dependency on `../arbiter-core` with `default-features = false`, then added explicit façade feature passthroughs.

### `just check-examples` wasm build regressed due Tokio net features

- **Failure:** `examples/leader` wasm build pulled in `mio` and failed because `arbiter-core` enabled Tokio `net` unconditionally.
- **Solution:** Moved Tokio `net` and `io-util` activation behind `arbiter-core`'s `tcp` feature (`tcp = ["tokio/io-util", "tokio/net"]`).

### Flaky broker tests with `frame is too large: 0x7b224164 ("{\"Ad")`

- **Failure:** Broker tests intermittently failed with framing corruption (`{\"Ad`, `{\"Gr}` interpreted as frame lengths).
- **Cause:** `run_connection` used `tokio::select!` directly over `transport.recv_frame()`. Canceling that future mid-read dropped partially consumed bytes, desynchronizing the stream.
- **Solution:** Added framed TCP split halves and refactored broker connection I/O into dedicated reader/writer tasks. The main broker loop now selects on channel events (cancel-safe), not raw socket reads.

## Fallback Plan

- If façade re-export curation causes churn, fall back to `pub use arbiter_core::*` only and keep passthrough features while postponing curated submodules.
- If `just ci-pr` proves too slow for daily use, split it into `ci-pr-fast` and `ci-pr-full` without removing explicit example/TOML checks.
- If split framed TCP halves need redesign, keep the API internal and preserve the broker fix by retaining the dedicated reader/writer-task pattern.

## Validation

- `just lint`
- `just test`
- `just check-examples`
- `just ci-pr`
