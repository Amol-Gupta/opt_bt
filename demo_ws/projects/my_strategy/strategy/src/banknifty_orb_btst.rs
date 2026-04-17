use chrono::{Datelike, Timelike};
use opt_bt::common::context::Context;
use opt_bt::common::event::{FillEvent, MarketEvent, OrderEvent, SignalEvent};
use opt_bt::common::types::{OptionType, OrderType, Side, SimTime, PRICE_SCALE};
use opt_bt::strategy::Strategy;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum LegSide {
    Call,
    Put,
}

impl LegSide {
    fn as_str(self) -> &'static str {
        match self {
            Self::Call => "CE",
            Self::Put => "PE",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExitReason {
    Target,
    StopLoss,
    Time,
}

impl ExitReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Target => "target",
            Self::StopLoss => "stop_loss",
            Self::Time => "time",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegStatus {
    WaitingForBreakout,
    PendingEntry,
    Active,
    PendingExit,
    ClosedNoEntry,
    Exited,
}

#[derive(Debug, Clone)]
struct LegState {
    instrument_id: u32,
    reference_high: i64,
    status: LegStatus,
    entry_order_id: u64,
    exit_order_id: u64,
    entry_price: i64,
    target_price: i64,
    stop_price: i64,
    entry_timestamp: Option<SimTime>,
    exit_timestamp: Option<SimTime>,
    exit_price: i64,
    exit_reason: Option<ExitReason>,
}

impl LegState {
    fn new(instrument_id: u32, reference_high: i64) -> Self {
        Self {
            instrument_id,
            reference_high,
            status: LegStatus::WaitingForBreakout,
            entry_order_id: 0,
            exit_order_id: 0,
            entry_price: 0,
            target_price: 0,
            stop_price: 0,
            entry_timestamp: None,
            exit_timestamp: None,
            exit_price: 0,
            exit_reason: None,
        }
    }

    fn is_terminal(&self) -> bool {
        matches!(self.status, LegStatus::ClosedNoEntry | LegStatus::Exited)
    }
}

#[derive(Debug, Clone)]
struct DailyOrbState {
    expiry_yyyymmdd: i32,
    ce: LegState,
    pe: LegState,
    range_finalized: bool,
}

#[derive(Debug, Default, Clone)]
struct StrategyStats {
    instances_created: u32,
    entries: u32,
    exits_target: u32,
    exits_stop: u32,
    exits_time: u32,
    no_breakout_closures: u32,
}

#[derive(Debug, Clone)]
struct SessionSummaryRow {
    trade_day: i32,
    expiry_yyyymmdd: i32,
    leg: &'static str,
    symbol: String,
    reference_high: i64,
    entry_timestamp: Option<SimTime>,
    entry_price: i64,
    exit_timestamp: Option<SimTime>,
    exit_price: i64,
    exit_reason: String,
    final_status: String,
}

#[derive(Debug)]
pub struct BankniftyOrbBtstStrategy {
    index_symbol: String,
    option_underlying: String,
    quantity: i64,
    closest_premium: i64,
    range_start_seconds: i64,
    range_end_seconds: i64,
    entry_cutoff_seconds: i64,
    day2_exit_seconds: i64,
    target_pct: f64,
    stop_loss_pct: f64,
    index_instrument_id: Option<u32>,
    day_state: HashMap<i32, DailyOrbState>,
    entry_orders: HashMap<u64, (i32, LegSide)>,
    exit_orders: HashMap<u64, (i32, LegSide, ExitReason)>,
    stats: StrategyStats,
}

impl BankniftyOrbBtstStrategy {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        index_symbol: &str,
        option_underlying: &str,
        quantity: i64,
        closest_premium: i64,
        range_start_seconds: i64,
        range_end_seconds: i64,
        entry_cutoff_seconds: i64,
        day2_exit_seconds: i64,
        target_pct: f64,
        stop_loss_pct: f64,
    ) -> Self {
        Self {
            index_symbol: index_symbol.to_string(),
            option_underlying: option_underlying.to_ascii_uppercase(),
            quantity,
            closest_premium,
            range_start_seconds,
            range_end_seconds,
            entry_cutoff_seconds,
            day2_exit_seconds,
            target_pct,
            stop_loss_pct,
            index_instrument_id: None,
            day_state: HashMap::new(),
            entry_orders: HashMap::new(),
            exit_orders: HashMap::new(),
            stats: StrategyStats::default(),
        }
    }

    fn current_seconds_of_day(timestamp: SimTime) -> i64 {
        let dt = timestamp.local_datetime();
        (dt.hour() as i64 * 3600) + (dt.minute() as i64 * 60) + dt.second() as i64
    }

    fn yyyymmdd_to_date(yyyymmdd: i32) -> Option<chrono::NaiveDate> {
        let year = yyyymmdd / 10_000;
        let month = ((yyyymmdd / 100) % 100) as u32;
        let day = (yyyymmdd % 100) as u32;
        chrono::NaiveDate::from_ymd_opt(year, month, day)
    }

    fn exact_bar(&self, ctx: &Context, instrument_id: u32, timestamp: SimTime) -> Option<opt_bt::data::models::Bar> {
        ctx.market_data.get_bar_at(instrument_id, timestamp)
    }

    fn resolve_index_instrument_id(&mut self, ctx: &Context) -> Option<u32> {
        if self.index_instrument_id.is_some() {
            return self.index_instrument_id;
        }

        let aliases = [
            self.index_symbol.as_str(),
            "BANKNIFTY",
            "NIFTY BANK",
        ];
        for alias in aliases {
            if let Some(instrument_id) = ctx.market_data.get_id(alias) {
                self.index_instrument_id = Some(instrument_id);
                if alias != self.index_symbol {
                    log::info!(
                        "banknifty_orb_btst: resolved index symbol alias requested={} resolved={}",
                        self.index_symbol,
                        alias
                    );
                }
                break;
            }
        }

        self.index_instrument_id
    }

    fn nearest_monthly_expiry(&self, ctx: &Context, today_yyyymmdd: i32) -> Option<i32> {
        let today = Self::yyyymmdd_to_date(today_yyyymmdd)?;
        let mut month_to_expiry: BTreeMap<(i32, u32), i32> = BTreeMap::new();

        for (instrument_id, _) in ctx.market_data.iter_ids() {
            let instrument = match ctx.market_data.get_instrument(instrument_id) {
                Some(value) => value,
                None => continue,
            };
            let option = match &instrument.option {
                Some(value) => value,
                None => continue,
            };
            if !option
                .underlying
                .to_ascii_uppercase()
                .starts_with(&self.option_underlying)
            {
                continue;
            }

            let Some(expiry_date) = Self::yyyymmdd_to_date(option.expiry_yyyymmdd) else {
                continue;
            };
            if expiry_date <= today {
                continue;
            }

            let month_key = (expiry_date.year(), expiry_date.month());
            month_to_expiry
                .entry(month_key)
                .and_modify(|current| {
                    if option.expiry_yyyymmdd > *current {
                        *current = option.expiry_yyyymmdd;
                    }
                })
                .or_insert(option.expiry_yyyymmdd);
        }

        month_to_expiry.into_values().min()
    }

    fn resolve_pair_by_premium(
        &self,
        ctx: &Context,
        expiry_yyyymmdd: i32,
        timestamp: SimTime,
    ) -> Option<(u32, u32)> {
        let mut best_ce: Option<(u32, i64)> = None;
        let mut best_pe: Option<(u32, i64)> = None;

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

            let Some(bar) = self.exact_bar(ctx, instrument.id, timestamp) else {
                continue;
            };
            let distance = (bar.close - self.closest_premium).abs();

            match option.option_type {
                OptionType::Call => {
                    if best_ce.map(|(_, best_distance)| distance < best_distance).unwrap_or(true) {
                        best_ce = Some((instrument.id, distance));
                    }
                }
                OptionType::Put => {
                    if best_pe.map(|(_, best_distance)| distance < best_distance).unwrap_or(true) {
                        best_pe = Some((instrument.id, distance));
                    }
                }
            }
        }

        match (best_ce, best_pe) {
            (Some((ce_id, _)), Some((pe_id, _))) => Some((ce_id, pe_id)),
            _ => None,
        }
    }

