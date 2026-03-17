# opt_bt

High-performance event-driven options backtesting engine in Rust.


## Install from GitHub

For end users, install the CLI and engine binaries from GitHub:

```bash
export OPT_BT_RELEASE_REPO="https://github.com/Amol-Gupta/opt_bt"
export OPT_BT_RELEASE_REV="<tag-or-commit>"

cargo install \
  --git https://github.com/Amol-Gupta/opt_bt \
  --rev "$OPT_BT_RELEASE_REV" \
  --bin bt \
  --bin opt_bt \
  --force
```

Notes:
- install both `bt` and `opt_bt`; the `bt` CLI uses `opt_bt` for built-in engine runs
- use a tag or commit SHA for `OPT_BT_RELEASE_REV`; a branch name works for testing, but tags/commits are safer
- make sure `~/.cargo/bin` is on `PATH`

For local development, see [Contributing Guide](CONTRIBUTING.md#development-mode--binary-deployment).

## Binaries Overview

- `bt`
  - workspace/project CLI for day-to-day usage (`workspace`, `project`, `run`, `sweep`, `cache`, `data`, `clean`)
- `opt_bt`
  - engine executable and lower-level runtime entrypoint used by `bt` and direct engine runs
- project-generated strategy runner
  - generated per workspace project for project-local strategy registration/run glue

## `bt` CLI (workspace-oriented)

Build once:
```bash
cargo build --release --bin bt
```

Show command help:
```bash
bt --help
bt <command> --help
```

### Command reference

#### `bt workspace`
- `bt workspace init [--path <dir>] [--force]`
- Initializes workspace metadata and folders:
  - `.bt/workspace.toml`
  - `.bt/cache/`
  - `projects/`

#### `bt project`
- `bt project init [--workspace <dir>] [--force] <name>`
- Scaffolds a project under `projects/<name>/` with:
  - `bt.toml`
  - strategy crate (`strategy/`)
  - generated glue (`generated/`)

#### `bt run`
- `bt run --project <name> [options]`
- Runs a single backtest for a project strategy.
- Key options:
  - `--workspace <dir>`
  - `--strategy <id>`
  - `--data <parquet_path>`
  - `--start-date YYYY-MM-DD`
  - `--end-date YYYY-MM-DD`
  - `--benchmark <symbol>` (default: `NIFTY 50`)
  - `--params key=value` (repeatable)
  - `--log-time-mode simulation|wall`
- Writes run artifacts into:
  - `projects/<name>/backtests/<strategy>_<timestamp>_<run_id>/`

#### `bt sweep`
- `bt sweep --project <name> --config <file> [--workspace <dir>]`
- Runs a parameter sweep based on sweep config JSON/TOML input.

#### `bt list-strategies`
- `bt list-strategies --project <name> [--workspace <dir>] [--json]`
- Lists discoverable strategy IDs in a project strategy crate.

#### `bt clean`
- `bt clean [--workspace <dir>] [--project <name>] [--generated-only] [--all-cache]`
- Cleanup utility:
  - `--generated-only`: remove generated project glue
  - `--all-cache`: remove workspace cache artifacts

#### `bt cache`
- `bt cache server [--bind <host:port>] [--include-sha256]`
  - Starts long-lived in-memory cache service.
- `bt cache warm [--data <path>] [--start-date ... --end-date ...] [--project <name>] [--workspace <dir>]`
  - Preloads data into cache (and snapshot) for faster warm runs.
- `bt cache status [--json]`
  - Shows cache entry count and keys.
- `bt cache evict --data <path> [--json]`
  - Removes cache entries for a dataset path.

Cache behavior concept:

- first run is typically **cold** (load/decode from parquet)
- repeated runs become **warm** (reuse cached dataset state)
- this is especially useful for repeated parameter sweeps/backtests on the same data
- cache snapshots are stored in `rkyv` serialized market-data format (`rkyv_market_data_v1`) to reduce restore overhead
- `bt data` is cache-first and reads from warmed cache snapshots; see the `bt data` section below for dataset resolution order and examples

#### `bt data`
- `bt data index [--data <path>] [--project <name>] [--workspace <dir>] --symbol <name> [--date YYYY-MM-DD | --start-date ... --end-date ...] [--minute HH:MM] [--window-minutes N]`
  - Prints index bars for a symbol (single day or date range).
- `bt data contract [--data <path>] [--project <name>] [--workspace <dir>] --symbol <option_symbol> --start-date ... --end-date ... [--start-time HH:MM] [--end-time HH:MM]`
  - Prints bars for a specific option contract over a time window.
- `bt data slice [--data <path>] [--project <name>] [--workspace <dir>] --date YYYY-MM-DD --time HH:MM --center-strike <strike> --points <N> [--expiry YYYY-MM-DD] [--fill-forward]`
  - Prints CE/PE strike ladder around a center strike at one timestamp.

`bt data` reads from cache snapshots only (it does not load parquet on demand). Run `bt cache warm` first.

Data-path resolution order for `bt data`:
- `--data`
- `BT_DATA`
- `.bt/workspace.toml` -> `default_data`
- `--project` + project `bt.toml` -> `[run].data`
- exactly one dataset currently present in cache

Why `bt data` is useful:

- quick contract/index validation without running a full backtest
- fast spot checks while analyzing backtest outcomes
- helpful to verify whether expected symbols/timestamps exist in source data

Examples:
```bash
# if there is exactly one dataset in cache, --data can be omitted
BT_CACHE_ADDR=127.0.0.1:7878 bt data index \
  --symbol "NIFTY 50" \
  --date 2024-06-12 \
  --minute 11:00 \
  --window-minutes 1

# resolve data via project config [run].data
BT_CACHE_ADDR=127.0.0.1:7878 bt data contract \
  --project my_strategy \
  --workspace ./demo_ws \
  --symbol NIFTY13JUN2423400CE \
  --start-date 2024-06-12 \
  --end-date 2024-06-12 \
  --start-time 10:55 \
  --end-time 11:10

# explicit dataset override still works
BT_CACHE_ADDR=127.0.0.1:7878 bt data slice \
  --data /quant/nifty_with_options.parquet \
  --date 2024-06-12 \
  --time 11:00 \
  --center-strike 23450 \
  --points 300 \
  --fill-forward
```

Each `bt data` command prints the selected cache entry summary first (`cache_addr`, `cache_key`, `cache_dataset`, `cache_range`, `cache_snapshot`, etc.), then the requested rows.

## End-to-end workflow

This is the recommended flow to create workspace, create strategy project, warm cache, run backtest, and inspect results.

### 1) Create workspace
```bash
bt workspace init --path ./demo_ws
```

### 2) Create strategy project
```bash
bt project init --workspace ./demo_ws my_strategy
```

### 3) Implement/update strategy code
Edit project strategy crate files (for example):
- `demo_ws/projects/my_strategy/strategy/src/my_strategy.rs`

Optional sanity checks:
```bash
cargo check --bin bt
cd demo_ws/projects/my_strategy/strategy && cargo check
```

### 4) Configure run defaults
Edit:
- `demo_ws/projects/my_strategy/bt.toml`

Set at least:
- `[run].default_strategy`
- `[run].data`
- `[run].start_date`
- `[run].end_date`
- `[run].benchmark` (optional; default `NIFTY 50`)

### 5) Start cache server (terminal A)
```bash
bt cache server --bind 127.0.0.1:7878
```

### 6) Warm cache (terminal B)
```bash
BT_CACHE_ADDR=127.0.0.1:7878 bt cache warm \
  --project my_strategy \
  --workspace ./demo_ws
```

Optional cache inspection:
```bash
BT_CACHE_ADDR=127.0.0.1:7878 bt cache status
```

### 7) Run backtest
Using config defaults from `bt.toml`:
```bash
BT_CACHE_ADDR=127.0.0.1:7878 bt run \
  --project my_strategy \
  --workspace ./demo_ws \
  --strategy my_strategy
```

Or override data/date directly:
```bash
BT_CACHE_ADDR=127.0.0.1:7878 bt run \
  --project my_strategy \
  --workspace ./demo_ws \
  --strategy my_strategy \
  --data /quant/nifty_with_options.parquet \
  --start-date 2024-01-01 \
  --end-date 2024-01-05
```

### 8) See results
Latest run folder:
```bash
latest=$(ls -td demo_ws/projects/my_strategy/backtests/my_strategy_* | head -1)
echo "$latest"
```

Inspect logs and report:
```bash
head -n 20 "$latest/engine.log"
jq '.runtime_timing' "$latest/report.json"
```

### 9) (Optional) Evict and re-warm
```bash
BT_CACHE_ADDR=127.0.0.1:7878 bt cache evict --data /quant/nifty_with_options.parquet
BT_CACHE_ADDR=127.0.0.1:7878 bt cache warm --project my_strategy --workspace ./demo_ws
```

## Strategy lifecycle hooks
The strategy trait currently supports:
- `init`
- `on_start`
- `on_date_change`
- `before_open`
- `on_market_event`
- `on_signal`
- `on_order_event`
- `on_fill`
- `after_close`
- `on_stop`

Per-day runner sequencing:
`on_date_change -> before_open -> intraday events -> after_close`

## Tutorial: Write Your Own Strategy

This is the fastest path to add a new strategy to the engine.

### 1) Create a strategy struct
Add your strategy in `src/strategy/examples/` (or another module under `src/strategy/`).

```rust
use crate::common::context::Context;
use crate::common::event::{FillEvent, MarketEvent, OrderEvent, SignalEvent};
use crate::common::types::{OrderType, Side};
use crate::strategy::Strategy;

pub struct TutorialStrategy {
  instrument_id: u32,
  entered_day: Option<i64>,
}

impl TutorialStrategy {
  pub fn new(instrument_id: u32) -> Self {
    Self {
      instrument_id,
      entered_day: None,
    }
  }
}

impl Strategy for TutorialStrategy {
  fn before_open(&mut self, ctx: &mut Context) {
    // Optional: choose subscriptions for this day.
    ctx.set_desired_subscriptions(vec![self.instrument_id]);
    let _ = ctx.apply_subscription_diff();
  }

  fn on_market_event(&mut self, ctx: &mut Context, event: &MarketEvent) {
    if event.instrument_id != self.instrument_id {
      return;
    }

    let day_key = event.timestamp.div_euclid(86_400);
    let seconds_of_day = event.timestamp.rem_euclid(86_400);

    // Example: enter once at/after 10:00
    if self.entered_day != Some(day_key) && seconds_of_day >= 10 * 60 * 60 {
      ctx.place_order(self.instrument_id, Side::Buy, OrderType::Market, 1);
      self.entered_day = Some(day_key);
    }
  }

  fn after_close(&mut self, ctx: &mut Context) {
    // Optional: clear subscriptions at end of day.
    ctx.clear_desired_subscriptions();
    let _ = ctx.apply_subscription_diff();
  }

  fn on_signal(&mut self, _ctx: &mut Context, _event: &SignalEvent) {}
  fn on_order_event(&mut self, _ctx: &mut Context, _event: &OrderEvent) {}
  fn on_fill(&mut self, _ctx: &mut Context, _event: &FillEvent) {}
}
```

### 2) Export the strategy
If you add a new module/file, export it from `src/strategy/mod.rs` so the binary can use it.

### 3) Register strategy kind in CLI factory
Update `build_child_strategy` in `src/main.rs` and add a new match arm:

```rust
"tutorial" => {
  let instrument_id = spec
    .params
    .get("instrument_id")
    .and_then(|s| s.parse::<u32>().ok())
    .unwrap_or(1);
  Some(Box::new(TutorialStrategy::new(instrument_id)))
}
```

### 4) Run it
```bash
cargo run --release -- run \
  --data-dir ./sample_data/niftyIndex2024.sample.parquet \
  --strategy tutorial \
  --params instrument_id=1
```

### 5) Add tests
See [Contributing Guide](CONTRIBUTING.md#integration-testing) for integration testing guidance and reference examples.

## Project Configuration Reference

#### `bt.toml` (project runtime contract)

`projects/<name>/bt.toml` defines project defaults and runtime behavior:

- `[project]` – project identity
- `[engine]` – engine path/bin binding
- `[run]` – benchmark, default strategy, data path, date range, capital, log mode
- `[run.params]` – default strategy parameters
- `[strategy_registry]` – strategy discovery/fallback behavior

#### `generated/strategy_registry.rs`

Generated glue that maps strategy IDs to constructors.

- deterministic registration behavior across environments
- used by run/list-strategies flow
- treat as generated artifact (avoid manual business logic edits)

For more details on strategy discovery and compilation, see [CompileFlow.md](CompileFlow.md).

## Portfolio Composition Examples

### Portfolio of different strategies

```rust
use opt_bt::strategy::portfolio::PortfolioStrategy;

let mut portfolio = PortfolioStrategy::new();
portfolio.add_strategy("trend", Box::new(TrendStrategy::new(20, 50)));
portfolio.add_strategy("mr", Box::new(MeanReversionStrategy::new(14)));
```

### Portfolio using same strategy with different parameters

```rust
use opt_bt::strategy::portfolio::PortfolioStrategy;

let mut portfolio = PortfolioStrategy::new();
portfolio.add_strategy("straddle_fast", Box::new(WeeklyStraddle::new(0.55, 50, 42)));
portfolio.add_strategy("straddle_slow", Box::new(WeeklyStraddle::new(0.35, 75, 99)));
```

This pattern is useful for parameter diversification and attribution comparison.

### Compile/check before run (recommended)
If strategy discovery fails, run `cargo check` before `bt run`.

- Check `bt` CLI itself from repository root (`/home/amol/opt_bt`):
  - `cargo check --bin bt`
- Check your project strategy crate from workspace project path:
  - `cd <workspace>/projects/<project_name>/strategy`
  - `cargo check`

Example for current setup:
- `cd /home/amol/opt_bt/demo_ws/projects/my_strategy/strategy && cargo check`

### Multiple strategies in one project
Yes, a single project can expose multiple strategies from its `strategy/` crate.
- Use `bt list-strategies --project <name>` to see all discovered strategy IDs.
- Run a specific one with `bt run --project <name> --strategy <strategy_id> ...`.
- If `--strategy` is omitted, `bt` uses `[run].default_strategy` (or env/CLI precedence rules).

### Scaffold naming alignment
`bt project init <name>` aligns generated naming with your project name:
- strategy id: snake_case project name (example: `my_strategy`)
- strategy struct: PascalCase + `Strategy` (example: `MyStrategy`)
- strategy crate/package: snake_case project name
- strategy library file: `strategy/src/<project_name_snake_case>.rs` via Cargo `[lib].path`
- `bt.toml` default strategy: generated strategy id

### Config layering (stage 1)
`bt` resolves config values in this order:
1. CLI flags
2. Environment variables
3. `projects/<name>/bt.toml`
4. defaults

### `bt run` required vs optional inputs
- Required: `--project`
- Optional (can be provided via CLI/env/config):
  - `--strategy` or `BT_STRATEGY` or `[run].default_strategy`
  - `--data` or `BT_DATA` or `[run].data`
  - `--start-date` or `BT_START_DATE` or `[run].start_date`
  - `--end-date` or `BT_END_DATE` or `[run].end_date`
  - `--benchmark` or `BT_BENCHMARK` or `[run].benchmark` (defaults to `NIFTY 50`)
  - `--params key=value` (repeatable)

### Benchmark behavior
- Default benchmark is `NIFTY 50`.
- Benchmark-relative metrics now use benchmark return series when available:
  - `alpha`, `beta`, `tracking_error`, `information_ratio`, `treynor_ratio`
- Report metadata includes:
  - `metrics.benchmark_symbol`
  - `metrics.benchmark_available`
- Internal report computation reads `BT_BENCHMARK_SYMBOL` (set automatically by `bt run` / `opt_bt run`).

### Parameter sourcing details
- Base params come from `[run.params]` in `bt.toml`
- Environment override comes from `BT_PARAMS` as comma-separated `key=value` tokens
- Final override comes from CLI `--params key=value` (repeatable)
- Effective priority is: CLI params > `BT_PARAMS` > `[run.params]`

### Logger behavior and configuration
- Backtesting mode defaults to simulation-time logs only (`SIM[YYYY-MM-DD HH:MM:SS]`).
- Logger mode should be configured in project config and forwarded by `bt run` every time.
- Use `[run].log_time_mode` in `projects/<name>/bt.toml`:
  - `simulation` (default for backtests)
  - `wall` (planned live-style mode)
- Default logger level in stage-1 `bt run` path is `info`.

Example (strategy code):
```rust
fn on_market_event(&mut self, _ctx: &mut Context, event: &MarketEvent) {
  log::info!("market event ts={} instrument={}", event.timestamp, event.instrument_id);
}
```

Example (`bt.toml`):
```toml
[run]
log_time_mode = "simulation"
benchmark = "NIFTY 50"
```

Example (override with CLI for a run):
```bash
bt run \
  --project my_strategy \
  --workspace ./demo_ws \
  --strategy my_strategy \
  --log-time-mode wall \
  --data ./sample_data/niftyIndex2024.sample.parquet
```

### Per-backtest output folders (required workflow)
Each project can have multiple backtests. `bt run` now creates a separate folder for every run under the project automatically.

Recommended layout:
```text
projects/<project_name>/
└── backtests/
    ├── <strategy>_<YYYY>_<mm>_<dd>_<HH>_<MM>_<SS>_<run_id>/
    │   ├── report.json       ← full JSON metrics + reproducibility
    │   ├── report.html       ← human-readable HTML report (open in browser)
    │   └── engine.log
    └── <strategy>_<YYYY>_<mm>_<dd>_<HH>_<MM>_<SS>_<run_id>/
        ├── report.json
        ├── report.html
        └── engine.log
```

Example execution (no manual redirection needed):
```bash
bt run \
  --project my_strategy \
  --workspace ./demo_ws \
  --strategy my_strategy \
  --data ./sample_data/niftyIndex2024.sample.parquet
```

### Contract reference
Detailed stage-1 CLI command contract is documented in:
`specs/001-options-backtest-engine/contracts/bt-cli-contract.md`

## Performance benchmark and profiling

### Event loop benchmark (Criterion)
Run the dedicated engine hot-path benchmark:

```bash
cargo bench --bench engine_bench
```

Benchmark target:
- `engine_run_random_prob_0` (loads fixture data, runs full `Engine::init()` + `Engine::run()` loop)

### CPU profiling (flamegraph)
If `cargo-flamegraph` is installed:

```bash
cargo flamegraph --bench engine_bench -- --bench
```

This generates a flamegraph SVG to inspect hot functions in the event loop.

### Allocation profiling (dhat)
If `heaptrack` is available:

```bash
heaptrack target/release/deps/engine_bench-*
```

Or use `dhat` in a dedicated profiling run build (recommended for CI-independent local optimization).

### Notes
- Stage 1 uses generated/static registration (no macro discovery yet).
- Stage 2 introduces attribute-driven registration (`#[bt_strategy(...)]`) and `bt list-strategies`, while keeping top-level `bt` UX compatible.

## Reports
Single `run` prints JSON report to stdout including:
- simulation + metrics
- strategy attribution
- post-analysis + portfolio view
- reproducibility block (engine/config/dataset SHA metadata)

For stage-1 `bt run`, artifacts are written automatically under `projects/<name>/backtests/<strategy>_<timestamp>_<run_id>/`.

### HTML report
`bt run` generates `report.html` alongside `report.json` in the backtest output folder. Open it in any browser to see a formatted summary of metrics, trades, and portfolio view:

```bash
# open the most recent backtest's HTML report
latest=$(ls -td projects/my_strategy/backtests/my_strategy_* | head -1)
xdg-open "$latest/report.html"        # Linux
# open "$latest/report.html"           # macOS
```

Or from the workspace root:
```bash
bt run --project my_strategy --workspace ./demo_ws --strategy my_strategy \
  --data ./sample_data/niftyIndex2024.sample.parquet
# bt prints: bt: HTML report: <full path to report.html>
```

The HTML file is self-contained (no external dependencies) and includes:
- **Summary metrics** table (Sharpe, win rate, max drawdown, etc.)
- **Portfolio view** per-strategy equity and attribution
- **Trades** table with fills and P&L
- Benchmark fields (alpha, beta, tracking error) when benchmark data is present

## Notes
- If `--data-dir` is omitted, engine falls back to `sample_data/niftyIndex2024.sample.parquet` when available.
- `start_date` and `end_date` are date boundaries (`YYYY-MM-DD`); intraday entry/exit time (for example 10:00/11:00) is defined inside strategy logic.
- For fixture preparation details, see `sample_data/README.md`.
- For feature planning artifacts, see `specs/001-options-backtest-engine/`.
