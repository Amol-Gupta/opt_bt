use crate::common::context::Context;
use crate::common::event::{FillEvent, MarketEvent, OrderEvent, SignalEvent};
use crate::common::types::{InstrumentKind, OptionType, OrderType, Side, PRICE_SCALE};
use crate::strategy::Strategy;
use std::collections::{HashMap, HashSet};

pub struct NiftyNearestExpiryStraddleStrategy {
    index_symbol: String,
    quantity: i64,
    entered_days: HashSet<i64>,
    exited_days: HashSet<i64>,
    active_legs: HashMap<i64, (u32, u32)>,
    event_log: Vec<String>,
}

impl NiftyNearestExpiryStraddleStrategy {
    pub fn new(index_symbol: &str, quantity: i64) -> Self {
        Self {
            index_symbol: index_symbol.to_string(),
            quantity,
            entered_days: HashSet::new(),
            exited_days: HashSet::new(),
            active_legs: HashMap::new(),
            event_log: Vec::new(),
        }
    }

    pub fn event_log(&self) -> &[String] {
        &self.event_log
    }

    fn log_event<S: Into<String>>(&mut self, message: S) {
        let msg = message.into();
        println!("NiftyNearestExpiryStraddleStrategy: {msg}");
        self.event_log.push(msg);
    }

    fn timestamp_to_yyyymmdd(timestamp: i64) -> Option<i32> {
        chrono::DateTime::from_timestamp(timestamp, 0)
            .and_then(|dt| dt.format("%Y%m%d").to_string().parse::<i32>().ok())
    }

    fn nearest_expiry_for_day(&self, ctx: &Context, today_yyyymmdd: i32) -> Option<i32> {
        ctx.market_data
            .iter_ids()
            .into_iter()
            .filter_map(|(instrument_id, _)| ctx.market_data.get_instrument(instrument_id))
            .filter_map(|instrument| {
                if instrument.kind != InstrumentKind::Option {
                    return None;
                }
                let option = instrument.option.as_ref()?;
                if option.underlying != "NIFTY" {
                    return None;
                }
                if option.expiry_yyyymmdd < today_yyyymmdd {
                    return None;
                }
                Some(option.expiry_yyyymmdd)
            })
            .min()
    }

    fn build_nearest_expiry_chain(&self, ctx: &Context, expiry_yyyymmdd: i32) -> Vec<u32> {
        let mut ids: Vec<u32> = ctx
            .market_data
            .iter_ids()
            .into_iter()
            .filter_map(|(instrument_id, _)| ctx.market_data.get_instrument(instrument_id))
            .filter_map(|instrument| {
                if instrument.kind != InstrumentKind::Option {
                    return None;
                }
                let option = instrument.option.as_ref()?;
                if option.underlying != "NIFTY" || option.expiry_yyyymmdd != expiry_yyyymmdd {
                    return None;
                }
                Some(instrument.id)
            })
            .collect();
        ids.sort_unstable();
        ids
    }

    fn resolve_atm_legs(
        &self,
        ctx: &Context,
        spot_points: i64,
        day_key: i64,
    ) -> Option<(u32, u32)> {
        let mut best_ce: Option<(u32, i64)> = None;
        let mut best_pe: Option<(u32, i64)> = None;

        for instrument_id in ctx.active_subscriptions() {
            let instrument = match ctx.market_data.get_instrument(instrument_id) {
                Some(v) => v,
                None => continue,
            };
            let option = match &instrument.option {
                Some(v) => v,
                None => continue,
            };

            let distance = ((option.strike / PRICE_SCALE) - spot_points).abs();
            match option.option_type {
                OptionType::Call => {
                    if best_ce.map(|(_, d)| distance < d).unwrap_or(true) {
                        best_ce = Some((instrument_id, distance));
                    }
                }
                OptionType::Put => {
                    if best_pe.map(|(_, d)| distance < d).unwrap_or(true) {
                        best_pe = Some((instrument_id, distance));
                    }
                }
            }
        }

        match (best_ce, best_pe) {
            (Some((ce, _)), Some((pe, _))) => Some((ce, pe)),
            _ => {
                println!(
                    "NiftyNearestExpiryStraddleStrategy: no ATM pair for day={} spot_points={}",
                    day_key, spot_points
                );
                None
            }
        }
    }

    fn log_position_pnl(&mut self, ctx: &Context, stage: &str) {
        let snapshots = ctx.position_wise_pnl();
        if snapshots.is_empty() {
            self.log_event(format!("{} position_pnl: none", stage));
            return;
        }

        for snapshot in snapshots {
            let symbol = ctx
                .market_data
                .get_symbol(snapshot.instrument_id)
                .unwrap_or_else(|| format!("ID:{}", snapshot.instrument_id));
            self.log_event(format!(
                "{} position_pnl instrument_id={} symbol={} qty={} realized={} unrealized={} mtm={}",
                stage,
                snapshot.instrument_id,
                symbol,
                snapshot.quantity,
                snapshot.realized_pnl,
                snapshot.unrealized_pnl,
                snapshot.mtm_pnl
            ));
        }
    }
}

