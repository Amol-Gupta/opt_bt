use crate::common::types::{OrderType, Side, Status, Price};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub id: u64,
    pub instrument_id: u32,
    pub quantity: i64,
    pub side: Side,
    pub order_type: OrderType,
    pub filled_quantity: i64,
    pub avg_fill_price: Option<Price>,
    pub status: Status,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trade {
    pub id: u64,
    pub order_id: u64,
    pub strategy_id: String,
    pub instrument_id: u32,
    pub side: Side,
    pub quantity: i64,
    pub price: Price,
    pub timestamp: i64,
    pub fee: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub instrument_id: u32,
    pub quantity: i64, // Positive for long, negative for short
    pub avg_cost: Price,
    pub realized_pnl: i64,
}

impl Position {
    pub fn new(instrument_id: u32) -> Self {
        Self {
            instrument_id,
            quantity: 0,
            avg_cost: 0,
            realized_pnl: 0,
        }
    }
    
    pub fn update(&mut self, trade: &Trade) {
        // Basic update logic:
        // 1. If increasing position size (same side): avg_cost updates.
        // 2. If decreasing position size (opposite side): realized_pnl updates.
        // 3. If flipping position: close out old (realize PnL), open new (set cost).
        
        let trade_qty = trade.quantity;
        let trade_price = trade.price;
        let trade_sign = match trade.side {
            Side::Buy => 1,
            Side::Sell => -1,
        };
        
        let signed_trade_qty = trade_qty * (trade_sign as i64);
        
        if self.quantity == 0 {
            // New position
            self.quantity = signed_trade_qty;
            self.avg_cost = trade_price;
        } else {
            let current_sign = self.quantity.signum();
            let trade_sign_i64 = trade_sign as i64;
            
            if current_sign == trade_sign_i64 {
                // Increasing position
                // Weighted average cost
                // total_start_cost = qty * avg_cost
                // trade_cost = t_qty * t_price
                // new_avg = (total + trade) / new_qty
                
                let total_cost = (self.quantity.abs() as i128) * (self.avg_cost as i128);
                let trade_cost = (trade_qty as i128) * (trade_price as i128);
                let new_qty_abs = self.quantity.abs() + trade_qty;
                
                self.avg_cost = ((total_cost + trade_cost) / (new_qty_abs as i128)) as i64;
                self.quantity += signed_trade_qty;
            } else {
                // Decreasing or Flipping
                // Closing out existing position first
                
                // If closing partial or full: PnL = (Exit Price - Entry Price) * Qty * Side
                // Side is the side of the OPENING position (Long -> +1).
                
                if trade_qty <= self.quantity.abs() {
                    // Close partial/full without flip
                    let pnl_per_unit = (trade_price - self.avg_cost) * current_sign; // if long, price-cost. if short, cost-price -> wait.
                    // If Log (current_sign=1): Sell (trade_price). PnL = price - cost. Correct.
                    // If Short (current_sign=-1): Buy (trade_price). PnL = cost - price.
                    // My formula: (price - cost) * (-1) = cost - price. Correct.
                    
                    self.realized_pnl += pnl_per_unit * trade_qty; // PnL is scaled by 10000? No, PnL usually currency.
                    // Wait, Price is i64 scaled. Qty is i64 units.
                    // PnL calculated here is Price * Qty.
                    // If Price is scaled 10000, then PnL is scaled 10000.
                    // We need to be careful about units.
                    
                    self.quantity += signed_trade_qty;
                    if self.quantity == 0 {
                        self.avg_cost = 0;
                    }
                } else {
                    // Flip
                    // 1. Close current completely
                    let close_qty = self.quantity.abs();
                    let pnl_per_unit = (trade_price - self.avg_cost) * current_sign;
                    self.realized_pnl += pnl_per_unit * close_qty;
                    
                    // 2. Open remainder
                    let remainder = trade_qty - close_qty;
                    self.quantity = remainder * trade_sign_i64;
                    self.avg_cost = trade_price;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_update() {
        let mut pos = Position::new(1);
        
        // Buy 10 @ 100
        let t1 = Trade { id: 1, order_id: 1, strategy_id: "test".to_string(), instrument_id: 1, side: Side::Buy, quantity: 10, price: 100, timestamp: 1, fee: 0 };
        pos.update(&t1);
        assert_eq!(pos.quantity, 10);
        assert_eq!(pos.avg_cost, 100);
        assert_eq!(pos.realized_pnl, 0);
        
        // Buy 10 @ 120 -> Avg cost 110, Qty 20
        let t2 = Trade { id: 2, order_id: 2, strategy_id: "test".to_string(), instrument_id: 1, side: Side::Buy, quantity: 10, price: 120, timestamp: 2, fee: 0 };
        pos.update(&t2);
        assert_eq!(pos.quantity, 20);
        assert_eq!(pos.avg_cost, 110);
        
        // Sell 10 @ 130 -> PnL (130-110)*10 = 200. Qty 10. AvgCost 110.
        let t3 = Trade { id: 3, order_id: 3, strategy_id: "test".to_string(), instrument_id: 1, side: Side::Sell, quantity: 10, price: 130, timestamp: 3, fee: 0 };
        pos.update(&t3);
        assert_eq!(pos.quantity, 10);
        assert_eq!(pos.avg_cost, 110); // FIFO/Average Cost accounting? Usually Average Cost implies cost basis doesn't change on reduction.
        assert_eq!(pos.realized_pnl, 200); 
        
        // Sell 20 @ 100 -> Flip to Short 10.
        // Close 10 (Long) @ 100. PnL (100-110)*10 = -100. Total PnL 200 - 100 = 100.
        // Open 10 (Short) @ 100. AvgCost 100. Qty -10.
        let t4 = Trade { id: 4, order_id: 4, strategy_id: "test".to_string(), instrument_id: 1, side: Side::Sell, quantity: 20, price: 100, timestamp: 4, fee: 0 };
        pos.update(&t4);
        assert_eq!(pos.quantity, -10);
        assert_eq!(pos.avg_cost, 100);
        assert_eq!(pos.realized_pnl, 100);
    }
}
