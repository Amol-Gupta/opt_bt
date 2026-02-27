# Specification: Options Backtesting Engine

**Feature**: High-Performance Event-Driven Backtesting Engine
**Status**: DRAFT (Updated for portfolio strategy routing)
**Owner**: @author
**Created**: 2026-02-24

## 1. Introduction

### 1.1 Executive Summary
Development of a high-performance, event-driven backtesting engine for Indian market options (Nifty, BankNifty, Sensex). The system prioritizes execution speed (targeting <2 seconds for a 3-year simulation) to enable extensive parameter optimization and portfolio construction. It features a precise event simulation architecture, optimized arithmetic for performance, and comprehensive reporting capabilities.

### 1.2 Goals
-   **Extreme Performance**: Achieve backtest runtimes of ~2 seconds for 3 years of data to facilitate large-scale parameter sweeps.
-   **Accurate Simulation**: Implement a realistic event-driven architecture that processes market events (ticks, orders) in strict time order.
-   **Scalability**: Utilize multi-core processors efficiently by distributing independent backtest jobs (parameter sweeps) across available cores.
-   **Usability**: Provide both machine-readable (JSON) and human-readable (HTML) reports.
-   **Modularity**: Decouple strategy logic from the execution engine to allow easy development of diverse trading strategies.

### 1.3 Scope
**In Scope:**
-   Core event simulation engine (Clock, Event Queue).
-   Order management system (Market, Limit, Stop Market orders).
-   Strategy interface allowing multiple instrument handling (e.g., Nifty CE/PE).
-   Parameter optimization module (Multicore sweep).
-   Reporting module (JSON, HTML, Performance metrics).
-   Post-trade analysis (Slippage, Taxes applied post-simulation).
-   Portfolio construction (Combining multiple strategy results).
-   Portfolio-of-strategies runtime model: many strategies sharing one account with deterministic event routing.
-   Support for Nifty, BankNifty, and Sensex options.

**Out of Scope:**
-   Live trading execution (initially restricted to backtesting).
-   Real-time data fetching (assumes historical data availability).
-   GUI for strategy building (code-first approach).

## 2. Global Functional Requirements (GFR)

### 2.1 System Architecture
-   [GFR-001] **Event-Driven Core**: The engine MUST utilize a time-sequenced mechanism to process events (MarketData, OrderUpdates, Timer) strictly in order. (See FR-010 for detailed processing rules.)
-   [GFR-002] **Optimized Arithmetic**: All price and instrument identifiers MUST be processed using high-performance numerical types (e.g., integers) internally.
-   [GFR-003] **Concurrency Model**: The system MUST support parallel execution of independent backtests (parameter sweeps) on separate CPU cores without shared state contention.
-   [GFR-004] **Shared Immutable Data**: For parameter optimization, the market data set MUST be loaded once into memory and shared across all worker threads as a read-only (immutable) reference to prevent memory duplication and locking overhead.
-   [GFR-005] **Data Locality**: The internal data structure MUST be organized to maximize CPU cache efficiency. (See FR-012 for detailed organization rules.)
-   [GFR-006] **Non-Blocking Logging**: Logging operations MUST be offloaded asynchronously to prevent blocking the main simulation loop.

### 2.2 Performance
-   [GFR-007] **Execution Speed**: A single backtest instance spanning 3 years of data MUST complete in under 2 seconds (on standard modern hardware).
-   [GFR-008] **Memory Efficiency**: The system SHOULD optimize for memory locality and minimize dynamic allocations during the simulation loop.

## 3. User Scenarios


### 3.1 Strategy Development & Backtesting
**Actor**: Quantitative Researcher
**Scenario**:
1.  User implements the strategy logic (handling `on_data`, `before_open`, etc.).
2.  User configures backtest parameters (start date, end date, initial capital, instruments).
3.  At each trading-day start, strategy derives candidate option instruments and subscribes to required contracts.
4.  At each trading-day end, strategy unsubscribes contracts to reset daily universe deterministically.
5.  User runs the backtest.
6.  System simulates the market, processing events and executing orders.
7.  System generates a JSON report with trade list and performance metrics.
8.  System generates an HTML report for visual analysis.

### 3.2 Parameter Optimization
**Actor**: Quantitative Researcher
**Scenario**:
1.  User defines a range of parameters (e.g., Moving Average period 10-50, Stop Loss 1-5%).
2.  User initiates a "Parameter Sweep".
3.  System distributes backtest jobs across all available CPU cores.
4.  System aggregates results into a structured JSON file.
5.  User analyzes the aggregated results to identify optimal parameter sets.

