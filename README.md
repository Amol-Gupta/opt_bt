# opt_bt

High-performance event-driven options backtesting engine in Rust.

## What it does
- Runs single backtests over Parquet historical data
- Runs multicore parameter sweeps
- Supports portfolio-of-strategies runtime with shared account routing
- Emits JSON report output with reproducibility metadata

## Build
```bash
cargo build --release
```

## CLI
```bash
cargo run --release -- --help
```

Current commands:
- `run`
- `sweep --config <file>`

## Run a single strategy
### Nearest-expiry Nifty straddle (day lifecycle + event logging)
```bash
cargo run --release -- run \
  --data-dir ./sample_data/niftyIndex2024.sample.parquet \
  --start-date 2024-04-01 \
  --end-date 2024-04-30 \
  --strategy nifty_nearest_expiry_straddle \
  --params index_symbol=NIFTY\ 50 \
  --params qty=1
```

This strategy:
- subscribes nearest-expiry Nifty option chain at market day open
- sells ATM straddle at 10:00
- exits at 11:00
- unsubscribes chain after market close
- logs every received callback/event

### ATM straddle baseline
```bash
cargo run --release -- run \
  --data-dir ./sample_data/niftyIndex2024.sample.parquet \
  --strategy atm_straddle \
  --params index_symbol=NIFTY\ 50 \
  --params qty=1
```

### SMA NIFTY50 strategy
```bash
cargo run --release -- run \
  --data-dir ./sample_data/niftyIndex2024.sample.parquet \
  --strategy sma_nifty50 \
  --params index_symbol=NIFTY\ 50 \
  --params qty=1 \
  --params short_period=20 \
  --params long_period=50
```

This strategy:
- computes short/long SMA on NIFTY50 index close prices
- goes long when short SMA crosses above long SMA
- goes short when short SMA crosses below long SMA

### Random strategy
```bash
cargo run --release -- run \
  --data-dir ./sample_data/niftyIndex2024.sample.parquet \
  --strategy random \
  --params prob=0.5 \
  --params qty=1 \
  --params seed=42
```

## Run with JSON config
Use `--config-file` for full configuration, including portfolio composition.

### Where to set start/end range
- CLI flags: `--start-date YYYY-MM-DD` and `--end-date YYYY-MM-DD`
- JSON config fields: `start_date` and `end_date`

Example (CLI):
```bash
cargo run --release -- run \
  --data-dir ./sample_data/niftyIndex2024.sample.parquet \
  --start-date 2024-01-01 \
  --end-date 2024-03-31 \
  --strategy random \
  --params prob=0.5 \
  --params qty=1
```

Example `run_config.json`:
```json
{
  "data_dir": "./sample_data/niftyIndex2024.sample.parquet",
  "start_date": "2024-01-01",
  "end_date": "2024-03-31",
  "initial_capital": 1000000,
  "portfolio": {
    "strategies": [
      {
        "id": "s1",
        "kind": "nifty_nearest_expiry_straddle",
        "params": {
          "index_symbol": "NIFTY 50",
          "qty": "1"
        },
        "capital_limit": 300000,
        "max_exposure_ratio": 0.6
      },
      {
        "id": "s2",
        "kind": "random",
        "params": {
          "prob": "0.25",
          "qty": "1",
          "seed": "7"
        },
        "capital_limit": 200000,
        "max_exposure_ratio": 0.5
      },
      {
        "id": "s3",
        "kind": "sma_nifty50",
        "params": {
          "index_symbol": "NIFTY 50",
          "qty": "1",
          "short_period": "20",
          "long_period": "50"
        },
        "capital_limit": 250000,
        "max_exposure_ratio": 0.5
      }
    ]
  }
}
```

Run:
```bash
cargo run --release -- run --config-file ./run_config.json
```

## Sweep
Run parameter sweep from JSON file:
```bash
cargo run --release -- sweep --config ./test_sweep.json
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

## Tutorial: write your own strategy

This is the fastest path to add a new strategy to this engine.

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
Use integration tests under `tests/`:
- create synthetic `MarketData`
- run `Engine::new(...).init(); engine.run();`
- assert trades/positions and lifecycle behavior

Reference examples:
- `tests/single_run.rs`
- `tests/atm_straddle.rs`
- `tests/portfolio_test.rs`

## Stage 1 `bt` CLI workflow (alpha)

This is the current stage-1 direction for external strategy development UX:
- users do not write `main`
- users do not wire strategy factory match-arms manually
- `bt` orchestrates scaffolding, compile/link, and run

### Command surface
- `bt workspace init`
- `bt project init <name>`
- `bt run --project <name> ...`
- `bt sweep --project <name> --config <file>`
- `bt list-strategies --project <name> [--json]`
- `bt clean --project <name>`

### Workspace model
One workspace contains many backtesting projects:

```text
my_bt_workspace/
├── .bt/
│   ├── workspace.toml
│   └── cache/
└── projects/
    ├── nifty_sma/
    │   ├── bt.toml
    │   ├── strategy/
    │   └── generated/    # bt-managed runner/registration glue
    └── options_straddle/
        ├── bt.toml
        ├── strategy/
        └── generated/
```

### Expected flow
1. Initialize workspace: `bt workspace init`
2. Create project scaffold: `bt project init nifty_sma`
3. Edit strategy code only in `projects/nifty_sma/strategy/`
4. Run backtest: `bt run --project nifty_sma --strategy sma_nifty50 --data ./sample_data/niftyIndex2024.sample.parquet`
5. Run sweep: `bt sweep --project nifty_sma --config ./sweep.json`
6. Discover registered strategies: `bt list-strategies --project nifty_sma --json`

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
  - `--params key=value` (repeatable)

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
    │   ├── report.json
    │   └── engine.log
    └── <strategy>_<YYYY>_<mm>_<dd>_<HH>_<MM>_<SS>_<run_id>/
        ├── report.json
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

## Notes
- If `--data-dir` is omitted, engine falls back to `sample_data/niftyIndex2024.sample.parquet` when available.
- `start_date` and `end_date` are date boundaries (`YYYY-MM-DD`); intraday entry/exit time (for example 10:00/11:00) is defined inside strategy logic.
- For fixture preparation details, see `sample_data/README.md`.
- For feature planning artifacts, see `specs/001-options-backtest-engine/`.
