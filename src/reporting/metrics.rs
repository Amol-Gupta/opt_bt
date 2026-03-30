use std::collections::HashMap;

use crate::common::types::PRICE_SCALE;
use crate::portfolio::manager::Account;
use crate::portfolio::models::{Position, Trade};

#[derive(Debug, Clone, Default)]
pub struct MetricValues {
    pub compounding_annual_return_pct: f64,
    pub total_return_pct: f64,
    pub cagr_pct: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub probabilistic_sharpe_ratio_pct: f64,
    pub max_drawdown_pct: f64,
    pub trade_count: u64,
    pub average_win_pct: f64,
    pub average_loss_pct: f64,
    pub expectancy: f64,
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
    pub start_equity: f64,
    pub end_equity: f64,
    pub total_fees: f64,
    pub portfolio_turnover_pct: f64,
    pub drawdown_recovery: u64,
    pub final_cash_balance: f64,
}

pub fn calculate_metrics(
    account: &Account,
    trades: &[Trade],
    benchmark_returns: &[f64],
) -> MetricValues {
    let initial_equity = account.initial_capital as f64;
    let final_equity = account.equity(&HashMap::new()) as f64;

    let total_return_pct = if initial_equity > 0.0 {
        ((final_equity / initial_equity) - 1.0) * 100.0
    } else {
        0.0
    };

    let (realized_deltas, first_ts, last_ts) = realized_pnl_deltas(trades);
    let years = match (first_ts, last_ts) {
        (Some(start), Some(end)) if end > start => ((end - start) as f64) / 31_557_600.0,
        _ => 0.0,
    };

    let cagr_pct = if initial_equity > 0.0 && years > 0.0 {
        ((final_equity / initial_equity).powf(1.0 / years) - 1.0) * 100.0
    } else {
        0.0
    };

    let compounding_annual_return_pct = cagr_pct;

    let trade_count = realized_deltas.len() as u64;
    let wins = realized_deltas.iter().filter(|delta| **delta > 0).count() as f64;
    let win_rate_pct = if trade_count > 0 {
        (wins / trade_count as f64) * 100.0
    } else {
        0.0
    };
    let loss_rate_pct = if trade_count > 0 {
        100.0 - win_rate_pct
    } else {
        0.0
    };

    let close_stats = realized_close_stats(trades);
    let average_win_pct = mean(
        &close_stats
            .iter()
            .filter(|s| s.pnl_scaled > 0)
            .map(|s| s.pnl_pct)
            .collect::<Vec<_>>(),
    );
    let average_loss_pct = mean(
        &close_stats
            .iter()
            .filter(|s| s.pnl_scaled < 0)
            .map(|s| s.pnl_pct)
            .collect::<Vec<_>>(),
    );

    let avg_win_abs = average_win_pct.max(0.0);
    let avg_loss_abs = (-average_loss_pct).max(0.0);
    let profit_loss_ratio = if avg_loss_abs > 0.0 {
        avg_win_abs / avg_loss_abs
    } else if avg_win_abs > 0.0 {
        f64::INFINITY
    } else {
        0.0
    };

    let win_rate = win_rate_pct / 100.0;
    let loss_rate = loss_rate_pct / 100.0;
    let expectancy = if trade_count > 0 {
        (win_rate * profit_loss_ratio) - loss_rate
    } else {
        0.0
    };

    let gross_profit: f64 = realized_deltas
        .iter()
        .filter(|delta| **delta > 0)
        .map(|delta| *delta as f64)
        .sum();
    let gross_loss_abs: f64 = realized_deltas
        .iter()
        .filter(|delta| **delta < 0)
        .map(|delta| (-*delta) as f64)
        .sum();

    let profit_factor = if gross_loss_abs > 0.0 {
        gross_profit / gross_loss_abs
    } else if gross_profit > 0.0 {
        f64::INFINITY
    } else {
        0.0
    };

    let equity_points = build_equity_curve(account.initial_capital, &realized_deltas);
    let max_drawdown_pct = max_drawdown_pct(&equity_points);
    let drawdown_recovery = drawdown_recovery_periods(&equity_points).unwrap_or(0);

    let returns = periodic_returns(&equity_points);
    let periods_per_year = if years > 0.0 && !returns.is_empty() {
        (returns.len() as f64 / years).max(1.0)
    } else {
        252.0
    };
    let (sharpe_ratio, sortino_ratio) = risk_metrics(&returns, periods_per_year, 0.05);
    let annual_variance = annualized_variance(&returns, periods_per_year);
    let annual_standard_deviation = annual_variance.sqrt();
    let (alpha, beta, information_ratio, tracking_error, treynor_ratio) =
        benchmark_relative_metrics(&returns, benchmark_returns, periods_per_year, 0.05);
    let probabilistic_sharpe_ratio_pct =
        probabilistic_sharpe_ratio_pct(&returns, sharpe_ratio, 0.0);

    let margin_utilization_pct = margin_utilization_pct(account);
    let total_fees_scaled: i64 = trades.iter().map(|t| t.fee).sum();
    let total_fees = total_fees_scaled as f64 / PRICE_SCALE as f64;
    let gross_notional: i128 = trades
        .iter()
        .map(|t| (t.price as i128) * (t.quantity as i128))
        .map(|n| n.abs())
        .sum();
    let portfolio_turnover_pct = if initial_equity > 0.0 {
        ((gross_notional as f64) / initial_equity) * 100.0
    } else {
        0.0
    };

    MetricValues {
        compounding_annual_return_pct,
        total_return_pct,
        cagr_pct,
        sharpe_ratio,
        sortino_ratio,
        probabilistic_sharpe_ratio_pct,
        max_drawdown_pct,
        trade_count,
        average_win_pct,
        average_loss_pct,
        expectancy,
        win_rate_pct,
        loss_rate_pct,
        profit_loss_ratio,
        profit_factor,
        annual_standard_deviation,
        annual_variance,
        alpha,
        beta,
        information_ratio,
        tracking_error,
        treynor_ratio,
        margin_utilization_pct,
        start_equity: initial_equity / PRICE_SCALE as f64,
        end_equity: final_equity / PRICE_SCALE as f64,
        total_fees,
        portfolio_turnover_pct,
        drawdown_recovery,
        final_cash_balance: (account.cash as f64) / (PRICE_SCALE as f64),
    }
}

