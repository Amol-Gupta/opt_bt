use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::common::types::PRICE_SCALE;
use crate::portfolio::manager::Account;
use crate::portfolio::models::{Position, Trade};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaxAdjustedTrade {
    pub trade_id: u64,
    pub gross_pnl: f64,
    pub tax: f64,
    pub net_pnl: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaxSummary {
    pub model_name: String,
    pub total_gross_pnl: f64,
    pub total_tax: f64,
    pub total_net_pnl: f64,
    pub trade_count: u64,
    pub trades: Vec<TaxAdjustedTrade>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StressScenario {
    pub name: String,
    pub price_shock_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StressResult {
    pub scenario: String,
    pub price_shock_pct: f64,
    pub shocked_equity: f64,
    pub pnl_impact: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PostAnalysisSummary {
    pub tax: TaxSummary,
}

pub trait TaxModel {
    fn name(&self) -> &str;

    fn compute_tax(&self, gross_pnl_scaled: i64) -> i64;
}

#[derive(Debug, Clone)]
pub struct FlatRateTaxModel {
    name: String,
    rate: f64,
}

impl FlatRateTaxModel {
    pub fn new(name: &str, rate: f64) -> Self {
        Self {
            name: name.to_string(),
            rate: rate.clamp(0.0, 1.0),
        }
    }
}

impl TaxModel for FlatRateTaxModel {
    fn name(&self) -> &str {
        &self.name
    }

    fn compute_tax(&self, gross_pnl_scaled: i64) -> i64 {
        if gross_pnl_scaled <= 0 {
            return 0;
        }

        ((gross_pnl_scaled as f64) * self.rate).round() as i64
    }
}

pub fn default_stress_scenarios() -> Vec<StressScenario> {
    vec![
        StressScenario {
            name: "down_10pct".to_string(),
            price_shock_pct: -0.10,
        },
        StressScenario {
            name: "up_10pct".to_string(),
            price_shock_pct: 0.10,
        },
        StressScenario {
            name: "down_20pct".to_string(),
            price_shock_pct: -0.20,
        },
    ]
}

pub fn run_post_analysis(trades: &[Trade], tax_model: &dyn TaxModel) -> PostAnalysisSummary {
    let tax = apply_tax_model(trades, tax_model);

    PostAnalysisSummary { tax }
}

pub fn apply_tax_model(trades: &[Trade], tax_model: &dyn TaxModel) -> TaxSummary {
    let realized_by_trade = realized_trade_deltas(trades);

    let mut adjusted = Vec::new();
    let mut total_gross = 0i64;
    let mut total_tax = 0i64;

    for (trade_id, gross_pnl_scaled) in realized_by_trade {
        let tax_scaled = tax_model.compute_tax(gross_pnl_scaled);
        let net_scaled = gross_pnl_scaled - tax_scaled;

        total_gross += gross_pnl_scaled;
        total_tax += tax_scaled;

        adjusted.push(TaxAdjustedTrade {
            trade_id,
            gross_pnl: to_f64(gross_pnl_scaled),
            tax: to_f64(tax_scaled),
            net_pnl: to_f64(net_scaled),
        });
    }

    TaxSummary {
        model_name: tax_model.name().to_string(),
        total_gross_pnl: to_f64(total_gross),
        total_tax: to_f64(total_tax),
        total_net_pnl: to_f64(total_gross - total_tax),
        trade_count: adjusted.len() as u64,
        trades: adjusted,
    }
}

pub fn run_stress_tests(account: &Account, scenarios: &[StressScenario]) -> Vec<StressResult> {
    let baseline_equity = account.equity(&HashMap::new());

    scenarios
        .iter()
        .map(|scenario| {
            let mut shocked_prices = HashMap::new();

            for (instrument_id, position) in &account.positions {
                if position.quantity == 0 {
                    continue;
                }

                let shocked_price = ((position.avg_cost as f64) * (1.0 + scenario.price_shock_pct))
                    .round()
                    .max(0.0) as i64;
                shocked_prices.insert(*instrument_id, shocked_price);
            }

            let shocked_equity = account.equity(&shocked_prices);
            let impact = shocked_equity - baseline_equity;

            StressResult {
                scenario: scenario.name.clone(),
                price_shock_pct: scenario.price_shock_pct,
                shocked_equity: to_f64(shocked_equity),
                pnl_impact: to_f64(impact),
            }
        })
        .collect()
}

fn realized_trade_deltas(trades: &[Trade]) -> Vec<(u64, i64)> {
    let mut positions: HashMap<u32, Position> = HashMap::new();
    let mut deltas = Vec::new();

    for trade in trades {
        let position = positions
            .entry(trade.instrument_id)
            .or_insert_with(|| Position::new(trade.instrument_id));
        let before = position.realized_pnl;
        position.update(trade);
        let delta = position.realized_pnl - before;
        if delta != 0 {
            deltas.push((trade.id, delta));
        }
    }

    deltas
}

fn to_f64(value: i64) -> f64 {
    (value as f64) / (PRICE_SCALE as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::types::{Side, SimTime};

    fn trade(id: u64, side: Side, price_points: i64) -> Trade {
        Trade {
            id,
            order_id: id,
            strategy_id: "s1".to_string(),
            instrument_id: 1,
            side,
            quantity: 1,
            price: price_points * PRICE_SCALE,
            timestamp: SimTime::utc(id as i64),
            fee: 0,
        }
    }

    #[test]
    fn test_tax_model_applies_on_positive_realized_pnl() {
        let trades = vec![
            trade(1, Side::Buy, 100),
            trade(2, Side::Sell, 110),
            trade(3, Side::Buy, 120),
            trade(4, Side::Sell, 110),
        ];

        let tax_model = FlatRateTaxModel::new("flat_20pct", 0.20);
        let summary = apply_tax_model(&trades, &tax_model);

        assert_eq!(summary.trade_count, 2);
        assert!((summary.total_gross_pnl - 0.0).abs() < 1e-9);
        assert!((summary.total_tax - 2.0).abs() < 1e-9);
        assert!((summary.total_net_pnl + 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_stress_downside_impacts_long_exposure() {
        let mut account = Account::new(100_000 * PRICE_SCALE);
        account.positions.insert(
            1,
            Position {
                instrument_id: 1,
                quantity: 10,
                avg_cost: 100 * PRICE_SCALE,
                realized_pnl: 0,
            },
        );

        let results = run_stress_tests(
            &account,
            &[StressScenario {
                name: "down_10".to_string(),
                price_shock_pct: -0.10,
            }],
        );

        assert_eq!(results.len(), 1);
        assert!(results[0].pnl_impact < 0.0);
    }
}
