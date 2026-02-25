pub mod examples;
pub use examples::RandomStrategy; 

pub mod portfolio;
pub use portfolio::PortfolioStrategy;

use crate::common::context::Context;
use crate::common::event::{MarketEvent, OrderEvent, FillEvent, SignalEvent};

pub trait Strategy {
    // Lifecycle
    fn init(&mut self, _ctx: &mut Context) {}
    
    // Engine hooks
    fn on_start(&mut self, _ctx: &mut Context) {}
    fn on_stop(&mut self, _ctx: &mut Context) {}
    fn on_date_change(&mut self, _ctx: &mut Context) {} // For daily boundaries?
    
    // Data & Execution
    fn on_market_event(&mut self, _ctx: &mut Context, _event: &MarketEvent) {}
    fn on_signal(&mut self, _ctx: &mut Context, _event: &SignalEvent) {}
    fn on_order_event(&mut self, _ctx: &mut Context, _event: &OrderEvent) {}
    fn on_fill(&mut self, _ctx: &mut Context, _event: &FillEvent) {}
    
    // Timer
    // fn on_timer(&mut self, ctx: &mut Context, timer_id: u64);
}

