use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use bt_strategy_macros::bt_strategy;
use chrono::{Datelike, NaiveDate};
use opt_bt::common::context::Context;
use opt_bt::common::event::{AlarmEvent, FillEvent, MarketEvent, OrderEvent, SignalEvent};
use opt_bt::common::types::{OrderType, Side, PRICE_SCALE};
use opt_bt::strategy::Strategy;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// 9:10 AM in seconds since midnight (IST local time)
const ENTRY_SECONDS: i64 = 9 * 3600 + 10 * 60;
/// 9:30 AM in seconds since midnight (IST local time)
const EXIT_SECONDS: i64 = 9 * 3600 + 30 * 60;

// ---------------------------------------------------------------------------
// CSV row — one order candidate from the pricing model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct CsvOrder {
    /// Resolved ticker, e.g. "NIFTY06JAN2625500PE"
    ticker: String,
    quantity: i64,
    entry_price: i64,  // stored as PRICE_SCALE units (×100)
    target_price: i64, // stored as PRICE_SCALE units (×100)
}

// ---------------------------------------------------------------------------
// Per-order tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct PendingEntry {
    instrument_id: u32,
    quantity: i64,
    target_price: i64,
}

// ---------------------------------------------------------------------------
// Trade recording
// ---------------------------------------------------------------------------

/// An entry fill that has not yet been exited.
#[derive(Debug, Clone)]
struct OpenTrade {
    ticker: String,
    date: String,       // YYYY-MM-DD
    entry_time: String, // HH:MM:SS
    entry_price: i64,   // PRICE_SCALE units
    quantity: i64,
}

/// A fully closed round-trip trade.
#[derive(Debug, Clone)]
struct TradeRecord {
    ticker: String,
    date: String,
    entry_time: String,
    exit_time: String,
    exit_reason: String, // "TARGET" or "TIME"
    entry_price: f64,
    exit_price: f64,
    quantity: i64,
    pnl: f64,
}

// ---------------------------------------------------------------------------
// Daily metrics
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
struct DailyStats {
    orders_placed: u32,
    filled_count: u32,
    target_hit_count: u32,
}

// ---------------------------------------------------------------------------
// Strategy
// ---------------------------------------------------------------------------

#[bt_strategy(
    id = "option_buying",
    display_name = "Option Buying",
    description = "Deep OTM option buying: limit entries at 9:10 AM, target exits, hard cutoff at 9:30 AM"
)]
pub fn option_buying_registration() {}

pub struct OptionBuyingStrategy {
    csv_folder: String,
    /// All CSV orders indexed by YYYYMMDD
    daily_orders: HashMap<i32, Vec<CsvOrder>>,
    /// Pending entry (buy) orders for today: order_id → info
    pending_entries: HashMap<u64, PendingEntry>,
    /// Pending target (sell) orders for today: order_id → instrument_id
    pending_targets: HashMap<u64, u32>,
    /// Days where we already placed entry orders (dedup guard)
    entered_days: HashSet<i32>,
    /// Cumulative totals across all days
    total_stats: DailyStats,
    /// Stats for the current day only (reset on date change)
    today_stats: DailyStats,
    /// instrument_id → ticker name (populated when placing entry orders)
    ticker_map: HashMap<u32, String>,
    /// instrument_id → open trade info (entry filled but not yet exited)
    open_trades: HashMap<u32, OpenTrade>,
    /// All completed round-trip trades
    completed_trades: Vec<TradeRecord>,
}

impl OptionBuyingStrategy {
    pub fn new(csv_folder: &str) -> Self {
        Self {
            csv_folder: csv_folder.to_string(),
            daily_orders: HashMap::new(),
            pending_entries: HashMap::new(),
            pending_targets: HashMap::new(),
            entered_days: HashSet::new(),
            total_stats: DailyStats::default(),
            today_stats: DailyStats::default(),
            ticker_map: HashMap::new(),
            open_trades: HashMap::new(),
            completed_trades: Vec::new(),
        }
    }