#[derive(Debug, Clone)]
struct CloseStat {
    pnl_scaled: i64,
    pnl_pct: f64,
}

fn realized_pnl_deltas(trades: &[Trade]) -> (Vec<i64>, Option<i64>, Option<i64>) {
    let mut positions: HashMap<u32, Position> = HashMap::new();
    let mut deltas = Vec::new();

    let first_ts = trades
        .first()
        .map(|trade| trade.timestamp.as_epoch_seconds());
    let last_ts = trades
        .last()
        .map(|trade| trade.timestamp.as_epoch_seconds());

    for trade in trades {
        let pos = positions
            .entry(trade.instrument_id)
            .or_insert_with(|| Position::new(trade.instrument_id));
        let before = pos.realized_pnl;
        pos.update(trade);
        let delta = pos.realized_pnl - before;
        if delta != 0 {
            deltas.push(delta);
        }
    }

    (deltas, first_ts, last_ts)
}

fn build_equity_curve(initial_capital: i64, realized_deltas: &[i64]) -> Vec<f64> {
    let mut equity = initial_capital as f64;
    let mut curve = vec![equity];
    for delta in realized_deltas {
        equity += *delta as f64;
        curve.push(equity);
    }
    curve
}

fn realized_close_stats(trades: &[Trade]) -> Vec<CloseStat> {
    let mut stats = Vec::new();
    let mut positions: HashMap<u32, Position> = HashMap::new();

    for trade in trades {
        let position = positions
            .entry(trade.instrument_id)
            .or_insert_with(|| Position::new(trade.instrument_id));

        let qty_before = position.quantity;
        let avg_cost_before = position.avg_cost;

        let trade_sign = match trade.side {
            crate::common::types::Side::Buy => 1,
            crate::common::types::Side::Sell => -1,
        };

        if qty_before != 0 && qty_before.signum() != trade_sign {
            let closed_qty = qty_before.abs().min(trade.quantity.abs());
            if closed_qty > 0 && avg_cost_before > 0 {
                let current_sign = qty_before.signum();
                let pnl_scaled = (trade.price - avg_cost_before) * current_sign * closed_qty;
                let basis_scaled = avg_cost_before.saturating_mul(closed_qty);
                if basis_scaled > 0 {
                    let pnl_pct = (pnl_scaled as f64 / basis_scaled as f64) * 100.0;
                    stats.push(CloseStat {
                        pnl_scaled,
                        pnl_pct,
                    });
                }
            }
        }

        position.update(trade);
    }

    stats
}

