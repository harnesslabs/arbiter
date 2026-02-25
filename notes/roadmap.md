---
date: 2026-02-25
owner: codex
status: active
tags:
  - roadmap
  - arbiter-core
  - distributed
---

# Arbiter Roadmap (Core-First, Crates-Only-When-Needed)

## Planning Inputs (Preflight)

- Read local directives: `/Users/autoparallel/Code/harnesslabs/arbiter/.codex/AGENTS.md`
- Notes state: `notes/` did not exist at roadmap seed time
- Archive state: no archived milestones yet
- Recent history reviewed via `git log --oneline` (latest includes `feat(\`arbiter-core\`): \`Network\` and \`Connection\`s + async`)

## Principles

- Keep new functionality in `arbiter-core` until there is clear pressure to split crates.
- Use feature flags to isolate optional capabilities.
- Prioritize correctness and test coverage over premature topology/protocol expansion.
- `arbiter` remains the façade re-export crate.

## Roadmap Phases

## Phase 0: Core Contract Cleanup (Complete)

Goal: make core delivery and dispatch semantics distributed-capable without adding crate sprawl.

Milestones:

- [x] Seed protocol metadata primitives in `arbiter-core` (`MessageKind`, IDs, recipients, envelope metadata)
- [x] Add envelope routing metadata and addressed/broadcast semantics for in-memory delivery
- [x] Replace panic-prone handler decode path with structured errors
- [x] Add remote-capable message registration keyed by stable `MessageKind` + `SchemaVersion`
- [x] Refactor runtime logic out of `Agent::<_, InMemory>::process` specialization into shared runtime module(s)
- [x] Align docs/examples with current public API (remove stale paths like `runtime::Runtime`)

Acceptance targets:

- [x] Existing in-memory tests pass
- [x] In-memory addressed delivery works
- [x] Decode mismatch returns structured error (no panic)
- [x] Stable identifier dispatch path exists at wire boundary (not just metadata on envelope)
- [x] Docs/examples compile against current APIs

## Phase 1: LAN MVP (Brokered Mesh, Library-First) (Complete)

Goal: run agents across multiple processes on a LAN through a brokered topology.

Milestones:

- [x] Functional TCP transport in `arbiter-core`
- [x] Framing (length-delimited)
- [x] Versioned handshake (protocol, node id, capabilities, codec)
- [x] Broker runtime + node runtime modules
- [x] Addressed + broadcast routing across processes
- [x] Heartbeats/liveness and disconnect handling
- [x] Bounded queue/backpressure semantics

## Phase 2: Observability + Replay (Complete)

Goal: inspect and replay distributed runs before expanding coordination primitives.

Milestones:

- [x] `observe` module with structured runtime events
- [x] Correlation metadata end-to-end
- [x] Tracing/file sink integration
- [x] Feature-gated `replay` module (recorded-order replay)

## Phase 3: Coordination Primitives (Complete)

Goal: add higher-level coordination APIs on top of networked core.

Milestones:

- [x] Service/agent registry (`AgentId` registry lookup helpers on broker/node runtime)
- [x] Group/topic routing (join/leave + broker fanout for `Recipient::Group`)
- [x] Request/reply helpers with timeout (correlation-based node helper)
- [x] Supervision policies (restart/retry policy evaluator)
- [x] Timers/scheduling hooks (one-shot + interval timer helpers)
- [x] Coordination capability negotiation hooks (handshake capability constants + broker/node helpers)

## Phase 4: Ecosystem Polish + Crate Extraction Review (Complete)

Goal: improve adoption and decide if any modules should split into crates.

Milestones:

- [x] Refresh `arbiter` façade exports (feature passthroughs + curated module re-exports)
- [x] End-to-end docs for local, LAN, replay
- [x] CI validation for examples and integration scenarios (`just check-examples`, `just ci-pr`)
- [x] Crate extraction review using explicit trigger criteria (documented no-split decision)

## Phase 5: Next-Hardening Wave (Seeded)

Goal: harden distributed behavior and operator workflows without breaking the core-first packaging strategy.

Milestones:

- [ ] Broker transport cancel-safety regression tests + fault-injection coverage (partial fix landed in Phase 4, expand coverage)
- [ ] Node runtime request/reply router abstraction (multiple concurrent in-flight requests without manual frame polling)
- [ ] Service-name aliases over `AgentId` registry (named service registry separate from concrete agent IDs)
- [ ] Broker/node metrics summary APIs (counters + snapshots for operations dashboards)
- [ ] Optional auth/TLS design doc and feature-flag scaffold (no implementation yet)

## Crate Extraction Triggers (Do Not Split Before These)

- Protocol needs independent semver for external/non-Rust consumers
- Optional subsystem adds heavy dependencies and hurts default compile times
- Subsystem requires independent release cadence
- Public API usability materially improves with separation
- Build/test time becomes a sustained productivity bottleneck