    fn ensure_today_instance(&mut self, ctx: &Context, today_yyyymmdd: i32) {
        if self.day_state.contains_key(&today_yyyymmdd) {
            return;
        }

        let Some(expiry_yyyymmdd) = self.nearest_monthly_expiry(ctx, today_yyyymmdd) else {
            log::warn!(
                "banknifty_orb_btst: no monthly expiry found for trade_day={}",
                today_yyyymmdd
            );
            return;
        };
        let Some((ce_id, pe_id)) = self.resolve_pair_by_premium(ctx, expiry_yyyymmdd, ctx.now()) else {
            log::warn!(
                "banknifty_orb_btst: no CE/PE premium pair found trade_day={} expiry={}",
                today_yyyymmdd,
                expiry_yyyymmdd
            );
            return;
        };

        let Some(ce_bar) = self.exact_bar(ctx, ce_id, ctx.now()) else {
            return;
        };
        let Some(pe_bar) = self.exact_bar(ctx, pe_id, ctx.now()) else {
            return;
        };

        let ce_symbol = ctx
            .market_data
            .get_symbol(ce_id)
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let pe_symbol = ctx
            .market_data
            .get_symbol(pe_id)
            .unwrap_or_else(|| "UNKNOWN".to_string());
        log::info!(
            "banknifty_orb_btst: selected trade_day={} expiry={} ce_symbol={} ce_close={:.2} pe_symbol={} pe_close={:.2} target_premium={:.2}",
            today_yyyymmdd,
            expiry_yyyymmdd,
            ce_symbol,
            ce_bar.close as f64 / PRICE_SCALE as f64,
            pe_symbol,
            pe_bar.close as f64 / PRICE_SCALE as f64,
            self.closest_premium as f64 / PRICE_SCALE as f64,
        );

        self.day_state.insert(
            today_yyyymmdd,
            DailyOrbState {
                expiry_yyyymmdd,
                ce: LegState::new(ce_id, ce_bar.high),
                pe: LegState::new(pe_id, pe_bar.high),
                range_finalized: false,
            },
        );
        self.stats.instances_created += 1;
    }

