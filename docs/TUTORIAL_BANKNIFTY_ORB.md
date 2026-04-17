# BTST Option Buy Strategy

## Overview

This is a 2-day option buying strategy on BankNifty monthly options. It is a long-only open range breakout strategy based on option premium. The strategy has two legs: one Call Option (CE) leg and one Put Option (PE) leg. Both legs are monitored independently, but each leg moves through the same three phases:

- Phase 1: Range formation
- Phase 2: Waiting for range breakout and entry
- Phase 3: Managing the trade after entry until exit

If a breakout never happens for a leg within the allowed waiting period, that leg is closed by time without taking any entry.

Because this is a BTST strategy, two strategy instances can be active on any given trading day:

- One instance carried forward from yesterday
- One fresh instance that starts today

This means the engine must support concurrent state for both the previous day's carry-forward logic and the current day's new setup logic.

The strategy should also be described in terms of parameters rather than fixed constants so the same logic can later be used for parameter sweeping.

## Implemented Strategy

This tutorial is now implemented as project strategy `banknifty_orb_btst` in:

- `demo_ws/projects/my_strategy/strategy/src/banknifty_orb_btst.rs`

The strategy is registered in:

- `demo_ws/projects/my_strategy/strategy/src/my_strategy.rs`

The current implementation uses BankNifty monthly options from the nearest available monthly expiry.
In code, that means the strategy finds the nearest future expiry month and then picks the last expiry date available in that month.

Monthly-expiry selection recipe:

1. Scan all option instruments for the configured underlying.
2. Ignore expiries on or before the current trade date so the session never selects a contract that expires the same day.
3. Group remaining expiries by `(year, month)`.
4. Within each month, keep the last available expiry date.
5. Choose the earliest remaining month and use that month's last expiry.

This is how the strategy arrives at expiries such as `20240131` for January 2024.

## Strategy Parameters

- `closest_premium_value`: target premium used to select the CE and PE strikes, for example 200
- `range_start_time`: time at which strike selection and range formation begin
- `range_end_time`: time at which the initial observation window ends and breakout monitoring begins
- `entry_cutoff_time`: last time until which breakout entry is allowed while still in the waiting phase
- `target_pct`: profit target percentage above entry price
- `stop_loss_pct`: stop-loss percentage below entry price
- `day2_exit_time`: force-exit time for open carry-forward positions on Day 2

For the current example narrative, the parameter values are:

- `closest_premium_value = 200`
- `range_start_time = 9:16 AM`
- `range_end_time = 11:30 AM`
- `target_pct = 30%`
- `stop_loss_pct = 25%`

In the current implementation, the CLI parameter keys are:

- `closest_premium_value`
- `range_start_seconds`
- `range_end_seconds`
- `entry_cutoff_seconds`
- `target_pct`
- `stop_loss_pct`
- `day2_exit_seconds`
- `qty`
- `index_symbol`
- `option_underlying`

All time parameters are passed as seconds from midnight IST.

For parameter sweeps and automation, prefer the real CLI keys over prose-only names.
Use these runtime names consistently in configs and sweep files:

- `closest_premium_value`
- `range_start_seconds`
- `range_end_seconds`
- `entry_cutoff_seconds`
- `target_pct`
- `stop_loss_pct`
- `day2_exit_seconds`

## Engine Execution Semantics Used By This Strategy

These points are important because they affect how the tutorial should be interpreted when mapped to the engine.

- Strike selection uses exact option bars at `range_start_time`, not stale bars from earlier timestamps.
- Range formation updates only when an exact option bar exists at that timestamp for that contract.
- Breakout is checked using `Close > Reference High` on exact option bars after `range_end_time`.
- On breakout, the strategy submits a market buy order, and in the current engine market orders fill at the same bar close.
- Exit checks begin only after the entry bar has completed.
- Target and stop-loss are evaluated from later exact option bars.
- If both target and stop-loss are touched in the same exit bar, the implementation uses conservative precedence and treats it as a stop-loss exit.
- If no breakout happens before `entry_cutoff_time`, that leg is marked closed without entry.

This means the tutorial phrases:

- "entry happens when close breaks the range"
- "exit on target or stop-loss"

should be interpreted as explicit bar-close rules in the current backtest engine, not intrabar tick-level behavior.


## Core Idea

At `range_start_time` on Day 1, identify:

- One Call Option (CE) strike whose premium is closest to `closest_premium_value`
- One Put Option (PE) strike whose premium is closest to `closest_premium_value`

Once these two strikes are selected at `range_start_time`, they remain fixed for that strategy instance for the day.

From `range_start_time` to `range_end_time`, observe the OHLC data for both selected option contracts and note the highest `High` made during that observation window.

After `range_end_time`, wait for the selected option premium to break above its recorded range high. If the `Close` of the selected option premium moves above its recorded highest `High`, enter a long trade in that option.

If no breakout happens before `entry_cutoff_time`, no trade is taken in that leg and the strategy stops monitoring that leg for entry.

This rule applies independently to the Call and Put sides.

## Step-by-Step Rules

### Phase 1. Range Formation

At `range_start_time` on Day 1:

- Find the CE strike with premium closest to `closest_premium_value`
- Find the PE strike with premium closest to `closest_premium_value`
- Freeze these two strikes for the rest of the strategy

Observation window:

- Start: `range_start_time`
- End: `range_end_time`

During this phase, for each selected option:

- Track Open, High, Low, and Close
- Record the maximum `High` reached during the window

Define:

- `Call Reference High` = highest `High` of the selected CE between `range_start_time` and `range_end_time`
- `Put Reference High` = highest `High` of the selected PE between `range_start_time` and `range_end_time`

At the end of Phase 1, the range is fully formed for both legs.

## Phase 2. Waiting for Range Breakout and Entry

After `range_end_time` on Day 1, each leg enters a waiting phase:

- Buy the selected CE if its `Close` goes above the `Call Reference High`
- Buy the selected PE if its `Close` goes above the `Put Reference High`

Notes:

- CE and PE are evaluated separately
- Both trades may trigger, only one may trigger, or neither may trigger
- The trigger is based on `1 min Close` crossing above the recorded reference high, not just an intraday spike
- This is still a long-only setup because entry happens only on upward premium breakout

If a breakout does not happen for a leg within the allowed waiting time up to `entry_cutoff_time`:

- No position is opened in that leg
- The leg is closed by time while still in the waiting state
- That leg does not move to Phase 3

## Phase 3. Post-Entry Exit Management

Once a breakout entry happens in any leg, that leg moves to the exit-management phase.

### Entry Price

The entry price is the option premium at the breakout bar close, because the current engine fills the market entry order on the same bar where the breakout is detected.

Example:

- If at 12:15 PM the CE premium closes above the `Call Reference High`, enter a buy trade in that CE at the prevailing entry price

### Risk Management

For every trade entered:

- Profit Target: `target_pct` above entry price
- Stop-Loss: `stop_loss_pct` below entry price

Formula:

- `Target Price = Entry Price x (1 + target_pct)`
- `Stop-Loss Price = Entry Price x (1 - stop_loss_pct)`

### Exit Rules on Day 1

After entry on Day 1:

- Exit on the first later bar where target is hit
- Exit on the first later bar where stop-loss is hit

If neither target nor stop-loss is hit on Day 1:

- Carry the open position forward to Day 2

### Exit Rules on Day 2

For any carried-forward position on Day 2:

- Exit if the target is hit
- Exit if the stop-loss is hit
- If neither is hit, liquidate the position at `day2_exit_time`

## BTST State Management Requirement

Since positions can be carried from Day 1 into Day 2, any trading day may have two live strategy instances running in parallel:

- Yesterday's instance, which is already in Phase 3 and is only being monitored for exit
- Today's instance, which starts from Phase 1 and may progress into Phase 2 and Phase 3

This overlap is a core behavior of the strategy, not an edge case. The system design must therefore maintain separate state, timestamps, selected strikes, entry status, and exit status for each trading date's strategy instance.

## Strategy Summary

1. At `range_start_time`, pick one CE and one PE whose premiums are closest to `closest_premium_value`.
2. Phase 1: track both selected options from `range_start_time` to `range_end_time` and record the highest `High` for each option.
3. Phase 2: after `range_end_time`, wait for breakout above the recorded range high to trigger a long entry.
4. If no breakout happens within the waiting cut-off up to `entry_cutoff_time`, close that leg by time without entry.
5. Phase 3: once entry happens, manage that trade with `target_pct` target and `stop_loss_pct` stop-loss.
6. Exit the same day if target or stop-loss is hit.
7. Otherwise carry forward to the next day.
8. On Day 2, exit on target, stop-loss, or force close at `day2_exit_time`.
9. On any given day, support both yesterday's carry-forward instance and today's new instance simultaneously.

## Important Assumptions

- The selected CE and PE strikes are fixed based on the `range_start_time` premium closest to `closest_premium_value`.
- Selection and breakout evaluation use exact option bars at the current timestamp. Contracts without an exact bar at that timestamp are ignored for that step.
- The breakout condition uses `Close > Reference High`.
- A leg can remain in the waiting phase without entry, and if no breakout happens before `entry_cutoff_time`, that leg is closed by time.
- The same OHLC candle interval should be used consistently throughout the strategy.
- End-of-day liquidation on Day 2 should happen at `day2_exit_time`.
- On each trading day, there may be one logic running for yesterday and one for today, so appropriate state isolation is essential.
- All major timings and risk values should remain parameterized so the strategy can later be used for parameter sweeps.

## How To Run

Build the project strategy crate first:

```bash
cargo check --manifest-path /home/amol/opt_bt/demo_ws/projects/my_strategy/strategy/Cargo.toml
```

Use the existing cache server and run the strategy like this:

```bash
BT_CACHE_ADDR=127.0.0.1:7878 /home/amol/opt_bt/target/release/bt run \
	--project my_strategy \
	--workspace /home/amol/opt_bt/demo_ws \
	--strategy banknifty_orb_btst \
	--data /quant/nifty_bank_full.parquet \
	--start-date 2024-01-03 \
	--end-date 2024-01-10 \
	--params qty=15 \
	--params closest_premium_value=200 \
	--params range_start_seconds=33360 \
	--params range_end_seconds=41400 \
	--params entry_cutoff_seconds=54900 \
	--params day2_exit_seconds=54900 \
	--params target_pct=0.30 \
	--params stop_loss_pct=0.25 \
	--params index_symbol=BANKNIFTY \
	--params option_underlying=BANKNIFTY
```

