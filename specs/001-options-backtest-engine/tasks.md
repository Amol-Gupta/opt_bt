# Tasks: Options Backtest Engine

## Phase 1: Setup
- [x] T001 Initialize Cargo workspace with `bin` and `lib` targets in `src/main.rs` and `src/lib.rs`
- [x] T001b Configure `cargo clippy -- -D warnings` and `rustfmt --check` as CI/pre-commit gate
- [x] T002 Implement flexible `Config` struct with Clap/Env/JSON priority in `src/config.rs` (includes serialization for reproducibility)
- [x] T003 Set up high-performance logging with `ftlog` in `src/core/logging.rs` (coordinate sim-clock with T013)
- [x] T004 [P] Define core domain types (`OrderType`, `Side`, `Status`) in `src/core/types.rs` (ensure all prices use `i64` scaled arithmetic; document scale factor)

## Phase 2: Foundational 
- [x] T005 [P] Implement `Bar` and `MarketData` structs in `src/data/models.rs` (with `#[test]` coverage)
- [x] T006 Implement Parquet data loader using `polars` in `src/data/loader.rs` (return `Arc<MarketData>` for thread-safe sharing; compute dataset SHA256)
- [x] T007 [P] Implement `Event` enum and `BinaryHeap` priority queue logic in `src/core/event.rs` (with `#[test]` coverage)
- [x] T008 [P] Define `Context` struct (API Gateway for Strategy) in `src/core/context.rs` (with `#[test]` coverage)

## Phase 2A: Dataset Fixtures for Testing (Delta 2026-02-25)
- [x] T008a Create `uv` utility project for data prep in `tools/data_prep_uv/`
- [x] T008b Inspect schema for `/quant/merged_data.parquet` and `/quant/niftyIndex2024.parquet`
- [x] T008c Extract deterministic small subsets into repository fixtures:
	- `sample_data/merged_data.sample.parquet`
	- `sample_data/niftyIndex2024.sample.parquet`
	- `sample_data/dataset_inspection.json`
- [x] T008d Wire fixture parquet files into integration test flows (loader + end-to-end smoke)

## Phase 3: [US1] Strategy Development & Backtesting
- [x] T009 [P] [US1] Implement `Order`, `Trade`, and `Position` structs in `src/portfolio/models.rs` (with `#[test]` coverage)
- [x] T010 [US1] Implement `Account` struct for capital/position tracking in `src/portfolio/manager.rs` (with `#[test]` coverage)
- [x] T011 [US1] Define `Strategy` trait with lifecycle hooks (`init`, `on_before_open`, `on_data`, `on_order`, `on_timer`, `on_after_close`) in `src/strategy/mod.rs`
- [x] T012 [P] [US1] Implement default `FillModel` for order execution in `src/execution/fill.rs` (with `#[test]` coverage)
- [x] T013 [US1] Implement core Event Loop (`run`) in `src/engine/runner.rs` (with `#[test]` coverage)
- [x] T014 [US1] Connect `Context` methods to `Account` and `Order` execution in `src/common/context.rs`
- [x] T015 [US1] Implement `JSON` reporter for trade list and summary in `src/reporting/json.rs` (must conform to `contracts/report-schema.json`)
- [x] T016 [US1] Create example `RandomStrategy` for verification in `src/strategy/examples.rs`
- [x] T017 [US1] Add integration test for single-strategy execution in `tests/single_run.rs`
- [x] T017b [US1] Add integration test for independent multi-leg execution (straddle CE+PE) in `tests/multi_leg.rs`
- [x] T018 [US1] Implement stale data detection logic during order execution in `src/execution/stale.rs` (with `#[test]` coverage)
- [x] T018b [US1] Implement simulation-time slippage model in `src/execution/slippage.rs` (with `#[test]` coverage)
- [x] T018c [US1] Implement `AtmStraddleSellStrategy` (entry 10:00, exit 11:00) in `src/strategy/examples.rs`
- [x] T018d [US1] Add integration test for ATM straddle schedule behavior in `tests/atm_straddle.rs`

## Phase 4: [US2] Parameter Optimization
- [x] T019 [P] [US2] Update `Configs` to support parameter ranges/permutations in `src/config.rs`
- [x] T020 [US2] Implement `rayon` parallel iterator for parameter sweep in `src/engine/sweep.rs`
- [x] T021 [US2] Add result aggregation logic for multi-run summaries in `src/reporting/aggregator.rs`
- [x] T022 [US2] Add CLI command `sweep` to trigger optimization in `src/main.rs`

## Phase 5: [US3] Portfolio Construction
- [x] T023 [P] [US3] Implement `PortfolioAllocator` for fixed-capital allocation in `src/portfolio/allocator.rs`
- [X] T024 [US3] Implement detailed performance metrics (Sharpe, Sortino, Drawdown, Profit Factor) in `src/reporting/metrics.rs` (with `#[test]` coverage; risk-free rate: 5% annual)
- [X] T025 [US3] Implement post-simulation analysis logic (stress tests, taxes via `TaxModel` trait) in `src/reporting/post_analysis.rs` (with `#[test]` coverage)
- [X] T026 [US3] Update reporting to handle consolidated portfolio view in `src/reporting/portfolio.rs`