### 3.3 Portfolio Construction
**Actor**: Portfolio Manager
**Scenario**:
1.  User selects multiple successful strategy instances (specific parameters) from previous steps.
2.  User defines a "Portfolio" configuration, allocating a **fixed capital amount (INR)** to each strategy instance.
3.  System performs a combined backtest calculation, aggregating equity curves and managing shared capital/margin based on these fixed allocations.
4.  System reports portfolio-level metrics (Sharpe, Drawdown) accounting for diversification effects.

## 4. Functional Requirements

### 4.1 Strategy Interface
-   [FR-001] **Lifecycle Hooks**: The system MUST provide clear hooks for strategy logic:
    -   `init`: Called once at strategy initialization to set up state and load params.
    -   `before_open`: Called at the beginning of each trading day before market events for that day.
    -   `after_close`: Called at the end of each trading day after market events for that day.
    -   `on_start`: Called once before event loop begins.
    -   `on_market_event`: Called when new market data (tick/bar) is available.
    -   `on_signal`: Called when a signal is emitted.
    -   `on_order_event`: Called when order status changes.
    -   `on_fill`: Called on fills.
    -   `on_date_change`: Called on trading-date boundary.
    -   `on_stop`: Called once after event loop ends.
-   [FR-002] **Multiple Instrument Support**: A single strategy instance MUST be able to subscribe to and trade multiple option contracts (e.g., Nifty CE and PE simultaneously).
-   [FR-002a] **Portfolio Strategy Composition**: The system MUST support a portfolio container strategy that holds multiple child strategies under one engine run.
-   [FR-002b] **Option Instrument Modeling**: The system MUST model option instruments with fields required for deterministic selection and filtering (at minimum: underlying, expiry, strike, option type, and tradable identifier).
-   [FR-002c] **Daily Subscription Lifecycle**: Strategies MUST be able to compute instrument filters and subscribe at `before_open`, and unsubscribe at `after_close`, with deterministic behavior across runs.

### 4.2 Order Management
-   [FR-003] **Order Types**: The engine MUST support at least:
    -   **Market Orders**: Executed immediately at current price.
    -   **Limit Orders**: Executed only at specified price or better.
    -   **Stop Market Orders**: Triggered when price crosses a threshold, becoming a market order.
-   [FR-004] **Order Lifecycle Tracking**: Each order MUST have a trackable status (Pending, Filled, Cancelled, Rejected) with timestamps for state transitions.
-   [FR-005] **Slippage Implementation**: The system MUST support two distinct modes of slippage:
    1.  **Simulation Impact**: A configurable model (e.g., % of price) applied *during* order execution, altering the fill price and potentially triggering stops. This can be set to 0.
    2.  **Post-Analysis Stress Test**: A reporting-layer adjustment allowing users to apply additional theoretical slippage to the final results without re-running the simulation, to analyze strategy robustness.
-   [FR-006] **Pluggable Fill Model**: The system MUST define a `FillModel` trait to govern execution prices (e.g., for Stop orders).
    -   **Same-Bar Execution**: Fills MUST occur on the same bar timestamp where the condition is met.
    -   **Configurability**: Users MUST be able to choose between implementations (e.g., "Optimistic" at Stop Price, "Pessimistic" at Worst Case High/Low).
-   [FR-007] **Independent Leg Execution**: Multi-leg strategies (e.g., Spreads, Straddles) MUST be executed as independent single-leg orders. The system MUST NOT guarantee atomic execution of all legs simultaneously, simulating real-world execution risk.
-   [FR-008] **Post-Trade Adjustments**: The system MUST allow applying tax models to the results after the simulation completes.
-   [FR-009] **Stale Fill Detection**: The execution logic MUST detect and log a warning (in both text logs and the JSON report) if a trade is executed against stale data (where no market data exists for the current minute).
-   [FR-017] **Strategy Identity on Events**: `SignalEvent`, `OrderEvent`, and `FillEvent` MUST include `strategy_id` and preserve it through the full event lifecycle.
-   [FR-018] **Order Event Routing**: In portfolio mode, `on_order_event` MUST be routed only to the strategy identified by `strategy_id`.
-   [FR-019] **Fill Event Routing**: In portfolio mode, `on_fill` MUST be routed to exactly one strategy using `FillEvent.strategy_id`, with `order_id -> strategy_id` mapping as deterministic fallback.
-   [FR-020] **Shared Account Semantics**: All child strategies in a portfolio MUST trade against a single shared account (cash, positions, realized PnL).
-   [FR-021] **Unroutable Event Handling**: If an order/fill cannot be routed to a known strategy, system MUST log a warning and continue simulation deterministically.