    // -----------------------------------------------------------------------
    // CSV loading
    // -----------------------------------------------------------------------

    fn load_all_csvs(&mut self) {
        let folder = Path::new(&self.csv_folder);
        let entries = match std::fs::read_dir(folder) {
            Ok(e) => e,
            Err(err) => {
                log::error!(
                    "option_buying: cannot read csv_folder={} err={}",
                    self.csv_folder,
                    err
                );
                return;
            }
        };

        let mut loaded = 0usize;
        let mut skipped = 0usize;

        for dir_entry in entries.flatten() {
            let path = dir_entry.path();
            let Some(fname) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !fname.starts_with("BUY") || !fname.ends_with(".csv") {
                continue;
            }
            // "BUY02JAN2026.csv" → date_part = "02JAN2026"
            let date_part = &fname[3..fname.len() - 4];
            let Some(yyyymmdd) = parse_filename_date(date_part) else {
                log::warn!(
                    "option_buying: cannot parse date from filename={}",
                    fname
                );
                skipped += 1;
                continue;
            };

            match Self::load_csv_file(&path, yyyymmdd) {
                Ok(orders) => {
                    self.daily_orders.insert(yyyymmdd, orders);
                    loaded += 1;
                }
                Err(err) => {
                    log::warn!("option_buying: failed to load {} err={}", fname, err);
                    skipped += 1;
                }
            }
        }

        log::info!(
            "option_buying: loaded_csv_days={} skipped={}",
            loaded,
            skipped
        );
    }

    fn load_csv_file(
        path: &Path,
        yyyymmdd: i32,
    ) -> Result<Vec<CsvOrder>, Box<dyn std::error::Error>> {
        let mut reader = csv::Reader::from_path(path)?;
        let headers = reader.headers()?.clone();

        let col_idx = |name: &str| -> Option<usize> {
            headers
                .iter()
                .position(|h| h.trim().eq_ignore_ascii_case(name))
        };

        let idx_symbol = col_idx("Symbol").ok_or("missing Symbol column")?;
        let idx_expiry = col_idx("Expiry Date").ok_or("missing Expiry Date column")?;
        let idx_strike = col_idx("Today_Strike").ok_or("missing Strike Price column")?;
        let idx_opt_type = col_idx("Option Type").ok_or("missing Option Type column")?;
        let idx_qty = col_idx("Quantity").ok_or("missing Quantity column")?;
        let idx_entry = col_idx("Entry_Price").ok_or("missing Entry_Price column")?;
        let idx_target = col_idx("Target_price").ok_or("missing Target_price column")?;

        let mut orders = Vec::new();

        for result in reader.records() {
            let record = result?;

            let symbol = record
                .get(idx_symbol)
                .map(str::trim)
                .unwrap_or("")
                .to_ascii_uppercase();
            let expiry_str = record.get(idx_expiry).map(str::trim).unwrap_or("");
            let strike_str = record.get(idx_strike).map(str::trim).unwrap_or("");
            let opt_type = record
                .get(idx_opt_type)
                .map(str::trim)
                .unwrap_or("")
                .to_ascii_uppercase();
            let qty_str = record.get(idx_qty).map(str::trim).unwrap_or("");
            let entry_str = record.get(idx_entry).map(str::trim).unwrap_or("");
            let target_str = record.get(idx_target).map(str::trim).unwrap_or("");

            if symbol.is_empty() || expiry_str.is_empty() {
                continue;
            }

            let Some(expiry_yyyymmdd) = parse_iso_date(expiry_str) else {
                log::debug!(
                    "option_buying: yyyymmdd={} bad expiry='{}' skipping row",
                    yyyymmdd,
                    expiry_str
                );
                continue;
            };

            let Ok(strike) = strike_str.parse::<f64>() else {
                continue;
            };
            let Ok(quantity) = qty_str.parse::<f64>() else {
                continue;
            };
            let Ok(entry_f) = entry_str.parse::<f64>() else {
                continue;
            };
            let Ok(target_f) = target_str.parse::<f64>() else {
                continue;
            };

            let entry_price = (entry_f * PRICE_SCALE as f64).round() as i64;
            let target_price = (target_f * PRICE_SCALE as f64).round() as i64;
            let quantity = quantity as i64;
            let strike = strike as i64;

            if entry_price <= 0 || target_price <= 0 || quantity <= 0 {
                continue;
            }

            let ticker = build_ticker(&symbol, expiry_yyyymmdd, strike, &opt_type);
            orders.push(CsvOrder {
                ticker,
                quantity,
                entry_price,
                target_price,
            });
        }

        Ok(orders)
    }

