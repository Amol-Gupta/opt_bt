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

See `contracts/report-schema.json` for the authoritative JSON schema (includes `reproducibility`, `simulation`, `metrics`, `trades`, `equity_curve` sections).

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
  "summary": {
    "total_return_pct": 12.5,
    "cagr": 0.04,
    "sharpe": 1.2,
    "drawdown_pct": -5.5,
    "win_rate": 0.65
  },
  "equity_curve": [
    { "timestamp": 1234567890, "equity": 100000 },
    { "timestamp": 1234567950, "equity": 100150 }
  ],
  "trades": [
    {
      "id": 1,
      "symbol": "NIFTY23JAN18000CE",
      "side": "Buy",
      "qty": 50,
      "price": 150.5,
      "timestamp": "2023-01-01T09:15:00Z"
    }
  ]
}
```