### 4.3 Data & Simulation
-   [FR-010] **Event Processing**: The core loop MUST process events strictly by timestamp. Time advances only when no events remain for the current timestamp.
-   [FR-011] **Data Ingestion**: The system MUST support ingestion of historical data from **Parquet** files.
    -   **Granularity**: The system MUST support 1-minute trade bars (OHLCV).
    -   **Optimization**: Data MUST be optimized for internal processing (e.g., pre-processing to binary/memory-mapped format).
-   [FR-012] **Data Organization**: Market data MUST be organized internally by timestamp, then by instrument ID, to optimize for sequential access and cache locality.

### 4.4 Reporting & Analytics
-   [FR-013] **Performance Metrics**: The system MUST calculate and report (formulas and assumptions documented in data-model.md):
    -   Total Return, CAGR.
    -   Max Drawdown (MDD).
    -   Sharpe Ratio, Sortino Ratio (risk-free rate assumption: 5% annual).
    -   Win/Loss Ratio, Profit Factor.
    -   Margin Utilization, Cash Balance tracking.
-   [FR-014] **JSON Output**: All results MUST be serializable to JSON for machine consumption.
-   [FR-015] **HTML Report**: The system MUST generate a standalone HTML report visualizing the equity curve and key metrics.

### 4.5 Logging & Diagnostics
-   [FR-016] **Simulation Time Logging**: The logging system MUST include the current simulation timestamp in every log entry, distinct from the system wall-clock time.
    -   Implementation Note: Use a thread-local or static atomic simulation clock accessed by a custom `ftlog` formatter.

## 5. Non-Functional Requirements
-   **Reliability**: Deterministic execution (same data + same code = exact same result).
-   **Maintainability**: Modular architecture (Core, Strategy, Data, Reporting separated).
-   **Observability**: Clear error messages for data issues or strategy runtime errors.
-   **Technology Constraint**:
    -   Must be implemented in Rust.
    -   **Logging**: Must use `ftlog` for zero-blocking, low-latency logging (hot path < 100ns).

## 6. Assumptions
-   Historical market data (1-minute trade bars) is available in **Parquet** format.
-   Strategies are written in Rust (compiled code).
-   Transaction costs/taxes can be approximated post-facto for backtesting speed.
-   **Reproducibility**: Backtest results are reproducible when strategy code, parameters, engine configuration, engine version, and dataset version (SHA256) are identical.

## 7. Success Criteria
-   [SC-001] **Speed Benchmark**: A standard 3-year backtest with minute data completes in < 2.0 seconds on Intel i7-10700K / 32GB RAM / 8 cores (x86_64-unknown-linux-gnu). Benchmark validated with `criterion` in `benches/engine_bench.rs`.
-   [SC-002] **Order Correctness**: Verifiable order execution (e.g., Limit Buy only executes if Low <= Price).
-   [SC-003] **Resource Utilization**: Parameter sweeps utilize >90% of available CPU cores on reference hardware.
-   [SC-004] **Report Accuracy**: Generated reports match expected metrics for known inputs (verified via integration tests).

## 8. Questions / Clarifications
-   [RESOLVED: Multi-Leg Orders] Independent Legs (User decision: 2026-02-24). The system models execution risk by treating legs as separate orders.

## 9. Clarifications
### Session 2026-02-24
-   Q: Are multi-leg orders atomic? → A: No, each leg is executed independently.
-   Q: When is slippage applied? → A: Both. The system supports "During Simulation" (impacts fill price) and "Post-Simulation" (stress testing robustness).
-   Q: Portfolio Allocation Strategy? → A: Fixed Capital (INR) per strategy instance.
-   Q: Stop Orders on 1-min bars? → A: Same-bar execution with pluggable fill logic (e.g., Optimistic vs Pessimistic). "Next Bar Open" logic is ruled out.

### Session 2026-02-25
-   Q: How is portfolio strategy modeled at runtime? → A: A `PortfolioStrategy` wraps many child strategies under one engine/account.
-   Q: How are events routed? → A: Market events are broadcast to all child strategies; signal/order/fill events are routed by `strategy_id` (with `order_id` mapping fallback for fills).
-   Q: How is capital handled? → A: Current model is one shared account for all child strategies; per-strategy allocation controls remain planned work.