## Phase 5A: [US3] Portfolio-of-Strategies Routing (Delta 2026-02-25)
- [x] T023a [US3] Add `PortfolioStrategy` wrapper to manage multiple child strategies in `src/strategy/portfolio.rs`
- [x] T023b [US3] Add `strategy_id` on `SignalEvent`, `OrderEvent`, and `FillEvent` in `src/common/event.rs`
- [x] T023c [US3] Propagate active strategy identity from context order placement in `src/common/context.rs`
- [x] T023d [US3] Preserve `strategy_id` while generating fills in `src/execution/fill.rs`
- [x] T023e [US3] Route `on_signal`/`on_order_event`/`on_fill` callbacks to target strategy in `src/strategy/portfolio.rs`
- [x] T023f [US3] Add integration test for basic portfolio routing in `tests/portfolio_test.rs`
- [x] T023g [US3] Implement explicit unroutable-event diagnostics (unknown strategy id / missing mapping) in `src/strategy/portfolio.rs` and reporting warning surface
- [x] T023h [US3] Add per-strategy attribution fields to trade/account models and JSON report in `src/portfolio/models.rs`, `src/portfolio/manager.rs`, `src/reporting/json.rs`
- [x] T023i [US3] Enforce fixed-capital allocation and risk limits per strategy using `PortfolioAllocator` + shared account controls
- [x] T023j [US3] Wire portfolio composition into CLI/config so users can run multi-strategy portfolios from config in `src/main.rs`, `src/config.rs`
- [x] T023k [US3] Expand tests for routing edge cases (unknown strategy id, fallback via order map, out-of-order events) in `tests/portfolio_test.rs`

## Phase 6: Polish
- [X] T027 [P] Create HTML report template and generator in `src/reporting/html.rs`
- [X] T028 Add benchmarks for hot path (Event Loop) in `benches/engine_bench.rs`
- [X] T028a Profile and optimize hot path (Event Loop) using `cargo flamegraph` to verify <2s target; include allocation profiling via `dhat` or `heaptrack`
- [X] T029 Finalize documentation and API examples in `README.md`
- [X] T029b [US1,US2,US3] Embed reproducibility metadata in JSON reports (strategy version, parameters, engine version, dataset SHA256) in `src/reporting/reproducibility.rs`

## Phase 7: [US1/US3] Day-Boundary Instrument Subscription
- [X] T030 [US1] Define option instrument domain model and identifiers for deterministic subscription in `src/data/models.rs` and `src/common/types.rs`
- [X] T031 [US1] Add configurable option filtering criteria (underlying, expiry window, strike/moneyness constraints) in `src/config.rs` and `src/strategy/mod.rs`
- [X] T032 [US1] Implement subscription registry/state (desired vs active) with deterministic diff application in `src/common/context.rs`
- [X] T033 [US1,US3] Wire per-day lifecycle sequencing in engine runner (`on_date_change` -> `before_open` -> intraday -> `after_close`) in `src/engine/runner.rs`
- [X] T034 [US1,US3] Add tests for day-open subscribe and day-close unsubscribe behavior, including `PortfolioStrategy` fan-out, in `tests/single_run.rs`, `tests/portfolio_test.rs`, and `src/engine/runner.rs`
- [X] T035 [US1] Implement `NiftyNearestExpiryStraddleStrategy` that subscribes nearest-expiry Nifty option chain at day open, sells ATM straddle at 10:00, exits at 11:00, and logs every received lifecycle/event callback in `src/strategy/examples.rs`
- [X] T036 [US1] Add integration tests that validate complete event flow/callback coverage for `NiftyNearestExpiryStraddleStrategy` in `tests/atm_straddle.rs` and `tests/single_run.rs`

## Phase 8: [US4] Stage 1 `bt` Workspace CLI
- [x] T037 [US4] Create `bt` CLI binary crate with commands: `workspace init`, `project init`, `run`, `sweep`, `clean`
- [x] T038 [US4] Implement workspace manifest and project folder discovery model in `.bt/workspace.toml`
- [x] T039 [US4] Add `bt project init <name>` scaffold template with sample strategy crate, project `bt.toml`, and generated runner wiring
- [x] T040 [US4] Define stage-1 static strategy registration file generation and sync logic in bt codegen module
- [x] T041 [US4] Implement `bt run` orchestration (resolve params -> generate/sync runner -> compile/link -> execute)
- [x] T042 [US4] Implement `bt sweep` orchestration path reusing stage-1 runner and config resolution
- [x] T043 [US4] Add end-to-end integration tests for scaffolded project bootstrap and one-command backtest run
- [x] T044 [US4] Add command contract docs and examples for stage-1 CLI in `specs/001-options-backtest-engine/contracts/bt-cli-contract.md`
- [x] T044a [US4] Validate `T037`–`T044` against `specs/001-options-backtest-engine/contracts/bt-cli-acceptance-checklist.md`

## Phase 9: [US5] Stage 2 Auto-Discovery (Planned)
- [x] T045 [US5] Add strategy registration attribute macro crate (e.g., `#[bt_strategy(...)]`) with metadata capture
- [x] T046 [US5] Implement compile-time strategy registry and runtime lookup replacing stage-1 generated static map
- [x] T047 [US5] Add `bt list-strategies` command with strategy metadata and parameter schema rendering
- [x] T048 [US5] Implement typed parameter validation from strategy metadata before run/sweep launch
- [x] T049 [US5] Add migration compatibility mode: retain stage-1 static registration fallback behind project flag
- [x] T050 [US5] Add integration tests for mixed mode projects (stage-1 fallback + stage-2 registry)

## Implementation Strategy
- **MVP (Phase 1-3)**: Focus on getting a single strategy to run correctly with `polars` data loading and `ftlog`. 
- **Scale (Phase 4)**: Once correctness is verified, enable `rayon` for parallel sweeps.
- **Context Pattern**: The `Context` struct (T008, T014) is critical—it acts as the safe interface between the Strategy and the Engine internals.
- **External UX Roadmap**: Deliver stage-1 `bt` workspace CLI first (US4), then stage-2 auto-discovery and richer introspection (US5).
