use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::common::types::Price;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bar {
    pub timestamp: i64, // Unix Timestamp (seconds)
    pub open: Price,    // Price * 10,000
    pub high: Price,
    pub low: Price,
    pub close: Price,
    pub volume: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketData {
    // Map instrument ID to sorted bars
    pub bars: HashMap<u32, Vec<Bar>>,
    // Metadata for instruments (Symbol -> ID)
    pub instruments: HashMap<String, u32>,
    // Reverse lookup (ID -> Symbol)
    pub ids: HashMap<u32, String>,
}

impl MarketData {
    pub fn new() -> Self {
        Self {
            bars: HashMap::new(),
            instruments: HashMap::new(),
            ids: HashMap::new(),
        }
    }

    pub fn add_bar(&mut self, symbol: &str, bar: Bar) {
        let id = *self.instruments.entry(symbol.to_string()).or_insert_with(|| {
            let next_id = self.ids.len() as u32 + 1;
            self.ids.insert(next_id, symbol.to_string());
            next_id
        });
        
        self.bars.entry(id).or_insert_with(Vec::new).push(bar);
    }
    
    pub fn get_id(&self, symbol: &str) -> Option<u32> {
        self.instruments.get(symbol).copied()
    }
    
    pub fn get_symbol(&self, id: u32) -> Option<&str> {
        self.ids.get(&id).map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::types::PRICE_SCALE;

    #[test]
    fn test_market_data_basic() {
        let mut md = MarketData::new();
        let bar = Bar {
            timestamp: 1000,
            open: 100 * PRICE_SCALE,
            high: 105 * PRICE_SCALE,
            low: 95 * PRICE_SCALE,
            close: 102 * PRICE_SCALE,
            volume: 500,
        };
        
        md.add_bar("NIFTY", bar);
        
        assert_eq!(md.instruments.len(), 1);
        assert_eq!(md.get_id("NIFTY"), Some(1));
        assert_eq!(md.bars.get(&1).unwrap().len(), 1);
        assert_eq!(md.bars.get(&1).unwrap()[0], bar);
    }
}