impl Strategy for NiftyNearestExpiryStraddleStrategy {
    fn on_start(&mut self, _ctx: &mut Context) {
        self.log_event("on_start");
    }

    fn on_stop(&mut self, _ctx: &mut Context) {
        self.log_event("on_stop");
    }

    fn on_date_change(&mut self, ctx: &mut Context) {
        self.log_event(format!("on_date_change ts={}", ctx.now()));
    }

    fn before_open(&mut self, ctx: &mut Context) {
        self.log_event(format!("before_open ts={}", ctx.now()));
        let Some(today_yyyymmdd) = Self::timestamp_to_yyyymmdd(ctx.now()) else {
            self.log_event("before_open skipped: unable to derive yyyymmdd".to_string());
            return;
        };

        let Some(expiry_yyyymmdd) = self.nearest_expiry_for_day(ctx, today_yyyymmdd) else {
            self.log_event(format!(
                "before_open: no nearest expiry found for date={}",
                today_yyyymmdd
            ));
            ctx.clear_desired_subscriptions();
            let _ = ctx.apply_subscription_diff();
            return;
        };

        let chain = self.build_nearest_expiry_chain(ctx, expiry_yyyymmdd);
        ctx.set_desired_subscriptions(chain);
        let diff = ctx.apply_subscription_diff();
        self.log_event(format!(
            "before_open subscribed={} unsubscribed={} unchanged={} expiry={}",
            diff.subscribed.len(),
            diff.unsubscribed.len(),
            diff.unchanged.len(),
            expiry_yyyymmdd
        ));
    }

    fn after_close(&mut self, ctx: &mut Context) {
        self.log_event(format!("after_close ts={}", ctx.now()));
        self.log_position_pnl(ctx, "after_close");
        ctx.clear_desired_subscriptions();
        let diff = ctx.apply_subscription_diff();
        self.log_event(format!(
            "after_close subscribed={} unsubscribed={} unchanged={}",
            diff.subscribed.len(),
            diff.unsubscribed.len(),
            diff.unchanged.len()
        ));
    }

    fn on_market_event(&mut self, ctx: &mut Context, event: &MarketEvent) {
        self.log_event(format!(
            "on_market_event ts={} instrument_id={}",
            event.timestamp, event.instrument_id
        ));

        let symbol = match ctx.market_data.get_symbol(event.instrument_id) {
            Some(value) => value,
            None => return,
        };

        if symbol != self.index_symbol {
            return;
        }

        let day_key = event.timestamp.div_euclid(86_400);
        let seconds_of_day = event.timestamp.rem_euclid(86_400);
        let entry_time = 10 * 60 * 60;
        let exit_time = 11 * 60 * 60;

        if !self.entered_days.contains(&day_key)
            && seconds_of_day >= entry_time
            && seconds_of_day < exit_time
        {
            let Some(index_bar) = ctx.get_bar(event.instrument_id) else {
                return;
            };
            let spot_points = index_bar.close / PRICE_SCALE;
            if let Some((ce_id, pe_id)) = self.resolve_atm_legs(ctx, spot_points, day_key) {
                ctx.place_order(ce_id, Side::Sell, OrderType::Market, self.quantity);
                ctx.place_order(pe_id, Side::Sell, OrderType::Market, self.quantity);
                self.active_legs.insert(day_key, (ce_id, pe_id));
                self.entered_days.insert(day_key);
                self.log_event(format!(
                    "entered straddle day={} ce_id={} pe_id={} qty={}",
                    day_key, ce_id, pe_id, self.quantity
                ));
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
                self.log_event(format!(
                    "exited straddle day={} ce_id={} pe_id={} qty={}",
                    day_key, ce_id, pe_id, self.quantity
                ));
            }
        }
    }

    fn on_signal(&mut self, _ctx: &mut Context, event: &SignalEvent) {
        self.log_event(format!(
            "on_signal ts={} strategy_id={} instrument_id={} qty={}",
            event.timestamp, event.strategy_id, event.instrument_id, event.quantity
        ));
    }

    fn on_order_event(&mut self, _ctx: &mut Context, event: &OrderEvent) {
        self.log_event(format!(
            "on_order_event ts={} order_id={} strategy_id={} qty={}",
            event.timestamp, event.order_id, event.strategy_id, event.quantity
        ));
    }

    fn on_fill(&mut self, _ctx: &mut Context, event: &FillEvent) {
        self.log_event(format!(
            "on_fill ts={} order_id={} strategy_id={} qty={} price={}",
            event.timestamp, event.order_id, event.strategy_id, event.quantity, event.fill_price
        ));
    }
}
