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

## Phase 0: Core Contract Cleanup (In Progress)

Goal: make core delivery and dispatch semantics distributed-capable without adding crate sprawl.

Milestones:

- [x] Seed protocol metadata primitives in `arbiter-core` (`MessageKind`, IDs, recipients, envelope metadata)
- [x] Add envelope routing metadata and addressed/broadcast semantics for in-memory delivery
- [x] Replace panic-prone handler decode path with structured errors
- [x] Add remote-capable message registration keyed by stable `MessageKind` + `SchemaVersion`
- [x] Refactor runtime logic out of `Agent::<_, InMemory>::process` specialization into shared runtime module(s)
- [ ] Align docs/examples with current public API (remove stale paths like `runtime::Runtime`)

Acceptance targets:

- [x] Existing in-memory tests pass
- [x] In-memory addressed delivery works
- [x] Decode mismatch returns structured error (no panic)
- [x] Stable identifier dispatch path exists at wire boundary (not just metadata on envelope)
- [ ] Docs/examples compile against current APIs

## Phase 1: LAN MVP (Brokered Mesh, Library-First)

Goal: run agents across multiple processes on a LAN through a brokered topology.

Milestones:

- [ ] Functional TCP transport in `arbiter-core`
- [ ] Framing (length-delimited)
- [ ] Versioned handshake (protocol, node id, capabilities, codec)
- [ ] Broker runtime + node runtime modules
- [ ] Addressed + broadcast routing across processes
- [ ] Heartbeats/liveness and disconnect handling
- [ ] Bounded queue/backpressure semantics

## Phase 2: Observability + Replay

Goal: inspect and replay distributed runs before expanding coordination primitives.

Milestones:

- [ ] `observe` module with structured runtime events
- [ ] Correlation metadata end-to-end
- [ ] Tracing/file sink integration
- [ ] Feature-gated `replay` module (recorded-order replay)

## Phase 3: Coordination Primitives

Goal: add higher-level coordination APIs on top of networked core.

Milestones:

- [ ] Service/agent registry
- [ ] Group/topic routing
- [ ] Request/reply helpers with timeout
- [ ] Supervision policies
- [ ] Timers/scheduling hooks

## Phase 4: Ecosystem Polish + Crate Extraction Review

Goal: improve adoption and decide if any modules should split into crates.

Milestones:

- [ ] Refresh `arbiter` façade exports
- [ ] End-to-end docs for local, LAN, replay
- [ ] CI validation for examples and integration scenarios
- [ ] Crate extraction review using explicit trigger criteria

## Crate Extraction Triggers (Do Not Split Before These)

- Protocol needs independent semver for external/non-Rust consumers
- Optional subsystem adds heavy dependencies and hurts default compile times
- Subsystem requires independent release cadence
- Public API usability materially improves with separation
- Build/test time becomes a sustained productivity bottleneck