    fn update_range(&mut self, ctx: &Context, trade_day: i32) {
        let Some((ce_id, pe_id)) = self
            .day_state
            .get(&trade_day)
            .map(|state| (state.ce.instrument_id, state.pe.instrument_id))
        else {
            return;
        };

        let ce_bar = self.exact_bar(ctx, ce_id, ctx.now());
        let pe_bar = self.exact_bar(ctx, pe_id, ctx.now());

        let Some(state) = self.day_state.get_mut(&trade_day) else {
            return;
        };

        if let Some(bar) = ce_bar {
            state.ce.reference_high = state.ce.reference_high.max(bar.high);
        }
        if let Some(bar) = pe_bar {
            state.pe.reference_high = state.pe.reference_high.max(bar.high);
        }
    }

    fn finalize_range_if_needed(&mut self, ctx: &Context, trade_day: i32) {
        let Some(state) = self.day_state.get_mut(&trade_day) else {
            return;
        };
        if state.range_finalized {
            return;
        }

        let ce_symbol = ctx
            .market_data
            .get_symbol(state.ce.instrument_id)
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let pe_symbol = ctx
            .market_data
            .get_symbol(state.pe.instrument_id)
            .unwrap_or_else(|| "UNKNOWN".to_string());
        log::info!(
            "banknifty_orb_btst: range_finalized trade_day={} expiry={} ce_symbol={} ce_reference_high={:.2} pe_symbol={} pe_reference_high={:.2}",
            trade_day,
            state.expiry_yyyymmdd,
            ce_symbol,
            state.ce.reference_high as f64 / PRICE_SCALE as f64,
            pe_symbol,
            state.pe.reference_high as f64 / PRICE_SCALE as f64,
        );
        state.range_finalized = true;
    }

    fn maybe_trigger_breakout_entry(&mut self, ctx: &mut Context, trade_day: i32, leg_side: LegSide) {
        let (instrument_id, reference_high, status) = match self.day_state.get(&trade_day) {
            Some(state) => {
                let leg = match leg_side {
                    LegSide::Call => &state.ce,
                    LegSide::Put => &state.pe,
                };
                (leg.instrument_id, leg.reference_high, leg.status)
            }
            None => return,
        };

        if status != LegStatus::WaitingForBreakout {
            return;
        }

        let Some(bar) = self.exact_bar(ctx, instrument_id, ctx.now()) else {
            return;
        };
        if bar.close <= reference_high {
            return;
        }

        let order_id = ctx.place_order(instrument_id, Side::Buy, OrderType::Market, self.quantity);
        if order_id == 0 {
            log::warn!(
                "banknifty_orb_btst: entry rejected trade_day={} leg={} instrument_id={}",
                trade_day,
                leg_side.as_str(),
                instrument_id
            );
            return;
        }

        if let Some(state) = self.day_state.get_mut(&trade_day) {
            let leg = match leg_side {
                LegSide::Call => &mut state.ce,
                LegSide::Put => &mut state.pe,
            };
            leg.status = LegStatus::PendingEntry;
            leg.entry_order_id = order_id;
        }
        self.entry_orders.insert(order_id, (trade_day, leg_side));
        log::info!(
            "banknifty_orb_btst: breakout_trigger trade_day={} leg={} symbol={} close={:.2} reference_high={:.2}",
            trade_day,
            leg_side.as_str(),
            ctx.market_data
                .get_symbol(instrument_id)
                .unwrap_or_else(|| "UNKNOWN".to_string()),
            bar.close as f64 / PRICE_SCALE as f64,
            reference_high as f64 / PRICE_SCALE as f64,
        );
    }

