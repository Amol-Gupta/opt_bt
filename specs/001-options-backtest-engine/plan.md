# Implementation Plan: Options Backtest Engine

**Branch**: `001-options-backtest-engine` | **Date**: 2026-02-25 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/001-options-backtest-engine/spec.md`

## Summary

Development of a high-performance options backtesting engine in Rust, optimized for speed (<2s for 3 years) and scalability (multi-core parameter sweeps). The system uses an event-driven architecture with integer arithmetic for pricing, Parquet data ingestion, and lock-free logging via `ftlog`. It supports Nifty/BankNifty/Sensex options with comprehensive reporting (JSON/HTML).

## Current Implementation Snapshot (2026-02-25)

### Implemented
- `PortfolioStrategy` composition exists (`src/strategy/portfolio.rs`) with child strategy registration.
- Shared-account model exists via `Context.account` and engine-level fill accounting (`src/common/context.rs`, `src/engine/runner.rs`, `src/portfolio/manager.rs`).
- `strategy_id` is present in `SignalEvent`, `OrderEvent`, `FillEvent` and is propagated from order creation to fill generation (`src/common/event.rs`, `src/common/context.rs`, `src/execution/fill.rs`).
- Dataset fixture workflow is added via `uv` subproject (`tools/data_prep_uv`) to inspect parquet schema and generate lightweight repository fixtures in `sample_data/`.
- ATM straddle sell strategy is implemented (`AtmStraddleSellStrategy`) with time-window rules: short CE+PE at 10:00 and square-off at 11:00.
- Portfolio routing behavior is partially implemented:
    - Market events broadcast to all child strategies.
    - Signal, order, and fill callbacks routed by strategy id.
    - Fill routing has fallback lookup via `order_id -> strategy_id` map.

### Incomplete / Risks
- Portfolio allocator guardrails are now enforced at order placement with strategy exposure tracking.
- Strategy-level attribution is now available in account state and JSON reporting.
- Unroutable event diagnostics are now explicit in runtime and surfaced in report warnings.
- Portfolio runtime path is wired through CLI/config with strategy composition support.
- Routing tests now cover unknown/missing strategy-id and out-of-order lifecycle edge cases.
- Detailed performance metrics are now computed in reporting (Sharpe, Sortino, drawdown, profit factor).
- Post-simulation analysis now includes stress-test scenarios and pluggable tax modeling in reporting.
- Consolidated portfolio reporting now includes aggregate and per-strategy breakdown views.
- HTML report template and generator now exist for report rendering/export.
- Reproducibility metadata is now populated from runtime config and dataset SHA256 hashing.
- Data loader still needs mapping from real parquet schema (`Ticker`, `DateTime`, `Open/High/Low/Close`) to internal `MarketData` for fixture-driven E2E tests.
- Day-boundary instrument universe management is not wired yet: `before_open`/`after_close` need runner scheduling and subscription-state handling.
- Option instrument domain modeling is currently minimal; richer option metadata and filter contracts are needed for deterministic subscription.

## Technical Context

**Language/Version**: Rust (Latest Stable)

**Primary Dependencies**:
- **Logging**: `ftlog` (Zero-blocking, low latency).
- **Data**: `polars` (Parquet/Arrow ingestion convenience).
- **CLI**: `clap` (Command line argument parsing).
- **Serialization**: `serde`, `serde_json` (Report generation).
- **Time**: `chrono` (Date handling), `criterion` (Benchmarking).
- **Concurrency**: `rayon` (Parallel parameter optimization).
- **Math**: `i64` (Scaled Integer Arithmetic preferred for speed).

**Architecture Decisions**:
- **Core Loop**: `BinaryHeap` for event priority queue (Time-ordered).
- **Data Loading**: Using `polars` to read Parquet efficiently, then converting to optimized `Vec<Bar>` structs for the core engine.
- **Entry Point**: `main.rs` serves as a CLI wrapper using `clap`. All business logic (including the parameter sweep loop) resides in `lib.rs` / modules.
- **Project Structure**: Standard Rust Workspace or Single Crate (Binary + Library targets). No frontend/backend split.

## Constitution Check

### I. Performance-Critical Rust
- [x] **Target**: <2s for 3 years.
- [x] **Optimization**: Using `i64` arithmetic, `ftlog`.

### II. Event-Driven Architecture
- [x] **Simulation**: Sequential event processing via `BinaryHeap`.
- [x] **Decoupling**: Strategy trait decoupled from Engine.

### III. Testing Discipline
- [x] **Unit Testing**: Will use standard `#[test]`.
- [x] **Benchmarks**: `criterion` for hot path.

### IV. Documentation Excellence
- [x] **User Docs**: `rustdoc` + `quickstart.md`.
- [x] **Sync**: Plan includes doc updates.

### V. Simplicity & Modular Design
- [x] **Modularity**: Crates: `core`, `strategy`, `data`, `reporting`.
- [x] **Dependencies**: `polars` accepted for data loading simplicity.