    // -----------------------------------------------------------------------
    // Alarm keys
    // -----------------------------------------------------------------------

    fn enter_key(day: i32) -> String {
        format!("ob_enter:{}", day)
    }

    fn cutoff_key(day: i32) -> String {
        format!("ob_cutoff:{}", day)
    }

    fn parse_alarm_key(key: &str) -> Option<(&str, i32)> {
        let (kind, raw) = key.split_once(':')?;
        let day = raw.parse::<i32>().ok()?;
        Some((kind, day))
    }

    // -----------------------------------------------------------------------
    // Schedule alarms for today
    // -----------------------------------------------------------------------

    fn schedule_day_alarms(&self, ctx: &mut Context) {
        let now = ctx.now();
        let day_start = now.start_of_local_day();
        let day_key = now.local_date_key();

        ctx.schedule_alarm_at(
            day_start.add_seconds(ENTRY_SECONDS),
            Self::enter_key(day_key),
        );
        ctx.schedule_alarm_at(
            day_start.add_seconds(EXIT_SECONDS),
            Self::cutoff_key(day_key),
        );

        log::debug!("option_buying: scheduled alarms for day={}", day_key);
    }

    // -----------------------------------------------------------------------
    // 9:10 AM — place limit buy orders from today's CSV
    // -----------------------------------------------------------------------

    fn process_entry(&mut self, ctx: &mut Context, day_key: i32) {
        if self.entered_days.contains(&day_key) {
            return;
        }
        self.entered_days.insert(day_key);

        let Some(orders) = self.daily_orders.get(&day_key).cloned() else {
            log::info!(
                "option_buying: no CSV orders for day={}, skipping entry",
                day_key
            );
            return;
        };

        let mut placed = 0u32;
        for csv_order in &orders {
            let Some(instrument_id) = ctx.market_data.get_id(&csv_order.ticker) else {
                log::debug!(
                    "option_buying: ticker not in market data ticker={} day={}",
                    csv_order.ticker,
                    day_key
                );
                continue;
            };

            let order_id = ctx.place_order(
                instrument_id,
                Side::Buy,
                OrderType::Limit(csv_order.entry_price),
                csv_order.quantity,
            );

            self.ticker_map
                .insert(instrument_id, csv_order.ticker.clone());
            self.pending_entries.insert(
                order_id,
                PendingEntry {
                    instrument_id,
                    quantity: csv_order.quantity,
                    target_price: csv_order.target_price,
                },
            );

            placed += 1;
        }

        self.today_stats.orders_placed = placed;
        log::info!(
            "option_buying: day={} placed_entry_orders={}",
            day_key,
            placed
        );
    }

    // -----------------------------------------------------------------------
    // 9:30 AM — cancel everything, close open positions
    // -----------------------------------------------------------------------