Before the first run on a new BANKNIFTY dataset, verify the symbol names and sparse-bar behavior:

```bash
BT_CACHE_ADDR=127.0.0.1:7878 /home/amol/opt_bt/target/release/bt data index \
	--data /quant/nifty_bank_full.parquet \
	--symbol BANKNIFTY \
	--date 2024-01-03 \
	--minute 09:16 \
	--window-minutes 2

BT_CACHE_ADDR=127.0.0.1:7878 /home/amol/opt_bt/target/release/bt data contract \
	--data /quant/nifty_bank_full.parquet \
	--symbol BANKNIFTY31JAN2450000CE \
	--start-date 2024-01-03 \
	--end-date 2024-01-03 \
	--start-time 09:16 \
	--end-time 09:20
```

Artifacts are written under:

- `demo_ws/projects/my_strategy/backtests/<run-folder>/report.json`
- `demo_ws/projects/my_strategy/backtests/<run-folder>/engine.log`
- `demo_ws/projects/my_strategy/backtests/<run-folder>/btst_session_summary.csv`

The session summary CSV contains one row per leg per trade day with:

- `trade_day`
- `expiry_yyyymmdd`
- `leg`
- `symbol`
- `reference_high`
- `entry_time`
- `entry_price`
- `exit_time`
- `exit_price`
- `exit_reason`
- `final_status`

Inspect the latest run folder like this:

```bash
latest=$(ls -td /home/amol/opt_bt/demo_ws/projects/my_strategy/backtests/banknifty_orb_btst_* | head -1)

jq '.metrics' "$latest/report.json"
jq '.fills' "$latest/report.json"
jq '.order_events' "$latest/report.json"
grep -E 'selected|range_finalized|breakout_trigger|entry_fill|exit_fill|no_breakout_close' "$latest/engine.log"
column -s, -t < "$latest/btst_session_summary.csv"
```

## Verified Example

The strategy was verified on `/quant/nifty_bank_full.parquet` for `2024-01-03` to `2024-01-10`.
One successful verification path is:

- `2024-01-03`: selected `BANKNIFTY31JAN2450000CE` and `BANKNIFTY31JAN2446000PE`
- `2024-01-03`: CE exact 09:16 close was `177.10`
- `2024-01-03`: CE range high through the observation window was `178.75`
- `2024-01-03 11:45`: CE close printed `189.65`, which triggered breakout entry
- `2024-01-03 14:04`: that CE leg exited on stop-loss at `142.40`

The BTST overlap was also verified in the backtest logs:

- `2024-01-05`: PE leg `BANKNIFTY31JAN2447000PE` entered and remained open after Day 1
- `2024-01-08`: that `2024-01-05` carry-forward leg exited while the `2024-01-08` strategy instance was also active
- `2024-01-09`: the `2024-01-08` carry-forward leg exited while the `2024-01-09` instance had already started

This confirms that the implementation maintains simultaneous state for yesterday's carry-forward logic and today's fresh setup logic.

One concrete overlap sequence from the verified run:

- `2024-01-05 14:05`: `BANKNIFTY31JAN2447000PE` entered for the `20240105` session
- `2024-01-08 03:46`: a new `20240108` session started and selected its own CE/PE pair
- `2024-01-08 14:28`: the older `20240105` PE exited on target while the `20240108` session already existed

That is the expected BTST overlap pattern the state model must support.

## Benchmark Caveat

The generic demo project still defaults to benchmark symbol `NIFTY 50`.
When you run this strategy on BANKNIFTY-only data, benchmark-relative metrics may be unavailable.

Always inspect these fields in `report.json` before reading benchmark-relative numbers:

- `metrics.benchmark_symbol`
- `metrics.benchmark_available`

If `benchmark_available` is `false`, do not interpret alpha, beta, tracking error, information ratio, or Treynor ratio for that run.

## Example Logic

Day 1:

- At `range_start_time`, select one CE and one PE with premiums closest to `closest_premium_value`.
- Phase 1: from `range_start_time` to `range_end_time`, record the highest premium high for both.
- Phase 2: after `range_end_time`, wait for breakout in each leg.
- If at 12:15 PM the CE closes above its recorded reference high, enter CE buy trade.
- If the PE never breaks out before `entry_cutoff_time`, no PE trade is taken and that leg is closed by time.
- Phase 3: for the entered CE trade, set target at `target_pct` above entry and stop-loss at `stop_loss_pct` below entry.
- If neither is hit by market close, carry the position overnight.

Day 2:

- Yesterday's carried-forward instance continues to run for exit handling.
- Today's new instance starts again from Phase 1 at `range_start_time`.
- If target is hit, exit.
- If stop-loss is hit, exit.
- If neither is hit, liquidate the position at `day2_exit_time`.
