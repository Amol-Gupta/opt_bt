use crate::common::context::Context;
use crate::common::event::MarketEvent;
use crate::common::types::{OrderType, Side, PRICE_SCALE};
use crate::strategy::Strategy;
use std::collections::{HashMap, HashSet};

pub struct AtmStraddleSellStrategy {
    index_symbol: String,
    quantity: i64,
    entered_days: HashSet<i64>,
    exited_days: HashSet<i64>,
    active_legs: HashMap<i64, (u32, u32)>,
}

impl AtmStraddleSellStrategy {
    pub fn new(index_symbol: &str, quantity: i64) -> Self {
        Self {
            index_symbol: index_symbol.to_string(),
            quantity,
            entered_days: HashSet::new(),
            exited_days: HashSet::new(),
            active_legs: HashMap::new(),
        }
    }

    fn parse_strike(symbol: &str) -> Option<i64> {
        if !(symbol.ends_with("CE") || symbol.ends_with("PE")) {
            return None;
        }

        let core = &symbol[..symbol.len().saturating_sub(2)];
        let mut start = core.len();
        for (idx, ch) in core.char_indices().rev() {
            if ch.is_ascii_digit() {
                start = idx;
            } else {
                break;
            }
        }

        if start >= core.len() {
            return None;
        }

        core[start..].parse::<i64>().ok()
    }

    fn resolve_atm_legs(&self, ctx: &Context, spot_points: i64) -> Option<(u32, u32)> {
        let mut best_ce: Option<(u32, i64)> = None;
        let mut best_pe: Option<(u32, i64)> = None;

        for (instrument_id, symbol) in ctx.market_data.iter_ids() {
            if ctx.get_bar(instrument_id).is_none() {
                continue;
            }

            let strike = match Self::parse_strike(&symbol) {
                Some(value) => value,
                None => continue,
            };

            let distance = (strike - spot_points).abs();

            if symbol.ends_with("CE") {
                if best_ce.map(|(_, d)| distance < d).unwrap_or(true) {
                    best_ce = Some((instrument_id, distance));
                }
            } else if symbol.ends_with("PE") && best_pe.map(|(_, d)| distance < d).unwrap_or(true) {
                best_pe = Some((instrument_id, distance));
            }
        }

        match (best_ce, best_pe) {
            (Some((ce_id, _)), Some((pe_id, _))) => Some((ce_id, pe_id)),
            _ => None,
        }
    }
}

impl Strategy for AtmStraddleSellStrategy {
    fn on_market_event(&mut self, ctx: &mut Context, event: &MarketEvent) {
        let symbol = match ctx.market_data.get_symbol(event.instrument_id) {
            Some(value) => value,
            None => return,
        };

        if symbol != self.index_symbol {
            return;
        }

        let seconds_of_day = event.timestamp.rem_euclid(86_400);
        let day_key = event.timestamp.div_euclid(86_400);
        // 10:00 IST and 11:00 IST in local market clock seconds-of-day.
        let entry_time = 10 * 60 * 60;
        let exit_time = 11 * 60 * 60;

        if !self.entered_days.contains(&day_key)
            && seconds_of_day >= entry_time
            && seconds_of_day < exit_time
        {
            let index_bar = match ctx.get_bar(event.instrument_id) {
                Some(bar) => bar,
                None => return,
            };

            let spot_points = index_bar.close / PRICE_SCALE;
            if let Some((ce_id, pe_id)) = self.resolve_atm_legs(ctx, spot_points) {
                ctx.place_order(ce_id, Side::Sell, OrderType::Market, self.quantity);
                ctx.place_order(pe_id, Side::Sell, OrderType::Market, self.quantity);
                self.active_legs.insert(day_key, (ce_id, pe_id));
                self.entered_days.insert(day_key);
            }
            return;
        }

        if self.entered_days.contains(&day_key)
            && !self.exited_days.contains(&day_key)
            && seconds_of_day >= exit_time
        {
            if let Some((ce_id, pe_id)) = self.active_legs.get(&day_key).copied() {
                ctx.place_order(ce_id, Side::Buy, OrderType::Market, self.quantity);
                ctx.place_order(pe_id, Side::Buy, OrderType::Market, self.quantity);
                self.exited_days.insert(day_key);
            }
        }
    }
}
