use opt_bt::common::context::Context;
use opt_bt::common::event::{AlarmEvent, FillEvent, MarketEvent, OrderEvent, SignalEvent};
use opt_bt::common::types::{OptionType, OrderType, Side, SimTime, PRICE_SCALE};
use opt_bt::strategy::Strategy;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
struct DailyState {
    ce_id: u32,
    pe_id: u32,
    ce_stop_order_id: u64,
    pe_stop_order_id: u64,
    ce_open: bool,
    pe_open: bool,
}

#[derive(Debug)]
pub struct NiftyPremiumStraddleStrategy {
    index_symbol: String,
    index_instrument_id: Option<u32>,
    option_underlying: String,
    quantity: i64,
    entry_seconds: i64,
    exit_seconds: i64,
    stop_loss_ratio: f64,
    target_premium: i64,
    entered_days: HashSet<i64>,
    day_state: HashMap<i64, DailyState>,
}

impl NiftyPremiumStraddleStrategy {
    pub fn new(
        index_symbol: &str,
        option_underlying: &str,
        quantity: i64,
        entry_seconds: i64,
        exit_seconds: i64,
        stop_loss_ratio: f64,
        target_premium: i64,
    ) -> Self {
        Self {
            index_symbol: index_symbol.to_string(),
            index_instrument_id: None,
            option_underlying: option_underlying.to_ascii_uppercase(),
            quantity,
            entry_seconds,
            exit_seconds,
            stop_loss_ratio,
            target_premium,
            entered_days: HashSet::new(),
            day_state: HashMap::new(),
        }
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

    fn entry_alarm_key(day_key: i64) -> String {
        format!("ps_entry:{}", day_key)
    }

    fn exit_alarm_key(day_key: i64) -> String {
        format!("ps_exit:{}", day_key)
    }

    fn parse_alarm_key(key: &str) -> Option<(&str, i64)> {
        let (kind, raw_day) = key.split_once(':')?;
        let day_key = raw_day.parse::<i64>().ok()?;
        Some((kind, day_key))
    }

    fn schedule_day_alarms(&self, ctx: &mut Context, now: SimTime) {
        let day_key = now.local_date_key() as i64;
        let day_start = now.start_of_local_day();
        ctx.schedule_alarm_at(
            day_start.add_seconds(self.entry_seconds + 59),
            Self::entry_alarm_key(day_key),
        );
        ctx.schedule_alarm_at(
            day_start.add_seconds(self.exit_seconds + 59),
            Self::exit_alarm_key(day_key),
        );
    }

    fn ensure_index_instrument_id(&mut self, ctx: &Context) -> Option<u32> {
        if self.index_instrument_id.is_none() {
            self.index_instrument_id = ctx.market_data.get_id(&self.index_symbol);
        }
        self.index_instrument_id
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

    fn resolve_pair_by_premium(
        &self,
        ctx: &Context,
        expiry_yyyymmdd: i32,
    ) -> Option<(u32, u32)> {
        let mut best_ce: Option<(u32, i64)> = None;
        let mut best_pe: Option<(u32, i64)> = None;

        for (instrument_id, _) in ctx.market_data.iter_ids() {
            let instrument = match ctx.market_data.get_instrument(instrument_id) {
                Some(v) => v,
                None => continue,
            };
            let option = match &instrument.option {
                Some(v) => v,
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

            let bar = match ctx.get_bar(instrument.id) {
                Some(b) => b,
                None => continue,
            };

            let distance = (bar.close - self.target_premium).abs();

            match option.option_type {
                OptionType::Call => {
                    if best_ce.map(|(_, d)| distance < d).unwrap_or(true) {
                        best_ce = Some((instrument.id, distance));
                    }
                }
                OptionType::Put => {
                    if best_pe.map(|(_, d)| distance < d).unwrap_or(true) {
                        best_pe = Some((instrument.id, distance));
                    }
                }
            }
        }

        match (best_ce, best_pe) {
            (Some((ce_id, ce_dist)), Some((pe_id, pe_dist))) => {
                let ce_premium = ctx.get_bar(ce_id).map(|b| b.close).unwrap_or(0);
                let pe_premium = ctx.get_bar(pe_id).map(|b| b.close).unwrap_or(0);
                log::info!(
                    "resolve_pair_by_premium: target={} ce_id={} ce_premium={:.2} dist={:.2} pe_id={} pe_premium={:.2} dist={:.2}",
                    self.target_premium as f64 / PRICE_SCALE as f64,
                    ce_id,
                    ce_premium as f64 / PRICE_SCALE as f64,
                    ce_dist as f64 / PRICE_SCALE as f64,
                    pe_id,
                    pe_premium as f64 / PRICE_SCALE as f64,
                    pe_dist as f64 / PRICE_SCALE as f64,
                );
                Some((ce_id, pe_id))
            }
            _ => {
                log::warn!(
                    "resolve_pair_by_premium: no pair found for expiry={} target_premium={}",
                    expiry_yyyymmdd,
                    self.target_premium as f64 / PRICE_SCALE as f64,
                );
                None
            }
        }
    }

    fn process_entry(&mut self, ctx: &mut Context, day_key: i64) {
        if self.entered_days.contains(&day_key) {
            return;
        }

        let Some(_index_id) = self.ensure_index_instrument_id(ctx) else {
            return;
        };
        let today_yyyymmdd = ctx.now().local_date_key();
        let Some(expiry_yyyymmdd) = self.nearest_weekly_expiry(ctx, today_yyyymmdd) else {
            log::warn!("nifty_premium_straddle: no weekly expiry for {}", today_yyyymmdd);
            return;
        };

        let Some((ce_id, pe_id)) = self.resolve_pair_by_premium(ctx, expiry_yyyymmdd) else {
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

        log::info!(
            "nifty_premium_straddle: entering day={} ce_id={} ce_entry={:.2} ce_stop={:.2} pe_id={} pe_entry={:.2} pe_stop={:.2}",
            day_key,
            ce_id,
            ce_entry as f64 / PRICE_SCALE as f64,
            ce_stop_price as f64 / PRICE_SCALE as f64,
            pe_id,
            pe_entry as f64 / PRICE_SCALE as f64,
            pe_stop_price as f64 / PRICE_SCALE as f64,
        );

        ctx.place_order(ce_id, Side::Sell, OrderType::Market, self.quantity);
        ctx.place_order(pe_id, Side::Sell, OrderType::Market, self.quantity);

        let ce_stop_order_id =
            ctx.place_order(ce_id, Side::Buy, OrderType::Stop(ce_stop_price), self.quantity);
        let pe_stop_order_id =
            ctx.place_order(pe_id, Side::Buy, OrderType::Stop(pe_stop_price), self.quantity);

        self.day_state.insert(
            day_key,
            DailyState {
                ce_id,
                pe_id,
                ce_stop_order_id,
                pe_stop_order_id,
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
            if let Some(s) = self.day_state.get_mut(&day_key) {
                s.ce_open = false;
                s.pe_open = false;
            }
        }
    }

    fn sync_leg_state_from_positions(&mut self, ctx: &Context, day_key: i64) {
        if let Some(state) = self.day_state.get_mut(&day_key) {
            state.ce_open = ctx.position_qty(state.ce_id) < 0;
            state.pe_open = ctx.position_qty(state.pe_id) < 0;
        }
    }
}

impl Strategy for NiftyPremiumStraddleStrategy {
    fn on_start(&mut self, ctx: &mut Context) {
        self.index_instrument_id = ctx.market_data.get_id(&self.index_symbol);
    }

    fn on_date_change(&mut self, ctx: &mut Context) {
        self.schedule_day_alarms(ctx, ctx.now());
    }

    fn on_market_event(&mut self, _ctx: &mut Context, _event: &MarketEvent) {}

    fn on_alarm(&mut self, ctx: &mut Context, event: &AlarmEvent) {
        let Some((kind, day_key)) = Self::parse_alarm_key(&event.key) else {
            return;
        };
        match kind {
            "ps_entry" => self.process_entry(ctx, day_key),
            "ps_exit" => self.process_exit(ctx, day_key),
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
