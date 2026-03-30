# Strategy Compile & Link Flow

This document explains how project strategies are discovered, compiled, linked, and executed through `bt`.

## Overview

At runtime, `bt` uses two strategy-related binaries from a project strategy crate:

- `bt_list_strategies` for discovery/metadata
- `bt_run_project` for execution

Both binaries are built from `projects/<name>/strategy/Cargo.toml`.

## End-to-end flow (`bt run`)

1. `bt run` resolves workspace/project and loads `projects/<name>/bt.toml`.
2. `bt` validates strategy IDs using metadata discovery.
3. If the selected strategy is built-in (stage-1 known set), `bt` runs `opt_bt`.
4. Otherwise, `bt` runs project strategy mode (`run_project_strategy`).
5. Project strategy mode builds/resolves `bt_run_project`, then executes it with resolved args/env.

## Strategy discovery (`bt list-strategies` and pre-run validation)

Discovery happens by compiling/running project binary `bt_list_strategies`:

- `discover_strategy_metadata_json` invokes:
  - `cargo run --manifest-path <project>/strategy/Cargo.toml --bin bt_list_strategies`
- `bt_list_strategies` links the project strategy crate and prints SDK metadata as JSON.
- `bt` parses that JSON to derive known strategy IDs and parameter specs.

If discovery fails:

- and `[strategy_registry].stage1_fallback = true` (default), `bt` falls back to `known_stage1_strategies()`.
- and fallback is disabled, command returns an error.

## Build & relink behavior

`build_and_resolve_binary` drives project-binary compilation for `bt_run_project`:

- candidate path: `<project>/strategy/target/{release|debug}/<bin>`
- profile source: `BT_ENGINE_PROFILE` (defaults to `release`)
- forced rebuild: `BT_FORCE_BUILD=1|true|yes`

Rebuild occurs when candidate is missing or stale (`binary_needs_rebuild`), based on mtime checks for:

- `strategy/Cargo.toml`
- `strategy/Cargo.lock` (if present)
- `strategy/src/**`
- `strategy/build.rs` (if present)

If not stale, existing binary is reused.

## How `bt_run_project` links strategy code

Project scaffolding generates `strategy/src/bin/bt_run_project.rs` with:

- `use <project_crate>::create_strategy_by_id;`

This is the key link point:

- The binary is compiled against the project strategy crate.
- At runtime, `create_strategy_by_id(strategy_id, params)` constructs the selected strategy.
- The strategy is registered into `PortfolioStrategy` and executed by `Engine`.

So linking is standard Rust/Cargo crate linking at compile time; strategy selection is dynamic by string ID at runtime.

## Role of `generated/strategy_registry.rs`

Current stage-1 behavior:

- `sync_generated_files` writes a deterministic placeholder `generated/strategy_registry.rs`.
- It documents the stage (`stage1-static`) and fixed fallback strategy IDs.

Important:

- Current discovery/execution primarily uses `bt_list_strategies` + project crate metadata and `bt_run_project`.
- `generated/strategy_registry.rs` is generated glue/marker and fallback context, not the primary dynamic discovery source.

## Command-level summary

- `bt list-strategies`
  - compile/link path: project crate -> `bt_list_strategies`
  - output: metadata JSON/human list
- `bt run --strategy <id>` (project strategy)
  - compile/link path: project crate -> `bt_run_project`
  - runtime: strategy ID -> `create_strategy_by_id` -> engine run -> report output
