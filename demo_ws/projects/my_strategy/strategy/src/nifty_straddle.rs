use std::collections::{BTreeMap, HashMap, HashSet};

use opt_bt::common::context::Context;
use opt_bt::common::event::{AlarmEvent, FillEvent, MarketEvent, OrderEvent, SignalEvent};
use opt_bt::common::types::{OptionType, OrderType, Side, PRICE_SCALE};
use opt_bt::strategy::Strategy;

// ---------------------------------------------------------------------------
// DailyState — per-day open positions and stop-loss order IDs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) struct DailyState {
    pub ce_id: u32,
    pub pe_id: u32,
    pub ce_stop_order_id: u64,
    pub pe_stop_order_id: u64,
    pub ce_stop_price: i64,
    pub pe_stop_price: i64,
    pub ce_open: bool,
    pub pe_open: bool,
}

// ---------------------------------------------------------------------------
// AlgotestWeeklyStraddleStrategy
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct AlgotestWeeklyStraddleStrategy {
    index_symbol: String,
    index_instrument_id: Option<u32>,
    option_underlying: String,
    quantity: i64,
    entry_seconds: i64,
    exit_seconds: i64,
    stop_loss_ratio: f64,
    entered_days: HashSet<i64>,
    day_state: HashMap<i64, DailyState>,
}

impl AlgotestWeeklyStraddleStrategy {
    pub fn new(
        index_symbol: &str,
        option_underlying: &str,
        quantity: i64,
        entry_seconds: i64,
        exit_seconds: i64,
        stop_loss_ratio: f64,
    ) -> Self {
        Self {
            index_symbol: index_symbol.to_string(),
            index_instrument_id: None,
            option_underlying: option_underlying.to_ascii_uppercase(),
            quantity,
            entry_seconds,
            exit_seconds,
            stop_loss_ratio,
            entered_days: HashSet::new(),
            day_state: HashMap::new(),
        }
    }

    fn timestamp_to_yyyymmdd(timestamp: i64) -> Option<i32> {
        chrono::DateTime::from_timestamp(timestamp, 0)
            .and_then(|dt| dt.format("%Y%m%d").to_string().parse::<i32>().ok())
    }

    fn yyyymmdd_to_date(yyyymmdd: i32) -> Option<chrono::NaiveDate> {
        let year = yyyymmdd / 10_000;
        let month = ((yyyymmdd / 100) % 100) as u32;
        let day = (yyyymmdd % 100) as u32;
        chrono::NaiveDate::from_ymd_opt(year, month, day)
    }

    fn dte_days(today_yyyymmdd: i32, expiry_yyyymmdd: i32) -> Option<i64> {
        let today = Self::yyyymmdd_to_date(today_yyyymmdd)?;
        let expiry = Self::yyyymmdd_to_date(expiry_yyyymmdd)?;
        Some((expiry - today).num_days())
    }

    fn day_start(day_key: i64) -> i64 {
        day_key * 86_400
    }

    fn entry_alarm_key(day_key: i64) -> String {
        format!("entry:{}", day_key)
    }

    fn exit_alarm_key(day_key: i64) -> String {
        format!("exit:{}", day_key)
    }

    fn parse_alarm_key(key: &str) -> Option<(&str, i64)> {
        let (kind, raw_day) = key.split_once(':')?;
        let day_key = raw_day.parse::<i64>().ok()?;
        Some((kind, day_key))
    }

    fn schedule_day_alarms(&self, ctx: &mut Context, day_key: i64) {
        let day_start = Self::day_start(day_key);
        let entry_ts = day_start + self.entry_seconds + 59;
        let exit_ts = day_start + self.exit_seconds + 59;
        ctx.schedule_alarm_at(entry_ts, Self::entry_alarm_key(day_key));
        ctx.schedule_alarm_at(exit_ts, Self::exit_alarm_key(day_key));
    }

    fn ensure_index_instrument_id(&mut self, ctx: &Context) -> Option<u32> {
        if self.index_instrument_id.is_none() {
            self.index_instrument_id = ctx.market_data.get_id(&self.index_symbol);
        }
        self.index_instrument_id
    }

