use crate::engine::sweep::SweepResult;
use crate::reporting::json::Metrics;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
pub struct AggregatedSummary {
    pub total_runs: usize,
    
    // Summary Statistics
    pub best_return_pct: f64,
    pub worst_return_pct: f64,
    pub average_return_pct: f64,
    pub median_return_pct: f64,
    
    // Best Run Details
    pub best_run_params: HashMap<String, String>,
    pub best_run_metrics: Metrics,
}

// Clone implementation for Metrics (needed for best_run_metrics)
// Since Metrics is in json.rs and might not derive Clone, we need to check.
// If not, we might need to derive Clone there.
// Assuming Metrics is lightweight (f64s), adding Clone there is best.
// If I can't modify json.rs easily right now, I can manually copy fields.
// But I can modify json.rs.

impl AggregatedSummary {
    pub fn compute(results: &[SweepResult]) -> Self {
        if results.is_empty() {
             panic!("Cannot aggregate empty results");
        }

        let mut returns: Vec<f64> = results.iter().map(|r| r.report.metrics.total_return_pct).collect();
        returns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        
        let total_runs = results.len();
        let best_return_pct = *returns.last().unwrap();
        let worst_return_pct = *returns.first().unwrap();
        let average_return_pct = returns.iter().sum::<f64>() / total_runs as f64;
        let median_return_pct = returns[total_runs / 2];

        // Find best run (by total_return_pct)
        let best_run = results.iter()
            .max_by(|a, b| a.report.metrics.total_return_pct.partial_cmp(&b.report.metrics.total_return_pct).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap();
            
        // Manual copy of metrics if Clone not available
        let best_metrics = Metrics {
            total_return_pct: best_run.report.metrics.total_return_pct,
            cagr_pct: best_run.report.metrics.cagr_pct,
            sharpe_ratio: best_run.report.metrics.sharpe_ratio,
            sortino_ratio: best_run.report.metrics.sortino_ratio,
            max_drawdown_pct: best_run.report.metrics.max_drawdown_pct,
            trade_count: best_run.report.metrics.trade_count,
            win_rate_pct: best_run.report.metrics.win_rate_pct,
            profit_factor: best_run.report.metrics.profit_factor,
            margin_utilization_pct: best_run.report.metrics.margin_utilization_pct,
            final_cash_balance: best_run.report.metrics.final_cash_balance,
        };

        AggregatedSummary {
            total_runs,
            best_return_pct,
            worst_return_pct,
            average_return_pct,
            median_return_pct,
            best_run_params: best_run.params.clone(),
            best_run_metrics: best_metrics,
        }
    }
}
