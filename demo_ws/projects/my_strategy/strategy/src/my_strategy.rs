pub mod nifty_premium_straddle;
pub mod nifty_straddle;
pub mod nifty_straddle_portfolio;

use bt_strategy_macros::bt_strategy;
use opt_bt::common::context::Context;
use opt_bt::common::event::{FillEvent, MarketEvent, OrderEvent, SignalEvent};
use opt_bt::common::types::PRICE_SCALE;
use opt_bt::strategy::Strategy;

use nifty_premium_straddle::NiftyPremiumStraddleStrategy;
use nifty_straddle::AlgotestWeeklyStraddleStrategy;
use nifty_straddle_portfolio::NiftyStraddlePortfolioStrategy;

#[bt_strategy(
    id = "my_strategy",
    display_name = "My Strategy",
    description = "Sample scaffold strategy with simple bullish bar behavior"
)]
pub fn my_strategy_registration() {}

#[bt_strategy(
    id = "algotest_weekly_straddle",
    display_name = "Algotest Weekly Straddle",
    description = "11:00 sell ATM CE/PE on nearest weekly expiry with 20% SL and 15:15 exit"
)]
pub fn algotest_weekly_straddle_registration() {}

#[bt_strategy(
    id = "nifty_straddle_sl",
    display_name = "Nifty ATM Straddle with Stop Loss",
    description = "Sell ATM CE+PE on nearest weekly expiry. Params: entry_seconds, exit_seconds, sl_pct, qty."
)]
pub fn nifty_straddle_sl_registration() {}

#[bt_strategy(
    id = "nifty_straddle_portfolio",
    display_name = "Nifty Straddle Portfolio (3 time windows)",
    description = "Portfolio of 3 ATM straddle strategies: morning (09:30-11:30), midday (11:30-13:30), afternoon (13:30-15:15). Params: sl_pct, qty."
)]
pub fn nifty_straddle_portfolio_registration() {}

#[bt_strategy(
    id = "nifty_premium_straddle",
    display_name = "Nifty Premium Straddle",
    description = "Sell CE/PE whose premium is closest to a target CP on nearest weekly expiry, with configurable entry/exit times and % stop loss"
)]
pub fn nifty_premium_straddle_registration() {}

#[derive(Debug)]
pub struct MyStrategy;

impl std::fmt::Display for MyStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Strategy for MyStrategy {
    fn on_start(&mut self, _ctx: &mut Context) {
        log::info!("MyStrategy on_start called");
    }
    fn on_stop(&mut self, _ctx: &mut Context) {
        log::info!("MyStrategy on_stop called");
    }
    fn on_date_change(&mut self, _ctx: &mut Context) {
        log::info!("MyStrategy on_date_change called");
    }

    fn before_open(&mut self, _ctx: &mut Context) {
        log::info!("MyStrategy before_open called");
    }
    fn after_close(&mut self, _ctx: &mut Context) {
        log::info!("MyStrategy after_close called");
    }

    fn on_market_event(&mut self, ctx: &mut Context, event: &MarketEvent) {
        if let Some(bar) = ctx.get_bar(event.instrument_id) {
            let _symbol = ctx
                .market_data
                .get_symbol(event.instrument_id)
                .unwrap_or_else(|| "UNKNOWN".to_string());
            let _open = bar.open as f64 / PRICE_SCALE as f64;
            let _high = bar.high as f64 / PRICE_SCALE as f64;
            let _low = bar.low as f64 / PRICE_SCALE as f64;
            let _close = bar.close as f64 / PRICE_SCALE as f64;
            let _volume = bar.volume;
        }
    }

    fn on_signal(&mut self, _ctx: &mut Context, _event: &SignalEvent) {
        log::info!("MyStrategy on_signal called {:?}", _event);
    }
    fn on_order_event(&mut self, _ctx: &mut Context, _event: &OrderEvent) {
        log::info!("MyStrategy on_order_event called {:?}", _event);
    }
    fn on_fill(&mut self, _ctx: &mut Context, _event: &FillEvent) {
        log::info!("MyStrategy on_fill called {:?}", _event);
    }
}

// ---------------------------------------------------------------------------
// Strategy factory — called by bt_run_project.rs
// ---------------------------------------------------------------------------

pub fn create_strategy_by_id(
    strategy_id: &str,
    params: &std::collections::BTreeMap<String, String>,
) -> Option<Box<dyn Strategy + Send>> {
    let quantity = params
        .get("qty")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(65);
    let stop_loss_pct = params
        .get("sl_pct")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.20);
    let entry_seconds = params
        .get("entry_seconds")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(5 * 3600 + 30 * 60); // 11:00 IST = 05:30 UTC
    let exit_seconds = params
        .get("exit_seconds")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(9 * 3600 + 45 * 60); // 15:15 IST = 09:45 UTC
    let target_premium_rupees: i64 = params
        .get("target_premium")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(100);
    let target_premium = target_premium_rupees * PRICE_SCALE;

    match strategy_id {
        "my_strategy" => Some(Box::new(MyStrategy)),
        "algotest_weekly_straddle" | "nifty_straddle_sl" => {
            Some(Box::new(AlgotestWeeklyStraddleStrategy::new(
                "NIFTY 50",
                "NIFTY",
                quantity,
                entry_seconds,
                exit_seconds,
                stop_loss_pct,
            )))
        }
        "nifty_premium_straddle" => Some(Box::new(NiftyPremiumStraddleStrategy::new(
            "NIFTY 50",
            "NIFTY",
            quantity,
            entry_seconds,
            exit_seconds,
            stop_loss_pct,
            target_premium,
        ))),
        "nifty_straddle_portfolio" => {
            // Three non-overlapping intraday time windows (IST times, stored as UTC seconds).
            // All times in UTC (IST - 5:30):
            // Morning  : 09:30–11:30 IST = 04:00–06:00 UTC
            // Midday   : 11:30–13:30 IST = 06:00–08:00 UTC
            // Afternoon: 13:30–15:15 IST = 08:00–09:45 UTC
            Some(Box::new(NiftyStraddlePortfolioStrategy::new(
                "NIFTY 50",
                "NIFTY",
                quantity,
                stop_loss_pct,
                vec![
                    ("morning",   4 * 3600,             6 * 3600),       // 09:30–11:30 IST
                    ("midday",    6 * 3600,             8 * 3600),       // 11:30–13:30 IST
                    ("afternoon", 8 * 3600, 9 * 3600 + 45 * 60),        // 13:30–15:15 IST
                ],
            )))
        }
        _ => None,
    }
}
