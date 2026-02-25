# Implementation Plan: Options Backtest Engine

**Branch**: `001-options-backtest-engine` | **Date**: 2026-02-24 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/001-options-backtest-engine/spec.md`

## Summary

Development of a high-performance options backtesting engine in Rust, optimized for speed (<2s for 3 years) and scalability (multi-core parameter sweeps). The system uses an event-driven architecture with integer arithmetic for pricing, Parquet data ingestion, and lock-free logging via `ftlog`. It supports Nifty/BankNifty/Sensex options with comprehensive reporting (JSON/HTML).

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

### Phase 2: Order Management & Strategy (tasks.md Phase 3: US1)
- [ ] **Data Structures**: Implement `Order`, `Trade`, `Position`, `Account` structs. (T009, T010)
- [ ] **Execution Logic**: Implement `FillModel` trait, simulation-time slippage model. (T012, T018b)
- [ ] **Strategy Trait**: Define `trait Strategy`. (T011)
- [ ] **Engine Loop**: Implement core Event Loop (`run`). (T013)
- [ ] **Context Wiring**: Connect `Context` methods to `Account` and `Order` execution. (T014)
- [ ] **Reporting**: Implement JSON reporter for trade list and summary. (T015)
- [ ] **Validation**: Example strategy, integration test, stale fill detection. (T016, T017, T018)

### Phase 3: Parameter Optimization (tasks.md Phase 4: US2)
- [ ] **Config Ranges**: Update Config to support parameter ranges/permutations. (T019)
- [ ] **Parallel Sweep**: Implement `rayon` `par_iter` loop for parameter sweeps. (T020)
- [ ] **Aggregation**: Add result aggregation logic for multi-run summaries. (T021)
- [ ] **CLI**: Implement `clap` commands: `inspect`, `run`, `sweep`. (T022)

### Phase 4: Portfolio Construction (tasks.md Phase 5: US3)
- [ ] **Allocator**: Implement `PortfolioAllocator` for fixed-capital allocation. (T023)
- [ ] **Metrics**: Implement detailed performance metrics (Sharpe, Drawdown, Sortino). (T024)
- [ ] **Post-Analysis**: Post-simulation analysis logic (stress tests, taxes). (T025)
- [ ] **Portfolio View**: Consolidated portfolio reporting. (T026)

### Phase 5: Polish (tasks.md Phase 6)
- [ ] **HTML Report**: Create HTML report template and generator. (T027)
- [ ] **Benchmarks**: Add `criterion` benchmarks for hot path. (T028)
- [ ] **Profiling**: Profile and optimize hot path to verify <2s target. (T028a)
- [ ] **Docs**: Finalize documentation and API examples. (T029)
- [ ] **Reproducibility**: Embed reproducibility metadata in JSON reports. (T029b)

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