fn max_drawdown_pct(equity_curve: &[f64]) -> f64 {
    if equity_curve.is_empty() {
        return 0.0;
    }

    let mut peak = equity_curve[0];
    let mut max_drawdown = 0.0;

    for equity in equity_curve {
        if *equity > peak {
            peak = *equity;
        }

        if peak > 0.0 {
            let drawdown = ((equity / peak) - 1.0) * 100.0;
            if drawdown < max_drawdown {
                max_drawdown = drawdown;
            }
        }
    }

    max_drawdown
}

fn periodic_returns(equity_curve: &[f64]) -> Vec<f64> {
    let mut returns = Vec::new();
    for window in equity_curve.windows(2) {
        let prev = window[0];
        let curr = window[1];
        if prev > 0.0 {
            returns.push((curr / prev) - 1.0);
        }
    }
    returns
}

fn risk_metrics(returns: &[f64], periods_per_year: f64, annual_risk_free_rate: f64) -> (f64, f64) {
    if returns.is_empty() {
        return (0.0, 0.0);
    }

    let rf_period = annual_risk_free_rate / periods_per_year.max(1.0);
    let excess: Vec<f64> = returns.iter().map(|r| *r - rf_period).collect();
    let mean_excess = mean(&excess);

    let std_all = stddev_population(&excess);
    let sharpe = if std_all > 0.0 {
        (mean_excess / std_all) * periods_per_year.sqrt()
    } else {
        0.0
    };

    let downside: Vec<f64> = excess.into_iter().filter(|v| *v < 0.0).collect();
    let downside_std = stddev_population(&downside);
    let sortino = if downside_std > 0.0 {
        (mean_excess / downside_std) * periods_per_year.sqrt()
    } else {
        0.0
    };

    (sharpe, sortino)
}

fn benchmark_relative_metrics(
    strategy_returns: &[f64],
    benchmark_returns: &[f64],
    periods_per_year: f64,
    annual_risk_free_rate: f64,
) -> (f64, f64, f64, f64, f64) {
    let aligned_n = strategy_returns.len().min(benchmark_returns.len());
    if aligned_n < 2 {
        return (0.0, 0.0, 0.0, 0.0, 0.0);
    }

    let strategy = &strategy_returns[strategy_returns.len() - aligned_n..];
    let benchmark = &benchmark_returns[benchmark_returns.len() - aligned_n..];

    let mean_strategy = mean(strategy);
    let mean_benchmark = mean(benchmark);
    let var_benchmark = variance_population(benchmark);

    let beta = if var_benchmark > 0.0 {
        covariance_population(strategy, benchmark) / var_benchmark
    } else {
        0.0
    };

    let rf_period = annual_risk_free_rate / periods_per_year.max(1.0);
    let alpha_period = (mean_strategy - rf_period) - beta * (mean_benchmark - rf_period);
    let alpha = alpha_period * periods_per_year.max(1.0);

    let active_returns: Vec<f64> = strategy
        .iter()
        .zip(benchmark.iter())
        .map(|(s, b)| s - b)
        .collect();
    let mean_active = mean(&active_returns);
    let tracking_error = stddev_population(&active_returns) * periods_per_year.max(1.0).sqrt();
    let information_ratio = if tracking_error > 0.0 {
        (mean_active * periods_per_year.max(1.0)) / tracking_error
    } else {
        0.0
    };

    let treynor_ratio = if beta.abs() > f64::EPSILON {
        ((mean_strategy - rf_period) * periods_per_year.max(1.0)) / beta
    } else {
        0.0
    };

    (
        alpha,
        beta,
        information_ratio,
        tracking_error,
        treynor_ratio,
    )
}

