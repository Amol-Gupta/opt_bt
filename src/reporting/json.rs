use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use crate::engine::runner::Engine;
use crate::strategy::Strategy;
use crate::common::types::PRICE_SCALE;
use crate::portfolio::manager::StrategyAttribution;
use crate::reporting::metrics::calculate_metrics;
use crate::reporting::post_analysis::{
    run_post_analysis, FlatRateTaxModel, PostAnalysisSummary,
};
use crate::reporting::portfolio::{build_portfolio_view, PortfolioView};
use chrono::{DateTime, Utc};

#[derive(Serialize, Debug, Clone)]
pub struct BacktestReport {
    pub reproducibility: Reproducibility,
    pub simulation: Simulation,
    pub metrics: Metrics,
    pub post_analysis: PostAnalysisSummary,
    pub portfolio: PortfolioView,
    pub strategy_attribution: HashMap<String, StrategyAttributionRecord>,
    pub trades: Vec<TradeRecord>,
    pub warnings: Vec<String>,
    // pub equity_curve: Vec<EquityPoint>, // Commented out for now
}

#[derive(Serialize, Debug, Clone)]
pub struct Reproducibility {
    pub engine_version: String,
    pub strategy_version: String,
    pub strategy_name: String,
    pub parameters: HashMap<String, String>, // Simplified for MVP
    pub config: HashMap<String, String>,
    pub dataset: DatasetMetadata,
}

#[derive(Serialize, Debug, Clone)]
pub struct DatasetMetadata {
    pub source: String,
    pub sha256: String,
    pub granularity: String,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct Simulation {
    pub start_time: String,
    pub end_time: String,
    pub duration_ms: u64,
    pub instrument_count: u32,
}

#[derive(Serialize, Debug, Clone)]
pub struct Metrics {
    pub total_return_pct: f64,
    pub cagr_pct: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub max_drawdown_pct: f64,
    pub trade_count: u64,
    pub win_rate_pct: f64,
    pub profit_factor: f64,
    pub margin_utilization_pct: f64,
    pub final_cash_balance: f64,
}

#[derive(Serialize, Debug, Clone)]
pub struct TradeRecord {
    pub id: u64,
    pub strategy_id: String,
    pub symbol: String, // Need lookup from instrument_id
    pub side: String,   // "Buy" or "Sell" (usually Entry side)
    pub entry_time: String,
    pub exit_time: String,
    pub qty: i64,
    pub entry_price: f64,
    pub exit_price: f64,
    pub pnl: f64,
    pub stale_fill: bool,
}

#[derive(Serialize, Debug, Clone)]
pub struct StrategyAttributionRecord {
    pub trade_count: u64,
    pub realized_pnl: f64,
    pub fees_paid: f64,
    pub gross_notional: f64,
}

#[derive(Serialize, Debug, Clone)]
pub struct EquityPoint {
    pub timestamp: i64,
    pub equity: f64,
    pub drawdown_pct: f64,
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
    // Trade reconstruction (FIFO)
    let trades = reconstruct_trades(&engine.context.account.trades, &engine.market_data);
    let strategy_attribution = reconstruct_strategy_attribution(&engine.context.account.strategy_attribution);
    let computed_metrics = calculate_metrics(&engine.context.account, &engine.context.account.trades);
    let metrics = Metrics {
        total_return_pct: computed_metrics.total_return_pct,
        cagr_pct: computed_metrics.cagr_pct,
        sharpe_ratio: computed_metrics.sharpe_ratio,
        sortino_ratio: computed_metrics.sortino_ratio,
        max_drawdown_pct: computed_metrics.max_drawdown_pct,
        trade_count: computed_metrics.trade_count,
        win_rate_pct: computed_metrics.win_rate_pct,
        profit_factor: computed_metrics.profit_factor,
        margin_utilization_pct: computed_metrics.margin_utilization_pct,
        final_cash_balance: computed_metrics.final_cash_balance,
    };
    let tax_model = FlatRateTaxModel::new("flat_rate_0pct", 0.0);
    let post_analysis = run_post_analysis(&engine.context.account.trades, &tax_model);
    let portfolio = build_portfolio_view(&engine.context.account);

    let reproducibility = reproducibility.unwrap_or_else(|| Reproducibility {
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        strategy_version: "unknown".to_string(),
        strategy_name: "Strategy".to_string(),
        parameters: HashMap::new(),
        config: HashMap::new(),
        dataset: DatasetMetadata {
            source: "unknown".to_string(),
            sha256: "unknown".to_string(),
            granularity: "unknown".to_string(),
            start_date: "unknown".to_string(),
            end_date: "unknown".to_string(),
        },
    });

