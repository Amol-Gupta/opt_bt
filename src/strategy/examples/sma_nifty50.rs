use crate::common::context::Context;
use crate::common::event::MarketEvent;
use crate::common::types::{OrderType, Side};
use crate::strategy::Strategy;
use std::collections::{HashMap, VecDeque};

pub struct SmaNifty50Strategy {
    index_symbol: String,
    quantity: i64,
    short_period: usize,
    long_period: usize,
    close_history: HashMap<u32, VecDeque<i64>>,
    position: HashMap<u32, i32>,
}

impl SmaNifty50Strategy {
    pub fn new(index_symbol: &str, quantity: i64, short_period: usize, long_period: usize) -> Self {
        let short_period = short_period.max(1);
        let long_period = long_period.max(short_period + 1);
        Self {
            index_symbol: index_symbol.to_string(),
            quantity,
            short_period,
            long_period,
            close_history: HashMap::new(),
            position: HashMap::new(),
        }
    }

    fn sma(window: &VecDeque<i64>, period: usize) -> Option<i64> {
        if window.len() < period {
            return None;
        }
        let start = window.len() - period;
        let sum: i128 = window.iter().skip(start).map(|v| *v as i128).sum();
        Some((sum / period as i128) as i64)
    }
}

impl Strategy for SmaNifty50Strategy {
    fn on_market_event(&mut self, ctx: &mut Context, event: &MarketEvent) {
        let symbol = match ctx.market_data.get_symbol(event.instrument_id) {
            Some(value) => value,
            None => return,
        };

        if symbol != self.index_symbol {
            return;
        }

        let bar = match ctx.get_bar(event.instrument_id) {
            Some(value) => value,
            None => return,
        };

        let history = self
            .close_history
            .entry(event.instrument_id)
            .or_default();
        history.push_back(bar.close);
        while history.len() > self.long_period {
            history.pop_front();
        }

        let short_sma = match Self::sma(history, self.short_period) {
            Some(value) => value,
            None => return,
        };
        let long_sma = match Self::sma(history, self.long_period) {
            Some(value) => value,
            None => return,
        };

        let state = self.position.entry(event.instrument_id).or_insert(0);
        if short_sma > long_sma && *state <= 0 {
            ctx.place_order(event.instrument_id, Side::Buy, OrderType::Market, self.quantity);
            *state = 1;
            return;
        }

        if short_sma < long_sma && *state >= 0 {
            ctx.place_order(event.instrument_id, Side::Sell, OrderType::Market, self.quantity);
            *state = -1;
        }
    }
}
