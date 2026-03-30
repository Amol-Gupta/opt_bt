# Acceptance Checklist: `bt` CLI Stage 1 (`T037`–`T044`)

This document defines executable acceptance criteria in Given/When/Then form for Stage 1 `bt` workspace CLI.

## T037 — Create `bt` CLI binary crate

### AC-T037-1 Command surface available
- Given the repository is built successfully
- When the user runs `bt --help`
- Then the CLI shows commands: `workspace init`, `project init`, `run`, `sweep`, `clean`

### AC-T037-2 Subcommand help works
- Given the `bt` binary exists
- When the user runs `bt <command> --help` for each command
- Then help exits with code `0` and shows required/optional arguments

## T038 — Workspace manifest and project discovery

### AC-T038-1 Workspace initialization
- Given an empty directory
- When the user runs `bt workspace init`
- Then `.bt/workspace.toml` is created with valid TOML

### AC-T038-2 Project discovery
- Given `.bt/workspace.toml` and two project folders under workspace
- When the user runs a command requiring `--project <name>`
- Then `bt` resolves the matching project folder deterministically

### AC-T038-3 Invalid workspace handling
- Given a directory without `.bt/workspace.toml`
- When the user runs `bt project init` or `bt run`
- Then `bt` exits non-zero with clear error: workspace not initialized

## T039 — Project scaffold template

### AC-T039-1 Scaffold structure
- Given an initialized workspace
- When the user runs `bt project init alpha`
- Then `projects/alpha/` is created with:
  - `bt.toml`
  - `strategy/` crate
  - generated wiring directory

### AC-T039-2 Sample strategy compiles
- Given a newly scaffolded project
- When the user runs `bt run --project alpha --strategy sample_strategy --data <valid-parquet>`
- Then build succeeds and simulation starts without manual file edits

### AC-T039-3 Duplicate name rejection
- Given a project `alpha` already exists
- When `bt project init alpha` is run again without force
- Then `bt` exits non-zero with explicit duplicate-project message

## T040 — Static registration generation/sync

### AC-T040-1 Registration file generated
- Given a scaffolded project
- When `bt run --project <name>` is executed
- Then `bt` generates/updates static strategy registration file(s)

### AC-T040-2 Deterministic generation
- Given unchanged project inputs
- When generation runs twice
- Then generated files are byte-identical (no noisy diffs)

### AC-T040-3 Invalid strategy ID handling
- Given a `--strategy` id not present in generated registration
- When `bt run` is executed
- Then `bt` exits non-zero with unknown-strategy message

## T041 — `bt run` orchestration

### AC-T041-1 End-to-end run success
- Given a valid workspace, project, strategy, and data path
- When `bt run --project <name> --strategy <id> --data <path>` is executed
- Then `bt` performs generation, build/link, run, and exits `0`

### AC-T041-2 Parameter forwarding
- Given a strategy expecting params
- When `bt run` is called with repeated `--params key=value`
- Then the strategy receives resolved parameters exactly as configured

### AC-T041-3 Config precedence
- Given conflicting values across CLI, env, and `bt.toml`
- When `bt run` executes
- Then final values follow precedence: CLI > env > `bt.toml` > defaults

### AC-T041-4 Error path clarity
- Given an invalid data path
- When `bt run` executes
- Then `bt` exits non-zero and prints precise path validation error

## T042 — `bt sweep` orchestration

### AC-T042-1 Sweep execution
- Given a valid sweep config for a project
- When `bt sweep --project <name> --config <file>` is run
- Then sweep executes and prints structured summary output

### AC-T042-2 Reuse run pipeline
- Given `bt run` and `bt sweep` on same project
- When both are executed
- Then both use the same generation/build orchestration path (single source of truth)

### AC-T042-3 Invalid sweep config
- Given malformed or incomplete sweep config
- When `bt sweep` is executed
- Then `bt` exits non-zero with schema/field-specific error message

## T043 — E2E integration tests for bootstrap + one-command run

### AC-T043-1 Bootstrap integration test
- Given test harness creates temporary workspace directory
- When it runs `bt workspace init` and `bt project init demo`
- Then expected scaffold files exist and are readable

### AC-T043-2 One-command run integration test
- Given scaffolded `demo` project and fixture parquet data
- When test runs `bt run --project demo --strategy sample_strategy --data <fixture>`
- Then command exits `0` and report JSON is produced

### AC-T043-3 CI-stable behavior
- Given integration tests run in CI with clean checkout
- When tests execute repeatedly
- Then outcomes are deterministic and no flaky filesystem assumptions exist

## T044 — Command contract docs and examples

### AC-T044-1 Contract document present
- Given repository docs
- When user opens `specs/001-options-backtest-engine/contracts/bt-cli-contract.md`
- Then command inputs, outputs, and failure modes are documented for stage 1

### AC-T044-2 Example commands validated
- Given README/quickstart command examples for `bt`
- When examples are run against scaffolded project
- Then examples are valid and produce expected behavior

### AC-T044-3 Stage boundary clarity
- Given stage-1 docs
- When user reads contract and plan/tasks
- Then it is explicit that auto-discovery/macros are stage 2, not stage 1
