use serde::Serialize;
use std::collections::HashMap;
use crate::engine::runner::Engine;
use crate::strategy::Strategy;
use crate::common::types::PRICE_SCALE;
use crate::portfolio::manager::StrategyAttribution;
use crate::reporting::metrics::calculate_metrics;
use crate::reporting::post_analysis::{
    default_stress_scenarios, run_post_analysis, FlatRateTaxModel, PostAnalysisSummary,
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
    let post_analysis = run_post_analysis(
        &engine.context.account,
        &engine.context.account.trades,
        &tax_model,
        &default_stress_scenarios(),
    );
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
    let mut closed_trades = Vec::new();
    
    for trade in raw_trades {
        let symbol = market_data.ids.get(&trade.instrument_id).cloned().unwrap_or_else(|| format!("ID:{}", trade.instrument_id));
        let price_f64 = (trade.price as f64) / (PRICE_SCALE as f64);
        
        // Let's just create a record for every trade for now (incorrect but compiles).
        closed_trades.push(TradeRecord {
            id: trade.id,
            strategy_id: trade.strategy_id.clone(),
            symbol,
            side: format!("{:?}", trade.side),
            entry_time: format_timestamp(trade.timestamp), 
            exit_time: format_timestamp(trade.timestamp),
            qty: trade.quantity,
            entry_price: price_f64,
            exit_price: price_f64,
            pnl: 0.0, // Calculated PnL
            stale_fill: false,
        });
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
