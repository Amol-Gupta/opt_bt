# Option Buying Strategy 1

## Concept

A pricing model pre-computes an entry limit price (`Entry_Price`) and a target exit limit price (`Target_price`) for a universe of deep OTM options. Each trading day:

1. **9:10 AM** — Place limit buy orders for every contract in today's CSV at `Entry_Price`.
2. **On fill** — Immediately place a limit sell (target) order at `Target_price`.
3. **9:30 AM** — Cancel all unfilled entry orders **and** any pending target orders. Close any open positions with a market sell. That is the end of trading for the day.
4. **After close** — Bookkeeping: accumulate daily metrics (total orders placed, fills, target hits, PnL). Can also be derived from fill events.

---

## Input Data

CSV files live in a tuned-parameter folder, one file per trading day:

```
/quant/penny_options/tuned/DTE8_CR30_DN60_DW40_DC40/
    BUY02JAN2026.csv
    BUY03JAN2026.csv
    ...
```

File naming convention: `BUY{DDMMMYYYY}.csv` (e.g. `BUY02JAN2026.csv`).

### CSV Columns

| Column        | Description |
|---------------|-------------|
| `Symbol`      | Underlying name (e.g. `NIFTY`) |
| `Expiry Date` | Option expiry in `YYYY-MM-DD` format (e.g. `2026-01-06`) |
| `Strike Price` | Strike in points |
| `Option Type` | `CE` or `PE` |
| `Quantity`    | Number of units (not lots) |
| `Price`       | Last market price at signal-generation time — **intermediate value, not used for execution** |
| `Entry_Price` | Tuned limit buy price — the actual order price used for entry |
| `Target_price` | Tuned limit sell price — placed immediately after a fill |
| `ATM_Strike`  | ATM strike at signal time |
| `ATM_Distance` | Distance from ATM (`Strike - ATM_Strike`) |
| `Today_Strike` | Nearest listed strike to the requested strike |
| `IV`          | Implied volatility at signal time |
| `MARGIN`      | Approximate margin per lot |

**Sample rows (actual format):**

| Symbol | Expiry Date | Strike Price | Option Type | Quantity | Price | Target_price | ATM_Strike | ATM_Distance | Today_Strike | IV    | Entry_Price | MARGIN |
|--------|-------------|--------------|-------------|----------|-------|--------------|------------|--------------|--------------|-------|-------------|--------|
| NIFTY  | 2026-01-06  | 25500        | PE          | 65       | 1.1   | 3.2          | 26150      | -650         | 25500        | 0.100 | 0.56        | 71.5   |
| NIFTY  | 2026-01-06  | 25550        | PE          | 65       | 1.2   | 4.1          | 26150      | -600         | 25550        | 0.095 | 0.63        | 78.0   |

---

## Instrument ID Resolution

The backtest engine identifies instruments by ticker string. Construct the ticker as:

```
{SYMBOL}{DDMMMYY}{STRIKE}{CE/PE}
```

Example: `Symbol=NIFTY`, `Expiry Date=2026-01-06`, `Strike=25500`, `Option Type=PE`
→ ticker: **`NIFTY06JAN2625500PE`**

Use `ctx.market_data.get_id("NIFTY06JAN2625500PE")` to resolve to an `InstrumentId`.

---

## Configuration (bt.toml strategy params)

The CSV folder path is passed as a strategy parameter so it can be overridden per run:

```toml
[strategy.params]
csv_folder = "/quant/penny_options/tuned/DTE8_CR30_DN60_DW40_DC40"
```

---

## Additional Metrics

At end of backtest, print:

```
Tuned Folder: DTE8_CR30_DN60_DW40_DC40

Total Orders:        14761    # limit buy orders placed
Filled Orders:         837    # entry orders that got filled
Target Hit Count:      421    # positions where target limit sell was hit
Entry Fill Rate %:    5.67%
Target Hit Rate %:   50.30%   # target hits / filled entries
Total PnL:          9161.05

Strategy vs Benchmark (Cash)
              Strategy
Total Return     9.16%
Sharpe Ratio     19.96
Max Drawdown    -0.67%
Win Rate        65.26%
Total Trades       190
Profit Factor     5.47
```

> `Target Hit Rate %` is the fraction of *filled* entries where the target was reached, not total orders placed.
