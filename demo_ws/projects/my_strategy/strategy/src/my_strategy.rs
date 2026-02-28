use bt_strategy_macros::bt_strategy;
use opt_bt::common::context::Context;
use opt_bt::common::event::{FillEvent, MarketEvent, OrderEvent, SignalEvent};
use opt_bt::common::types::{OptionType, OrderType, Side, PRICE_SCALE};
use opt_bt::strategy::Strategy;
use std::collections::{BTreeMap, HashMap, HashSet};

#[bt_strategy(
    id = "my_strategy",
    display_name = "My Strategy",
    description = "Sample scaffold strategy with simple bullish bar behavior"
)]
pub fn my_strategy_registration() {}

#[bt_strategy(
    id = "algotest_weekly_straddle",
    display_name = "Algotest Weekly Straddle",
    description = "11:00 sell ATM CE/PE on nearest weekly expiry with 20% SL and 15:15 exit"
)]
pub fn algotest_weekly_straddle_registration() {}

#[derive(Debug)]
pub struct MyStrategy;

impl std::fmt::Display for MyStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Strategy for MyStrategy {
    fn on_start(&mut self, _ctx: &mut Context) {
        log::info!("MyStrategy on_start called");
    }
    fn on_stop(&mut self, _ctx: &mut Context) {
        log::info!("MyStrategy on_stop called");    
    }
    fn on_date_change(&mut self, _ctx: &mut Context) {
        log::info!("MyStrategy on_date_change called");
    }

    fn before_open(&mut self, _ctx: &mut Context) {
        log::info!("MyStrategy before_open called");
    }
    fn after_close(&mut self, _ctx: &mut Context) {
        log::info!("MyStrategy after_close called");
    }
    
    fn on_market_event(&mut self, ctx: &mut Context, event: &MarketEvent) {
        if let Some(bar) = ctx.get_bar(event.instrument_id) {
            let symbol = ctx
                .market_data
                .get_symbol(event.instrument_id)
                .unwrap_or("UNKNOWN");
            // log::info!(
            //     "[BAR] ts={} symbol={} open={:.4} high={:.4} low={:.4} close={:.4} volume={}",
            //     bar.timestamp,
            //     symbol,
            //     bar.open as f64 / PRICE_SCALE as f64,
            //     bar.high as f64 / PRICE_SCALE as f64,
            //     bar.low as f64 / PRICE_SCALE as f64,
            //     bar.close as f64 / PRICE_SCALE as f64,
            //     bar.volume
            // );
            // log::info!("bar {:?}", bar);
                
                
            // if bar.close > bar.open {
            //     let _ = ctx.place_order(event.instrument_id, Side::Buy, OrderType::Market, 1);
            // }
        }
    }

    fn on_signal(&mut self, _ctx: &mut Context, _event: &SignalEvent) {
        log::info!("MyStrategy on_signal called {:?}", _event);
    }
    fn on_order_event(&mut self, _ctx: &mut Context, _event: &OrderEvent) {
        log::info!("MyStrategy on_order_event called {:?}", _event);
    }
    fn on_fill(&mut self, _ctx: &mut Context, _event: &FillEvent) {
        log::info!("MyStrategy on_fill called {:?}", _event);
    }
}

#[derive(Debug, Clone)]
struct DailyState {
    ce_id: u32,
    pe_id: u32,
    ce_stop_price: i64,
    pe_stop_price: i64,
    ce_open: bool,
    pe_open: bool,
}

#[derive(Debug)]
pub struct AlgotestWeeklyStraddleStrategy {
    index_symbol: String,
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

