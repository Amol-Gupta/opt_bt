# Data Model: Options Backtest Engine

## Core Entities

### 1. Market Data
**Purpose**: Represents historical price data for instruments.
**Structure**:
```rust
struct MarketData {
    // Map instrument ID to sorted bars
    bars: HashMap<u32, Vec<Bar>>,
    // Metadata for instruments (Symbol -> ID)
    instruments: HashMap<String, u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bar {
    timestamp: i64, // Unix Timestamp (seconds)
    open: i64,      // Price * 10,000
    high: i64,
    low: i64,
    close: i64,
    volume: u64,
}
```

### 2. Events
**Purpose**: Unified queue for simulation.
**Structure**:
```rust
enum EventType {
    MarketData { instrument_id: u32, bar: Bar },
    Signal { strategy_id: u32, signal: Signal },
    OrderUpdate { order_id: u64, status: OrderStatus },
    Timer { id: u64, strategy_id: u32, payload: i64 }, // Added strategy context
}

struct Event {
    timestamp: i64,
    priority: u8, // Secondary sort key (e.g., Data < Signal < Order)
    payload: EventType,
}
// Impl Ord for Event: Reverse(timestamp) then priority
```

### 3. Order Management
**Purpose**: Tracks orders and positions.
**Structure**:
```rust
enum OrderSide { Buy, Sell }
enum OrderType {
    Market,        // Executed at best available price (no price parameter)
    Limit(i64),    // Executed at price or better
    Stop(i64),     // Triggered at price
}

struct Order {
    id: u64,
    instrument_id: u32,
    side: OrderSide,
    order_type: OrderType,
    qty: u32,
    timestamp: i64,
    status: OrderStatus,
}

struct Trade {
    id: u64,
    order_id: u64,
    instrument_id: u32,
    price: i64,
    qty: u32,
    timestamp: i64,
    commission: i64,
}

struct Position {
    instrument_id: u32,
    qty: i32,     // +Long, -Short
    avg_price: i64,
}
```

### 4. Account & Strategy
**Purpose**: Strategy logic and single-strategy capital tracking.
**Structure**:
```rust
struct Config {
    start_date: i64, 
    end_date: i64,
    initial_cash: i64,
    params: HashMap<String, String>, // JSON/Env/CLI overrides
}

trait Strategy {
    fn init(&mut self, ctx: &mut Context, config: &Config); // Set up initial state, load params
    fn on_before_open(&mut self, ctx: &mut Context);
    fn on_data(&mut self, ctx: &mut Context, bar: &Bar);
    fn on_order(&mut self, ctx: &mut Context, order: &Order);
    fn on_timer(&mut self, ctx: &mut Context, id: u64, payload: i64);
    fn on_after_close(&mut self, ctx: &mut Context);
}

/// Single-strategy capital and position tracking.
/// Note: "Portfolio" (multi-strategy allocation) is a separate concern; see Phase 5.
struct Account {
    cash: i64,                // Cash balance
    equity: i64,              // Total Equity (Cash + Unrealized P&L)
    positions: HashMap<u32, Position>,
    orders: HashMap<u64, Order>,
}
```

## Reporting Schema (`report.json`)

See `contracts/report-schema.json` for the authoritative JSON schema.

Current top-level report sections are:
- `reproducibility`
- `simulation`
- `metrics`
- `post_analysis`
- `portfolio`
- `strategy_attribution`
- `fills` (execution records)
- `order_events` (intent records)
- `position_events` (position state transition records)
- `warnings`
- `runtime_timing` (injected by CLI runtime)

Metric terminology:
- `fill_count`: canonical execution count
- `round_trip_trade_count`: closed lifecycle count used for risk-style trade analytics

### Tax Model Interface
```rust
/// Pluggable post-simulation tax model (FR-008).
/// Applied after backtest completes to adjust trade P&L.
trait TaxModel {
    /// Apply tax adjustments to a completed trade list.
    fn apply(&self, trades: &[Trade]) -> Vec<TaxAdjustedTrade>;
}

struct TaxAdjustedTrade {
    trade_id: u64,
    gross_pnl: i64,
    tax: i64,
    net_pnl: i64,
}
```

### Example Report
```json
{
    "metrics": {
        "total_return_pct": 12.5,
        "cagr_pct": 4.0,
        "sharpe_ratio": 1.2,
        "sortino_ratio": 1.8,
        "max_drawdown_pct": -5.5,
        "fill_count": 988,
        "round_trip_trade_count": 492,
        "win_rate_pct": 37.2,
        "profit_factor": 1.34,
        "margin_utilization_pct": 13.8,
        "final_cash_balance": 410587.95
  },
    "fills": [
        {
            "id": 1,
            "order_id": 42,
            "strategy_id": "default",
            "symbol": "NIFTY24MAR22000CE",
            "side": "Buy",
            "timestamp": "2024-01-02T09:15:00Z",
            "qty": 50,
            "price": 150.5,
            "fee": 1.25,
            "stale_fill": false
        }
  ],
    "order_events": [
    {
      "id": 1,
            "order_id": 42,
            "strategy_id": "default",
            "instrument_id": 1001,
            "symbol": "NIFTY24MAR22000CE",
            "timestamp": "2024-01-02T09:15:00Z",
            "order_type": "Market",
      "side": "Buy",
      "qty": 50,
            "limit_price": null,
            "status": "Submitted"
        }
    ],
    "position_events": [
        {
            "id": 1,
            "level": "instrument",
            "timestamp": "2024-01-02T09:15:00Z",
            "strategy_id": "default",
            "instrument_id": 1001,
            "symbol": "NIFTY24MAR22000CE",
            "order_id": 42,
            "fill_id": 1,
            "instrument_qty_before": 0,
            "instrument_qty_after": 50,
            "portfolio_open_instruments_before": 0,
            "portfolio_open_instruments_after": 1,
            "portfolio_gross_qty_before": 0,
            "portfolio_gross_qty_after": 50,
            "portfolio_is_flat_before": true,
            "portfolio_is_flat_after": false,
            "change_type": "open",
            "changed_fields": ["instrument_qty", "portfolio_open_instruments", "portfolio_gross_qty", "portfolio_is_flat"]
    }
    ]
}
```
