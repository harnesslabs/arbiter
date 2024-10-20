# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.11.1](https://github.com/anthias-labs/arbiter/compare/arbiter-core-v0.11.0...arbiter-core-v0.11.1) - 2024-10-20

### Added
- fmt
- rebrand, workflows

### Other
- *(deps)* bump hashbrown from 0.14.3 to 0.14.5 ([#963](https://github.com/anthias-labs/arbiter/pull/963))

## [0.11.0](https://github.com/anthias-labs/arbiter/compare/arbiter-core-v0.10.3...arbiter-core-v0.11.0) - 2024-04-26

### Added
- environment logs ([#911](https://github.com/anthias-labs/arbiter/pull/911))

### Other
- Some repository Tender Loving Care. Should fix release please once merged.  ([#959](https://github.com/anthias-labs/arbiter/pull/959))
- *(deps)* bump chrono from 0.4.37 to 0.4.38 ([#954](https://github.com/anthias-labs/arbiter/pull/954))
- *(deps)* bump chrono from 0.4.34 to 0.4.35 ([#930](https://github.com/anthias-labs/arbiter/pull/930))

## [0.10.3](https://github.com/anthias-labs/arbiter/compare/arbiter-core-v0.10.2...arbiter-core-v0.10.3) - 2024-02-20

### Other
- update Cargo.toml dependencies

## [0.10.2](https://github.com/anthias-labs/arbiter/compare/arbiter-core-v0.10.1...arbiter-core-v0.10.2) - 2024-02-13

### Added
- *(arbiter-macros)* `#[arbiter_macros::main]` and a project example ([#880](https://github.com/anthias-labs/arbiter/pull/880))

## [0.10.0](https://github.com/anthias-labs/arbiter/compare/arbiter-core-v0.9.1...arbiter-core-v0.10.0) - 2024-01-23

### Added
- stream into process
- world/agent/behavior state machine
- generic decoded event stream
- database disk handling
- messaging layer + example starts
- pubsub provider worlds

### Fixed
- `arbiter-core` tests
- tests and test workflow

### Other
- Merge pull request [#811](https://github.com/anthias-labs/arbiter/pull/811) from anthias-labs/dependabot/cargo/chrono-0.4.32
- Merge pull request [#804](https://github.com/anthias-labs/arbiter/pull/804) from anthias-labs/arbiter-core/impl-signer
- rm `arbiter-derive`
- Merge branch 'main' into arbiter-engine/world-agent-behaviors
- calling it here
- make clippy happy
- last push
- save
- version finagle
- clean up
- mvp combo stream
- *(deps)* bump polars from 0.35.4 to 0.36.2
- *(deps)* bump futures from 0.3.29 to 0.3.30
- Merge branch 'main' into arbiter-core/db-backend
- mod database and `coprocessor`
- `arbiter-core` compiling
- Merge branch 'main' into arbiter-core/db-backend
- cleanup
- messager echo
- *(deps)* bump test-log from 0.2.13 to 0.2.14
- dependencies
- Revert "Merge pull request [#725](https://github.com/anthias-labs/arbiter/pull/725) from quidproquoo/migrate-alloy"

## [0.9.0](https://github.com/anthias-labs/arbiter/compare/arbiter-core-v0.8.0...arbiter-core-v0.9.0) - 2023-11-18

### Other
- *(deps)* bump polars from 0.34.2 to 0.35.2
- arbiter bind doesn't output nested bindings now
- Merge branch 'main' into arbiter_bindings
- arbiter-bindings

## [0.8.0](https://github.com/anthias-labs/arbiter/compare/arbiter-core-v0.7.6...arbiter-core-v0.8.0) - 2023-11-14

### Fixed
- fix hanging test
- fix shutdown receiver logic

### Other
- add missing shutdown signal for json filetype
- rm sleep in data capture test
- clippy + fmt
- revert back to old json style
- add shutdown sender and receiver oneshot for data collection/environment interaction
- log when file is done being written
- use polars JsonWriter
- rm useless warns
- try w warn and moving to singleton break
- break event collector loop

## [0.7.6](https://github.com/anthias-labs/arbiter/compare/arbiter-core-v0.7.5...arbiter-core-v0.7.6) - 2023-11-13

### Other
- Merge branch 'main' into feat/client-label
- Update mod.rs

## [0.7.5](https://github.com/anthias-labs/arbiter/compare/arbiter-core-v0.7.4...arbiter-core-v0.7.5) - 2023-11-13

### Added
- provide `RevmMiddleware` with label field

### Other
- tag allow unused on label
- Merge branch 'main' into feat/client-label

## [0.7.4](https://github.com/anthias-labs/arbiter/compare/arbiter-core-v0.7.3...arbiter-core-v0.7.4) - 2023-11-10

### Other
- release-plz
- clippy
- Merge branch 'main' into enhance/logging
- Merge pull request [#672](https://github.com/anthias-labs/arbiter/pull/672) from anthias-labs/outputfile_optionality
- abstract data_frame flattening
- Merge branch 'main' into outputfile_optionality
- *(deps)* update serde requirement in /arbiter-core
- *(deps)* bump serde from 1.0.190 to 1.0.191

## [0.7.2](https://github.com/anthias-labs/arbiter/compare/arbiter-core-v0.7.1...arbiter-core-v0.7.2) - 2023-11-06

### Added
- metadata
- pubsubclient test

### Fixed
- fix broken test

### Other
- remove generic
- Merge branch 'main' into feat/pubsubclient
- cleanup
- yo it work
- pubsub implemented

## [0.7.1](https://github.com/anthias-labs/arbiter/compare/arbiter-core-v0.7.0...arbiter-core-v0.7.1) - 2023-10-30

### Fixed
- fix test and change output directory

### Other
- *(deps)* bump serde_json from 1.0.107 to 1.0.108
- other test was broken too
- fmt
- remove other redundant file
- remove test
- fmt

## [0.7.0](https://github.com/anthias-labs/arbiter/compare/arbiter-core-v0.6.3...arbiter-core-v0.7.0) - 2023-10-27

### Fixed
- fix data output test to rmdir after completion
- fix clippy + fmt
- fix tests

### Other
- Merge pull request [#656](https://github.com/anthias-labs/arbiter/pull/656) from anthias-labs/colin/fix-event-logger
- rm printlns
- fmt
- create valid json
- Update data_collection.rs
- wip
- push json logs to vec
- it works!!!!!!!!!!!!
- wip
- unsafe transmute
- Update data_collection.rs
- broadcast enum
- create a channel for direct reads
- *(deps)* bump tracing from 0.1.39 to 0.1.40
