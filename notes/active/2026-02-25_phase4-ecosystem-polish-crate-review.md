---
date: 2026-02-25
commit: pending
status: in_progress
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
