use serde::Serialize;

use crate::common::types::PRICE_SCALE;
use crate::portfolio::manager::{Account, StrategyAttribution};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PortfolioView {
    pub strategy_count: u64,
    pub total_trade_count: u64,
    pub total_realized_pnl: f64,
    pub total_fees_paid: f64,
    pub total_gross_notional: f64,
    pub strategy_breakdown: Vec<StrategyPortfolioBreakdown>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StrategyPortfolioBreakdown {
    pub strategy_id: String,
    pub trade_count: u64,
    pub realized_pnl: f64,
    pub fees_paid: f64,
    pub gross_notional: f64,
    pub pnl_contribution_pct: f64,
}

pub fn build_portfolio_view(account: &Account) -> PortfolioView {
    let total_realized_pnl_scaled: i64 = account
        .strategy_attribution
        .values()
        .map(|a| a.realized_pnl)
        .sum();

    let mut strategy_breakdown: Vec<StrategyPortfolioBreakdown> = account
        .strategy_attribution
        .iter()
        .map(|(strategy_id, attribution)| {
            to_breakdown(strategy_id, attribution, total_realized_pnl_scaled)
        })
        .collect();

    strategy_breakdown.sort_by(|a, b| a.strategy_id.cmp(&b.strategy_id));

    let total_fees_paid_scaled: i64 = account
        .strategy_attribution
        .values()
        .map(|a| a.fees_paid)
        .sum();

    let total_gross_notional_scaled: i64 = account
        .strategy_attribution
        .values()
        .map(|a| a.gross_notional)
        .sum();

    PortfolioView {
        strategy_count: account.strategy_attribution.len() as u64,
        total_trade_count: account.trades.len() as u64,
        total_realized_pnl: scale_to_f64(total_realized_pnl_scaled),
        total_fees_paid: scale_to_f64(total_fees_paid_scaled),
        total_gross_notional: scale_to_f64(total_gross_notional_scaled),
        strategy_breakdown,
    }
}

fn to_breakdown(
    strategy_id: &str,
    attribution: &StrategyAttribution,
    total_realized_pnl_scaled: i64,
) -> StrategyPortfolioBreakdown {
    let contribution_pct = if total_realized_pnl_scaled == 0 {
        0.0
    } else {
        (attribution.realized_pnl as f64 / total_realized_pnl_scaled as f64) * 100.0
    };

    StrategyPortfolioBreakdown {
        strategy_id: strategy_id.to_string(),
        trade_count: attribution.trade_count,
        realized_pnl: scale_to_f64(attribution.realized_pnl),
        fees_paid: scale_to_f64(attribution.fees_paid),
        gross_notional: scale_to_f64(attribution.gross_notional),
        pnl_contribution_pct: contribution_pct,
    }
}

fn scale_to_f64(value: i64) -> f64 {
    (value as f64) / (PRICE_SCALE as f64)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::portfolio::manager::StrategyAttribution;

    #[test]
    fn test_build_portfolio_view_aggregates_and_sorts() {
        let mut account = Account::new(1_000_000 * PRICE_SCALE);
        account.trades = vec![];
        account.trades.resize_with(5, || crate::portfolio::models::Trade {
            id: 0,
            order_id: 0,
            strategy_id: "x".to_string(),
            instrument_id: 1,
            side: crate::common::types::Side::Buy,
            quantity: 1,
            price: 100 * PRICE_SCALE,
            timestamp: 0,
            fee: 0,
        });

        account.strategy_attribution = HashMap::from([
            (
                "b".to_string(),
                StrategyAttribution {
                    trade_count: 2,
                    realized_pnl: 50 * PRICE_SCALE,
                    fees_paid: 2 * PRICE_SCALE,
                    gross_notional: 500 * PRICE_SCALE,
                },
            ),
            (
                "a".to_string(),
                StrategyAttribution {
                    trade_count: 3,
                    realized_pnl: 150 * PRICE_SCALE,
                    fees_paid: 3 * PRICE_SCALE,
                    gross_notional: 800 * PRICE_SCALE,
                },
            ),
        ]);

        let view = build_portfolio_view(&account);

        assert_eq!(view.strategy_count, 2);
        assert_eq!(view.total_trade_count, 5);
        assert!((view.total_realized_pnl - 200.0).abs() < 1e-9);
        assert!((view.total_fees_paid - 5.0).abs() < 1e-9);
        assert!((view.total_gross_notional - 1300.0).abs() < 1e-9);
        assert_eq!(view.strategy_breakdown[0].strategy_id, "a");
        assert_eq!(view.strategy_breakdown[1].strategy_id, "b");
    }

    #[test]
    fn test_contribution_zero_when_total_realized_zero() {
        let mut account = Account::new(100 * PRICE_SCALE);
        account.strategy_attribution = HashMap::from([(
            "flat".to_string(),
            StrategyAttribution {
                trade_count: 1,
                realized_pnl: 0,
                fees_paid: 0,
                gross_notional: 10 * PRICE_SCALE,
            },
        )]);

        let view = build_portfolio_view(&account);
        assert_eq!(view.strategy_breakdown.len(), 1);
        assert_eq!(view.strategy_breakdown[0].pnl_contribution_pct, 0.0);
    }
}
