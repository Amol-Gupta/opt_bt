use crate::common::types::{OrderType, PRICE_SCALE};
use crate::data::view::MarketDataView;
use crate::engine::runner::Engine;
use crate::portfolio::manager::StrategyAttribution;
use crate::portfolio::models::Position;
use crate::reporting::metrics::calculate_metrics;
use crate::reporting::portfolio::{build_portfolio_view, PortfolioView};
use crate::reporting::post_analysis::{run_post_analysis, FlatRateTaxModel, PostAnalysisSummary};
use crate::strategy::Strategy;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

pub const DEFAULT_BENCHMARK_SYMBOL: &str = "NIFTY 50";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BacktestReport {
    pub reproducibility: Reproducibility,
    pub simulation: Simulation,
    pub metrics: Metrics,
    pub post_analysis: PostAnalysisSummary,
    pub portfolio: PortfolioView,
    pub strategy_attribution: HashMap<String, StrategyAttributionRecord>,
    pub fills: Vec<FillRecord>,
    pub order_events: Vec<OrderEventRecord>,
    pub position_events: Vec<PositionEventRecord>,
    #[serde(default)]
    pub daily_equity_curve: Vec<DailyEquityPoint>,
    #[serde(default)]
    pub daily_drawdown_curve: Vec<DailyDrawdownPoint>,
    pub warnings: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Reproducibility {
    pub engine_version: String,
    pub strategy_version: String,
    pub strategy_name: String,
    pub parameters: HashMap<String, String>, // Simplified for MVP
    #[serde(default)]
    pub strategy_parameters: HashMap<String, HashMap<String, String>>,
    pub config: HashMap<String, String>,
    pub dataset: DatasetMetadata,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DatasetMetadata {
    pub source: String,
    pub sha256: Option<String>,
    pub granularity: String,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Simulation {
    pub start_time: String,
    pub end_time: String,
    pub duration_ms: u64,
    pub instrument_count: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Metrics {
    pub benchmark_symbol: String,
    pub benchmark_available: bool,
    pub total_orders: u64,
    pub average_win_pct: f64,
    pub average_loss_pct: f64,
    pub compounding_annual_return_pct: f64,
    pub expectancy: f64,
    pub total_return_pct: f64,
    pub cagr_pct: f64,
    pub start_equity: f64,
    pub end_equity: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub probabilistic_sharpe_ratio_pct: f64,
    pub max_drawdown_pct: f64,
    pub fill_count: u64,
    pub round_trip_trade_count: u64,
    pub win_rate_pct: f64,
    pub loss_rate_pct: f64,
    pub profit_loss_ratio: f64,
    pub profit_factor: f64,
    pub annual_standard_deviation: f64,
    pub annual_variance: f64,
    pub alpha: f64,
    pub beta: f64,
    pub information_ratio: f64,
    pub tracking_error: f64,
    pub treynor_ratio: f64,
    pub margin_utilization_pct: f64,
    pub estimated_strategy_capacity: Option<f64>,
    pub lowest_capacity_asset: Option<String>,
    pub total_fees: f64,
    pub portfolio_turnover_pct: f64,
    pub drawdown_recovery: u64,
    pub final_cash_balance: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FillRecord {
    pub id: u64,
    pub order_id: u64,
    pub strategy_id: String,
    pub symbol: String,
    pub side: String,
    pub timestamp: String,
    pub qty: i64,
    pub price: f64,
    pub fee: f64,
    pub stale_fill: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrderEventRecord {
    pub id: u64,
    pub order_id: u64,
    pub strategy_id: String,
    pub instrument_id: u32,
    pub symbol: String,
    pub timestamp: String,
    pub order_type: String,
    pub side: String,
    pub qty: i64,
    pub limit_price: Option<f64>,
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PositionEventRecord {
    pub id: u64,
    pub level: String,
    pub timestamp: String,
    pub strategy_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_qty: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument_qty_before: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument_qty_after: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument_delta_qty: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument_avg_cost_before: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument_avg_cost_after: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realized_pnl_delta: Option<f64>,
    pub portfolio_open_instruments_before: u64,
    pub portfolio_open_instruments_after: u64,
    pub portfolio_gross_qty_before: i64,
    pub portfolio_gross_qty_after: i64,
    pub portfolio_is_flat_before: bool,
    pub portfolio_is_flat_after: bool,
    pub change_type: String,
    pub changed_fields: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StrategyAttributionRecord {
    pub trade_count: u64,
    pub realized_pnl: f64,
    pub fees_paid: f64,
    pub gross_notional: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EquityPoint {
    pub timestamp: i64,
    pub equity: f64,
    pub drawdown_pct: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DailyEquityPoint {
    pub date: String,
    pub equity: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DailyDrawdownPoint {
    pub date: String,
    pub drawdown_pct: f64,
    pub drawdown_abs: f64,
}

fn format_timestamp(ts: i64) -> String {
    // Assuming ts is seconds? Or millis?
    // Bar timestamp usually seconds.
    if let Some(dt) = DateTime::<Utc>::from_timestamp(ts, 0) {
        return dt.to_rfc3339();
    }
    ts.to_string()
}

pub fn generate_report<S: Strategy>(engine: &Engine<S>) -> BacktestReport {
    generate_report_with_reproducibility(engine, None)
}

pub fn generate_report_with_reproducibility<S: Strategy>(
    engine: &Engine<S>,
    reproducibility: Option<Reproducibility>,
) -> BacktestReport {
    let raw_trades = &engine.context.account.trades;
    let raw_orders = &engine.context.order_events;
    let trade_min_ts = raw_trades.iter().map(|t| t.timestamp).min();
    let trade_max_ts = raw_trades.iter().map(|t| t.timestamp).max();
    let simulation_start_ts = engine
        .start_timestamp
        .or(trade_min_ts)
        .unwrap_or(engine.context.current_timestamp);
    let simulation_end_ts = engine
        .end_timestamp
        .or(trade_max_ts)
        .unwrap_or(engine.context.current_timestamp);
    let duration_ms = if simulation_end_ts >= simulation_start_ts {
        ((simulation_end_ts - simulation_start_ts + 1) as u64) * 1000
    } else {
        0
    };

    let fills = build_fill_records(raw_trades, engine.market_data.as_ref());
    let order_events = build_order_event_records(raw_orders, engine.market_data.as_ref());
    let position_events = build_position_event_records(raw_trades, engine.market_data.as_ref());
    let strategy_attribution =
        reconstruct_strategy_attribution(&engine.context.account.strategy_attribution);
    let benchmark_symbol = std::env::var("BT_BENCHMARK_SYMBOL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_BENCHMARK_SYMBOL.to_string());
    let benchmark_returns = build_benchmark_returns(
        engine.market_data.as_ref(),
        &benchmark_symbol,
        simulation_start_ts,
        simulation_end_ts,
    );
    let computed_metrics = calculate_metrics(
        &engine.context.account,
        &engine.context.account.trades,
        &benchmark_returns,
    );
    let total_orders = raw_orders.len() as u64;
    let fill_count = raw_trades.len() as u64;
    let round_trip_trade_count = computed_metrics.trade_count;
    let metrics = Metrics {
        benchmark_symbol,
        benchmark_available: !benchmark_returns.is_empty(),
        total_orders,
        average_win_pct: computed_metrics.average_win_pct,
        average_loss_pct: computed_metrics.average_loss_pct,
        compounding_annual_return_pct: computed_metrics.compounding_annual_return_pct,
        expectancy: computed_metrics.expectancy,
        total_return_pct: computed_metrics.total_return_pct,
        cagr_pct: computed_metrics.cagr_pct,
        start_equity: computed_metrics.start_equity,
        end_equity: computed_metrics.end_equity,
        sharpe_ratio: computed_metrics.sharpe_ratio,
        sortino_ratio: computed_metrics.sortino_ratio,
        probabilistic_sharpe_ratio_pct: computed_metrics.probabilistic_sharpe_ratio_pct,
        max_drawdown_pct: computed_metrics.max_drawdown_pct,
        fill_count,
        round_trip_trade_count,
        win_rate_pct: computed_metrics.win_rate_pct,
        loss_rate_pct: computed_metrics.loss_rate_pct,
        profit_loss_ratio: computed_metrics.profit_loss_ratio,
        profit_factor: computed_metrics.profit_factor,
        annual_standard_deviation: computed_metrics.annual_standard_deviation,
        annual_variance: computed_metrics.annual_variance,
        alpha: computed_metrics.alpha,
        beta: computed_metrics.beta,
        information_ratio: computed_metrics.information_ratio,
        tracking_error: computed_metrics.tracking_error,
        treynor_ratio: computed_metrics.treynor_ratio,
        margin_utilization_pct: computed_metrics.margin_utilization_pct,
        estimated_strategy_capacity: None,
        lowest_capacity_asset: None,
        total_fees: computed_metrics.total_fees,
        portfolio_turnover_pct: computed_metrics.portfolio_turnover_pct,
        drawdown_recovery: computed_metrics.drawdown_recovery,
        final_cash_balance: computed_metrics.final_cash_balance,
    };
    let tax_model = FlatRateTaxModel::new("flat_rate_0pct", 0.0);
    let post_analysis = run_post_analysis(&engine.context.account.trades, &tax_model);
    let portfolio = build_portfolio_view(&engine.context.account);
    let (daily_equity_curve, daily_drawdown_curve) = build_daily_curves(
        raw_trades,
        engine.market_data.as_ref(),
        engine.context.account.initial_capital,
        simulation_start_ts,
        simulation_end_ts,
    );

    let reproducibility = reproducibility.unwrap_or_else(|| Reproducibility {
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        strategy_version: "unknown".to_string(),
        strategy_name: "Strategy".to_string(),
        parameters: HashMap::new(),
        strategy_parameters: HashMap::new(),
        config: HashMap::new(),
        dataset: DatasetMetadata {
            source: "unknown".to_string(),
            sha256: None,
            granularity: "unknown".to_string(),
            start_date: "unknown".to_string(),
            end_date: "unknown".to_string(),
        },
    });

    BacktestReport {
        reproducibility,
        simulation: Simulation {
            start_time: format_timestamp(simulation_start_ts),
            end_time: format_timestamp(simulation_end_ts),
            duration_ms,
            instrument_count: engine.market_data.instrument_count() as u32,
        },
        metrics,
        post_analysis,
        portfolio,
        strategy_attribution,
        fills,
        order_events,
        position_events,
        daily_equity_curve,
        daily_drawdown_curve,
        warnings: engine.context.warnings.clone(),
    }
}

fn build_daily_curves(
    raw_trades: &[crate::portfolio::models::Trade],
    market_data: &dyn MarketDataView,
    initial_capital: i64,
    start_ts: i64,
    end_ts: i64,
) -> (Vec<DailyEquityPoint>, Vec<DailyDrawdownPoint>) {
    let mut last_timestamp_by_day: BTreeMap<String, i64> = BTreeMap::new();
    for timestamp in market_data.market_timeline() {
        if timestamp < start_ts || timestamp > end_ts {
            continue;
        }
        let Some(dt) = DateTime::<Utc>::from_timestamp(timestamp, 0) else {
            continue;
        };
        let day_key = dt.format("%Y-%m-%d").to_string();
        last_timestamp_by_day
            .entry(day_key)
            .and_modify(|existing| {
                if timestamp > *existing {
                    *existing = timestamp;
                }
            })
            .or_insert(timestamp);
    }

    if last_timestamp_by_day.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let mut sorted_trades = raw_trades.to_vec();
    sorted_trades.sort_by_key(|trade| trade.timestamp);

    let mut trade_index = 0usize;
    let mut cash = initial_capital;
    let mut positions: HashMap<u32, Position> = HashMap::new();

    let mut peak_equity = initial_capital as f64 / PRICE_SCALE as f64;
    let mut equity_curve = Vec::new();
    let mut drawdown_curve = Vec::new();

    for (day, day_timestamp) in last_timestamp_by_day {
        while trade_index < sorted_trades.len()
            && sorted_trades[trade_index].timestamp <= day_timestamp
        {
            let trade = &sorted_trades[trade_index];
            let cost = (trade.quantity as i128 * trade.price as i128) as i64;
            match trade.side {
                crate::common::types::Side::Buy => {
                    cash -= cost;
                    cash -= trade.fee;
                }
                crate::common::types::Side::Sell => {
                    cash += cost;
                    cash -= trade.fee;
                }
            }

            let position = positions
                .entry(trade.instrument_id)
                .or_insert_with(|| Position::new(trade.instrument_id));
            position.update(trade);
            trade_index += 1;
        }

        let mut position_value: i64 = 0;
        for (instrument_id, position) in &positions {
            if position.quantity == 0 {
                continue;
            }
            let mark = market_data
                .get_bar_at_or_before(*instrument_id, day_timestamp)
                .map(|bar| bar.close)
                .unwrap_or(position.avg_cost);
            position_value += (position.quantity as i128 * mark as i128) as i64;
        }

        let equity_scaled = cash + position_value;
        let equity = equity_scaled as f64 / PRICE_SCALE as f64;
        if equity > peak_equity {
            peak_equity = equity;
        }
        let drawdown_abs = equity - peak_equity;
        let drawdown_pct = if peak_equity.abs() > f64::EPSILON {
            (drawdown_abs / peak_equity) * 100.0
        } else {
            0.0
        };

        equity_curve.push(DailyEquityPoint {
            date: day.clone(),
            equity,
        });
        drawdown_curve.push(DailyDrawdownPoint {
            date: day,
            drawdown_pct,
            drawdown_abs,
        });
    }

    (equity_curve, drawdown_curve)
}

fn build_benchmark_returns(
    market_data: &dyn MarketDataView,
    benchmark_symbol: &str,
    start_ts: i64,
    end_ts: i64,
) -> Vec<f64> {
    let benchmark_id = resolve_benchmark_id(market_data, benchmark_symbol);
    let Some(benchmark_id) = benchmark_id else {
        return Vec::new();
    };

    let mut timestamps: Vec<i64> = market_data
        .market_timeline()
        .into_iter()
        .filter(|ts| *ts >= start_ts && *ts <= end_ts)
        .collect();
    timestamps.sort_unstable();
    timestamps.dedup();

    let mut returns = Vec::new();
    let mut prev_close: Option<i64> = None;
    for ts in timestamps {
        let Some(bar) = market_data.get_bar_at_or_before(benchmark_id, ts) else {
            continue;
        };
        if let Some(prev) = prev_close {
            if prev > 0 {
                returns.push((bar.close as f64 / prev as f64) - 1.0);
            }
        }
        prev_close = Some(bar.close);
    }

    returns
}

fn resolve_benchmark_id(market_data: &dyn MarketDataView, benchmark_symbol: &str) -> Option<u32> {
    if let Some(id) = market_data.get_id(benchmark_symbol) {
        return Some(id);
    }

    let target = benchmark_symbol.to_ascii_lowercase();
    market_data
        .iter_ids()
        .into_iter()
        .find(|(_, symbol)| symbol.to_ascii_lowercase() == target)
        .map(|(id, _)| id)
}

fn build_fill_records(
    raw_trades: &[crate::portfolio::models::Trade],
    market_data: &dyn MarketDataView,
) -> Vec<FillRecord> {
    raw_trades
        .iter()
        .map(|trade| FillRecord {
            id: trade.id,
            order_id: trade.order_id,
            strategy_id: trade.strategy_id.clone(),
            symbol: market_data
                .get_symbol(trade.instrument_id)
                .unwrap_or_else(|| format!("ID:{}", trade.instrument_id)),
            side: match trade.side {
                crate::common::types::Side::Buy => "Buy".to_string(),
                crate::common::types::Side::Sell => "Sell".to_string(),
            },
            timestamp: format_timestamp(trade.timestamp),
            qty: trade.quantity,
            price: (trade.price as f64) / (PRICE_SCALE as f64),
            fee: (trade.fee as f64) / (PRICE_SCALE as f64),
            stale_fill: false,
        })
        .collect()
}

fn build_order_event_records(
    raw_orders: &[crate::common::event::OrderEvent],
    market_data: &dyn MarketDataView,
) -> Vec<OrderEventRecord> {
    raw_orders
        .iter()
        .enumerate()
        .map(|(idx, order)| {
            let (order_type, limit_price) = match order.order_type {
                OrderType::Market => ("Market".to_string(), None),
                OrderType::Limit(price) => (
                    "Limit".to_string(),
                    Some((price as f64) / (PRICE_SCALE as f64)),
                ),
                OrderType::Stop(price) => (
                    "Stop".to_string(),
                    Some((price as f64) / (PRICE_SCALE as f64)),
                ),
            };

            OrderEventRecord {
                id: idx as u64 + 1,
                order_id: order.order_id,
                strategy_id: order.strategy_id.clone(),
                instrument_id: order.instrument_id,
                symbol: market_data
                    .get_symbol(order.instrument_id)
                    .unwrap_or_else(|| format!("ID:{}", order.instrument_id)),
                timestamp: format_timestamp(order.timestamp),
                order_type,
                side: match order.side {
                    crate::common::types::Side::Buy => "Buy".to_string(),
                    crate::common::types::Side::Sell => "Sell".to_string(),
                },
                qty: order.quantity,
                limit_price,
                status: "Submitted".to_string(),
            }
        })
        .collect()
}

fn build_position_event_records(
    raw_trades: &[crate::portfolio::models::Trade],
    market_data: &dyn MarketDataView,
) -> Vec<PositionEventRecord> {
    let mut events = Vec::new();
    let mut positions_by_instrument: HashMap<u32, crate::portfolio::models::Position> =
        HashMap::new();
    let mut event_id: u64 = 1;

    for trade in raw_trades {
        let (open_before, gross_before, flat_before) =
            portfolio_position_stats(&positions_by_instrument);

        let pos = positions_by_instrument
            .entry(trade.instrument_id)
            .or_insert_with(|| crate::portfolio::models::Position::new(trade.instrument_id));

        let qty_before = pos.quantity;
        let avg_cost_before = pos.avg_cost;
        let realized_before = pos.realized_pnl;

        pos.update(trade);

        let qty_after = pos.quantity;
        let avg_cost_after = pos.avg_cost;
        let realized_after = pos.realized_pnl;

        let (open_after, gross_after, flat_after) =
            portfolio_position_stats(&positions_by_instrument);

        let mut changed_fields = vec!["instrument_qty".to_string()];
        if avg_cost_after != avg_cost_before {
            changed_fields.push("instrument_avg_cost".to_string());
        }
        if realized_after != realized_before {
            changed_fields.push("realized_pnl".to_string());
        }
        if open_after != open_before {
            changed_fields.push("portfolio_open_instruments".to_string());
        }
        if gross_after != gross_before {
            changed_fields.push("portfolio_gross_qty".to_string());
        }
        if flat_after != flat_before {
            changed_fields.push("portfolio_is_flat".to_string());
        }

        let change_type = if qty_before == 0 && qty_after != 0 {
            "open"
        } else if qty_before != 0 && qty_after == 0 {
            "close"
        } else if qty_before.signum() != 0 && qty_after.signum() != qty_before.signum() {
            "flip"
        } else if qty_after.abs() > qty_before.abs() {
            "increase"
        } else {
            "reduce"
        };

        events.push(PositionEventRecord {
            id: event_id,
            level: "instrument".to_string(),
            timestamp: format_timestamp(trade.timestamp),
            strategy_id: trade.strategy_id.clone(),
            instrument_id: Some(trade.instrument_id),
            symbol: Some(
                market_data
                    .get_symbol(trade.instrument_id)
                    .unwrap_or_else(|| format!("ID:{}", trade.instrument_id)),
            ),
            order_id: Some(trade.order_id),
            fill_id: Some(trade.id),
            side: Some(match trade.side {
                crate::common::types::Side::Buy => "Buy".to_string(),
                crate::common::types::Side::Sell => "Sell".to_string(),
            }),
            fill_qty: Some(trade.quantity),
            fill_price: Some((trade.price as f64) / (PRICE_SCALE as f64)),
            instrument_qty_before: Some(qty_before),
            instrument_qty_after: Some(qty_after),
            instrument_delta_qty: Some(qty_after - qty_before),
            instrument_avg_cost_before: Some((avg_cost_before as f64) / (PRICE_SCALE as f64)),
            instrument_avg_cost_after: Some((avg_cost_after as f64) / (PRICE_SCALE as f64)),
            realized_pnl_delta: Some(
                ((realized_after - realized_before) as f64) / (PRICE_SCALE as f64),
            ),
            portfolio_open_instruments_before: open_before,
            portfolio_open_instruments_after: open_after,
            portfolio_gross_qty_before: gross_before,
            portfolio_gross_qty_after: gross_after,
            portfolio_is_flat_before: flat_before,
            portfolio_is_flat_after: flat_after,
            change_type: change_type.to_string(),
            changed_fields,
        });
        event_id += 1;

        if flat_before != flat_after {
            events.push(PositionEventRecord {
                id: event_id,
                level: "portfolio".to_string(),
                timestamp: format_timestamp(trade.timestamp),
                strategy_id: trade.strategy_id.clone(),
                instrument_id: None,
                symbol: None,
                order_id: Some(trade.order_id),
                fill_id: Some(trade.id),
                side: None,
                fill_qty: None,
                fill_price: None,
                instrument_qty_before: None,
                instrument_qty_after: None,
                instrument_delta_qty: None,
                instrument_avg_cost_before: None,
                instrument_avg_cost_after: None,
                realized_pnl_delta: None,
                portfolio_open_instruments_before: open_before,
                portfolio_open_instruments_after: open_after,
                portfolio_gross_qty_before: gross_before,
                portfolio_gross_qty_after: gross_after,
                portfolio_is_flat_before: flat_before,
                portfolio_is_flat_after: flat_after,
                change_type: if flat_after {
                    "flatten_book".to_string()
                } else {
                    "open_book".to_string()
                },
                changed_fields: vec![
                    "portfolio_open_instruments".to_string(),
                    "portfolio_gross_qty".to_string(),
                    "portfolio_is_flat".to_string(),
                ],
            });
            event_id += 1;
        }
    }

    events
}

fn portfolio_position_stats(
    positions_by_instrument: &HashMap<u32, crate::portfolio::models::Position>,
) -> (u64, i64, bool) {
    let open_count = positions_by_instrument
        .values()
        .filter(|position| position.quantity != 0)
        .count() as u64;
    let gross_qty = positions_by_instrument
        .values()
        .map(|position| position.quantity.abs())
        .sum::<i64>();
    let is_flat = open_count == 0;
    (open_count, gross_qty, is_flat)
}

fn reconstruct_strategy_attribution(
    raw: &HashMap<String, StrategyAttribution>,
) -> HashMap<String, StrategyAttributionRecord> {
    raw.iter()
        .map(|(strategy_id, attribution)| {
            (
                strategy_id.clone(),
                StrategyAttributionRecord {
                    trade_count: attribution.trade_count,
                    realized_pnl: (attribution.realized_pnl as f64) / (PRICE_SCALE as f64),
                    fees_paid: (attribution.fees_paid as f64) / (PRICE_SCALE as f64),
                    gross_notional: (attribution.gross_notional as f64) / (PRICE_SCALE as f64),
                },
            )
        })
        .collect()
}
