---
date: 2026-02-25
commit: 6ae6046a
status: completed
tags:
  - active
  - phase4
  - docs
  - ci
  - façade
components:
  - arbiter/src/lib.rs
  - docs/src
  - justfile
  - notes/roadmap.md
---

# Phase 4: Ecosystem Polish + Crate Extraction Review

## Scope for Current Cycle

- Refresh `arbiter` façade exports for a more intentional stable surface
- Add end-to-end docs covering local, LAN, and replay workflows
- Improve CI-parity `just` targets for example/integration validation
- Document crate extraction review triggers and current decision

## Starting Context

- Phase 0-3 are complete in `arbiter-core`
- `arbiter` currently re-exports everything from `arbiter_core`
- LAN runtime, observability/replay, and coordination primitives exist but docs/tutorials lag behind features

## Risks / Design Notes

- Avoid breaking users unnecessarily while refining the façade crate; additive exports and modules first.
- Prefer documenting explicit non-goals and extraction triggers over speculative crate splits.
- CI parity additions should reflect real checks we can run locally with `just`, not aspirational jobs.

## Completed Work

- Added `arbiter` façade feature passthroughs (`in-memory`, `tcp`, `replay`, `fixtures`) and curated module re-exports while preserving compatibility glob exports.
- Added docs guides for local simulation, LAN runtime, and replay plus a compatibility/feature-flag/crate-review reference page.
- Added `just check-examples` and `just ci-pr`, including explicit Taplo manifest targets and example validation commands.
- Fixed a wasm compatibility regression by gating `tokio` `net`/`io-util` dependency features behind `arbiter-core`'s `tcp` feature.
- Fixed flaky broker tests caused by canceling `recv_frame()` inside `tokio::select!` by splitting framed TCP transport into read/write halves and moving broker connection I/O to dedicated reader/writer tasks.
