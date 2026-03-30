# Nifty Weekly Premium Straddle — Strategy & Tutorial

## Table of Contents

1. [Strategy Concept](#strategy-concept)
2. [How It Differs from ATM Straddle](#how-it-differs-from-atm-straddle)
3. [Strategy Logic — Step by Step](#strategy-logic--step-by-step)
4. [Parameters Reference](#parameters-reference)
5. [Prerequisites](#prerequisites)
6. [Project Setup](#project-setup)
7. [Strategy Code Walkthrough](#strategy-code-walkthrough)
8. [Running a Single Backtest](#running-a-single-backtest)
9. [Parameter Sweep](#parameter-sweep)
10. [Viewing the Report](#viewing-the-report)
11. [Common Mistakes](#common-mistakes)

---

## Strategy Concept

The **Nifty Weekly Premium Straddle** (`nifty_premium_straddle`) is a delta-neutral short options
strategy.  Instead of selling the at-the-money (ATM) strike (i.e. the strike nearest to spot), it
sells the **call and put options whose current market premium is closest to a target price (CP)**.

This lets you control the exact credit received per leg, which directly determines:

- The maximum profit (total premium collected).
- Where the breakeven points lie.
- The strike distance from spot.

### Typical use-cases

| CP (₹) | Effect |
|--------|--------|
| Very low (e.g. ₹20) | Far OTM strangle — low premium, high probability of profit but poor risk/reward |
| Medium (e.g. ₹100) | Balanced strangle / near-ATM straddle |
| High (e.g. ₹200+)  | Deep ITM or near-ATM straddle — large premium, tighter breakevens |

---

## How It Differs from ATM Straddle

| Feature | ATM Straddle (`algotest_weekly_straddle`) | Premium Straddle (`nifty_premium_straddle`) |
|---------|-------------------------------------------|----------------------------------------------|
| Strike selection | Strike nearest to spot price | Strike whose option premium ≈ CP |
| Credit received | Variable (depends on IV) | Approximately `2 × CP` (controlled) |
| CE strike == PE strike | Always | Usually not (skew means CE ≠ PE premium) |
| Suitable for | Delta-neutral, IV mean reversion | Premium-targeting, defined-credit trades |

---

## Strategy Logic — Step by Step

### Entry (at `entry_seconds` each trading day)

1. Determine the **nearest current weekly expiry** (options expiring within the next 0–7 calendar
   days).
2. Scan all call options for that expiry; pick the one whose last bar close price is **closest to
   `target_premium`**.
3. Scan all put options for that expiry; pick the one whose last bar close price is **closest to
   `target_premium`**.
4. **Sell** both options at market.
5. **Immediately place a buy-stop** on each leg:
   - Stop price = `entry_price × (1 + sl_pct)`
   - If the option price rises above this level (loss), the stop triggers and closes the leg
     automatically.

### During the Day

- No further action is taken unless a stop-loss order is triggered.
- When a stop-loss fills, the leg is marked closed; the other leg continues independently.

### Exit (at `exit_seconds` each trading day)

1. **Cancel** any remaining pending stop-loss orders.
2. **Buy back** any leg that is still short (net short position < 0).

---

## Parameters Reference

| Parameter | CLI key | Type | Default | Description |
|-----------|---------|------|---------|-------------|
| Entry time | `entry_seconds` | integer | `39600` (11:00 IST) | Seconds since midnight IST (market local time) when entry alarm fires |
| Exit time | `exit_seconds` | integer | `54900` (15:15 IST) | Seconds since midnight IST (market local time) when exit alarm fires |
| Target premium (CP) | `target_premium` | integer (₹) | `100` | Target option premium in whole rupees; engine multiplies by `PRICE_SCALE` internally |
| Stop loss % | `sl_pct` | float | `0.20` | Fraction above entry price that triggers the buy-stop (e.g. `0.30` = 30 %) |
| Quantity | `qty` | integer | `65` | Number of units / lots per leg |

### IST seconds quick-reference

```
09:30 → 34200    10:00 → 36000    10:30 → 37800
11:00 → 39600    11:30 → 41400    12:00 → 43200
14:00 → 50400    15:00 → 54000    15:15 → 54900
15:25 → 55500    15:29 → 55740
```

---

## Prerequisites

1. **Build the `bt` and `opt_bt` binaries** (one-time):

   ```bash
   cd /home/amol/opt_bt          # repository root
   cargo build --release --bin bt --bin opt_bt
   # Binaries land in target/release/; ensure they are on PATH or use full paths
   export PATH="$PWD/target/release:$PATH"
   ```

2. **Verify the data file exists**:

   ```bash
   ls -lh /quant/nifty_with_options_01Jan2023_06Mar2026.parquet
   ```

3. **Warm the cache** (strongly recommended before running multiple backtests):

   ```bash
   # Start the cache server in the background
   bt cache server --bind 127.0.0.1:7878 &
   export BT_CACHE_ADDR=127.0.0.1:7878

   # Warm the full date range once
   bt cache warm \
     --data /quant/nifty_with_options_01Jan2023_06Mar2026.parquet \
     --start-date 2023-01-01 \
     --end-date   2026-03-06

   bt cache status
   ```

---

## Project Setup

### For cargo-install users (without repository access)

Scaffold a fresh project:

```bash
bt workspace init --path ./my_workspace
bt project init --workspace ./my_workspace my_strategy
```

Download the strategy files from GitHub:

```bash
# Download the registration / factory code
curl https://raw.githubusercontent.com/Amol-Gupta/opt_bt/main/demo_ws/projects/my_strategy/strategy/src/my_strategy.rs \
  -o ./my_workspace/projects/my_strategy/strategy/src/my_strategy.rs

# Download the premium straddle implementation
curl https://raw.githubusercontent.com/Amol-Gupta/opt_bt/main/demo_ws/projects/my_strategy/strategy/src/nifty_premium_straddle.rs \
  -o ./my_workspace/projects/my_strategy/strategy/src/nifty_premium_straddle.rs

# Optional: Download helper strategies
curl https://raw.githubusercontent.com/Amol-Gupta/opt_bt/main/demo_ws/projects/my_strategy/strategy/src/nifty_straddle.rs \
  -o ./my_workspace/projects/my_strategy/strategy/src/nifty_straddle.rs

curl https://raw.githubusercontent.com/Amol-Gupta/opt_bt/main/demo_ws/projects/my_strategy/strategy/src/nifty_straddle_portfolio.rs \
  -o ./my_workspace/projects/my_strategy/strategy/src/nifty_straddle_portfolio.rs
```

Update `Cargo.toml` in the strategy crate:

```bash
cd ./my_workspace/projects/my_strategy/strategy
# Add chrono dependency (required by strategies)
# Edit Cargo.toml and add: chrono = "0.4" under [dependencies]
cargo check
```

Update `bt.toml` in the project:

```toml
[run]
data = "/quant/nifty_with_options_01Jan2023_06Mar2026.parquet"
start_date = "2024-01-04"
end_date = "2024-01-04"
default_strategy = "nifty_premium_straddle"
initial_capital = 500000

[run.params]
qty = "65"
sl_pct = "0.20"
target_premium = "100"
entry_seconds = "39600"
exit_seconds = "54900"
```

Verify strategy discovery:

```bash
bt list-strategies --project my_strategy --workspace ./my_workspace
```

You should see `nifty_premium_straddle` in the output.

### For repo developers

If you have the full repository cloned, the `nifty_premium_straddle` strategy is already registered in `demo_ws/projects/my_strategy`. You can run it directly or copy files as above.

### Capital requirement

Selling a Nifty short straddle (1 CE + 1 PE, naked) requires approximately **₹2.2 lac margin per leg**.

| Legs | Margin per leg | Minimum capital | Recommended capital |
|------|---------------|-----------------|---------------------|
| 1 lot short straddle (qty=65) | ₹2.2 lac | ₹4.4 lac | ₹5.0 lac |

Use `initial_capital = 500000` in `bt.toml` (or pass via CLI) for 1 lot.  Scale linearly for multiple lots.

### Compile / sanity check

```bash
cd ./my_workspace/projects/my_strategy/strategy && cargo check
```

Verify strategy discovery:

```bash
bt list-strategies --project my_strategy --workspace ./my_workspace
```

Expected output includes `nifty_premium_straddle`.

---

## Strategy Code Walkthrough

The strategy is implemented in:

```
./my_workspace/projects/my_strategy/strategy/src/nifty_premium_straddle.rs
```

And registered/parameter-wired in:

```
./my_workspace/projects/my_strategy/strategy/src/my_strategy.rs
```

### Key struct

```rust
pub struct NiftyPremiumStraddleStrategy {
    index_symbol: String,          // "NIFTY 50" — used to read spot price
    option_underlying: String,     // "NIFTY" — prefix match on option.underlying
    quantity: i64,
    entry_seconds: i64,            // seconds since midnight IST for entry alarm
    exit_seconds: i64,             // seconds since midnight IST for exit alarm
    stop_loss_ratio: f64,          // e.g. 0.30 for 30 %
    target_premium: i64,           // rupees × PRICE_SCALE
    entered_days: HashSet<i64>,    // day keys already traded
    day_state: HashMap<i64, DailyState>,  // per-day CE/PE ids and stop orders
}
```

`SimTime` is timezone-aware in the engine, so these values are interpreted on the market's local
clock (IST), not as UTC offsets.

### Strike selection: `resolve_pair_by_premium`

```rust
// For each option in the nearest weekly expiry:
//   distance = |option_bar.close - target_premium|
// Pick the CE with minimum distance and the PE with minimum distance.
```

Both legs are selected independently, so the CE and PE strikes may differ (reflecting IV skew).

### Entry flow

```rust
// 1. Sell CE at market
ctx.place_order(ce_id, Side::Sell, OrderType::Market, qty);
// 2. Sell PE at market
ctx.place_order(pe_id, Side::Sell, OrderType::Market, qty);
// 3. Place CE stop-loss
let ce_sl = (ce_entry_price * (1 + sl_pct)) as i64;
ctx.place_order(ce_id, Side::Buy, OrderType::Stop(ce_sl), qty);
// 4. Place PE stop-loss
let pe_sl = (pe_entry_price * (1 + sl_pct)) as i64;
ctx.place_order(pe_id, Side::Buy, OrderType::Stop(pe_sl), qty);
```

### Exit flow

```rust
// 1. Cancel pending stops
ctx.cancel_order(state.ce_stop_order_id);
ctx.cancel_order(state.pe_stop_order_id);
// 2. Close remaining open legs
if ctx.position_qty(state.ce_id) < 0 {
    ctx.place_order(state.ce_id, Side::Buy, OrderType::Market, qty);
}
if ctx.position_qty(state.pe_id) < 0 {
    ctx.place_order(state.pe_id, Side::Buy, OrderType::Market, qty);
}
```

---

## Running a Single Backtest

> **Capital note:** `initial_capital` is set in `bt.toml` (not a CLI flag).  The project is
> configured with `initial_capital = 500000` (₹5 lac), which covers the ~₹4.4 lac margin
> required to sell 1 lot (qty=65) of Nifty CE + PE naked.  Scale capital proportionally for
> larger qty.

### Example 1 — Default parameters (11:00 entry, 15:15 exit, ₹100 CP, 20 % SL)

```bash
BT_CACHE_ADDR=127.0.0.1:7878 bt run \
  --project   my_strategy \
  --workspace ./my_workspace \
  --strategy  nifty_premium_straddle \
  --data      /quant/nifty_with_options_01Jan2023_06Mar2026.parquet \
  --start-date 2024-01-04 \
  --end-date   2024-01-04 \
  --params qty=65 \
  --params target_premium=100 \
  --params sl_pct=0.20 \
  --params entry_seconds=39600 \
  --params exit_seconds=54900
```

### Example 2 — 9:30 entry, 15:25 exit, ₹150 CP, 30 % SL

```bash
BT_CACHE_ADDR=127.0.0.1:7878 bt run \
  --project   my_strategy \
  --workspace ./my_workspace \
  --strategy  nifty_premium_straddle \
  --data      /quant/nifty_with_options_01Jan2023_06Mar2026.parquet \
  --start-date 2024-01-04 \
  --end-date   2024-01-04 \
  --params qty=65 \
  --params target_premium=150 \
  --params sl_pct=0.30 \
  --params entry_seconds=34200 \
  --params exit_seconds=55500
```

### Example 3 — 10:00 entry, 15:29 exit, ₹80 CP, 40 % SL

```bash
BT_CACHE_ADDR=127.0.0.1:7878 bt run \
  --project   my_strategy \
  --workspace ./my_workspace \
  --strategy  nifty_premium_straddle \
  --data      /quant/nifty_with_options_01Jan2023_06Mar2026.parquet \
  --start-date 2024-01-04 \
  --end-date   2024-01-04 \
  --params qty=65 \
  --params target_premium=80 \
  --params sl_pct=0.40 \
  --params entry_seconds=36000 \
  --params exit_seconds=55740
```

---

## Parameter Sweep

Use `bt sweep` with a JSON config to run many parameter combinations in one shot.

### Create sweep config — `sweep_premium_straddle.json`

```json
{
  "strategy": "nifty_premium_straddle",
  "data": "/quant/nifty_with_options_01Jan2023_06Mar2026.parquet",
  "start_date": "2024-01-04",
  "end_date":   "2024-01-31",
  "params": {
    "qty":            ["65"],
    "target_premium": ["80", "100", "150"],
    "sl_pct":         ["0.20", "0.30", "0.40"],
    "entry_seconds":  ["36000", "39600"],
    "exit_seconds":   ["54900", "55500"]
  }
}
```

This produces **3 × 3 × 2 × 2 = 36 backtests** covering different CP / SL / timing combinations.

### Run the sweep

```bash
BT_CACHE_ADDR=127.0.0.1:7878 \
bt sweep \
  --project   my_strategy \
  --workspace ./my_workspace \
  --config    sweep_premium_straddle.json
```

Results land in:

```
./my_workspace/projects/my_strategy/backtests/
└── nifty_premium_straddle_<timestamp>_<run_id>/
    ├── report.json
    └── report.html
```

---

## Viewing the Report

```bash
# Find the most recent backtest folder
latest=$(ls -td \
  /home/amol/opt_bt/./my_workspace/projects/my_strategy/backtests/nifty_premium_straddle_* \
  | head -1)

echo "Report: $latest/report.html"

# Open in browser (Linux)
xdg-open "$latest/report.html"
```

The HTML report contains:

- **Summary metrics**: Sharpe ratio, max drawdown, win rate, total PnL.
- **Equity curve** with benchmark overlay.
- **Trades table**: every fill with entry/exit price, PnL per trade.
- **Per-strategy attribution** (useful when run inside a portfolio).

---

## Common Mistakes

### No trades generated

**Symptom**: Report shows zero trades.

**Causes**:
- `entry_seconds` falls on a market holiday or outside market hours.
- `target_premium` is too low (e.g. ₹1) — no option with such a small premium may exist in the
  weekly chain.
- Weekly expiry window: the engine only looks for expirations **0–7 calendar days** away.  If the
  data contains monthly options only, no weekly expiry will be found.

**Debug**:

```bash
# Warm cache first, then spot-check option prices at entry time
BT_CACHE_ADDR=127.0.0.1:7878 bt data index \
  --data /quant/nifty_with_options_01Jan2023_06Mar2026.parquet \
  --symbol "NIFTY 50" \
  --date 2024-06-12 \
  --minute 11:00 \
  --window-minutes 1

BT_CACHE_ADDR=127.0.0.1:7878 bt data slice \
  --data /quant/nifty_with_options_01Jan2023_06Mar2026.parquet \
  --date 2024-06-12 \
  --time 11:00 \
  --center-strike 22500 \
  --points 10
```

### Stop-loss immediately triggers

**Symptom**: Both legs stop out every day.

**Cause**: `sl_pct` too tight (e.g. `0.05` on a volatile expiry day).

**Fix**: Raise `sl_pct` (e.g. `0.30`–`0.50`) or use `target_premium` values for farther OTM
options.

### CE and PE strikes differ

This is **expected behaviour**.  IV skew means the put wing is usually more expensive than the call
wing for the same strike distance.  To target the same premium on both sides, the CE will typically
be closer to ATM than the PE.

### `stop price already breached` warning in logs

This happens when the option gap-opens above the stop price on the bar immediately after entry (e.g.
on a high-volatility morning).  The engine rejects the stop order and logs a warning.  The position
is still open but unprotected — consider widening the stop or using a later entry time.
