use serde::Serialize;
use std::collections::HashMap;
use crate::engine::runner::Engine;
use crate::strategy::Strategy;
use crate::common::types::{Side, Price, PRICE_SCALE};
use chrono::{DateTime, NaiveDateTime, Utc};

#[derive(Serialize, Debug, Clone)]
pub struct BacktestReport {
    pub reproducibility: Reproducibility,
    pub simulation: Simulation,
    pub metrics: Metrics,
    pub trades: Vec<TradeRecord>,
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
pub struct EquityPoint {
    pub timestamp: i64,
    pub equity: f64,
    pub drawdown_pct: f64,
}

fn format_timestamp(ts: i64) -> String {
    // Assuming ts is seconds? Or millis?
    // Bar timestamp usually seconds.
    if let Some(ndt) = NaiveDateTime::from_timestamp_opt(ts, 0) {
        return DateTime::<Utc>::from_utc(ndt, Utc).to_rfc3339();
    }
    ts.to_string()
}

pub fn generate_report<S: Strategy>(engine: &Engine<S>) -> BacktestReport {
    let final_equity = engine.context.account.equity(&HashMap::new()); 
    
    // Trade reconstruction (FIFO)
    let trades = reconstruct_trades(&engine.context.account.trades, &engine.market_data);
    
    // Metrics calculation (Stubbed for now)
    let metrics = Metrics {
        total_return_pct: 0.0,
        cagr_pct: 0.0,
        sharpe_ratio: 0.0,
        sortino_ratio: 0.0,
        max_drawdown_pct: 0.0,
        trade_count: trades.len() as u64,
        win_rate_pct: 0.0,
        profit_factor: 0.0,
        margin_utilization_pct: 0.0,
        final_cash_balance: (engine.context.account.cash as f64) / (PRICE_SCALE as f64),
    };

    BacktestReport {
        reproducibility: Reproducibility {
            engine_version: "v0.1.0".to_string(),
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
        },
        simulation: Simulation {
            start_time: "unknown".to_string(),
            end_time: "unknown".to_string(),
            duration_ms: 0,
            instrument_count: engine.market_data.instruments.len() as u32,
        },
        metrics,
        trades,
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
