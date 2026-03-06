# Contract: `bt` CLI (Stage 1)

## Scope
Stage 1 defines a simple orchestration CLI where users do not write `main`.

- CLI name: `bt`
- Entry model: workspace + projects
- Strategy registration: stage-1 generated/static registration (no compile-time macro discovery)

## Command Surface

### 1) `bt workspace init`
Initializes a workspace root for backtesting projects.

**Input**
- optional path (default: current directory)

**Output/Side Effects**
- Creates `.bt/workspace.toml`
- Creates default folders:
  - `projects/`
  - `.bt/cache/`
  - `.bt/templates/` (if needed)

**Exit behavior**
- non-zero if path is invalid or workspace already exists and `--force` not provided

---

### 2) `bt project init <name>`
Creates a new backtesting project folder with sample strategy and runner glue.

**Input**
- positional: `name`
- options:
  - `--workspace <path>`
  - `--template <id>` (default: `sample`)

**Output/Side Effects**
- Creates `projects/<name>/`
- Creates `projects/<name>/bt.toml`
- Creates `projects/<name>/strategy/` crate with sample strategy
- Creates generated runner wiring files under project-controlled generated folder

**Naming Convention (Scaffold Alignment)**
- `project.name` in `bt.toml` is `<name>`
- Scaffold strategy id is derived from project name in snake_case (e.g., `my_strategy`)
- Scaffold strategy struct is derived in PascalCase + `Strategy` suffix (e.g., `MyStrategy`)
- Strategy crate/package name is snake_case project name
- Library entry file is aligned to project name via Cargo lib path (`src/<project_name_snake_case>.rs`)

**Exit behavior**
- non-zero if name already exists or template is invalid

---

### 3) `bt run`
Compiles/links and runs a project backtest by orchestrating generated runner + engine.

**Input**
- options:
  - `--project <name>` (required)
  - `--strategy <id>`
  - `--data <path>`
  - `--start-date <YYYY-MM-DD>`
  - `--end-date <YYYY-MM-DD>`
  - `--benchmark <symbol>`
  - `--log-time-mode <simulation|wall>`
  - `--params key=value` (repeatable)
  - `--config <path>`

**Resolution Order**
1. CLI flags
2. Environment variables
3. `projects/<name>/bt.toml`
4. project defaults

**Required vs Optional Inputs**
- Required:
  - `--project <name>`
- Optional (may be sourced from env/config):
  - `--strategy <id>` or `BT_STRATEGY` or `[run].default_strategy`
  - `--data <path>` or `BT_DATA` or `[run].data`
  - `--start-date <YYYY-MM-DD>` or `BT_START_DATE` or `[run].start_date`
  - `--end-date <YYYY-MM-DD>` or `BT_END_DATE` or `[run].end_date`
  - `--benchmark <symbol>` or `BT_BENCHMARK` or `[run].benchmark` (default `NIFTY 50`)
  - `--log-time-mode <simulation|wall>` or `[run].log_time_mode`
  - `--params key=value` (repeatable)

**Parameter Merge Rules**
- Base params come from `[run.params]`
- Then `BT_PARAMS` (comma-separated `key=value`) overrides config values
- Then repeated `--params key=value` overrides env/config values
- Effective parameter priority: CLI `--params` > `BT_PARAMS` > `[run.params]`

**Output/Side Effects**
- Syncs generated registration/runner files
- Invokes build (`cargo build` or `cargo run`) for the project runner target
- Executes simulation binary
- Emits JSON report to stdout and writes `report.json` under run artifact folder
- Logger output is written to stderr; default timestamp mode in backtest is simulation time (`SIM[...]`)
- Writes stderr stream to `engine.log` under run artifact folder while also printing to console

**Logger Configuration**
- Time mode:
  - Default (backtest): simulation time
  - Configure in `bt.toml` via `[run].log_time_mode` (`simulation` or `wall`)
  - Optional per-run override via `bt run --log-time-mode ...`
- Log level:
  - Stage-1 project runner currently uses `info` level by default

**Backtest Output Folder Requirement**
- A project may have multiple backtests; each run MUST use a separate output folder under project scope.
- `bt run` creates this folder automatically using pattern:
  - `projects/<name>/backtests/<strategy>_<YYYY>_<mm>_<dd>_<HH>_<MM>_<SS>_<run_id>/`
- Files written automatically by `bt run`:
  - `report.json` (captured stdout report)
  - `engine.log` (captured stderr stream)

**Exit behavior**
- non-zero on config validation failure, build failure, or runtime failure

---

### 4) `bt sweep`
Runs parameter sweeps for a project using stage-1 generated runner orchestration.

**Input**
- options:
  - `--project <name>` (required)
  - `--config <path>` (required for sweep ranges)

**Output/Side Effects**
- Compiles/links runner if needed
- Executes sweep
- Emits sweep summary JSON to stdout by default

---

### 5) `bt clean`
Removes generated files and optionally build/cache artifacts.

**Input**
- options:
  - `--project <name>`
  - `--generated-only`
  - `--all-cache`

**Output/Side Effects**
- Removes generated runner wiring and/or cached artifacts per flags

## Project Files (Stage 1)

Required per project:
- `projects/<name>/bt.toml`
- `projects/<name>/strategy/` crate
- generated registration/runner files (tool-managed)

Minimal `bt.toml` fields:
- `project.name`
- `engine.source` (path/git)
- `run.default_strategy`
- `run.data`
- `run.initial_capital`
- `run.benchmark` (optional; default `NIFTY 50`)

## Non-Goals (Stage 1)
- No compile-time strategy auto-discovery macros.
- No runtime dynamic loading (`cdylib`) plugin system.
- No user-authored runner `main`.

## Stage 2 Compatibility
Stage 2 may replace static generated registration with attribute-based registry; command names and top-level UX should remain compatible.
