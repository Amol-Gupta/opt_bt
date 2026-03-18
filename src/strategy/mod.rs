pub mod examples;
pub use examples::AtmStraddleSellStrategy;
pub use examples::NiftyNearestExpiryStraddleStrategy;
pub use examples::RandomStrategy;
pub use examples::SmaNifty50Strategy;

pub mod portfolio;
pub use portfolio::PortfolioStrategy;

use crate::common::context::Context;
use crate::common::event::{AlarmEvent, FillEvent, MarketEvent, OrderEvent, OrderRejectionEvent, SignalEvent};
use crate::common::types::OptionType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionFilterCriteria {
    pub underlying: String,
    pub nearest_expiry_only: bool,
    pub min_days_to_expiry: Option<i32>,
    pub max_days_to_expiry: Option<i32>,
    pub strike_step: Option<i64>,
    pub strike_band_steps: Option<i32>,
    pub option_types: Vec<OptionType>,
}

pub trait Strategy: Send {
    // Lifecycle
    fn init(&mut self, _ctx: &mut Context) {}
    fn before_open(&mut self, _ctx: &mut Context) {}
    fn after_close(&mut self, _ctx: &mut Context) {}

    // Engine hooks
    fn on_start(&mut self, _ctx: &mut Context) {}
    fn on_stop(&mut self, _ctx: &mut Context) {}
    fn on_date_change(&mut self, _ctx: &mut Context) {} // For daily boundaries?

    // Data & Execution
    fn on_market_event(&mut self, _ctx: &mut Context, _event: &MarketEvent) {}
    fn on_alarm(&mut self, _ctx: &mut Context, _event: &AlarmEvent) {}
    fn on_signal(&mut self, _ctx: &mut Context, _event: &SignalEvent) {}
    fn on_order_event(&mut self, _ctx: &mut Context, _event: &OrderEvent) {}
    fn on_order_rejected(&mut self, _ctx: &mut Context, _event: &OrderRejectionEvent) {}
    fn on_fill(&mut self, _ctx: &mut Context, _event: &FillEvent) {}

    // Instrument selection
    fn option_filter_criteria(&self, _ctx: &Context) -> Option<OptionFilterCriteria> {
        None
    }

    // Timer
    // fn on_timer(&mut self, ctx: &mut Context, timer_id: u64);
}
