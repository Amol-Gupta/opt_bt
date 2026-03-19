use bt_strategy_macros::bt_strategy;
use opt_bt::common::context::Context;
use opt_bt::common::event::{FillEvent, MarketEvent, OrderEvent, SignalEvent};
use opt_bt::common::types::PRICE_SCALE;
use opt_bt::strategy::Strategy;

mod nifty_premium_straddle;
mod nifty_straddle;

use nifty_premium_straddle::NiftyPremiumStraddleStrategy;
use nifty_straddle::AlgotestWeeklyStraddleStrategy;

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
        .unwrap_or(11 * 60 * 60);
    let exit_seconds = params
        .get("exit_seconds")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(15 * 60 + 15 * 60 * 60);
    let target_premium_rupees: i64 = params
        .get("target_premium")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(100);
    let target_premium = target_premium_rupees * PRICE_SCALE;

    match strategy_id {
        "my_strategy" => Some(Box::new(MyStrategy)),
        "algotest_weekly_straddle" => Some(Box::new(AlgotestWeeklyStraddleStrategy::new(
            "NIFTY 50",
            "NIFTY",
            quantity,
            entry_seconds,
            exit_seconds,
            stop_loss_pct,
        ))),
        "nifty_premium_straddle" => Some(Box::new(NiftyPremiumStraddleStrategy::new(
            "NIFTY 50",
            "NIFTY",
            quantity,
            entry_seconds,
            exit_seconds,
            stop_loss_pct,
            target_premium,
        ))),
        _ => None,
    }
}
