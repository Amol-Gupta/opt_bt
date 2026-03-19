use std::collections::HashMap;
use std::collections::HashSet;

use bt_strategy_macros::bt_strategy;
use opt_bt::common::context::Context;
use opt_bt::common::event::{AlarmEvent, FillEvent, OrderEvent, SignalEvent};
use opt_bt::common::types::{OptionType, OrderType, Side, PRICE_SCALE};
use opt_bt::strategy::Strategy;

#[bt_strategy(
    id = "nifty_straddle_portfolio",
    display_name = "Nifty Straddle Portfolio (3 time windows)",
    description = "Portfolio of 3 ATM straddle strategies: morning (09:30-11:30), midday (11:30-13:30), afternoon (13:30-15:15). Params: sl_pct, qty."
)]
pub fn nifty_straddle_portfolio_registration() {}

// ---------------------------------------------------------------------------
// NiftyStraddlePortfolioStrategy
//
// Runs N independent ATM short-straddle windows inside a single strategy
// instance. Each window has its own entry/exit times and state. Alarm keys
// are namespaced "entry:<window_idx>:<day_key>" so they never clash across
// windows and route cleanly through the single outer PortfolioStrategy that
// bt_run_project.rs creates.
// ---------------------------------------------------------------------------

struct PortfolioWindow {
    name: String,
    entry_seconds: i64,
    exit_seconds: i64,
}

#[derive(Clone, Debug, Default)]
struct WindowDayState {
    ce_id: u32,
    pe_id: u32,
    ce_stop_order_id: u64,
    pe_stop_order_id: u64,
    ce_open: bool,
    pe_open: bool,
}

pub struct NiftyStraddlePortfolioStrategy {
    index_symbol: String,
    index_instrument_id: Option<u32>,
    option_underlying: String,
    quantity: i64,
    stop_loss_ratio: f64,
    windows: Vec<PortfolioWindow>,
    day_state: HashMap<(i64, usize), WindowDayState>,
    entered: HashSet<(i64, usize)>,
}

impl NiftyStraddlePortfolioStrategy {
    pub fn new(
        index_symbol: &str,
        option_underlying: &str,
        quantity: i64,
        stop_loss_ratio: f64,
        windows: Vec<(&str, i64, i64)>,
    ) -> Self {
        Self {
            index_symbol: index_symbol.to_string(),
            index_instrument_id: None,
            option_underlying: option_underlying.to_ascii_uppercase(),
            quantity,
            stop_loss_ratio,
            windows: windows
                .into_iter()
                .map(|(name, entry, exit)| PortfolioWindow {
                    name: name.to_string(),
                    entry_seconds: entry,
                    exit_seconds: exit,
                })
                .collect(),
            day_state: HashMap::new(),
            entered: HashSet::new(),
        }
    }

    fn ensure_index_id(&mut self, ctx: &Context) -> Option<u32> {
        if self.index_instrument_id.is_none() {
            self.index_instrument_id = ctx.market_data.get_id(&self.index_symbol);
        }
        self.index_instrument_id
    }

    fn yyyymmdd_to_date(v: i32) -> Option<chrono::NaiveDate> {
        chrono::NaiveDate::from_ymd_opt(v / 10_000, ((v / 100) % 100) as u32, (v % 100) as u32)
    }

    fn dte(today: i32, expiry: i32) -> Option<i64> {
        let a = Self::yyyymmdd_to_date(today)?;
        let b = Self::yyyymmdd_to_date(expiry)?;
        Some((b - a).num_days())
    }

    fn nearest_weekly_expiry(&self, ctx: &Context, today: i32) -> Option<i32> {
        ctx.market_data
            .iter_ids()
            .into_iter()
            .filter_map(|(id, _)| ctx.market_data.get_instrument(id))
            .filter_map(|ins| {
                let opt = ins.option.as_ref()?;
                if !opt.underlying.to_ascii_uppercase().starts_with(&self.option_underlying) {
                    return None;
                }
                let dte = Self::dte(today, opt.expiry_yyyymmdd)?;
                if (0..=7).contains(&dte) { Some(opt.expiry_yyyymmdd) } else { None }
            })
            .min()
    }

    fn resolve_atm_pair(&self, ctx: &Context, expiry: i32, spot: i64) -> Option<(u32, u32)> {
        let mut pairs: std::collections::BTreeMap<i64, (Option<u32>, Option<u32>)> =
            std::collections::BTreeMap::new();
        for (id, _) in ctx.market_data.iter_ids() {
            let ins = match ctx.market_data.get_instrument(id) {
                Some(v) => v,
                None => continue,
            };
            let opt = match &ins.option {
                Some(v) => v,
                None => continue,
            };
            if opt.expiry_yyyymmdd != expiry { continue; }
            if !opt.underlying.to_ascii_uppercase().starts_with(&self.option_underlying) {
                continue;
            }
            if ctx.get_bar(ins.id).is_none() { continue; }
            let strike = opt.strike / PRICE_SCALE;
            let entry = pairs.entry(strike).or_default();
            match opt.option_type {
                OptionType::Call => { if entry.0.is_none() { entry.0 = Some(ins.id); } }
                OptionType::Put  => { if entry.1.is_none() { entry.1 = Some(ins.id); } }
            }
        }
        let strikes: Vec<i64> = pairs.keys().copied().collect();
        let step = strikes.windows(2).filter_map(|w| {
            let g = w[1] - w[0]; if g > 0 { Some(g) } else { None }
        }).min().unwrap_or(50);
        let target = ((spot + step / 2) / step) * step;
        pairs.iter()
            .filter_map(|(s, (ce, pe))| {
                let (ce_id, pe_id) = ((*ce)?, (*pe)?);
                Some(((s - target).unsigned_abs(), ce_id, pe_id))
            })
            .min_by_key(|(d, _, _)| *d)
            .map(|(_, ce, pe)| (ce, pe))
    }