    BacktestReport {
        reproducibility,
        simulation: Simulation {
            start_time: "unknown".to_string(),
            end_time: "unknown".to_string(),
            duration_ms: 0,
            instrument_count: engine.market_data.instruments.len() as u32,
        },
        metrics,
        post_analysis,
        portfolio,
        strategy_attribution,
        trades,
        warnings: engine.context.warnings.clone(),
    }
}

fn reconstruct_trades(raw_trades: &[crate::portfolio::models::Trade], market_data: &crate::data::models::MarketData) -> Vec<TradeRecord> {
    #[derive(Clone)]
    struct OpenLot {
        qty: i64,
        price: i64,
        timestamp: i64,
    }

    #[derive(Default)]
    struct OpenLots {
        longs: VecDeque<OpenLot>,
        shorts: VecDeque<OpenLot>,
    }

    let mut closed_trades = Vec::new();
    let mut lots_by_key: HashMap<(String, u32), OpenLots> = HashMap::new();
    let mut sorted_trades = raw_trades.to_vec();
    sorted_trades.sort_by_key(|trade| (trade.timestamp, trade.id));
    let mut record_id: u64 = 1;

    for trade in &sorted_trades {
        let key = (trade.strategy_id.clone(), trade.instrument_id);
        let open_lots = lots_by_key.entry(key).or_default();
        let symbol = market_data
            .ids
            .get(&trade.instrument_id)
            .cloned()
            .unwrap_or_else(|| format!("ID:{}", trade.instrument_id));
        let mut remaining_qty = trade.quantity;

        match trade.side {
            crate::common::types::Side::Buy => {
                while remaining_qty > 0 {
                    let Some(short_lot) = open_lots.shorts.front_mut() else {
                        break;
                    };

                    let matched_qty = remaining_qty.min(short_lot.qty);
                    let pnl_scaled = (short_lot.price - trade.price) * matched_qty;

                    closed_trades.push(TradeRecord {
                        id: record_id,
                        strategy_id: trade.strategy_id.clone(),
                        symbol: symbol.clone(),
                        side: "Sell".to_string(),
                        entry_time: format_timestamp(short_lot.timestamp),
                        exit_time: format_timestamp(trade.timestamp),
                        qty: matched_qty,
                        entry_price: (short_lot.price as f64) / (PRICE_SCALE as f64),
                        exit_price: (trade.price as f64) / (PRICE_SCALE as f64),
                        pnl: (pnl_scaled as f64) / (PRICE_SCALE as f64),
                        stale_fill: false,
                    });
                    record_id += 1;

                    short_lot.qty -= matched_qty;
                    remaining_qty -= matched_qty;
                    if short_lot.qty == 0 {
                        open_lots.shorts.pop_front();
                    }
                }

                if remaining_qty > 0 {
                    open_lots.longs.push_back(OpenLot {
                        qty: remaining_qty,
                        price: trade.price,
                        timestamp: trade.timestamp,
                    });
                }
            }
            crate::common::types::Side::Sell => {
                while remaining_qty > 0 {
                    let Some(long_lot) = open_lots.longs.front_mut() else {
                        break;
                    };

                    let matched_qty = remaining_qty.min(long_lot.qty);
                    let pnl_scaled = (trade.price - long_lot.price) * matched_qty;

                    closed_trades.push(TradeRecord {
                        id: record_id,
                        strategy_id: trade.strategy_id.clone(),
                        symbol: symbol.clone(),
                        side: "Buy".to_string(),
                        entry_time: format_timestamp(long_lot.timestamp),
                        exit_time: format_timestamp(trade.timestamp),
                        qty: matched_qty,
                        entry_price: (long_lot.price as f64) / (PRICE_SCALE as f64),
                        exit_price: (trade.price as f64) / (PRICE_SCALE as f64),
                        pnl: (pnl_scaled as f64) / (PRICE_SCALE as f64),
                        stale_fill: false,
                    });
                    record_id += 1;

                    long_lot.qty -= matched_qty;
                    remaining_qty -= matched_qty;
                    if long_lot.qty == 0 {
                        open_lots.longs.pop_front();
                    }
                }

                if remaining_qty > 0 {
                    open_lots.shorts.push_back(OpenLot {
                        qty: remaining_qty,
                        price: trade.price,
                        timestamp: trade.timestamp,
                    });
                }
            }
        }
    }

    closed_trades
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