    fn process_cutoff(&mut self, ctx: &mut Context, day_key: i32) {
        // Cancel all still-pending entry (buy) orders
        let entry_ids: Vec<u64> = self.pending_entries.keys().copied().collect();
        for oid in &entry_ids {
            ctx.cancel_order(*oid);
        }
        self.pending_entries.clear();

        // Cancel all still-pending target (sell) orders
        let target_ids: Vec<u64> = self.pending_targets.keys().copied().collect();
        for oid in &target_ids {
            ctx.cancel_order(*oid);
        }

        // Market-sell every open long position
        let open = ctx.open_positions();
        let mut closed = 0usize;
        for (instrument_id, qty, _cost) in open {
            if qty > 0 {
                ctx.place_order(instrument_id, Side::Sell, OrderType::Market, qty);
                closed += 1;
            }
        }
        self.pending_targets.clear();

        // Roll today stats into total
        self.total_stats.orders_placed += self.today_stats.orders_placed;
        self.total_stats.filled_count += self.today_stats.filled_count;
        self.total_stats.target_hit_count += self.today_stats.target_hit_count;

        log::info!(
            "option_buying: cutoff day={} cancelled_entries={} cancelled_targets={} market_closed={}",
            day_key,
            entry_ids.len(),
            target_ids.len(),
            closed
        );
    }

    // -----------------------------------------------------------------------
    // Write completed trades to CSV
    // -----------------------------------------------------------------------