    fn close_waiting_legs_after_cutoff(&mut self, ctx: &Context, trade_day: i32) {
        let Some(state) = self.day_state.get_mut(&trade_day) else {
            return;
        };

        for (leg_side, leg) in [(LegSide::Call, &mut state.ce), (LegSide::Put, &mut state.pe)] {
            if leg.status == LegStatus::WaitingForBreakout {
                leg.status = LegStatus::ClosedNoEntry;
                self.stats.no_breakout_closures += 1;
                log::info!(
                    "banknifty_orb_btst: no_breakout_close trade_day={} leg={} symbol={} cutoff_reached=true",
                    trade_day,
                    leg_side.as_str(),
                    ctx.market_data
                        .get_symbol(leg.instrument_id)
                        .unwrap_or_else(|| "UNKNOWN".to_string()),
                );
            }
        }
    }

    fn maybe_trigger_active_exit(
        &mut self,
        ctx: &mut Context,
        trade_day: i32,
        leg_side: LegSide,
        current_timestamp: SimTime,
    ) {
        let snapshot = match self.day_state.get(&trade_day) {
            Some(state) => {
                let leg = match leg_side {
                    LegSide::Call => &state.ce,
                    LegSide::Put => &state.pe,
                };
                (
                    leg.instrument_id,
                    leg.status,
                    leg.entry_timestamp,
                    leg.target_price,
                    leg.stop_price,
                )
            }
            None => return,
        };

        let (instrument_id, status, entry_timestamp, target_price, stop_price) = snapshot;
        if status != LegStatus::Active {
            return;
        }

        let Some(entry_timestamp) = entry_timestamp else {
            return;
        };
        if current_timestamp <= entry_timestamp {
            return;
        }

        let Some(bar) = self.exact_bar(ctx, instrument_id, current_timestamp) else {
            return;
        };
        let stop_hit = bar.low <= stop_price;
        let target_hit = bar.high >= target_price;
        if !stop_hit && !target_hit {
            return;
        }

        let exit_reason = if stop_hit {
            ExitReason::StopLoss
        } else {
            ExitReason::Target
        };
        if stop_hit && target_hit {
            log::warn!(
                "banknifty_orb_btst: both target and stop touched in same bar trade_day={} leg={} symbol={} close={:.2} using_conservative_exit={}",
                trade_day,
                leg_side.as_str(),
                ctx.market_data
                    .get_symbol(instrument_id)
                    .unwrap_or_else(|| "UNKNOWN".to_string()),
                bar.close as f64 / PRICE_SCALE as f64,
                exit_reason.as_str(),
            );
        }

        let order_id = ctx.place_order(instrument_id, Side::Sell, OrderType::Market, self.quantity);
        if order_id == 0 {
            log::warn!(
                "banknifty_orb_btst: exit rejected trade_day={} leg={} reason={} instrument_id={}",
                trade_day,
                leg_side.as_str(),
                exit_reason.as_str(),
                instrument_id
            );
            return;
        }

        if let Some(state) = self.day_state.get_mut(&trade_day) {
            let leg = match leg_side {
                LegSide::Call => &mut state.ce,
                LegSide::Put => &mut state.pe,
            };
            leg.status = LegStatus::PendingExit;
            leg.exit_order_id = order_id;
            leg.exit_reason = Some(exit_reason);
        }
        self.exit_orders.insert(order_id, (trade_day, leg_side, exit_reason));
        log::info!(
            "banknifty_orb_btst: exit_trigger trade_day={} leg={} symbol={} reason={} close={:.2} target={:.2} stop={:.2}",
            trade_day,
            leg_side.as_str(),
            ctx.market_data
                .get_symbol(instrument_id)
                .unwrap_or_else(|| "UNKNOWN".to_string()),
            exit_reason.as_str(),
            bar.close as f64 / PRICE_SCALE as f64,
            target_price as f64 / PRICE_SCALE as f64,
            stop_price as f64 / PRICE_SCALE as f64,
        );
    }