### VI. Flexible Configuration
- [x] **Hierarchy**: CLI > Env > JSON > Defaults (T002).
- [x] **Reproducibility**: Full config state serializable to JSON for exact reproduction.

## Gates

### Gate 1: Research & Validation
- [x] **Unknowns**: Parquet reading approach (Updated: `polars`).
- [x] **Tech Stack**: Confirmed `ftlog`, `i64`, `clap`.

### Gate 2: Design & Specification
- [x] **Data Model**: `data-model.md` created.
- [x] **Contracts**: JSON report schema created.

### Gate 3: Compliance
- [x] **Spec Quality**: Checked.

## Phases

### Phase 0: Research & Foundation (tasks.md Phase 1: Setup)
- [x] **Research**: Select Logging lib & Data Loader (`polars`). (Done)
- [ ] **Setup**: Initialize Cargo project with `lib` and `bin` targets. (T001)
- [ ] **Config**: Flexible Config struct with Clap/Env/JSON priority. (T002)
- [ ] **Logging**: Set up `ftlog` with simulation-time formatter. (T003)
- [ ] **Types**: Define core domain types. (T004)

### Phase 1: Core Engine & Data (tasks.md Phase 2: Foundational)
- [ ] **Data Module**: Implement `PolarsLoader` -> `Vec<Bar>` conversion. (T005, T006)
- [ ] **Data Inspection**: Implement logic to summarize loaded data (Start/End date, Instrument count, Memory usage).
- [ ] **Event Bus**: Implement `Event`, `EventQueue` (BinaryHeap). (T007)
- [ ] **Context**: Define `Context` struct (API for Strategy). (T008)

### Phase 1A: Dataset Fixture Pipeline (tasks.md Phase 2A)
- [x] **Utility Setup**: Create `uv`-based data prep utility project for repeatable fixture generation. (T008a)
- [x] **Schema Inspection**: Inspect sample source files from `/quant` and store inspection metadata. (T008b)
- [x] **Subset Extraction**: Commit tiny parquet subsets to `sample_data/` for faster testing. (T008c)
- [x] **Fixture Wiring**: Add fixture-driven loader/integration tests to reduce dependence on large external data files. (T008d)

### Phase 2: Order Management & Strategy (tasks.md Phase 3: US1)
- [ ] **Data Structures**: Implement `Order`, `Trade`, `Position`, `Account` structs. (T009, T010)
- [ ] **Execution Logic**: Implement `FillModel` trait, simulation-time slippage model. (T012, T018b)
- [ ] **Strategy Trait**: Define `trait Strategy`. (T011)
- [ ] **Engine Loop**: Implement core Event Loop (`run`). (T013)
- [ ] **Context Wiring**: Connect `Context` methods to `Account` and `Order` execution. (T014)
- [ ] **Reporting**: Implement JSON reporter for trade list and summary. (T015)
- [ ] **Validation**: Example strategy, integration test, stale fill detection. (T016, T017, T018)
- [x] **ATM Straddle Strategy**: Add scheduled ATM short straddle example (entry 10:00, exit 11:00). (T018c)
- [x] **ATM Strategy Test**: Add integration test validating timed entry/exit and square-off. (T018d)

### Phase 3: Parameter Optimization (tasks.md Phase 4: US2)
- [ ] **Config Ranges**: Update Config to support parameter ranges/permutations. (T019)
- [ ] **Parallel Sweep**: Implement `rayon` `par_iter` loop for parameter sweeps. (T020)
- [ ] **Aggregation**: Add result aggregation logic for multi-run summaries. (T021)
- [x] **CLI**: Implement `clap` commands: `inspect`, `run`, `sweep`. (T022)

### Phase 4: Portfolio Construction & Routing Completion (tasks.md Phase 5: US3)
- [x] **Portfolio Wrapper**: Add `PortfolioStrategy` to compose multiple strategies and route callbacks by strategy identity. (T023a)
- [x] **Identity Propagation**: Carry `strategy_id` through signal/order/fill events. (T023b)
- [x] **Allocator**: Implement `PortfolioAllocator` for fixed-capital allocation and exposure limits. (T023)
- [x] **Attribution**: Add strategy-level trade/PnL attribution alongside shared account totals. (T023c)
- [x] **Routing Hardening**: Define and implement unroutable-event policy + diagnostics in runtime and reports. (T023d)
- [x] **Metrics**: Implement detailed performance metrics (Sharpe, Drawdown, Sortino). (T024)
- [x] **Post-Analysis**: Post-simulation analysis logic (stress tests, taxes). (T025)
- [x] **Portfolio View**: Consolidated portfolio reporting (portfolio + per-strategy breakdown). (T026)

### Phase 5: Polish (tasks.md Phase 6)
- [x] **HTML Report**: Create HTML report template and generator. (T027)
- [ ] **Benchmarks**: Add `criterion` benchmarks for hot path. (T028)
- [ ] **Profiling**: Profile and optimize hot path to verify <2s target. (T028a)
- [x] **Docs**: Finalize documentation and API examples. (T029)
- [x] **Reproducibility**: Embed reproducibility metadata in JSON reports. (T029b)