    fn entry_key(widx: usize, day_key: i64) -> String {
        format!("entry:{}:{}", widx, day_key)
    }

    fn exit_key(widx: usize, day_key: i64) -> String {
        format!("exit:{}:{}", widx, day_key)
    }

    fn parse_key(key: &str) -> Option<(&str, usize, i64)> {
        let mut parts = key.splitn(3, ':');
        let kind = parts.next()?;
        let widx = parts.next()?.parse::<usize>().ok()?;
        let day_key = parts.next()?.parse::<i64>().ok()?;
        Some((kind, widx, day_key))
    }

    fn process_entry(&mut self, ctx: &mut Context, widx: usize, day_key: i64) {
        if self.entered.contains(&(day_key, widx)) {
            return;
        }
        let Some(idx_id) = self.ensure_index_id(ctx) else { return };
        let today = ctx.now().local_date_key();
        let Some(expiry) = self.nearest_weekly_expiry(ctx, today) else { return };
        let Some(idx_bar) = ctx.get_bar(idx_id) else { return };
        let spot = idx_bar.close / PRICE_SCALE;
        let Some((ce_id, pe_id)) = self.resolve_atm_pair(ctx, expiry, spot) else { return };
        let Some(ce_bar) = ctx.get_bar(ce_id) else { return };
        let Some(pe_bar) = ctx.get_bar(pe_id) else { return };

        let ce_stop = ((ce_bar.close as f64) * (1.0 + self.stop_loss_ratio)).round() as i64;
        let pe_stop = ((pe_bar.close as f64) * (1.0 + self.stop_loss_ratio)).round() as i64;

        ctx.place_order(ce_id, Side::Sell, OrderType::Market, self.quantity);
        ctx.place_order(pe_id, Side::Sell, OrderType::Market, self.quantity);
        let ce_sl_id = ctx.place_order(ce_id, Side::Buy, OrderType::Stop(ce_stop), self.quantity);
        let pe_sl_id = ctx.place_order(pe_id, Side::Buy, OrderType::Stop(pe_stop), self.quantity);

        let win_name = &self.windows[widx].name;
        log::info!("portfolio entry window={} day={} ce={} pe={} spot={}", win_name, day_key, ce_id, pe_id, spot);

        self.day_state.insert(
            (day_key, widx),
            WindowDayState {
                ce_id,
                pe_id,
                ce_stop_order_id: ce_sl_id,
                pe_stop_order_id: pe_sl_id,
                ce_open: true,
                pe_open: true,
            },
        );
        self.entered.insert((day_key, widx));
    }

    fn process_exit(&mut self, ctx: &mut Context, widx: usize, day_key: i64) {
        if let Some(state) = self.day_state.get(&(day_key, widx)).cloned() {
            ctx.cancel_order(state.ce_stop_order_id);
            ctx.cancel_order(state.pe_stop_order_id);
            if state.ce_open && ctx.position_qty(state.ce_id) < 0 {
                ctx.place_order(state.ce_id, Side::Buy, OrderType::Market, self.quantity);
            }
            if state.pe_open && ctx.position_qty(state.pe_id) < 0 {
                ctx.place_order(state.pe_id, Side::Buy, OrderType::Market, self.quantity);
            }
            if let Some(s) = self.day_state.get_mut(&(day_key, widx)) {
                s.ce_open = false;
                s.pe_open = false;
            }
        }
    }

    fn sync_fills(&mut self, ctx: &Context, day_key: i64) {
        for widx in 0..self.windows.len() {
            if let Some(state) = self.day_state.get_mut(&(day_key, widx)) {
                state.ce_open = ctx.position_qty(state.ce_id) < 0;
                state.pe_open = ctx.position_qty(state.pe_id) < 0;
            }
        }
    }
}

impl Strategy for NiftyStraddlePortfolioStrategy {
    fn on_start(&mut self, ctx: &mut Context) {
        self.index_instrument_id = ctx.market_data.get_id(&self.index_symbol);
    }

    fn on_date_change(&mut self, ctx: &mut Context) {
        let now = ctx.now();
        let day_key = now.local_date_key() as i64;
        let day_start = now.start_of_local_day();
        for (widx, win) in self.windows.iter().enumerate() {
            // +59s so alarm fires at the first bar at-or-after the target minute
            ctx.schedule_alarm_at(day_start.add_seconds(win.entry_seconds + 59), &Self::entry_key(widx, day_key));
            ctx.schedule_alarm_at(day_start.add_seconds(win.exit_seconds + 59),  &Self::exit_key(widx, day_key));
        }
    }

    fn on_alarm(&mut self, ctx: &mut Context, event: &AlarmEvent) {
        let Some((kind, widx, day_key)) = Self::parse_key(&event.key) else { return };
        if widx >= self.windows.len() { return; }
        match kind {
            "entry" => self.process_entry(ctx, widx, day_key),
            "exit"  => self.process_exit(ctx, widx, day_key),
            _ => {}
        }
    }

    fn on_fill(&mut self, ctx: &mut Context, event: &FillEvent) {
        let day_key = event.timestamp.div_euclid(86_400);
        self.sync_fills(ctx, day_key);
    }

    fn on_signal(&mut self, _ctx: &mut Context, _event: &SignalEvent) {}
    fn on_order_event(&mut self, _ctx: &mut Context, _event: &OrderEvent) {}
}
