# Strategy API Documentation

This document describes the core APIs available to strategies in the options backtest engine.

## Table of Contents

1. [Order Placement](#order-placement)
2. [Alarm Scheduling](#alarm-scheduling)
3. [Option Chain Search](#option-chain-search)
4. [Order Rejection Events](#order-rejection-events)
5. [Fill Events](#fill-events)
6. [Market Data Access](#market-data-access)
7. [Position & PnL Queries](#position--pnl-queries)

---

## Order Placement

The `Context` provides several methods to place orders. All order placement methods return an `order_id` (u64). A return value of `0` indicates rejection.

### Market Orders

A market order is filled at the last bar's close price generated when the order is submitted.

**Signature:**
```rust
pub fn place_order(
    &mut self,
    instrument_id: u32,
    side: Side,
    order_type: OrderType::Market,
    quantity: i64,
) -> u64
```

**Example:**
```rust
// Buy 10 units at market price
let order_id = ctx.place_order(
    instr_id,
    Side::Buy,
    OrderType::Market,
    10
);

if order_id == 0 {
    // Order was rejected - check on_order_rejected callback for reason
}
```

**Semantics:**
- Fills immediately at the close price of the current candle
- No price guarantee (may fill at worse price if volatility exists within candle)
- Useful for market entry/exit signals

---

### Limit Orders

A limit order is a pending order that fills when the market reaches your specified price.

**Signature:**
```rust
pub fn place_order(
    &mut self,
    instrument_id: u32,
    side: Side,
    order_type: OrderType::Limit(price),
    quantity: i64,
) -> u64
```

**Fill Logic:**
- **Buy limit**: Fills when `bar.low <= limit_price` (price reached from above)
- **Sell limit**: Fills when `bar.high >= limit_price` (price reached from below)

**Example:**
```rust
// Buy 5 units if price drops to 2950
let order_id = ctx.place_order(
    nifty_id,
    Side::Buy,
    OrderType::Limit(2950 * PRICE_SCALE),
    5
);

// Sell 5 units if price rallies to 3050
let order_id = ctx.place_order(
    nifty_id,
    Side::Sell,
    OrderType::Limit(3050 * PRICE_SCALE),
    5
);
```

**Rejection Conditions:**
- Quantity <= 0
- Insufficient buying power (for buy orders)
- Allocator notional limit exceeded
- Order already active as pending

---

### Stop Orders

A stop order is a pending order that becomes active only when the market price reaches/crosses your stop price.

**Signature:**
```rust
pub fn place_order(
    &mut self,
    instrument_id: u32,
    side: Side,
    order_type: OrderType::Stop(stop_price),
    quantity: i64,
) -> u64
```

**Fill Logic:**
- **Buy stop**: Fills when `bar.high >= stop_price` (price rises to trigger)
  - Useful for entering on breakout or exiting shorts on loss
- **Sell stop**: Fills when `bar.low <= stop_price` (price falls to trigger)
  - Useful for exiting longs on loss or entering shorts on breakdown

**Example:**
```rust
// Place long entry buy stop at 3050 (execute if market rallies above 3050)
let entry_stop = ctx.place_order(
    nifty_id,
    Side::Buy,
    OrderType::Stop(3050 * PRICE_SCALE),
    10
);

// Place stop loss sell stop at 2900 for existing long position
let sl_stop = ctx.place_order(
    nifty_id,
    Side::Sell,
    OrderType::Stop(2900 * PRICE_SCALE),
    10
);
```

**Rejection Conditions:**
- Stop price already breached in current bar
  - Prevents placing stops that would be immediately triggered
  - Example: Cannot place buy stop at 100 when current bar high is 105
- Quantity <= 0
- Insufficient buying power (for buy orders)
- Allocator notional limit exceeded

**Note:** Once a stop is filled, it becomes a regular position like any other order. Use subsequent market or limit orders to manage the position.

---

### Timestamp-Aware Placement

For precise timing in complex strategies, use variants that accept explicit timestamps:

```rust
pub fn place_order_at(
    &mut self,
    instrument_id: u32,
    side: Side,
    order_type: OrderType,
    quantity: i64,
    timestamp: SimTime,
) -> u64
```

This is typically called from `on_market_event()` callbacks where you have the exact market timestamp.

---

## Alarm Scheduling

Alarms allow strategies to trigger periodic or absolute-time callbacks without monitoring the market continuously.

### Schedule Relative Alarm

Schedule an alarm for a fixed time offset from now.

**Signature:**
```rust
pub fn schedule_alarm(
    &mut self,
    delay_seconds: i64,
    key: &str,
) -> AlarmHandle
```

**Example:**
```rust
fn on_market_event(&mut self, ctx: &mut Context, _event: &MarketEvent) {
    // Schedule entry signal for 5 minutes from now
    let entry_alarm = ctx.schedule_alarm(5 * 60, "entry");
    
    // Schedule exit signal for 2 hours from now
    let exit_alarm = ctx.schedule_alarm(2 * 60 * 60, "exit");
}

fn on_alarm(&mut self, ctx: &mut Context, event: &AlarmEvent) {
    match event.key.as_str() {
        "entry" => {
            println!("Entry signal triggered");
            // Place entry orders
        }
        "exit" => {
            println!("Exit signal triggered");
            // Close positions
        }
        _ => {}
    }
}
```

### Schedule Absolute-Time Alarm

Schedule an alarm for a specific timestamp.

**Signature:**
```rust
pub fn schedule_alarm_at(
    &mut self,
    timestamp: SimTime,
    key: &str,
) -> AlarmHandle
```

**Example:**
```rust
// Schedule entry alarm for 11:00 local market time
let alarm_11am = ctx.schedule_alarm_at(
    ctx.now().start_of_local_day().add_seconds(11 * 60 * 60),
    "morning_entry"
);

// Schedule exit alarm for 3:00 PM
let alarm_3pm = ctx.schedule_alarm_at(
    ctx.now().start_of_local_day().add_seconds(15 * 60 * 60),
    "afternoon_exit"
);
```

### Alarm with Correlation Key

Use the correlation key to scope alarms per security or condition:

```rust
pub fn schedule_alarm_with(
    &mut self,
    delay_seconds: i64,
    key: &str,
    correlation_key: &str,
) -> AlarmHandle
```

**Example:**
```rust
// Schedule separate alarms for different strikes
let call_alarm = ctx.schedule_alarm_with(5 * 60 * 1000, "slippage_check", "call_strike_3000");
let put_alarm = ctx.schedule_alarm_with(5 * 60 * 1000, "slippage_check", "put_strike_3000");

fn on_alarm(&mut self, ctx: &mut Context, event: &AlarmEvent) {
    if event.key == "slippage_check" {
        match event.correlation_key.as_str() {
            "call_strike_3000" => { /* check call position */ }
            "put_strike_3000" => { /* check put position */ }
            _ => {}
        }
    }
}
```

### Alarm Event Handler

Alarms arrive via callback:

```rust
fn on_alarm(&mut self, ctx: &mut Context, event: &AlarmEvent) {
    println!("Alarm triggered: key={}, scheduled_for={}", 
             event.key, event.scheduled_for);
}
```

**AlarmEvent Fields:**
- `alarm_id: u32` - Unique identifier (1-indexed)
- `key: String` - Your alarm identifier
- `correlation_key: String` - Optional correlation metadata
- `scheduled_for: SimTime` - Target timestamp when alarm should trigger
- `timestamp: SimTime` - Actual time when alarm triggered
- `strategy_id: String` - Strategy that scheduled the alarm

---

## Option Chain Search

The `Context` provides utility methods to search and filter option chains.

### Get Nearest Weekly Expiry

Find the next weekly expiration from a given date.

**Signature:**
```rust
pub fn nearest_weekly_expiry(&self, from_timestamp: SimTime) -> Option<i64>
```

**Example:**
```rust
fn on_market_event(&mut self, ctx: &mut Context, event: &MarketEvent) {
    if let Some(next_expiry) = ctx.nearest_weekly_expiry(event.timestamp) {
        println!("Next weekly expiry: {}", next_expiry);
        // Use this for searching ATM options
    }
}
```

### Resolve ATM Pair

Get the ATM (at-the-money) call and put strike for a given spot price and expiry.

**Signature:**
```rust
pub fn resolve_atm_pair(
    &self,
    spot_price: i64,
    expiry_yyyymmdd: i32,
) -> Option<(u32, u32)> // (call_instrument_id, put_instrument_id)
```

**Example:**
```rust
// Get ATM options for next weekly expiry
let spot_price = ctx.get_bar(nifty_id).map(|b| b.close)?;
let expiry = ctx.nearest_weekly_expiry(current_time)?;
let (call_id, put_id) = ctx.resolve_atm_pair(spot_price, expiry)?;

// Place ATM straddle
let call_qty = 100;
let put_qty = 100;

ctx.place_order(call_id, Side::Sell, OrderType::Market, call_qty);
ctx.place_order(put_id, Side::Sell, OrderType::Market, put_qty);
```

### Filter Strikes by DTE

Find option strikes within a specified days-to-expiration range.

**Signature:**
```rust
pub fn filter_by_dte(
    &self,
    min_dte_days: i64,
    max_dte_days: i64,
    current_time: SimTime,
) -> Vec<(String, i32)> // (symbol, expiry_yyyymmdd)
```

**Example:**
```rust
// Find all expirations 8-30 days away
let expirations = ctx.filter_by_dte(8, 30, current_time);
for (symbol, expiry) in expirations {
    println!("Available: {} expired at {}", symbol, expiry);
}
```

### Search by Strike and Expiry

Find calls and puts given strike and expiry.

**Signature:**
```rust
pub fn find_option_chain(
    &self,
    strike_price: i64,
    expiry_yyyymmdd: i32,
) -> Option<(u32, u32)> // (call_id, put_id)
```

**Example:**
```rust
// Search for 3100 call and put in weekly options
let strike = 3100 * PRICE_SCALE;
let expiry = ctx.nearest_weekly_expiry(current_time)?;

if let Some((call_id, put_id)) = ctx.find_option_chain(strike, expiry) {
    // Place non-directional trade (short strangle)
    ctx.place_order(call_id, Side::Sell, OrderType::Market, 50);
    ctx.place_order(put_id, Side::Sell, OrderType::Market, 50);
}
```

---

## Order Rejection Events

When an order is rejected, the strategy receives a callback instead of a silent failure (no order_id return).

### Rejection Callback

```rust
fn on_order_rejected(&mut self, ctx: &mut Context, event: &OrderRejectionEvent) {
    println!("Order rejected: {} (reason: {})", 
             event.order_id, event.reason);
}
```

### OrderRejectionEvent Fields

- `timestamp: SimTime` - When rejection occurred
- `instrument_id: u32` - Which instrument
- `order_type: OrderType` - Type of order (Market, Limit, Stop)
- `side: Side` - Buy or Sell
- `quantity: i64` - Requested quantity
- `reason: String` - Human-readable rejection reason
- `strategy_id: String` - Which strategy caused the rejection

### Common Rejection Reasons

1. **Quantity Validation** 
   - Reason: `"non-positive quantity=X"`
   - Cause: Quantity <= 0
   - Action: Verify quantity parameter is positive

2. **Insufficient Capital**
   - Reason: `"insufficient capital: required_cash=X available_cash=Y"`
   - Cause: Buy order requires more capital than available
   - Action: Reduce position size or close existing positions

3. **Stop Price Already Breached**
   - Reason: `"stop price already breached: side=Buy stop_price=X bar_high=Y bar_low=Z"`
   - Cause: Stop price already triggered in current bar
   - Action: Place stop at a safer level

4. **Allocator Rejected**
   - Reason: `"allocator rejected: current_open_notional=X additional_notional=Y"`
   - Cause: Notional exposure limit exceeded
   - Action: Close existing positions or reduce size

### Example: Handling Rejections

```rust
fn on_order_rejected(&mut self, ctx: &mut Context, event: &OrderRejectionEvent) {
    eprintln!("Order rejected for {}: {}", event.instrument_id, event.reason);
    
    // Attempt recovery based on reason
    if event.reason.contains("insufficient capital") {
        // Try with smaller quantity
        let reduced_qty = (event.quantity as i32 / 2) as i64;
        ctx.place_order(
            event.instrument_id,
            event.side,
            event.order_type.clone(),
            reduced_qty
        );
    } else if event.reason.contains("stop price already breached") {
        // Place stop at a wider level
        if let OrderType::Stop(stop_price) = event.order_type {
            let adjusted_stop = match event.side {
                Side::Buy => stop_price * 105 / 100,  // 5% higher
                Side::Sell => stop_price * 95 / 100,  // 5% lower
            };
            ctx.place_order(
                event.instrument_id,
                event.side,
                OrderType::Stop(adjusted_stop),
                event.quantity
            );
        }
    }
}
```

---

## Fill Events

When an order is filled (market executed or pending order triggered), the strategy receives a fill notification.

### Fill Callback

```rust
fn on_fill(&mut self, ctx: &mut Context, event: &FillEvent) {
    println!("Fill: {} {} @ {} qty={}", 
             event.side, event.instrument_id, 
             event.fill_price, event.quantity);
}
```

### FillEvent Fields

- `timestamp: SimTime` - When order was filled
- `order_id: u64` - Order that was filled
- `instrument_id: u32` - What was filled
- `side: Side` - Buy or Sell
- `quantity: i64` - Filled quantity
- `fill_price: i64` - Price per unit (scaled by PRICE_SCALE)
- `fee: i64` - Transaction fee deducted
- `status: Status` - Filled/PartiallyFilled
- `strategy_id: String` - Which strategy owns the fill

### Example: Tracking Fills

```rust
fn on_fill(&mut self, ctx: &mut Context, event: &FillEvent) {
    // Update position tracking
    let instrument_name = ctx.get_instrument_name(event.instrument_id);
    println!("Executed: {} {} @ {}", 
             event.side, instrument_name, event.fill_price);
    
    // Can query position immediately after fill
    let current_qty = ctx.position_qty(event.instrument_id);
    let avg_cost = ctx.position_avg_cost(event.instrument_id);
    println!("Position: {} units @ avg {}", current_qty, avg_cost);
}
```

### Fill Timing Semantics

- `OrderType::Market` currently fills at the close of the bar used for evaluation.
- If a strategy detects a breakout on the current bar and immediately submits a market order, the fill price will be that same bar's close in the current engine.
- `OrderType::Limit` fills at the submitted limit price once the bar crosses that level.
- `OrderType::Stop` fills at the submitted stop price once the bar crosses that level.

This matters when writing tutorials or strategies that say "enter at prevailing price". In the current backtest engine, that phrase should usually be read as "fill at the current evaluation bar close" for market orders.

### Exit-Bar Ambiguity For OHLC Strategies

If a strategy uses bar OHLC data to evaluate both target and stop-loss, a single bar can touch both levels.
The engine does not impose a universal policy for this ambiguity at the strategy layer. The strategy author should define and document one of these behaviors explicitly:

- Conservative stop-first precedence
- Favorable target-first precedence
- Skip exit on ambiguous bars
- Use lower-timeframe data to disambiguate

For `banknifty_orb_btst`, the strategy uses conservative stop-loss precedence when both levels are touched in the same exit bar.

---

## Market Data Access

Get current bar and price information.

**Get Current Bar:**
```rust
pub fn get_bar(&self, instrument_id: u32) -> Option<Bar>
```

### Important: `get_bar()` Can Return A Stale Option Bar

`Context::get_bar()` returns the latest bar at or before the current simulation timestamp.
That is often fine for dense index data, but it can be wrong for sparse option datasets where a contract does not print on every minute.

If you are selecting strikes, building a range, or checking breakout conditions for options, prefer exact timestamp access through market data:

```rust
let now = ctx.now();
let exact_bar = ctx.market_data.get_bar_at(instrument_id, now);
```

Use exact bars when:

- selecting a contract based on premium at a specific time
- building an ORB range from exact timestamps only
- checking breakout conditions that must be based on the current bar, not the last seen bar

Use `get_bar()` when:

- last known price is acceptable
- you are intentionally working with carry-forward marks
- the instrument data is known to be dense enough for your use case

**Example:**
```rust
if let Some(bar) = ctx.get_bar(nifty_id) {
    println!("NIFTY: O={}, H={}, L={}, C={}", 
             bar.open, bar.high, bar.low, bar.close);
}
```

**Get Exact Bar At Timestamp:**
```rust
pub fn get_bar_at(&self, instrument_id: u32, timestamp: SimTime) -> Option<Bar>
```

**Example:**
```rust
let now = ctx.now();
if let Some(bar) = ctx.market_data.get_bar_at(option_id, now) {
    println!("Exact option close at {} is {}", now, bar.close);
}
```

**Bar Fields:**
- `timestamp: SimTime` - Candle open time (timezone-aware)
- `open, high, low, close: i64` - OHLC prices (scaled by PRICE_SCALE)
- `volume: i64` - Trading volume

**Get Instrument ID by Name:**
```rust
pub fn get_id(&self, symbol: &str) -> Option<u32>
```

### Exact Monthly Expiry Selection Example

There is no dedicated `nearest_monthly_expiry()` helper in `Context` today. For monthly-expiry strategies, inspect option metadata and choose the last expiry date available in the nearest future expiry month.

Example pattern:

```rust
use std::collections::BTreeMap;

fn nearest_monthly_expiry(ctx: &Context, underlying: &str, today: i32) -> Option<i32> {
    let mut month_to_last_expiry: BTreeMap<(i32, u32), i32> = BTreeMap::new();

    for (instrument_id, _) in ctx.market_data.iter_ids() {
        let instrument = ctx.market_data.get_instrument(instrument_id)?;
        let option = instrument.option?;

        if !option.underlying.eq_ignore_ascii_case(underlying) {
            continue;
        }
        if option.expiry_yyyymmdd <= today {
            continue;
        }

        let year = option.expiry_yyyymmdd / 10_000;
        let month = ((option.expiry_yyyymmdd / 100) % 100) as u32;
        month_to_last_expiry
            .entry((year, month))
            .and_modify(|current| {
                if option.expiry_yyyymmdd > *current {
                    *current = option.expiry_yyyymmdd;
                }
            })
            .or_insert(option.expiry_yyyymmdd);
    }

    month_to_last_expiry.into_values().min()
}
```

This is the pattern used by `banknifty_orb_btst` for monthly BankNifty options.

---

## Position & PnL Queries

Monitor positions and profitability.

### Open Positions

```rust
pub fn open_positions(&self) -> Vec<(u32, i64, i64)> // (instrument_id, quantity, avg_cost)
```

### Position Quantity

```rust
pub fn position_qty(&self, instrument_id: u32) -> i64
```

### Average Cost

```rust
pub fn position_avg_cost(&self, instrument_id: u32) -> Option<i64>
```

### PnL Queries

```rust
pub fn mtm_pnl(&self) -> i64                                  // Mark-to-market PnL
pub fn realized_pnl(&self) -> i64                             // Already locked in
pub fn unrealized_pnl(&self) -> i64                           // Open position PnL
pub fn instrument_realized_pnl(&self, id: u32) -> i64         // Per instrument
pub fn instrument_unrealized_pnl(&self, id: u32) -> i64       // Per instrument
```

### Example: Position Monitoring

```rust
fn on_market_event(&mut self, ctx: &mut Context, _event: &MarketEvent) {
    for (instr_id, qty, avg_cost) in ctx.open_positions() {
        let unrealized = ctx.instrument_unrealized_pnl(instr_id);
        if unrealized < -100_000 { // Loss threshold
            // Cut position
            ctx.place_order(instr_id, Side::Sell, OrderType::Market, qty);
        }
    }
}
```

---

## Constants

```rust
pub const PRICE_SCALE: i64 = 10_000;  // All prices are multiplied by this factor
```

All price inputs and outputs use this scale. Example: To represent price 3000.50, use `3000_50 * PRICE_SCALE / 100` or `30005000`.