### Phase 5B: Day-Boundary Instrument Subscription (new)
- [x] **Instrument Modeling**: Define/extend option instrument domain fields for filtering and subscription identity. (T030)
- [x] **Filter Criteria**: Implement configurable option filter criteria (underlying, expiry, moneyness/strike band). (T031)
- [x] **Subscription Registry**: Add desired/active subscription state and deterministic subscribe/unsubscribe diffing. (T032)
- [x] **Lifecycle Wiring**: Trigger strategy `before_open` and `after_close` once per trading day in runner flow. (T033)
- [x] **Validation**: Add tests for subscribe-at-open and unsubscribe-at-close behavior, including portfolio wrapper forwarding. (T034)
- [x] **Strategy Scenario**: Implement nearest-expiry Nifty option-chain straddle strategy (subscribe at open, short ATM at 10:00, exit at 11:00, callback logging). (T035)
- [x] **Scenario Verification**: Add integration tests asserting full callback/event flow for the strategy scenario. (T036)

### Phase 6A: `bt` Workspace CLI (Stage 1)
- [ ] **Single Entry CLI**: Introduce a top-level `bt` CLI so users do not write `main` for strategy projects.
- [ ] **Workspace Model**: Add workspace concept where one root contains multiple backtesting projects in separate folders.
- [ ] **Project Scaffolding**: Add `bt project init <name>` to create a runnable project with sample strategy and config.
- [ ] **Generated Runner Glue**: Generate/maintain runner wiring and static strategy registration for each project.
- [ ] **Build + Run Orchestration**: `bt run` compiles, links, and executes simulation with forwarded parameters.
- [ ] **Sweep Orchestration**: Add `bt sweep` path that reuses the same generated runner architecture.
- [ ] **Config Resolution**: Support CLI/env/config layering in `bt`, then pass resolved values into the runner process.
- [ ] **Acceptance Gate**: Validate that a newly scaffolded project can run end-to-end with one command and no manual Rust glue edits.
- [ ] **Acceptance Specification**: Use `contracts/bt-cli-acceptance-checklist.md` (Given/When/Then) as the gate for `T037`–`T044`.

### Phase 6B: Auto-Discovery and Advanced UX (Stage 2)
- [ ] **Macro Registration**: Add attribute-based strategy registration (`#[bt_strategy(...)]`) for zero-manual registry edits.
- [ ] **Compile-Time Registry**: Replace stage-1 generated static map with compile-time strategy discovery registry.
- [ ] **Introspection Commands**: Add `bt list-strategies` and parameter/help introspection from strategy metadata.
- [ ] **Typed Parameter Validation**: Validate strategy params from metadata schema before launching simulation.
- [ ] **Backward Compatibility**: Keep stage-1 registration path available as fallback during migration.
- [ ] **Reproducible Packaging**: Strengthen version pinning for engine + strategy dependencies and emitted run metadata.

## Artifacts Generated
- `research.md`: Technical decisions.
- `data-model.md`: Struct definitions.
- `contracts/report-schema.json`: Output format definition.
- `quickstart.md`: Usage guide.

## Reproducibility Requirements

To ensure backtest results can be reproduced exactly, every backtest run MUST capture and document:

1. **Strategy Code Version**: Git commit hash or release tag of strategy implementation
2. **Strategy Parameters**: All parameter values used (e.g., MA periods, stop-loss %, thresholds)
3. **Engine Configuration**: Complete configuration state (data source, slippage model, fill logic, logging level)
4. **Engine Version**: Backtesting engine git commit/release tag (e.g., v0.1.0-alpha)
5. **Dataset Metadata**: 
   - Data source identifier (e.g., "nse-options-2023")
   - SHA256 checksum of Parquet file(s)
   - Granularity (1-minute bars)
   - Timestamp range (start/end dates)

**Implementation**: JSON report (Phase 3, T015) MUST embed reproducibility block with all above fields. Phase 6 (T029b) adds documentation on reproducibility workflow.

## Project Structure

### Documentation (this feature)

```text
specs/001-options-backtest-engine/
├── plan.md              # Implementation Plan
├── research.md          # Technical Decisions
├── data-model.md        # Core Struct Definitions
├── quickstart.md        # Usage Guide
└── contracts/           # Integration Schemas
    └── report-schema.json
```

### Source Code (repository root)

```text
# Standard Rust Project Layout
Cargo.toml
src/
├── main.rs              # CLI Entry point (clap parsers)
├── lib.rs               # Library root (exposes engine modules)
├── core/                # Event loop, Time, Types
├── data/                # Polars loader, Bar structs
├── execution/           # Order matching, Fill models
├── strategy/            # Traits and reference impls
└── reporting/           # JSON/HTML generation

tests/                   # Integration tests
benches/                 # Criterion benchmarks
```

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Polars Dependency | Rapid development of Parquet loading | Raw Arrow/Parquet crates are verbose and complex to maintain |

