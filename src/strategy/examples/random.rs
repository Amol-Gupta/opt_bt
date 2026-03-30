use crate::common::context::Context;
use crate::common::event::{FillEvent, MarketEvent};
use crate::common::types::{OrderType, Side};
use crate::strategy::Strategy;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

pub struct RandomStrategy {
    rng: StdRng,
    probability: f64,
    quantity: i64,
}

impl RandomStrategy {
    pub fn new(seed: u64, probability: f64, quantity: i64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
            probability,
            quantity,
        }
    }
}

impl Strategy for RandomStrategy {
    fn on_start(&mut self, _ctx: &mut Context) {
        println!(
            "RandomStrategy: Started with prob={} qty={}",
            self.probability, self.quantity
        );
    }

    fn on_market_event(&mut self, ctx: &mut Context, event: &MarketEvent) {
        if self.rng.random_bool(self.probability) {
            let side = if self.rng.random_bool(0.5) {
                Side::Buy
            } else {
                Side::Sell
            };

            let bar = match ctx.get_bar(event.instrument_id) {
                Some(bar) => bar,
                None => return,
            };

            let close_price = bar.close;
            let offset = (close_price as f64 * 0.01) as i64;
            let limit_price = if side == Side::Buy {
                close_price + offset
            } else {
                close_price - offset
            };

            ctx.place_order(
                event.instrument_id,
                side,
                OrderType::Limit(limit_price),
                self.quantity,
            );
        }
    }

    fn on_fill(&mut self, _ctx: &mut Context, _event: &FillEvent) {}
}
