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
            benchmark_symbol: best_run.report.metrics.benchmark_symbol.clone(),
            benchmark_available: best_run.report.metrics.benchmark_available,
            total_orders: best_run.report.metrics.total_orders,
            average_win_pct: best_run.report.metrics.average_win_pct,
            average_loss_pct: best_run.report.metrics.average_loss_pct,
            compounding_annual_return_pct: best_run.report.metrics.compounding_annual_return_pct,
            expectancy: best_run.report.metrics.expectancy,
            total_return_pct: best_run.report.metrics.total_return_pct,
            cagr_pct: best_run.report.metrics.cagr_pct,
            start_equity: best_run.report.metrics.start_equity,
            end_equity: best_run.report.metrics.end_equity,
            sharpe_ratio: best_run.report.metrics.sharpe_ratio,
            sortino_ratio: best_run.report.metrics.sortino_ratio,
            probabilistic_sharpe_ratio_pct: best_run.report.metrics.probabilistic_sharpe_ratio_pct,
            max_drawdown_pct: best_run.report.metrics.max_drawdown_pct,
            fill_count: best_run.report.metrics.fill_count,
            round_trip_trade_count: best_run.report.metrics.round_trip_trade_count,
            win_rate_pct: best_run.report.metrics.win_rate_pct,
            loss_rate_pct: best_run.report.metrics.loss_rate_pct,
            profit_loss_ratio: best_run.report.metrics.profit_loss_ratio,
            profit_factor: best_run.report.metrics.profit_factor,
            annual_standard_deviation: best_run.report.metrics.annual_standard_deviation,
            annual_variance: best_run.report.metrics.annual_variance,
            alpha: best_run.report.metrics.alpha,
            beta: best_run.report.metrics.beta,
            information_ratio: best_run.report.metrics.information_ratio,
            tracking_error: best_run.report.metrics.tracking_error,
            treynor_ratio: best_run.report.metrics.treynor_ratio,
            margin_utilization_pct: best_run.report.metrics.margin_utilization_pct,
            estimated_strategy_capacity: best_run.report.metrics.estimated_strategy_capacity,
            lowest_capacity_asset: best_run.report.metrics.lowest_capacity_asset.clone(),
            total_fees: best_run.report.metrics.total_fees,
            portfolio_turnover_pct: best_run.report.metrics.portfolio_turnover_pct,
            drawdown_recovery: best_run.report.metrics.drawdown_recovery,
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
