use crate::common::types::{Side, Price};
use crate::portfolio::models::{Trade, Position};
use crate::common::event::FillEvent;
use std::collections::HashMap;
// use crate::data::models::MarketData; 

#[derive(Debug, Clone, Default)]
pub struct StrategyAttribution {
    pub trade_count: u64,
    pub realized_pnl: i64,
    pub fees_paid: i64,
    pub gross_notional: i64,
}

#[derive(Debug)]
pub struct Account {
    pub initial_capital: i64, // Scaled by PRICE_SCALE
    pub cash: i64,            // Scaled
    pub positions: HashMap<u32, Position>,
    pub trades: Vec<Trade>,
    pub realized_pnl: i64,    // Scaled
    pub strategy_attribution: HashMap<String, StrategyAttribution>,
}

impl Account {
    pub fn new(initial_capital: i64) -> Self {
        Self {
            initial_capital,
            cash: initial_capital, // Start with cash = capital
            positions: HashMap::new(),
            trades: Vec::new(),
            realized_pnl: 0,
            strategy_attribution: HashMap::new(),
        }
    }
    
    pub fn on_fill(&mut self, fill: &FillEvent) {
        // Create Trade struct
        let trade = Trade {
            id: self.trades.len() as u64 + 1, // Simple ID
            order_id: fill.order_id,
            strategy_id: fill.strategy_id.clone(),
            instrument_id: fill.instrument_id,
            side: fill.side,
            quantity: fill.quantity, // i64
            price: fill.fill_price,
            timestamp: fill.timestamp,
            fee: fill.fee,
        };
        
        // Update Cash
        // Cost = Price * Quantity
        // If Buy: Cash -= Cost + Fee
        // If Sell: Cash += Cost - Fee
        
        // Check scaling: Price is scaled (e.g. 10000). Qty is integer units.
        // Cost = (Price * Qty).  Result is scaled by 10000. 
        // Example: Buy 1 share @ 100.00 (1,000,000). Cost = 1,000,000.
        // Cash (scaled) -= 1,000,000.
        
        let cost = (fill.quantity as i128 * fill.fill_price as i128) as i64;
        let fee = fill.fee; // Assumed scaled? Or flat? Usually scaled to currency.
        
        match fill.side {
            Side::Buy => {
                self.cash -= cost;
                self.cash -= fee;
            },
            Side::Sell => {
                self.cash += cost;
                self.cash -= fee;
            }
        }
        
        // Update Position
        let pos = self.positions.entry(fill.instrument_id)
            .or_insert_with(|| Position::new(fill.instrument_id));
        
        let old_realized = pos.realized_pnl;
        pos.update(&trade);
        let new_realized = pos.realized_pnl;
        
        self.realized_pnl += new_realized - old_realized;

        let attribution = self
            .strategy_attribution
            .entry(fill.strategy_id.clone())
            .or_default();
        attribution.trade_count += 1;
        attribution.fees_paid += fee;
        attribution.gross_notional += cost.abs();
        attribution.realized_pnl += new_realized - old_realized;
        
        self.trades.push(trade);
    }
    
    // Calculate total equity based on current market prices
    // Prices map: instrument_id -> current_price (scaled)
    pub fn equity(&self, current_prices: &HashMap<u32, Price>) -> i64 {
        let mut open_position_value: i64 = 0;
        
        for (id, pos) in &self.positions {
            if pos.quantity == 0 { continue; }
            
            let price = current_prices.get(id).unwrap_or(&pos.avg_cost); // Fallback to cost if no price
            // Value = Qty * Price
            // Again, result is scaled.
            let val = (pos.quantity as i128 * *price as i128) as i64;
            open_position_value += val;
        }
        
        self.cash + open_position_value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::types::{Side, Status};

    #[test]
    fn test_account_buy_sell() {
        let initial = 100_000 * 10_000; // 100k scaled
        let mut acc = Account::new(initial);
        
        // Buy 10 @ 100 (1,000,000)
        let fill1 = FillEvent {
            timestamp: 1,
            order_id: 1,
            instrument_id: 1,
            side: Side::Buy,
            quantity: 10,
            fill_price: 1_000_000, // 100.00
            fee: 0,
            status: Status::Filled,
            strategy_id: "test".to_string(),
        };
        
        acc.on_fill(&fill1);
        
        // Cash should decrease by 10 * 1,000,000 = 10,000,000
        assert_eq!(acc.cash, initial - 10_000_000);
        assert_eq!(acc.positions.get(&1).unwrap().quantity, 10);
        
        // Sell 5 @ 120 (1,200,000)
        let fill2 = FillEvent {
            timestamp: 2,
            order_id: 2,
            instrument_id: 1,
            side: Side::Sell,
            quantity: 5,
            fill_price: 1_200_000,
            fee: 0,
            status: Status::Filled,
            strategy_id: "test".to_string(),
        };
        
        acc.on_fill(&fill2);
        
        // Cash should increase by 5 * 1,200,000 = 6,000,000
        // Net change from initial: -10M + 6M = -4M.
        assert_eq!(acc.cash, initial - 4_000_000);
        
        // Position should be 5
        let pos = acc.positions.get(&1).unwrap();
        assert_eq!(pos.quantity, 5);
        
        // Realized PnL: (120 - 100) * 5 = 100 * 10000 = 1,000,000
        // Wait, 120.00 - 100.00 = 20.00 = 200,000.
        // 5 * 200,000 = 1,000,000.
        assert_eq!(pos.realized_pnl, 1_000_000);
        assert_eq!(acc.realized_pnl, 1_000_000);

        let strategy_attr = acc.strategy_attribution.get("test").expect("missing strategy attribution");
        assert_eq!(strategy_attr.trade_count, 2);
        assert_eq!(strategy_attr.realized_pnl, 1_000_000);
        assert_eq!(strategy_attr.fees_paid, 0);
        assert_eq!(strategy_attr.gross_notional, 16_000_000);
        
        // Equity check
        // Current price 130
        let mut prices = HashMap::new();
        prices.insert(1, 1_300_000);
        
        // Equity = Cash + Value(5 * 130)
        // Cash = Initial - 4M
        // Value = 5 * 1.3M = 6.5M
        // Equity = Initial + 2.5M
        // Total PnL = Realized (1M) + Unrealized (5 * (130-100)=1.5M) ?? 
        // Or Unrealized = (Mark - Cost) * Qty = (130 - 100) * 5 = 1.5M.
        // Total PnL = 1M + 1.5M = 2.5M. 
        // Passes.
        assert_eq!(acc.equity(&prices), initial + 2_500_000);
    }
}