fn annualized_variance(returns: &[f64], periods_per_year: f64) -> f64 {
    let variance = variance_population(returns);
    variance * periods_per_year.max(1.0)
}

fn probabilistic_sharpe_ratio_pct(
    returns: &[f64],
    sharpe_ratio: f64,
    benchmark_sharpe: f64,
) -> f64 {
    if returns.len() < 2 {
        return 0.0;
    }

    let n = returns.len() as f64;
    let skew = skewness_population(returns);
    let kurt = kurtosis_population(returns);
    let numerator = (sharpe_ratio - benchmark_sharpe) * (n - 1.0).sqrt();
    let denominator_sq =
        1.0 - skew * sharpe_ratio + ((kurt - 1.0) / 4.0) * sharpe_ratio * sharpe_ratio;

    if denominator_sq <= 0.0 {
        return 0.0;
    }

    let z = numerator / denominator_sq.sqrt();
    normal_cdf(z) * 100.0
}

fn drawdown_recovery_periods(equity_curve: &[f64]) -> Option<u64> {
    if equity_curve.is_empty() {
        return None;
    }

    let mut running_peak = equity_curve[0];
    let mut max_drawdown = 0.0;
    let mut trough_index: Option<usize> = None;
    let mut peak_before_trough = equity_curve[0];

    for (idx, &equity) in equity_curve.iter().enumerate() {
        if equity > running_peak {
            running_peak = equity;
        }
        if running_peak <= 0.0 {
            continue;
        }
        let drawdown = (equity / running_peak) - 1.0;
        if drawdown < max_drawdown {
            max_drawdown = drawdown;
            trough_index = Some(idx);
            peak_before_trough = running_peak;
        }
    }

    let Some(start_idx) = trough_index else {
        return Some(0);
    };

    for (idx, &equity) in equity_curve.iter().enumerate().skip(start_idx + 1) {
        if equity >= peak_before_trough {
            return Some((idx - start_idx) as u64);
        }
    }

    None
}

fn margin_utilization_pct(account: &Account) -> f64 {
    if account.initial_capital <= 0 {
        return 0.0;
    }

    let open_notional: i64 = account
        .positions
        .values()
        .filter(|p| p.quantity != 0)
        .map(|p| (p.quantity as i128).abs() as i64 * p.avg_cost)
        .sum();

    (open_notional as f64 / account.initial_capital as f64) * 100.0
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn variance_population(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mu = mean(values);
    values
        .iter()
        .map(|v| {
            let d = *v - mu;
            d * d
        })
        .sum::<f64>()
        / values.len() as f64
}

fn covariance_population(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len().min(y.len());
    if n == 0 {
        return 0.0;
    }

    let xs = &x[x.len() - n..];
    let ys = &y[y.len() - n..];
    let mean_x = mean(xs);
    let mean_y = mean(ys);

    xs.iter()
        .zip(ys.iter())
        .map(|(a, b)| (a - mean_x) * (b - mean_y))
        .sum::<f64>()
        / n as f64
}

fn stddev_population(values: &[f64]) -> f64 {
    variance_population(values).sqrt()
}

fn skewness_population(values: &[f64]) -> f64 {
    let n = values.len();
    if n < 2 {
        return 0.0;
    }

    let mu = mean(values);
    let sigma = stddev_population(values);
    if sigma == 0.0 {
        return 0.0;
    }

    values
        .iter()
        .map(|v| ((*v - mu) / sigma).powi(3))
        .sum::<f64>()
        / n as f64
}

fn kurtosis_population(values: &[f64]) -> f64 {
    let n = values.len();
    if n < 2 {
        return 3.0;
    }

    let mu = mean(values);
    let sigma = stddev_population(values);
    if sigma == 0.0 {
        return 3.0;
    }

    values
        .iter()
        .map(|v| ((*v - mu) / sigma).powi(4))
        .sum::<f64>()
        / n as f64
}

fn normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf_approx(x / 2f64.sqrt()))
}