    fn maybe_trigger_day2_time_exit(
        &mut self,
        ctx: &mut Context,
        trade_day: i32,
        current_day: i32,
        current_seconds: i64,
    ) {
        if current_day <= trade_day || current_seconds < self.day2_exit_seconds {
            return;
        }

        for leg_side in [LegSide::Call, LegSide::Put] {
            let (instrument_id, status) = match self.day_state.get(&trade_day) {
                Some(state) => {
                    let leg = match leg_side {
                        LegSide::Call => &state.ce,
                        LegSide::Put => &state.pe,
                    };
                    (leg.instrument_id, leg.status)
                }
                None => return,
            };

            if status != LegStatus::Active {
                continue;
            }

            let order_id = ctx.place_order(instrument_id, Side::Sell, OrderType::Market, self.quantity);
            if order_id == 0 {
                log::warn!(
                    "banknifty_orb_btst: day2 time exit rejected trade_day={} leg={} instrument_id={}",
                    trade_day,
                    leg_side.as_str(),
                    instrument_id
                );
                continue;
            }

            if let Some(state) = self.day_state.get_mut(&trade_day) {
                let leg = match leg_side {
                    LegSide::Call => &mut state.ce,
                    LegSide::Put => &mut state.pe,
                };
                leg.status = LegStatus::PendingExit;
                leg.exit_order_id = order_id;
                leg.exit_reason = Some(ExitReason::Time);
            }
            self.exit_orders
                .insert(order_id, (trade_day, leg_side, ExitReason::Time));
            log::info!(
                "banknifty_orb_btst: day2_time_exit_trigger trade_day={} active_day={} leg={} symbol={} cutoff_seconds={}",
                trade_day,
                current_day,
                leg_side.as_str(),
                ctx.market_data
                    .get_symbol(instrument_id)
                    .unwrap_or_else(|| "UNKNOWN".to_string()),
                self.day2_exit_seconds,
            );
        }
    }

    fn leg_mut(state: &mut DailyOrbState, leg_side: LegSide) -> &mut LegState {
        match leg_side {
            LegSide::Call => &mut state.ce,
            LegSide::Put => &mut state.pe,
        }
    }

    fn output_dir() -> Option<PathBuf> {
        std::env::var("BT_BACKTEST_OUTPUT_DIR")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
    }

    fn format_timestamp(timestamp: Option<SimTime>) -> String {
        timestamp
            .map(|value| value.local_datetime().format("%Y-%m-%d %H:%M:%S %:z").to_string())
            .unwrap_or_default()
    }

    fn format_price(price: i64) -> String {
        if price == 0 {
            String::new()
        } else {
            format!("{:.2}", price as f64 / PRICE_SCALE as f64)
        }
    }

    fn csv_escape(value: &str) -> String {
        if value.contains(',') || value.contains('"') || value.contains('\n') {
            let escaped = value.replace('"', "\"\"");
            format!("\"{}\"", escaped)
        } else {
            value.to_string()
        }
    }

    fn build_summary_rows(&self, ctx: &Context) -> Vec<SessionSummaryRow> {
        let mut trade_days = self.day_state.keys().copied().collect::<Vec<_>>();
        trade_days.sort_unstable();

        let mut rows = Vec::with_capacity(trade_days.len() * 2);
        for trade_day in trade_days {
            let Some(state) = self.day_state.get(&trade_day) else {
                continue;
            };

            for (leg_side, leg) in [(LegSide::Call, &state.ce), (LegSide::Put, &state.pe)] {
                let symbol = ctx
                    .market_data
                    .get_symbol(leg.instrument_id)
                    .unwrap_or_else(|| "UNKNOWN".to_string());
                let final_status = match leg.status {
                    LegStatus::WaitingForBreakout => "waiting_for_breakout",
                    LegStatus::PendingEntry => "pending_entry",
                    LegStatus::Active => "active",
                    LegStatus::PendingExit => "pending_exit",
                    LegStatus::ClosedNoEntry => "closed_no_entry",
                    LegStatus::Exited => "exited",
                }
                .to_string();
                let exit_reason = leg
                    .exit_reason
                    .map(|value| value.as_str().to_string())
                    .unwrap_or_else(|| {
                        if leg.status == LegStatus::ClosedNoEntry {
                            "no_breakout".to_string()
                        } else {
                            String::new()
                        }
                    });

                rows.push(SessionSummaryRow {
                    trade_day,
                    expiry_yyyymmdd: state.expiry_yyyymmdd,
                    leg: leg_side.as_str(),
                    symbol,
                    reference_high: leg.reference_high,
                    entry_timestamp: leg.entry_timestamp,
                    entry_price: leg.entry_price,
                    exit_timestamp: leg.exit_timestamp,
                    exit_price: leg.exit_price,
                    exit_reason,
                    final_status,
                });
            }
        }

        rows
    }

