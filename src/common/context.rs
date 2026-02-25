use std::sync::Arc;
use crate::data::models::{Bar, MarketData};
use crate::common::types::{OrderType, Side};
use crate::common::event::{Event, OrderEvent};
use crate::portfolio::manager::Account;

/// Context provides the Strategy with access to market data and execution capabilities.
/// It acts as a facade/gateway.
pub struct Context {
    // Market Data access (historical or current snapshot)
    // Could track latest prices for all instruments
    pub market_data: Arc<MarketData>,
    
    // Current simulation time
    pub current_timestamp: i64,
    
    // Output buffer for generated events (Orders, Logs, etc.)
    // Strategy pushes to this, Engine drains it.
    pub event_buffer: Vec<Event>,

    // Account State
    pub account: Account,

    // Strategy Identity
    pub active_strategy_id: String,
}

impl Context {
    pub fn new(market_data: Arc<MarketData>, initial_capital: i64) -> Self {
        Self {
            market_data,
            current_timestamp: 0,
            event_buffer: Vec::new(),
            account: Account::new(initial_capital),
            active_strategy_id: "default".to_string(),
        }
    }
    
    pub fn set_strategy_id(&mut self, id: &str) {
        self.active_strategy_id = id.to_string();
    }
    
    pub fn set_time(&mut self, timestamp: i64) {
        self.current_timestamp = timestamp;
    }
    
    pub fn now(&self) -> i64 {
        self.current_timestamp
    }
    
    pub fn get_bar(&self, instrument_id: u32) -> Option<&Bar> {
        // In a real engine, we would look up the bar at `current_timestamp` 
        // or the latest bar <= `current_timestamp`.
        // Since `MarketData` has `HashMap<u32, Vec<Bar>>`, we can search.
        // For efficiency, Engine usually maintains "Current Quote" state.
        // But for this MVP, let's just do a binary search or lookup if needed.
        // Or assume the strategy receives the bar in `on_data`.
        // This method allows looking up *other* instruments.
        if let Some(bars) = self.market_data.bars.get(&instrument_id) {
            // Find latest bar <= current_timestamp
            // This is O(log N) per lookup.
            let idx = bars.partition_point(|b| b.timestamp <= self.current_timestamp);
            if idx > 0 {
                return Some(&bars[idx - 1]);
            }
        }
        None
    }

    /// Place a new order
    pub fn place_order(&mut self, instrument_id: u32, side: Side, order_type: OrderType, quantity: i64) -> u64 {
        let order_id = self.generate_order_id();
        
        let price = match order_type {
            OrderType::Limit(p) => p,
            OrderType::Stop(p) => p,
            OrderType::Market => 0, // Market order has no limit price (or 0 placeholder)
        };

        let event = Event::Order(OrderEvent {
            timestamp: self.current_timestamp,
            order_id,
            instrument_id,
            order_type,
            side,
            price,
            quantity,
            strategy_id: self.active_strategy_id.clone(),
        });
        self.event_buffer.push(event);
        order_id
    }
    
    fn generate_order_id(&self) -> u64 {
        // Simple distinct ID generation
        // In production, use a robust ID generator
        // Using timestamp + buffer len for uniqueness in this scope
        (self.current_timestamp as u64) * 1000 + (self.event_buffer.len() as u64)
    }
    
    pub fn collect_events(&mut self) -> Vec<Event> {
        self.event_buffer.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::models::Bar;
    use std::collections::HashMap;

    #[test]
    fn test_context_place_order() {
        let md = Arc::new(MarketData::new());
        let mut ctx = Context::new(md.clone(), 1_000_000); // 1,000,000 * 10_000 not needed if already scaled, wait. Context::new takes initial_capital. 
        // We usually pass scaled capital.
        
        ctx.set_time(100);
        
        let order_id = ctx.place_order(1, Side::Buy, OrderType::Limit(1000), 10);
        
        // Assert order generation
        {
            let events = &ctx.event_buffer;
            assert_eq!(events.len(), 1);
            match &events[0] {
                Event::Order(o) => {
                    assert_eq!(o.order_id, order_id);
                    assert_eq!(o.timestamp, 100);
                    assert_eq!(o.instrument_id, 1);
                    assert_eq!(o.side, Side::Buy);
                    assert_eq!(o.quantity, 10);
                },
                _ => panic!("Expected OrderEvent"),
            }
        }
        
        // Assert account access
        assert_eq!(ctx.account.cash, 1_000_000);
        
        let events = ctx.collect_events();
        assert_eq!(events.len(), 1);
        assert!(ctx.event_buffer.is_empty());
    }
}