fn erf_approx(x: f64) -> f64 {
    // Abramowitz and Stegun approximation 7.1.26
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();

    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t + a3) * t + a2) * t + a1) * t) * (-x * x).exp();

    sign * y
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::types::{Side, SimTime};

    fn make_trade(id: u64, ts: i64, side: Side, price_points: i64) -> Trade {
        Trade {
            id,
            order_id: id,
            strategy_id: "s1".to_string(),
            instrument_id: 1,
            side,
            quantity: 1,
            price: price_points * PRICE_SCALE,
            timestamp: SimTime::utc(ts),
            fee: 0,
        }
    }

    #[test]
    fn test_metrics_profit_factor_and_win_rate() {
        let mut account = Account::new(1_000 * PRICE_SCALE);
        let trades = vec![
            make_trade(1, 1, Side::Buy, 100),
            make_trade(2, 2, Side::Sell, 110),
            make_trade(3, 3, Side::Buy, 120),
            make_trade(4, 4, Side::Sell, 110),
        ];

        for fill_trade in &trades {
            // emulate realized pnl on account via position update
            let pos = account
                .positions
                .entry(fill_trade.instrument_id)
                .or_insert_with(|| Position::new(fill_trade.instrument_id));
            let before = pos.realized_pnl;
            pos.update(fill_trade);
            account.realized_pnl += pos.realized_pnl - before;
        }
        account.cash = account.initial_capital + account.realized_pnl;

        let metrics = calculate_metrics(&account, &trades, &[]);
        assert_eq!(metrics.trade_count, 2);
        assert!((metrics.win_rate_pct - 50.0).abs() < 1e-9);
        assert!((metrics.loss_rate_pct - 50.0).abs() < 1e-9);
        assert!((metrics.profit_factor - 1.0).abs() < 1e-9);
        assert!((metrics.average_win_pct - 10.0).abs() < 1e-9);
        assert!((metrics.average_loss_pct + 8.333333333333334).abs() < 1e-9);
        assert!((metrics.profit_loss_ratio - 1.2).abs() < 1e-9);
        assert!(metrics.max_drawdown_pct <= 0.0);
        assert!(metrics.total_fees.abs() < 1e-9);
    }

    #[test]
    fn test_metrics_handles_empty_trades() {
        let account = Account::new(1_000 * PRICE_SCALE);
        let metrics = calculate_metrics(&account, &[], &[]);

        assert_eq!(metrics.trade_count, 0);
        assert_eq!(metrics.total_return_pct, 0.0);
        assert_eq!(metrics.sharpe_ratio, 0.0);
        assert_eq!(metrics.sortino_ratio, 0.0);
        assert_eq!(metrics.profit_factor, 0.0);
        assert_eq!(metrics.profit_loss_ratio, 0.0);
        assert_eq!(metrics.expectancy, 0.0);
        assert_eq!(metrics.average_win_pct, 0.0);
        assert_eq!(metrics.average_loss_pct, 0.0);
    }
}
