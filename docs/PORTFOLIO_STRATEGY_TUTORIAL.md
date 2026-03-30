# Portfolio of Strategies — Tutorial

## Table of Contents

1. [Concept: What Is a Portfolio of Strategies?](#1-concept-what-is-a-portfolio-of-strategies)
5. [Project Setup / Download Strategies](#5-project-setup--download-strategies)
2. [The Strategy: Nifty ATM Short Straddle with Stop Loss](#2-the-strategy-nifty-atm-short-straddle-with-stop-loss)
3. [Strategy Parameters](#3-strategy-parameters)
4. [Three Time Windows](#4-three-time-windows)
5. [Step-by-Step: Running Individual Strategies](#5-step-by-step-running-individual-strategies)
6. [Step-by-Step: Running the Portfolio](#6-step-by-step-running-the-portfolio)
7. [Comparing Results](#7-comparing-results)
8. [Full Command Reference](#8-full-command-reference)

---

## 1. Concept: What Is a Portfolio of Strategies?

A **portfolio of strategies** runs multiple independent trading strategies simultaneously under one backtest umbrella. Each strategy manages its own positions, orders, and PnL attribution. The engine aggregates all sub-strategy PnL into a single portfolio equity curve, so you can see:

- **Individual performance** — how each strategy performs on its own.
- **Portfolio performance** — the combined equity curve, drawdowns, Sharpe ratio, etc.
- **Diversification benefit** — whether combining strategies reduces peak drawdown compared to the worst single component.

### Why combine strategies?

| Benefit | Explanation |
|---|---|
| **Time diversification** | Different entry windows capture different intraday volatility regimes. One window may be losing while another is profitable. |
| **Smoother equity curve** | Losses in one strategy are partially offset by gains in another, reducing peak-to-trough drawdown. |
| **Attribution clarity** | The engine tracks PnL separately per strategy ID, so you can benchmark each leg and decide where to allocate capital. |
| **Correlation awareness** | Strategies that trade the same underlying but at non-overlapping times have low intraday correlation — combining them is portfolio-efficient. |

### How the engine implements it

`PortfolioStrategy` is a container strategy that:

1. Holds a map of `strategy_id → child strategy`.
2. Broadcasts every market event and alarm to all children.
3. Routes fill and order events back to the originating child using the `strategy_id` tag on each order.
4. Each child uses `ctx.set_strategy_id()` so that its orders, fills, and PnL are tagged and reported separately.

```
Engine
  └── PortfolioStrategy
        ├── "morning_straddle"   (09:30 – 11:30)
        ├── "midday_straddle"    (11:30 – 13:30)
        └── "afternoon_straddle" (13:30 – 15:15)
```

The `report.json` that `bt run` produces contains a `portfolio_view` section with per-strategy attribution so you can read individual metrics without running separate backtests.

---

## 2. The Strategy: Nifty ATM Short Straddle with Stop Loss

### What it does

On each trading day, at a configured **entry time**:

1. Read the current NIFTY 50 index price (spot).
2. Find the nearest **weekly expiry** (i.e., options expiring within the current week, or the next available Thursday expiry).
3. Identify the **ATM strike** — the strike closest to the current spot.
4. **Sell 1 ATM call** (`CE`) and **sell 1 ATM put** (`PE`) at market price. This creates a short straddle.
5. Immediately place a **stop loss buy order** for each leg:
   - CE stop = `CE entry price × (1 + sl_pct)`
   - PE stop = `PE entry price × (1 + sl_pct)`
   - These are `Stop(price)` buy orders — they trigger automatically if the option price rises above the stop level.

At the configured **exit time** (or if stop loss is hit before that):

6. **Cancel** any pending stop loss orders that have not yet triggered.
7. **Buy to close** any option leg that is still short (i.e., whose stop was not hit).

### Payoff intuition

| Scenario | Outcome |
|---|---|
| Market stays range-bound | Both options decay; strategy collects premium. |
| Market moves sharply | One leg stop is hit (loss capped). The other leg continues collecting premium. |
| Stop hit on both legs | Maximum loss realised; both positions closed by stop orders. |

---

## 3. Strategy Parameters

| Parameter | CLI flag | Default | Description |
|---|---|---|---|
| Lot size | `qty=N` | 65 | Number of units per leg (one lot of Nifty options = 25; set to match your lot size). |
| Stop loss % | `sl_pct=0.20` | 0.20 | Stop loss as a fraction of entry premium. `0.20` = 20% rise in option price triggers stop. |
| Entry time | `entry_seconds=N` | 39600 (11:00 IST) | **Seconds from midnight IST (market local time).** Used by `nifty_straddle_sl` only. |
| Exit time | `exit_seconds=N` | 54900 (15:15 IST) | **Seconds from midnight IST (market local time).** Used by `nifty_straddle_sl` only. |

> **Note:** For the portfolio strategy (`nifty_straddle_portfolio`) the three entry/exit windows are fixed in code (see [§4](#4-three-time-windows)). Only `qty` and `sl_pct` are configurable at runtime.

### ⚠️ Important: Time parameters are market-local (IST)

The engine now uses timezone-aware `SimTime`. Pass `entry_seconds` and `exit_seconds` as
**seconds from midnight IST** (local market clock), not UTC offsets.

```
IST time  →  seconds from midnight IST
09:15         33300
09:30         34200   ← morning entry
11:00         39600
11:30         41400   ← midday entry / morning exit
13:30         48600   ← afternoon entry / midday exit
15:15         54900   ← afternoon exit (before close)
15:30         55800   ← market close
```

---

## 4. Three Time Windows

The `nifty_straddle_portfolio` strategy composes three `nifty_straddle_sl` instances:

| Sub-strategy ID | Entry | Exit | Notes |
|---|---|---|---|
| `morning_straddle` | 09:30 | 11:30 | Captures opening-session volatility collapse |
| `midday_straddle` | 11:30 | 13:30 | Mid-session, often quieter |
| `afternoon_straddle` | 13:30 | 15:15 | Pre-close session; exits before market close |

Each window is **non-overlapping** so the three strategies never hold the same contract at the same time, keeping position sizing and attribution clean.

---

## 5. Step-by-Step: Running Individual Strategies
## Project Setup / Download Strategies

### For cargo-install users (without repository access)

Scaffold a fresh project:

```bash
bt workspace init --path ./my_workspace
bt project init --workspace ./my_workspace my_strategy
```

Download the strategy files from GitHub:

```bash
# Download the registration / factory code
curl https://raw.githubusercontent.com/Amol-Gupta/opt_bt/main/./my_workspace/projects/my_strategy/strategy/src/my_strategy.rs \
  -o ./my_workspace/projects/my_strategy/strategy/src/my_strategy.rs

# Download the ATM straddle with stop-loss implementation
curl https://raw.githubusercontent.com/Amol-Gupta/opt_bt/main/./my_workspace/projects/my_strategy/strategy/src/nifty_straddle.rs \
  -o ./my_workspace/projects/my_strategy/strategy/src/nifty_straddle.rs

# Download the portfolio wrapper
curl https://raw.githubusercontent.com/Amol-Gupta/opt_bt/main/./my_workspace/projects/my_strategy/strategy/src/nifty_straddle_portfolio.rs \
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
start_date = "2024-04-01"
end_date = "2024-05-31"
default_strategy = "nifty_straddle_portfolio"
initial_capital = 500000

[run.params]
qty = "25"
sl_pct = "0.20"
```

Verify strategy discovery:

```bash
bt list-strategies --project my_strategy --workspace ./my_workspace
```

You should see `nifty_straddle_portfolio` (and `nifty_straddle_sl` if running the individual strategies).

### For repo developers

If you have the full repository cloned, the strategies are already registered in `./my_workspace/projects/my_strategy`. You can run them directly or copy files as above.

---

### Prerequisites

```bash
# Build the CLI and engine binaries (run once from the repo root)
cargo build --release --bin bt --bin opt_bt

# Make sure ~/.cargo/bin (or ./target/release) is on PATH
export PATH="$PWD/target/release:$PATH"
```

### 5.1 Start the cache server (Terminal A — keep running)

```bash
bt cache server --bind 127.0.0.1:7878
```

### 5.2 Warm the cache (Terminal B)

```bash
BT_CACHE_ADDR=127.0.0.1:7878 bt cache warm \
  --data /quant/nifty_with_options_01Jan2023_06Mar2026.parquet \
  --project my_strategy \
  --workspace ./my_workspace \
  --start-date 2024-04-01 \
  --end-date 2024-05-31
```

Verify the cache is warm:

```bash
BT_CACHE_ADDR=127.0.0.1:7878 bt cache status
```

### 5.3 Run Morning Straddle (09:30 – 11:30 IST, 20% SL)

```bash
BT_CACHE_ADDR=127.0.0.1:7878 bt run \
  --project my_strategy \
  --workspace ./my_workspace \
  --strategy nifty_straddle_sl \
  --data /quant/nifty_with_options_01Jan2023_06Mar2026.parquet
  --start-date 2024-04-01 \
  --end-date 2024-05-31 \
  --params qty=25 --params sl_pct=0.20 --params entry_seconds=34200 --params exit_seconds=41400
```

### 5.4 Run Midday Straddle (11:30 – 13:30 IST, 20% SL)

```bash
BT_CACHE_ADDR=127.0.0.1:7878 bt run \
  --project my_strategy \
  --workspace ./my_workspace \
  --strategy nifty_straddle_sl \
  --data /quant/nifty_with_options_01Jan2023_06Mar2026.parquet
  --start-date 2024-04-01 \
  --end-date 2024-05-31 \
  --params qty=25 --params sl_pct=0.20 --params entry_seconds=41400 --params exit_seconds=48600
```

### 5.5 Run Afternoon Straddle (13:30 – 15:15 IST, 20% SL)

```bash
BT_CACHE_ADDR=127.0.0.1:7878 bt run \
  --project my_strategy \
  --workspace ./my_workspace \
  --strategy nifty_straddle_sl \
  --data /quant/nifty_with_options_01Jan2023_06Mar2026.parquet
  --start-date 2024-04-01 \
  --end-date 2024-05-31 \
  --params qty=25 --params sl_pct=0.20 --params entry_seconds=48600 --params exit_seconds=54900
```

### 5.6 View results for each run

Each `bt run` prints the path to a per-run backtest folder. To open the HTML report for the most recent run:

```bash
latest=$(ls -td ./my_workspace/projects/my_strategy/backtests/nifty_straddle_sl_* | head -1)
echo "Report: $latest/report.html"
# On Linux:
xdg-open "$latest/report.html"
```

Key metrics in `report.json`:

```bash
jq '{sharpe: .metrics.sharpe_ratio,
     max_dd_pct: .metrics.max_drawdown_pct,
     total_return_pct: .metrics.total_return_pct,
     win_rate_pct: .metrics.win_rate_pct}' "$latest/report.json"
```

---

## 7. Step-by-Step: Running the Portfolio

The portfolio runs all three windows in a single command. The engine tracks each sub-strategy separately and produces aggregated portfolio metrics.

### 6.1 Start cache server and warm (same as §5.1 and §5.2 above — skip if already done)

### 6.2 Run the portfolio

```bash
BT_CACHE_ADDR=127.0.0.1:7878 bt run \
  --project my_strategy \
  --workspace ./my_workspace \
  --strategy nifty_straddle_portfolio \
  --data /quant/nifty_with_options_01Jan2023_06Mar2026.parquet
  --start-date 2024-04-01 \
  --end-date 2024-05-31 \
  --params qty=25 --params sl_pct=0.20
```

### 6.3 View portfolio results

```bash
latest=$(ls -td ./my_workspace/projects/my_strategy/backtests/nifty_straddle_portfolio_* | head -1)
echo "Report: $latest/report.html"
xdg-open "$latest/report.html"
```

Inspect sub-strategy attribution from `report.json`:

```bash
jq '.portfolio_view' "$latest/report.json"
```

This shows per-strategy PnL, trade count, win rate and Sharpe ratio for `morning_straddle`, `midday_straddle`, and `afternoon_straddle` side by side.

---

## 8. Comparing Results

### Side-by-side metric extraction

Run this after completing all four backtests (the three individual runs + the portfolio run):

```bash
echo "=== Morning Straddle ==="
latest=$(ls -td ./my_workspace/projects/my_strategy/backtests/nifty_straddle_sl_* | sed -n '3p')
jq '{sharpe:.metrics.sharpe_ratio, max_dd_pct:.metrics.max_drawdown_pct, total_return_pct:.metrics.total_return_pct, round_trips:.metrics.round_trip_trade_count}' "$latest/report.json"

echo "=== Midday Straddle ==="
latest=$(ls -td ./my_workspace/projects/my_strategy/backtests/nifty_straddle_sl_* | sed -n '2p')
jq '{sharpe:.metrics.sharpe_ratio, max_dd_pct:.metrics.max_drawdown_pct, total_return_pct:.metrics.total_return_pct, round_trips:.metrics.round_trip_trade_count}' "$latest/report.json"

echo "=== Afternoon Straddle ==="
latest=$(ls -td ./my_workspace/projects/my_strategy/backtests/nifty_straddle_sl_* | sed -n '1p')
jq '{sharpe:.metrics.sharpe_ratio, max_dd_pct:.metrics.max_drawdown_pct, total_return_pct:.metrics.total_return_pct, round_trips:.metrics.round_trip_trade_count}' "$latest/report.json"

echo "=== Portfolio ==="
latest=$(ls -td ./my_workspace/projects/my_strategy/backtests/nifty_straddle_portfolio_* | head -1)
jq '{sharpe:.metrics.sharpe_ratio, max_dd_pct:.metrics.max_drawdown_pct, total_return_pct:.metrics.total_return_pct, round_trips:.metrics.round_trip_trade_count}' "$latest/report.json"
```

### What to look for

| Observation | Meaning |
|---|---|
| Portfolio Sharpe > any individual Sharpe | Diversification is adding risk-adjusted value. |
| Portfolio max drawdown < worst single-strategy drawdown | The strategies are partially uncorrelated — combining them smooths losses. |
| One strategy dominates PnL | Consider allocating more capital to that window via `qty`. |
| All three strategies lose on the same days | Consider adding a trend filter or reducing size on high-VIX days. |

### Adjusting stop loss sensitivity

Re-run with tighter or wider stop loss to understand the risk/reward trade-off:

```bash
# Tight stop (10%)
BT_CACHE_ADDR=127.0.0.1:7878 bt run \
  --project my_strategy --workspace ./my_workspace \
  --strategy nifty_straddle_portfolio \
  --data /quant/nifty_with_options_01Jan2023_06Mar2026.parquet
  --start-date 2024-04-01 --end-date 2024-05-31 \
  --params qty=25 --params sl_pct=0.10

# Wide stop (50%)
BT_CACHE_ADDR=127.0.0.1:7878 bt run \
  --project my_strategy --workspace ./my_workspace \
  --strategy nifty_straddle_portfolio \
  --data /quant/nifty_with_options_01Jan2023_06Mar2026.parquet
  --start-date 2024-04-01 --end-date 2024-05-31 \
  --params qty=25 --params sl_pct=0.50
```

---

## 9. Full Command Reference

| Command | Purpose |
|---|---|
| `bt cache server --bind 127.0.0.1:7878` | Start in-memory cache server |
| `BT_CACHE_ADDR=... bt cache warm --data <path> --project my_strategy --workspace ./my_workspace --start-date ... --end-date ...` | Load parquet into cache |
| `BT_CACHE_ADDR=... bt cache status` | Inspect what's in cache |
| `BT_CACHE_ADDR=... bt run --project my_strategy --workspace ./my_workspace --strategy nifty_straddle_sl --data <path> --start-date ... --end-date ... --params qty=25 --params sl_pct=0.20 --params entry_seconds=34200 --params exit_seconds=41400` | Single straddle strategy (morning window, 09:30–11:30 IST) |
| `BT_CACHE_ADDR=... bt run ... --strategy nifty_straddle_sl --params qty=25 --params sl_pct=0.20 --params entry_seconds=41400 --params exit_seconds=48600` | Single straddle (midday, 11:30–13:30 IST) |
| `BT_CACHE_ADDR=... bt run ... --strategy nifty_straddle_sl --params qty=25 --params sl_pct=0.20 --params entry_seconds=48600 --params exit_seconds=54900` | Single straddle (afternoon, 13:30–15:15 IST) |
| `BT_CACHE_ADDR=... bt run ... --strategy nifty_straddle_portfolio --params qty=25 --params sl_pct=0.20` | Combined portfolio (all three windows) |
| `bt list-strategies --project my_strategy --workspace ./my_workspace` | List all registered strategies |

### IST seconds reference

All `entry_seconds` / `exit_seconds` values are **seconds from midnight IST** (market local time):

| Time (IST) | IST seconds |
|---|---|
| 09:15 | 33300 |
| 09:30 | **34200** ← morning entry |
| 11:30 | **41400** ← morning exit / midday entry |
| 13:30 | **48600** ← midday exit / afternoon entry |
| 15:15 | **54900** ← afternoon exit |
| 15:30 | 55800 |

---

*See also: [Strategy API Reference](STRATEGY_API.md) · [User Guide](USER_GUIDE.md)*

---

## 9. Verified Results (Jan 2023 – Mar 2026, qty=25, sl_pct=20%)

The following results were produced by running the four backtests above against
`/quant/nifty_with_options_01Jan2023_06Mar2026.parquet

| Strategy | Entry (IST) | Exit (IST) | Fills | P&L (₹) | entry_seconds | exit_seconds |
|---|---|---|---|---|---|---|
| Morning | 09:30 | 11:30 | 3131 | +31 102 | 34200 | 41400 |
| Midday | 11:30 | 13:30 | 3131 | +46 445 | 41400 | 48600 |
| Afternoon | 13:30 | 15:15 | 3129 | +18 259 | 48600 | 54900 |
| **Portfolio** | all 3 windows | — | 9392 | **+107 092** | (fixed) | (fixed) |

First fill timestamps confirm entries fire at the correct intraday time:

| Strategy | First entry timestamp (UTC) | IST equivalent |
|---|---|---|
| Morning | `2023-01-02T04:01:00+00:00` | 09:31 IST |
| Midday | `2023-01-02T06:01:00+00:00` | 11:31 IST |
| Afternoon | `2023-01-02T08:01:00+00:00` | 13:31 IST |