    fn write_summary_csv(path: &Path, rows: &[SessionSummaryRow]) -> Result<(), std::io::Error> {
        let mut output = String::new();
        output.push_str(
            "trade_day,expiry_yyyymmdd,leg,symbol,reference_high,entry_time,entry_price,exit_time,exit_price,exit_reason,final_status\n",
        );

        for row in rows {
            let _ = writeln!(
                output,
                "{},{},{},{},{},{},{},{},{},{},{}",
                row.trade_day,
                row.expiry_yyyymmdd,
                row.leg,
                Self::csv_escape(&row.symbol),
                Self::format_price(row.reference_high),
                Self::csv_escape(&Self::format_timestamp(row.entry_timestamp)),
                Self::format_price(row.entry_price),
                Self::csv_escape(&Self::format_timestamp(row.exit_timestamp)),
                Self::format_price(row.exit_price),
                Self::csv_escape(&row.exit_reason),
                Self::csv_escape(&row.final_status),
            );
        }

        std::fs::write(path, output)
    }
}

impl Strategy for BankniftyOrbBtstStrategy {
    fn on_start(&mut self, ctx: &mut Context) {
        self.index_instrument_id = self.resolve_index_instrument_id(ctx);
        log::info!(
            "banknifty_orb_btst: start index_symbol={} option_underlying={} qty={} closest_premium={:.2} range_start_seconds={} range_end_seconds={} entry_cutoff_seconds={} day2_exit_seconds={} target_pct={} stop_loss_pct={}",
            self.index_symbol,
            self.option_underlying,
            self.quantity,
            self.closest_premium as f64 / PRICE_SCALE as f64,
            self.range_start_seconds,
            self.range_end_seconds,
            self.entry_cutoff_seconds,
            self.day2_exit_seconds,
            self.target_pct,
            self.stop_loss_pct,
        );
    }

    fn on_market_event(&mut self, ctx: &mut Context, event: &MarketEvent) {
        let current_timestamp = event.timestamp;
        let current_day = current_timestamp.local_date_key();
        let current_seconds = Self::current_seconds_of_day(current_timestamp);

        if current_seconds >= self.range_start_seconds {
            self.ensure_today_instance(ctx, current_day);
        }

        if current_seconds >= self.range_start_seconds && current_seconds <= self.range_end_seconds {
            self.update_range(ctx, current_day);
        }

        if current_seconds > self.range_end_seconds {
            self.finalize_range_if_needed(ctx, current_day);
        }

        if current_seconds > self.range_end_seconds && current_seconds <= self.entry_cutoff_seconds {
            self.maybe_trigger_breakout_entry(ctx, current_day, LegSide::Call);
            self.maybe_trigger_breakout_entry(ctx, current_day, LegSide::Put);
        }

        if current_seconds > self.entry_cutoff_seconds {
            self.close_waiting_legs_after_cutoff(ctx, current_day);
        }

        let tracked_days = self.day_state.keys().copied().collect::<Vec<_>>();
        for trade_day in tracked_days {
            self.maybe_trigger_active_exit(ctx, trade_day, LegSide::Call, current_timestamp);
            self.maybe_trigger_active_exit(ctx, trade_day, LegSide::Put, current_timestamp);
            self.maybe_trigger_day2_time_exit(ctx, trade_day, current_day, current_seconds);
        }
    }

    fn on_signal(&mut self, _ctx: &mut Context, _event: &SignalEvent) {}

    fn on_order_event(&mut self, _ctx: &mut Context, _event: &OrderEvent) {}