    fn process_entry(&mut self, ctx: &mut Context, day_key: i64) {
        if self.entered_days.contains(&day_key) {
            return;
        }

        let Some(index_instrument_id) = self.ensure_index_instrument_id(ctx) else {
            return;
        };
        let Some(today_yyyymmdd) = Self::timestamp_to_yyyymmdd(ctx.now()) else {
            return;
        };
        let Some(expiry_yyyymmdd) = self.nearest_weekly_expiry(ctx, today_yyyymmdd) else {
            return;
        };

        let Some(index_bar) = ctx.get_bar(index_instrument_id) else {
            return;
        };
        let spot_points = index_bar.close / PRICE_SCALE;

        let Some((ce_id, pe_id)) = self.resolve_atm_pair(ctx, expiry_yyyymmdd, spot_points) else {
            return;
        };

        let Some(ce_bar) = ctx.get_bar(ce_id) else {
            return;
        };
        let Some(pe_bar) = ctx.get_bar(pe_id) else {
            return;
        };

        let ce_entry = ce_bar.close;
        let pe_entry = pe_bar.close;
        let ce_stop_price = ((ce_entry as f64) * (1.0 + self.stop_loss_ratio)).round() as i64;
        let pe_stop_price = ((pe_entry as f64) * (1.0 + self.stop_loss_ratio)).round() as i64;

        ctx.place_order(ce_id, Side::Sell, OrderType::Market, self.quantity);
        ctx.place_order(pe_id, Side::Sell, OrderType::Market, self.quantity);

        let ce_stop_order_id = ctx.place_order(
            ce_id,
            Side::Buy,
            OrderType::Stop(ce_stop_price),
            self.quantity,
        );
        let pe_stop_order_id = ctx.place_order(
            pe_id,
            Side::Buy,
            OrderType::Stop(pe_stop_price),
            self.quantity,
        );

        self.day_state.insert(
            day_key,
            DailyState {
                ce_id,
                pe_id,
                ce_stop_order_id,
                pe_stop_order_id,
                ce_stop_price,
                pe_stop_price,
                ce_open: true,
                pe_open: true,
            },
        );
        self.entered_days.insert(day_key);
    }

    fn process_exit(&mut self, ctx: &mut Context, day_key: i64) {
        if let Some(state) = self.day_state.get(&day_key).cloned() {
            ctx.cancel_order(state.ce_stop_order_id);
            ctx.cancel_order(state.pe_stop_order_id);

            if state.ce_open && ctx.position_qty(state.ce_id) < 0 {
                ctx.place_order(state.ce_id, Side::Buy, OrderType::Market, self.quantity);
            }
            if state.pe_open && ctx.position_qty(state.pe_id) < 0 {
                ctx.place_order(state.pe_id, Side::Buy, OrderType::Market, self.quantity);
            }
            if let Some(updated_state) = self.day_state.get_mut(&day_key) {
                updated_state.ce_open = false;
                updated_state.pe_open = false;
            }
        }
    }

    fn sync_leg_state_from_positions(&mut self, ctx: &Context, day_key: i64) {
        if let Some(state) = self.day_state.get_mut(&day_key) {
            state.ce_open = ctx.position_qty(state.ce_id) < 0;
            state.pe_open = ctx.position_qty(state.pe_id) < 0;
        }
    }

    fn nearest_weekly_expiry(&self, ctx: &Context, today_yyyymmdd: i32) -> Option<i32> {
        ctx.market_data
            .iter_ids()
            .into_iter()
            .filter_map(|(instrument_id, _)| ctx.market_data.get_instrument(instrument_id))
            .filter_map(|instrument| {
                let option = instrument.option.as_ref()?;
                if !option
                    .underlying
                    .to_ascii_uppercase()
                    .starts_with(&self.option_underlying)
                {
                    return None;
                }

                let dte = Self::dte_days(today_yyyymmdd, option.expiry_yyyymmdd)?;
                if !(0..=7).contains(&dte) {
                    return None;
                }

                Some(option.expiry_yyyymmdd)
            })
            .min()
    }

