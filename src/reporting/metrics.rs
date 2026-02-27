use std::collections::HashMap;

use crate::portfolio::manager::Account;
use crate::portfolio::models::{Position, Trade};
use crate::common::types::PRICE_SCALE;

#[derive(Debug, Clone, Default)]
pub struct MetricValues {
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

pub fn calculate_metrics(account: &Account, trades: &[Trade]) -> MetricValues {
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

    let trade_count = realized_deltas.len() as u64;
    let wins = realized_deltas.iter().filter(|delta| **delta > 0).count() as f64;
    let win_rate_pct = if trade_count > 0 {
        (wins / trade_count as f64) * 100.0
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

    let returns = periodic_returns(&equity_points);
    let periods_per_year = if years > 0.0 && !returns.is_empty() {
        (returns.len() as f64 / years).max(1.0)
    } else {
        252.0
    };
    let (sharpe_ratio, sortino_ratio) = risk_metrics(&returns, periods_per_year, 0.05);

    let margin_utilization_pct = margin_utilization_pct(account);

    MetricValues {
        total_return_pct,
        cagr_pct,
        sharpe_ratio,
        sortino_ratio,
        max_drawdown_pct,
        trade_count,
        win_rate_pct,
        profit_factor,
        margin_utilization_pct,
        final_cash_balance: (account.cash as f64) / (PRICE_SCALE as f64),
    }
}

fn realized_pnl_deltas(trades: &[Trade]) -> (Vec<i64>, Option<i64>, Option<i64>) {
    let mut positions: HashMap<u32, Position> = HashMap::new();
    let mut deltas = Vec::new();

    let first_ts = trades.first().map(|trade| trade.timestamp);
    let last_ts = trades.last().map(|trade| trade.timestamp);

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

fn stddev_population(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mu = mean(values);
    let variance = values
        .iter()
        .map(|v| {
            let d = *v - mu;
            d * d
        })
        .sum::<f64>()
        / values.len() as f64;
    variance.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::types::Side;

    fn make_trade(id: u64, ts: i64, side: Side, price_points: i64) -> Trade {
        Trade {
            id,
            order_id: id,
            strategy_id: "s1".to_string(),
            instrument_id: 1,
            side,
            quantity: 1,
            price: price_points * PRICE_SCALE,
            timestamp: ts,
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

        let metrics = calculate_metrics(&account, &trades);
        assert_eq!(metrics.trade_count, 2);
        assert!((metrics.win_rate_pct - 50.0).abs() < 1e-9);
        assert!((metrics.profit_factor - 1.0).abs() < 1e-9);
        assert!(metrics.max_drawdown_pct <= 0.0);
    }

    #[test]
    fn test_metrics_handles_empty_trades() {
        let account = Account::new(1_000 * PRICE_SCALE);
        let metrics = calculate_metrics(&account, &[]);

        assert_eq!(metrics.trade_count, 0);
        assert_eq!(metrics.total_return_pct, 0.0);
        assert_eq!(metrics.sharpe_ratio, 0.0);
        assert_eq!(metrics.sortino_ratio, 0.0);
        assert_eq!(metrics.profit_factor, 0.0);
    }
}
