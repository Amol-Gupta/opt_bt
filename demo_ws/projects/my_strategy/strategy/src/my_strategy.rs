use bt_strategy_macros::bt_strategy;
use opt_bt::common::context::Context;
use opt_bt::common::event::{FillEvent, MarketEvent, OrderEvent, SignalEvent};
use opt_bt::common::types::{OrderType, Side, PRICE_SCALE};
use opt_bt::strategy::Strategy;

#[bt_strategy(
    id = "my_strategy",
    display_name = "My Strategy",
    description = "Sample scaffold strategy with simple bullish bar behavior"
)]
pub fn my_strategy_registration() {}

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
            let symbol = ctx
                .market_data
                .get_symbol(event.instrument_id)
                .unwrap_or("UNKNOWN");
            // log::info!(
            //     "[BAR] ts={} symbol={} open={:.4} high={:.4} low={:.4} close={:.4} volume={}",
            //     bar.timestamp,
            //     symbol,
            //     bar.open as f64 / PRICE_SCALE as f64,
            //     bar.high as f64 / PRICE_SCALE as f64,
            //     bar.low as f64 / PRICE_SCALE as f64,
            //     bar.close as f64 / PRICE_SCALE as f64,
            //     bar.volume
            // );
            log::info!("bar {:?}", bar);
                
                
            if bar.close > bar.open {
                let _ = ctx.place_order(event.instrument_id, Side::Buy, OrderType::Market, 1);
            }
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
    _params: &std::collections::BTreeMap<String, String>,
) -> Option<Box<dyn Strategy + Send>> {
    match strategy_id {
        "my_strategy" => Some(Box::new(MyStrategy)),
        _ => None,
    }
}
