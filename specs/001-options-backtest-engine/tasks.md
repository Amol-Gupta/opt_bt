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

## Phase 4: [US2] Parameter Optimization
- [x] T019 [P] [US2] Update `Configs` to support parameter ranges/permutations in `src/config.rs`
- [x] T020 [US2] Implement `rayon` parallel iterator for parameter sweep in `src/engine/sweep.rs`
- [x] T021 [US2] Add result aggregation logic for multi-run summaries in `src/reporting/aggregator.rs`
- [ ] T022 [US2] Add CLI command `sweep` to trigger optimization in `src/main.rs`

## Phase 5: [US3] Portfolio Construction
- [ ] T023 [P] [US3] Implement `PortfolioAllocator` for fixed-capital allocation in `src/portfolio/allocator.rs`
- [ ] T024 [US3] Implement detailed performance metrics (Sharpe, Sortino, Drawdown, Profit Factor) in `src/reporting/metrics.rs` (with `#[test]` coverage; risk-free rate: 5% annual)
- [ ] T025 [US3] Implement post-simulation analysis logic (stress tests, taxes via `TaxModel` trait) in `src/reporting/post_analysis.rs` (with `#[test]` coverage)
- [ ] T026 [US3] Update reporting to handle consolidated portfolio view in `src/reporting/portfolio.rs`

## Phase 6: Polish
- [ ] T027 [P] Create HTML report template and generator in `src/reporting/html.rs`
- [ ] T028 Add benchmarks for hot path (Event Loop) in `benches/engine_bench.rs`
- [ ] T028a Profile and optimize hot path (Event Loop) using `cargo flamegraph` to verify <2s target; include allocation profiling via `dhat` or `heaptrack`
- [ ] T029 Finalize documentation and API examples in `README.md`
- [ ] T029b [US1,US2,US3] Embed reproducibility metadata in JSON reports (strategy version, parameters, engine version, dataset SHA256) in `src/reporting/reproducibility.rs`

## Implementation Strategy
- **MVP (Phase 1-3)**: Focus on getting a single strategy to run correctly with `polars` data loading and `ftlog`. 
- **Scale (Phase 4)**: Once correctness is verified, enable `rayon` for parallel sweeps.
- **Context Pattern**: The `Context` struct (T008, T014) is critical—it acts as the safe interface between the Strategy and the Engine internals.