    fn nearest_weekly_expiry(&self, ctx: &Context, today_yyyymmdd: i32) -> Option<i32> {
        ctx.market_data
            .instrument_meta
            .values()
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

        log::info!(
            "resolve_atm_pair: start expiry={} spot_points={} underlying={}",
            expiry_yyyymmdd,
            spot_points,
            self.option_underlying
        );

        for instrument in ctx.market_data.instrument_meta.values() {
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
            let symbol = ctx.market_data.get_symbol(instrument.id).unwrap_or("UNKNOWN");
            let slot = strike_pairs.entry(strike_points).or_default();

            match option.option_type {
                OptionType::Call => {
                    call_candidates += 1;
                    if slot.ce_id.is_none() {
                        slot.ce_id = Some(instrument.id);
                        log::info!(
                            "resolve_atm_pair: CE candidate symbol={} strike={}",
                            symbol,
                            strike_points
                        );
                    }
                }
                OptionType::Put => {
                    put_candidates += 1;
                    if slot.pe_id.is_none() {
                        slot.pe_id = Some(instrument.id);
                        log::info!(
                            "resolve_atm_pair: PE candidate symbol={} strike={}",
                            symbol,
                            strike_points
                        );
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

        log::info!(
            "resolve_atm_pair: strike_grid count={} min_gap={} target_strike={} spot_points={}",
            strikes.len(),
            strike_step,
            target_strike,
            spot_points
        );

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
            Some((selected_strike, ce_id, pe_id)) => {
                let ce_symbol = ctx.market_data.get_symbol(ce_id).unwrap_or("UNKNOWN");
                let pe_symbol = ctx.market_data.get_symbol(pe_id).unwrap_or("UNKNOWN");
                let ce_strike = ctx
                    .market_data
                    .get_instrument(ce_id)
                    .and_then(|ins| ins.option.as_ref())
                    .map(|o| o.strike / PRICE_SCALE)
                    .unwrap_or_default();
                let pe_strike = ctx
                    .market_data
                    .get_instrument(pe_id)
                    .and_then(|ins| ins.option.as_ref())
                    .map(|o| o.strike / PRICE_SCALE)
                    .unwrap_or_default();
                let ce_dist = (ce_strike - spot_points).abs();
                let pe_dist = (pe_strike - spot_points).abs();

                log::info!(
                    "resolve_atm_pair: selected target_strike={} ce={} strike={} dist={} | pe={} strike={} dist={} | candidates ce={} pe={}",
                    selected_strike,
                    ce_symbol,
                    ce_strike,
                    ce_dist,
                    pe_symbol,
                    pe_strike,
                    pe_dist,
                    call_candidates,
                    put_candidates
                );

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
    fn on_market_event(&mut self, ctx: &mut Context, event: &MarketEvent) {
        let day_key = event.timestamp.div_euclid(86_400);
        let seconds_of_day = event.timestamp.rem_euclid(86_400);

        if let Some(state) = self.day_state.get(&day_key).cloned() {
            if state.ce_open {
                if let Some(bar) = ctx.get_bar(state.ce_id) {
                    if bar.high >= state.ce_stop_price {
                        ctx.place_order(
                            state.ce_id,
                            Side::Buy,
                            OrderType::Stop(state.ce_stop_price),
                            self.quantity,
                        );
                        if let Some(updated_state) = self.day_state.get_mut(&day_key) {
                            updated_state.ce_open = false;
                        }
                    }
                }
            }

            if state.pe_open {
                if let Some(bar) = ctx.get_bar(state.pe_id) {
                    if bar.high >= state.pe_stop_price {
                        ctx.place_order(
                            state.pe_id,
                            Side::Buy,
                            OrderType::Stop(state.pe_stop_price),
                            self.quantity,
                        );
                        if let Some(updated_state) = self.day_state.get_mut(&day_key) {
                            updated_state.pe_open = false;
                        }
                    }
                }
            }
        }

        if seconds_of_day >= self.exit_seconds {
            if let Some(state) = self.day_state.get(&day_key).cloned() {
                if state.ce_open {
                    ctx.place_order(state.ce_id, Side::Buy, OrderType::Market, self.quantity);
                }
                if state.pe_open {
                    ctx.place_order(state.pe_id, Side::Buy, OrderType::Market, self.quantity);
                }
                if let Some(updated_state) = self.day_state.get_mut(&day_key) {
                    updated_state.ce_open = false;
                    updated_state.pe_open = false;
                }
            }
            return;
        }

        let symbol = match ctx.market_data.get_symbol(event.instrument_id) {
            Some(value) => value,
            None => return,
        };

        if symbol != self.index_symbol {
            return;
        }

        if self.entered_days.contains(&day_key) {
            return;
        }

        if seconds_of_day < self.entry_seconds {
            return;
        }

        let Some(today_yyyymmdd) = Self::timestamp_to_yyyymmdd(event.timestamp) else {
            return;
        };
        let Some(expiry_yyyymmdd) = self.nearest_weekly_expiry(ctx, today_yyyymmdd) else {
            return;
        };

        let Some(index_bar) = ctx.get_bar(event.instrument_id) else {
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

        self.day_state.insert(
            day_key,
            DailyState {
                ce_id,
                pe_id,
                ce_stop_price,
                pe_stop_price,
                ce_open: true,
                pe_open: true,
            },
        );
        self.entered_days.insert(day_key);
    }

    fn on_signal(&mut self, _ctx: &mut Context, _event: &SignalEvent) {}
    fn on_order_event(&mut self, _ctx: &mut Context, _event: &OrderEvent) {}
    fn on_fill(&mut self, _ctx: &mut Context, _event: &FillEvent) {}
}

pub fn create_strategy_by_id(
    strategy_id: &str,
    params: &std::collections::BTreeMap<String, String>,
) -> Option<Box<dyn Strategy + Send>> {
    let quantity = params
        .get("qty")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(65);
    let stop_loss_pct = params
        .get("sl_pct")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.20);
    let entry_seconds = params
        .get("entry_seconds")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(11 * 60 * 60);
    let exit_seconds = params
        .get("exit_seconds")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(15 * 60 + 15 * 60 * 60);

    match strategy_id {
        "my_strategy" => Some(Box::new(MyStrategy)),
        "algotest_weekly_straddle" => Some(Box::new(AlgotestWeeklyStraddleStrategy::new(
            "NIFTY 50",
            "NIFTY",
            quantity,
            entry_seconds,
            exit_seconds,
            stop_loss_pct,
        ))),
        _ => None,
    }
}
