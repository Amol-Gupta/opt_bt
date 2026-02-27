use std::sync::Arc;
use crate::data::models::{Bar, MarketData};
use crate::core::types::{OrderType, Side, Price}; // Removed Order for now

/// Context is the Gateway for Strategy <-> Engine communication.
/// It provides access to data and allows placing orders.
#[derive(Debug)]
pub struct Context {
    pub market_data: Arc<MarketData>,
    pub current_timestamp: i64,
    // Add EventQueue sender or something similar to place orders
    // For now we can confirm API shape
}

impl Context {
    pub fn new(market_data: Arc<MarketData>) -> Self {
        Self {
            market_data,
            current_timestamp: 0,
        }
    }
    
    pub fn now(&self) -> i64 {
        self.current_timestamp
    }
    
    pub fn bar(&self, _instrument_id: u32) -> Option<&Bar> {
        // This is tricky. In an event driven system, "current" bar is the one in the event.
        // But strategies might want to look up "latest known" bar for other instruments.
        // The implementation needs to track "latest state".
        // For MVP/T008, just defining the struct is enough.
        None
    }
    
    // API for Strategy
    pub fn place_order(&mut self, _instrument_id: u32, _side: Side, _order_type: OrderType, _qty: u32) -> u64 {
        // Generate Order ID, push Order event
        0 // Stub
    }
    
    pub fn cancel_order(&mut self, _order_id: u64) {
        // Push Cancel event
    }
}