    fn resolve_atm_pair(
        &self,
        ctx: &Context,
        expiry_yyyymmdd: i32,
        spot_points: i64,
    ) -> Option<(u32, u32)> {
        #[derive(Clone, Copy, Debug, Default)]
        struct StrikePair {
            ce_id: Option<u32>,
            pe_id: Option<u32>,
        }

        let mut strike_pairs: BTreeMap<i64, StrikePair> = BTreeMap::new();
        let mut call_candidates = 0usize;
        let mut put_candidates = 0usize;

        for (instrument_id, _) in ctx.market_data.iter_ids() {
            let instrument = match ctx.market_data.get_instrument(instrument_id) {
                Some(value) => value,
                None => continue,
            };

            let option = match &instrument.option {
                Some(value) => value,
                None => continue,
            };

            if option.expiry_yyyymmdd != expiry_yyyymmdd {
                continue;
            }
            if !option
                .underlying
                .to_ascii_uppercase()
                .starts_with(&self.option_underlying)
            {
                continue;
            }
            if ctx.get_bar(instrument.id).is_none() {
                continue;
            }

            let strike_points = option.strike / PRICE_SCALE;
            let slot = strike_pairs.entry(strike_points).or_default();

            match option.option_type {
                OptionType::Call => {
                    call_candidates += 1;
                    if slot.ce_id.is_none() {
                        slot.ce_id = Some(instrument.id);
                    }
                }
                OptionType::Put => {
                    put_candidates += 1;
                    if slot.pe_id.is_none() {
                        slot.pe_id = Some(instrument.id);
                    }
                }
            }
        }

        let strikes: Vec<i64> = strike_pairs.keys().copied().collect();
        let strike_step = strikes
            .windows(2)
            .filter_map(|window| {
                let gap = window[1] - window[0];
                if gap > 0 { Some(gap) } else { None }
            })
            .min()
            .unwrap_or(50);
        let target_strike = ((spot_points + strike_step / 2) / strike_step) * strike_step;

        let selected = if let Some(pair) = strike_pairs.get(&target_strike) {
            if let (Some(ce_id), Some(pe_id)) = (pair.ce_id, pair.pe_id) {
                Some((target_strike, ce_id, pe_id))
            } else {
                None
            }
        } else {
            None
        }
        .or_else(|| {
            strike_pairs
                .iter()
                .filter_map(|(strike, pair)| {
                    let (Some(ce_id), Some(pe_id)) = (pair.ce_id, pair.pe_id) else {
                        return None;
                    };
                    Some((*strike, (*strike - target_strike).abs(), ce_id, pe_id))
                })
                .min_by_key(|(_, distance, _, _)| *distance)
                .map(|(strike, distance, ce_id, pe_id)| {
                    log::warn!(
                        "resolve_atm_pair: exact target strike {} missing complete CE/PE pair; fallback_strike={} fallback_distance={}",
                        target_strike,
                        strike,
                        distance
                    );
                    (strike, ce_id, pe_id)
                })
        });

        match selected {
            Some((_selected_strike, ce_id, pe_id)) => {
                let ce_strike = ctx
                    .market_data
                    .get_instrument(ce_id)
                    .and_then(|ins| ins.option.map(|option| option.strike / PRICE_SCALE))
                    .unwrap_or_default();
                let pe_strike = ctx
                    .market_data
                    .get_instrument(pe_id)
                    .and_then(|ins| ins.option.map(|option| option.strike / PRICE_SCALE))
                    .unwrap_or_default();

                if ce_strike != pe_strike {
                    log::warn!(
                        "resolve_atm_pair: strike divergence ce_strike={} pe_strike={} spot_points={} expiry={}",
                        ce_strike,
                        pe_strike,
                        spot_points,
                        expiry_yyyymmdd
                    );
                }

                Some((ce_id, pe_id))
            }
            _ => {
                let complete_pair_strikes = strike_pairs
                    .iter()
                    .filter_map(|(strike, pair)| {
                        if pair.ce_id.is_some() && pair.pe_id.is_some() {
                            Some(*strike)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                log::warn!(
                    "resolve_atm_pair: failed to resolve pair expiry={} spot_points={} candidates ce={} pe={} complete_pair_strikes={:?}",
                    expiry_yyyymmdd,
                    spot_points,
                    call_candidates,
                    put_candidates,
                    complete_pair_strikes
                );
                None
            }
        }
    }
}

impl Strategy for AlgotestWeeklyStraddleStrategy {
    fn on_start(&mut self, ctx: &mut Context) {
        self.index_instrument_id = ctx.market_data.get_id(&self.index_symbol);
    }

    fn on_date_change(&mut self, ctx: &mut Context) {
        let day_key = ctx.now().div_euclid(86_400);
        self.schedule_day_alarms(ctx, day_key);
    }

    fn on_market_event(&mut self, _ctx: &mut Context, _event: &MarketEvent) {}

    fn on_alarm(&mut self, ctx: &mut Context, event: &AlarmEvent) {
        let Some((kind, day_key)) = Self::parse_alarm_key(&event.key) else {
            return;
        };

        match kind {
            "entry" => self.process_entry(ctx, day_key),
            "exit" => self.process_exit(ctx, day_key),
            _ => {}
        }
    }

    fn on_signal(&mut self, _ctx: &mut Context, _event: &SignalEvent) {}
    fn on_order_event(&mut self, _ctx: &mut Context, _event: &OrderEvent) {}
    fn on_fill(&mut self, ctx: &mut Context, event: &FillEvent) {
        let day_key = event.timestamp.div_euclid(86_400);
        self.sync_leg_state_from_positions(ctx, day_key);
    }
}
