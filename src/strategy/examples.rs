use crate::strategy::Strategy;
use crate::common::context::Context;
use crate::common::event::{MarketEvent, OrderEvent, FillEvent, SignalEvent};
use crate::common::types::{OrderType, Side, Price};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::collections::HashMap;

pub struct RandomStrategy {
    rng: StdRng,
    probability: f64,
    quantity: i64,
    last_action_ts: HashMap<u32, i64>,
}

impl RandomStrategy {
    pub fn new(seed: u64, probability: f64, quantity: i64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
            probability,
            quantity,
            last_action_ts: HashMap::new(),
        }
    }
}

impl Strategy for RandomStrategy {
    fn on_start(&mut self, _ctx: &mut Context) {
        println!("RandomStrategy: Started with prob={} qty={}", self.probability, self.quantity);
    }
    
    fn on_market_event(&mut self, ctx: &mut Context, event: &MarketEvent) {
        // Simple throttling: Only act once per timestamp per instrument? 
        // Or every event?
        // Let's act every event with probability.
        
        if self.rng.gen_bool(self.probability) {
            let side = if self.rng.gen_bool(0.5) { Side::Buy } else { Side::Sell };
            
            // Generate a price around the close price
            // Assume 1% spread?
            // Market orders for simplicity in MVP, or Limit at Close
            
            // Get bar data from context
            let bars = match ctx.market_data.bars.get(&event.instrument_id) {
                Some(b) => b,
                None => return,
            };
            
            // Find specific bar for this event time
            let bar = match bars.binary_search_by_key(&event.timestamp, |b| b.timestamp) {
                Ok(idx) => &bars[idx],
                Err(_) => return, // Should not happen if events are generated from bars
            };

            // Limit order near Close price (±1%)
            // Price is scaled 10000.
            let close_price = bar.close;
            let offset = (close_price as f64 * 0.01) as i64; // 1%
            let limit_price = if side == Side::Buy {
                close_price + offset // Limit Buy quite high to fill
            } else {
                close_price - offset // Limit Sell quite low to fill
            };
            
            // For now, let's use Limit orders to be safe
            ctx.place_order(event.instrument_id, side, OrderType::Limit(limit_price), self.quantity);
        }
    }
    
    fn on_fill(&mut self, _ctx: &mut Context, _event: &FillEvent) {
        // Log fill
    }
}