    fn on_fill(&mut self, ctx: &mut Context, event: &FillEvent) {
        if event.side == Side::Buy {
            let Some((trade_day, leg_side)) = self.entry_orders.remove(&event.order_id) else {
                return;
            };

            if let Some(state) = self.day_state.get_mut(&trade_day) {
                let leg = Self::leg_mut(state, leg_side);
                leg.status = LegStatus::Active;
                leg.entry_price = event.fill_price;
                leg.entry_timestamp = Some(event.timestamp);
                leg.exit_timestamp = None;
                leg.exit_price = 0;
                leg.target_price = ((event.fill_price as f64) * (1.0 + self.target_pct)).round() as i64;
                leg.stop_price = ((event.fill_price as f64) * (1.0 - self.stop_loss_pct)).round() as i64;
                leg.entry_order_id = event.order_id;
            }

            self.stats.entries += 1;
            log::info!(
                "banknifty_orb_btst: entry_fill trade_day={} leg={} symbol={} entry_price={:.2} target_price={:.2} stop_price={:.2} fill_time={}",
                trade_day,
                leg_side.as_str(),
                ctx.market_data
                    .get_symbol(event.instrument_id)
                    .unwrap_or_else(|| "UNKNOWN".to_string()),
                event.fill_price as f64 / PRICE_SCALE as f64,
                self.day_state
                    .get(&trade_day)
                    .map(|state| match leg_side {
                        LegSide::Call => state.ce.target_price,
                        LegSide::Put => state.pe.target_price,
                    })
                    .unwrap_or_default() as f64
                    / PRICE_SCALE as f64,
                self.day_state
                    .get(&trade_day)
                    .map(|state| match leg_side {
                        LegSide::Call => state.ce.stop_price,
                        LegSide::Put => state.pe.stop_price,
                    })
                    .unwrap_or_default() as f64
                    / PRICE_SCALE as f64,
                event.timestamp.local_datetime().format("%Y-%m-%d %H:%M:%S %:z")
            );
            return;
        }

        let Some((trade_day, leg_side, exit_reason)) = self.exit_orders.remove(&event.order_id) else {
            return;
        };

        if let Some(state) = self.day_state.get_mut(&trade_day) {
            let leg = Self::leg_mut(state, leg_side);
            leg.status = LegStatus::Exited;
            leg.exit_order_id = event.order_id;
            leg.exit_timestamp = Some(event.timestamp);
            leg.exit_price = event.fill_price;
            leg.exit_reason = Some(exit_reason);
        }

        match exit_reason {
            ExitReason::Target => self.stats.exits_target += 1,
            ExitReason::StopLoss => self.stats.exits_stop += 1,
            ExitReason::Time => self.stats.exits_time += 1,
        }

        log::info!(
            "banknifty_orb_btst: exit_fill trade_day={} leg={} symbol={} reason={} fill_price={:.2} fill_time={}",
            trade_day,
            leg_side.as_str(),
            ctx.market_data
                .get_symbol(event.instrument_id)
                .unwrap_or_else(|| "UNKNOWN".to_string()),
            exit_reason.as_str(),
            event.fill_price as f64 / PRICE_SCALE as f64,
            event.timestamp.local_datetime().format("%Y-%m-%d %H:%M:%S %:z")
        );
    }

    fn on_stop(&mut self, _ctx: &mut Context) {
        let active_instances = self
            .day_state
            .values()
            .filter(|state| !state.ce.is_terminal() || !state.pe.is_terminal())
            .count();
        if let Some(output_dir) = Self::output_dir() {
            let rows = self.build_summary_rows(_ctx);
            let summary_path = output_dir.join("btst_session_summary.csv");
            match Self::write_summary_csv(&summary_path, &rows) {
                Ok(_) => log::info!(
                    "banknifty_orb_btst: wrote session summary rows={} path={}",
                    rows.len(),
                    summary_path.display()
                ),
                Err(err) => log::warn!(
                    "banknifty_orb_btst: failed to write session summary path={} err={}",
                    summary_path.display(),
                    err
                ),
            }
        }
        log::info!(
            "banknifty_orb_btst: summary instances_created={} entries={} exits_target={} exits_stop={} exits_time={} no_breakout_closures={} active_instances_remaining={}",
            self.stats.instances_created,
            self.stats.entries,
            self.stats.exits_target,
            self.stats.exits_stop,
            self.stats.exits_time,
            self.stats.no_breakout_closures,
            active_instances
        );
    }
}