    fn write_trades_csv(
        path: &Path,
        trades: &[TradeRecord],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut writer = csv::Writer::from_path(path)?;
        writer.write_record(&[
            "contract",
            "date",
            "entry_time",
            "exit_time",
            "exit_reason",
            "entry_price",
            "exit_price",
            "quantity",
            "pnl",
        ])?;
        for t in trades {
            writer.write_record(&[
                t.ticker.as_str(),
                t.date.as_str(),
                t.entry_time.as_str(),
                t.exit_time.as_str(),
                t.exit_reason.as_str(),
                &format!("{:.2}", t.entry_price),
                &format!("{:.2}", t.exit_price),
                &t.quantity.to_string(),
                &format!("{:.2}", t.pnl),
            ])?;
        }
        writer.flush()?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Strategy trait
// ---------------------------------------------------------------------------

impl Strategy for OptionBuyingStrategy {
    fn on_start(&mut self, _ctx: &mut Context) {
        self.load_all_csvs();
        log::info!(
            "option_buying: startup loaded_days={} csv_folder={}",
            self.daily_orders.len(),
            self.csv_folder
        );
    }

    fn on_date_change(&mut self, ctx: &mut Context) {
        self.today_stats = DailyStats::default();
        self.schedule_day_alarms(ctx);
    }

    fn before_open(&mut self, ctx: &mut Context) {
        let day_key = ctx.now().local_date_key();
        let csv_rows = self.daily_orders.get(&day_key).map(|v| v.len()).unwrap_or(0);
        log::debug!("option_buying: before_open day={} csv_rows={}", day_key, csv_rows);
    }

    fn on_alarm(&mut self, ctx: &mut Context, event: &AlarmEvent) {
        let Some((kind, day_key)) = Self::parse_alarm_key(&event.key) else {
            return;
        };
        match kind {
            "ob_enter" => self.process_entry(ctx, day_key),
            "ob_cutoff" => self.process_cutoff(ctx, day_key),
            _ => {}
        }
    }

    fn on_fill(&mut self, ctx: &mut Context, event: &FillEvent) {
        if event.side == Side::Buy {
            // Entry filled → place target limit sell
            if let Some(entry_info) = self.pending_entries.remove(&event.order_id) {
                self.today_stats.filled_count += 1;

                let target_oid = ctx.place_order(
                    entry_info.instrument_id,
                    Side::Sell,
                    OrderType::Limit(entry_info.target_price),
                    entry_info.quantity,
                );
                self.pending_targets
                    .insert(target_oid, entry_info.instrument_id);

                // Record the open trade so we can close it later
                let now = ctx.now().local_datetime();
                let ticker = self
                    .ticker_map
                    .get(&entry_info.instrument_id)
                    .cloned()
                    .unwrap_or_default();
                self.open_trades.insert(
                    entry_info.instrument_id,
                    OpenTrade {
                        ticker,
                        date: now.format("%Y-%m-%d").to_string(),
                        entry_time: now.format("%H:%M:%S").to_string(),
                        entry_price: event.fill_price,
                        quantity: entry_info.quantity,
                    },
                );

                log::debug!(
                    "option_buying: entry_fill instrument_id={} fill_price={} target_oid={}",
                    entry_info.instrument_id,
                    event.fill_price,
                    target_oid
                );
            }
        } else if event.side == Side::Sell {
            // Determine exit reason before consuming pending_targets
            let exit_reason = if self.pending_targets.contains_key(&event.order_id) {
                "TARGET"
            } else {
                "TIME"
            };

            let is_target_hit = self.pending_targets.remove(&event.order_id).is_some();
            if is_target_hit {
                self.today_stats.target_hit_count += 1;
            }

            // Record completed round-trip trade
            let now = ctx.now().local_datetime();
            let exit_time = now.format("%H:%M:%S").to_string();
            if let Some(open_trade) = self.open_trades.remove(&event.instrument_id) {
                let entry_price_f = open_trade.entry_price as f64 / PRICE_SCALE as f64;
                let exit_price_f = event.fill_price as f64 / PRICE_SCALE as f64;
                let pnl = (exit_price_f - entry_price_f) * open_trade.quantity as f64;
                self.completed_trades.push(TradeRecord {
                    ticker: open_trade.ticker,
                    date: open_trade.date,
                    entry_time: open_trade.entry_time,
                    exit_time,
                    exit_reason: exit_reason.to_string(),
                    entry_price: entry_price_f,
                    exit_price: exit_price_f,
                    quantity: open_trade.quantity,
                    pnl,
                });
            }

            log::debug!(
                "option_buying: sell_fill instrument_id={} fill_price={} reason={}",
                event.instrument_id,
                event.fill_price,
                exit_reason
            );
        }
    }

    fn on_stop(&mut self, ctx: &mut Context) {
        // Flush any remaining today stats (in case cutoff alarm didn't fire on last day)
        self.total_stats.orders_placed += self.today_stats.orders_placed;
        self.total_stats.filled_count += self.today_stats.filled_count;
        self.total_stats.target_hit_count += self.today_stats.target_hit_count;

        let realized_pnl = ctx.realized_pnl();
        let pnl_rupees = realized_pnl as f64 / PRICE_SCALE as f64;

        let entry_fill_rate = if self.total_stats.orders_placed > 0 {
            100.0 * self.total_stats.filled_count as f64 / self.total_stats.orders_placed as f64
        } else {
            0.0
        };
        let target_hit_rate = if self.total_stats.filled_count > 0 {
            100.0 * self.total_stats.target_hit_count as f64
                / self.total_stats.filled_count as f64
        } else {
            0.0
        };

        let summary_json = serde_json::json!({
            "csv_folder": self.csv_folder,
            "total_orders": self.total_stats.orders_placed,
            "filled_orders": self.total_stats.filled_count,
            "target_hit_count": self.total_stats.target_hit_count,
            "entry_fill_rate_pct": entry_fill_rate,
            "target_hit_rate_pct": target_hit_rate,
            "total_pnl": pnl_rupees
        });

        // Write trades.csv
        let trades_path = Path::new(&self.csv_folder).join("trades.csv");
        match Self::write_trades_csv(&trades_path, &self.completed_trades) {
            Ok(_) => log::info!(
                "option_buying: wrote {} trades to {}",
                self.completed_trades.len(),
                trades_path.display()
            ),
            Err(err) => log::warn!(
                "option_buying: failed to write trades.csv to {} err={}",
                trades_path.display(),
                err
            ),
        }

        let performance_path = Path::new(&self.csv_folder).join("performance.json");
        match serde_json::to_string_pretty(&summary_json) {
            Ok(payload) => {
                if let Err(err) = std::fs::write(&performance_path, payload) {
                    log::warn!(
                        "option_buying: failed to write performance summary to {} err={}",
                        performance_path.display(),
                        err
                    );
                } else {
                    log::info!(
                        "option_buying: wrote custom summary to {}",
                        performance_path.display()
                    );
                }
            }
            Err(err) => {
                log::warn!(
                    "option_buying: failed to serialize performance summary err={}",
                    err
                );
            }
        }

        println!();
        println!("=== Option Buying Summary ===");
        println!("CSV Folder:         {}", self.csv_folder);
        println!("Total Orders:       {}", self.total_stats.orders_placed);
        println!("Filled Orders:      {}", self.total_stats.filled_count);
        println!("Target Hit Count:   {}", self.total_stats.target_hit_count);
        println!("Entry Fill Rate %:  {:.2}%", entry_fill_rate);
        println!("Target Hit Rate %:  {:.2}%", target_hit_rate);
        println!("Total PnL:          {:.2}", pnl_rupees);
        println!("=============================");
    }

    fn on_market_event(&mut self, _ctx: &mut Context, _event: &MarketEvent) {}
    fn on_signal(&mut self, _ctx: &mut Context, _event: &SignalEvent) {}
    fn on_order_event(&mut self, _ctx: &mut Context, _event: &OrderEvent) {}
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

pub fn create_strategy_by_id(
    strategy_id: &str,
    params: &BTreeMap<String, String>,
) -> Option<Box<dyn Strategy + Send>> {
    match strategy_id {
        "option_buying" => {
            let csv_folder = params
                .get("csv_folder")
                .map(String::as_str)
                .unwrap_or("/quant/penny_options/tuned/DTE8_CR30_DN60_DW40_DC40");
            Some(Box::new(OptionBuyingStrategy::new(csv_folder)))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Parse ISO date "YYYY-MM-DD" → YYYYMMDD as i32
fn parse_iso_date(s: &str) -> Option<i32> {
    let date = NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()?;
    let y = date.year();
    let m = date.month() as i32;
    let d = date.day() as i32;
    Some(y * 10_000 + m * 100 + d)
}

/// Parse filename date token "02JAN2026" → YYYYMMDD as i32
fn parse_filename_date(s: &str) -> Option<i32> {
    if s.len() < 9 {
        return None;
    }
    let day: i32 = s[..2].parse().ok()?;
    let month_str = s[2..5].to_ascii_uppercase();
    let year: i32 = s[5..].parse().ok()?;
    let month = match month_str.as_str() {
        "JAN" => 1,
        "FEB" => 2,
        "MAR" => 3,
        "APR" => 4,
        "MAY" => 5,
        "JUN" => 6,
        "JUL" => 7,
        "AUG" => 8,
        "SEP" => 9,
        "OCT" => 10,
        "NOV" => 11,
        "DEC" => 12,
        _ => return None,
    };
    Some(year * 10_000 + month * 100 + day)
}

/// Build engine ticker from CSV option fields.
/// Format: `{SYMBOL}{DDMMMYY}{STRIKE}{CE/PE}`
/// E.g.: NIFTY + 20260106 + 25500 + PE → "NIFTY06JAN2625500PE"
fn build_ticker(symbol: &str, expiry_yyyymmdd: i32, strike: i64, opt_type: &str) -> String {
    let year = expiry_yyyymmdd / 10_000;
    let month = (expiry_yyyymmdd / 100) % 100;
    let day = expiry_yyyymmdd % 100;

    let month_str = match month {
        1 => "JAN",
        2 => "FEB",
        3 => "MAR",
        4 => "APR",
        5 => "MAY",
        6 => "JUN",
        7 => "JUL",
        8 => "AUG",
        9 => "SEP",
        10 => "OCT",
        11 => "NOV",
        12 => "DEC",
        _ => "UNK",
    };

    let year_suffix = year % 100; // 2-digit year
    format!("{}{:02}{}{:02}{}{}", symbol, day, month_str, year_suffix, strike, opt_type)
}
