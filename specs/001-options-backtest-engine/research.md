# Research & Technical Decisions

## 1. High-Performance Parquet Loading
**Context**: We need to load historical option data (1-minute bars) efficiently into a custom internal memory structure (`Vec<Bar>`) optimized for backtesting speed.
**Options Evaluated**:
1.  **`polars`**: High-level DataFrame library. Extremely ergonomic and fast reader. Adds dependency weight but drastically simplifies code (1-2 lines to read Parquet).
2.  **`parquet` + `arrow`**: Low-level. optimized for binary size but verbose implementation.

**Decision**: Use **`polars`**.
**Rationale**:
-   **Simplicity**: Drastically reduces boilerplate compared to raw Arrow readers.
-   **Performance**: Polars' Parquet reader is highly optimized and multi-threaded by default.
-   **Workflow**: Load into DataFrame -> Convert to `ndarray`/`Vec` for the core engine (using internal structs). We pay the dependency cost for developer velocity.

## 2. Low-Latency Logging
**Context**: Logging must not block the main simulation loop ("Hot Path") to ensure <2s runtime.
**Options Evaluated**:
1.  **`log` / `env_logger`**: Standard but blocking or high overhead.
2.  **`spdlog-rs`**: Fast, async, well-maintained. ~170ns latency.
3.  **`ftlog`**: Dedicated low-latency logger (HFT focus). Uses TSC clock, dedicated thread, drop strategies. ~75ns latency.

**Decision**: Use **`ftlog`**.
**Rationale**:
-   Specific design for high-frequency trading use cases.
-   Supports "discard" policy (bounded channel) to guarantee the main thread *never* blocks even under heavy load.
-   Simpler dependency footprint than `spdlog-rs`.
-   **Requirement**: Must wrap `ftlog` formatter to include **Simulation Time** (via thread-local/atomic) alongside/instead of System Time.

## 3. Price Representation
**Context**: Floating point (`f64`) introduces precision errors and is slower than integer arithmetic for certain operations. `Decimal` types are too slow.
**Decision**: **`i64` Scaled Integers**.
-   **Scale Factor**: `10,000` (4 decimal places).
-   **Implementation**: Prices are stored as `i64`. Calculations (slippage, P&L) handle scaling explicitely.
-   **Why `i64`?** 64-bit integers fit in registers, are atomic-friendly, and avoid `f64` NaN/Inf checks.

## 4. Event Queue Architecture
**Context**: The engine must process mixed events (Data, Signal, Order) in strict time order.
**Decision**: **`std::collections::BinaryHeap`**.
-   **Structure**: `BinaryHeap<Event>` where `Event` implements `Ord` (reverse time order -> Min-Heap).
-   **Complexity**: `O(log N)` insertion/extraction. Efficient enough given N events per timestamp is manageable.

## 5. Stop Order Execution Logic
**Context**: Backtesting on 1-minute bars loses intra-minute granularity. Standard "Next Open" fill is conservative but "Same Bar" logic often desired for stops.
**Decision**: **Pluggable `FillModel` Trait**.
-   **Default Impl**: "Optimistic-Pessimistic Hybrid" (configurable).
    -   *If Low <= Stop <= High*: Triggered.
    -   *Fill Price*: Configurable. Could be `Stop Price` (Optimistic) or `Worst Case` (Low/High).
-   **Constraint**: Execution happens on the *same bar* the condition is met.

## 6. Concurrency Structure
**Context**: Parameter sweeps must utilize all cores.
**Decision**: **`rayon`**.
-   **Pattern**: `par_iter()` over parameter combinations.
-   **Data Sharing**: `Arc<MarketData>` shared across threads. `MarketData` is read-only.
-   **State**: Each iteration gets a fresh `Engine` instance with its own state (Capital, Orders). No shared mutable state.
