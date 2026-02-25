---
date: 2026-02-25
commit: pending
status: in_progress
tags:
  - active
  - phase5
  - hardening
  - roadmap-extension
components:
  - arbiter-core/src/runtime/broker.rs
  - arbiter-core/src/runtime/node.rs
  - arbiter-core/src/protocol.rs
  - notes/roadmap.md
---

# Phase 5: Next-Hardening Wave

## Scope for Next Cycle

- Add fault-injection and regression coverage around broker transport/read-loop robustness
- Improve request/reply ergonomics for concurrent in-flight requests
- Add explicit service-name aliases on top of `AgentId`-based registry
- Add metrics/snapshot helpers for operator visibility
- Seed auth/TLS design and feature-flag scaffolding (design only)

## Starting Context

- Phases 0-4 completed
- Broker I/O cancel-safety bug fixed in Phase 4 via split reader/writer tasks
- CI parity and docs are now in better shape; next value is runtime hardening and operator ergonomics

## Risks / Design Notes

- Preserve the core-first packaging strategy; only split crates if the documented triggers are met.
- Keep hardening work incremental with tests first (fault injection/regression coverage before wider API changes).
- Avoid mixing auth/TLS implementation into the same milestone as runtime ergonomics; design and feature gates first.
