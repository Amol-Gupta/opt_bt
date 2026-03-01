use crate::common::types::{OrderType, Side, Status};
use crate::common::event::{OrderEvent, FillEvent};
use crate::data::view::MarketDataView;

pub trait FillModel {
    fn fill_order(&mut self, order: &OrderEvent, market_data: &dyn MarketDataView) -> Option<FillEvent>;
}

/// A simple fill model:
/// - Market orders fill at Open of the *next* bar (or current bar if assumed realtime, usually next).
/// - Limit orders fill if price crosses limit.
/// - We need to know "Current Price". In backtesting, usually we have the "Current Bar".
/// 
/// Wait. If we are processing `OrderEvent` at timestamp T.
/// The `MarketData` might have the bar for timestamp T or T+1.
/// 
/// If the strategy places an order at the END of bar T (on_close), it fills at OPEN of T+1.
/// If the strategy places an order during bar T (on_tick), it might fill at current price.
/// 
/// For simplicity in MVP:
/// - Assume we have access to the *current* bar for the instrument.
/// - Market orders fill at `Close` or `Next Open`?
/// - Let's assume we fill at the PRICE in the MarketEvent if available?
/// 
/// Actually, the `FillModel` is called by the Engine when a `MarketEvent` occurs OR when `OrderEvent` is processed.
/// The Engine loop will likely hold Pending Orders.
/// 
/// Let's define `StaticFillModel` that fills immediately if possible based on `last_known_price`.
use crate::execution::stale::{StaleDetector, DataStatus};
use crate::execution::slippage::SlippageModel;

pub struct DefaultFillModel {
    stale_detector: Option<StaleDetector>,
    slippage_model: Option<Box<dyn SlippageModel>>,
}

impl DefaultFillModel {
    pub fn new() -> Self {
        Self {
            stale_detector: None, 
            slippage_model: None,
        }
    }
    
    pub fn with_stale_detection(threshold_seconds: u64) -> Self {
        Self {
            stale_detector: Some(StaleDetector::new(threshold_seconds)),
            slippage_model: None,
        }
    }

    pub fn with_slippage(mut self, model: Box<dyn SlippageModel>) -> Self {
        self.slippage_model = Some(model);
        self
    }
}

impl FillModel for DefaultFillModel {
    fn fill_order(&mut self, order: &OrderEvent, market_data: &dyn MarketDataView) -> Option<FillEvent> {
        let target_bar = market_data.get_bar_at_or_before(order.instrument_id, order.timestamp)?;
        
        // Now check staleness on `target_bar`
        if let Some(detector) = &self.stale_detector {
             if detector.check(order.timestamp, target_bar.timestamp) != DataStatus::Fresh {
                 return None;
             }
        }

        // Check if we can fill using target_bar
        let bar = target_bar; // Alias for existing code below

        match order.order_type {
            OrderType::Market => {
                // Fill at Open? Close?
                // Let's assume simple backtest: Fill at Close of the current bar if timestamp matches.
                // Or if we are processing "Next Open", fill at Open.
                // Let's use `Close` for immediate fill at this timestamp.
                
                let mut fill_price = bar.close;
                if let Some(model) = &self.slippage_model {
                    fill_price = model.calculate_slippage(fill_price, order.quantity, order.side);
                }
                
                Some(FillEvent {
                    timestamp: order.timestamp,
                    order_id: order.order_id,
                    instrument_id: order.instrument_id,
                    side: order.side,
                    quantity: order.quantity,
                    fill_price, 
                    fee: 0,
                    status: Status::Filled,
                    strategy_id: order.strategy_id.clone(),
                })
            },
            OrderType::Limit(limit_price) => {
                // Check if price crossed limit
                // Buy Limit: Low <= Limit
                // Sell Limit: High >= Limit
                
                let can_fill = match order.side {
                    Side::Buy => bar.low <= limit_price,
                    Side::Sell => bar.high >= limit_price,
                };
                
                if can_fill {
                    Some(FillEvent {
                        timestamp: order.timestamp,
                        order_id: order.order_id,
                        instrument_id: order.instrument_id,
                        side: order.side,
                        quantity: order.quantity,
                        fill_price: limit_price, // Limit executes at Limit (conservative)
                        fee: 0,
                        status: Status::Filled,
                        strategy_id: order.strategy_id.clone(),
                    })
                } else {
                    None
                }
            },
            OrderType::Stop(stop_price) => {
                 // Buy Stop: High >= Stop
                 // Sell Stop: Low <= Stop
                 let triggered = match order.side {
                    Side::Buy => bar.high >= stop_price,
                    Side::Sell => bar.low <= stop_price,
                 };
                 
                 if triggered {
                     // Becomes Market Order - Fill at Stop Price (or worse)
                      Some(FillEvent {
                        timestamp: order.timestamp,
                        order_id: order.order_id,
                        instrument_id: order.instrument_id,
                        side: order.side,
                        quantity: order.quantity,
                        fill_price: stop_price, 
                        fee: 0,
                        status: Status::Filled,
                        strategy_id: order.strategy_id.clone(),
                    })
                 } else {
                     None
                 }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::models::{Bar, MarketData};
    use crate::common::types::{OrderType, Side, Status};

    #[test]
    fn test_fill_market_order() {
        let mut md = MarketData::new();
        let bar = Bar { timestamp: 100, open: 100, high: 110, low: 90, close: 105, volume: 1000 };
        md.add_bar("TEST", bar);
        let id = md.get_id("TEST").unwrap();
        
        let mut model = DefaultFillModel::new();
        let order = OrderEvent {
            timestamp: 100,
            order_id: 1,
            instrument_id: id,
            order_type: OrderType::Market,
            side: Side::Buy,
            price: 0,
            quantity: 10,
            strategy_id: "test".to_string(),
        };
        
        let fill = model.fill_order(&order, &md).unwrap();
        assert_eq!(fill.fill_price, 105);
        assert_eq!(fill.status, Status::Filled);
    }
    
    #[test]
    fn test_fill_limit_order() {
        let mut md = MarketData::new();
        let bar = Bar { timestamp: 100, open: 100, high: 110, low: 90, close: 105, volume: 1000 };
        md.add_bar("TEST", bar);
        let id = md.get_id("TEST").unwrap();
        
        let mut model = DefaultFillModel::new();
        
        // Buy Limit @ 95 (Low is 90, so it fills)
        let order_fill = OrderEvent {
            timestamp: 100,
            order_id: 1,
            instrument_id: id,
            order_type: OrderType::Limit(95),
            side: Side::Buy,
            price: 95,
            quantity: 10,
            strategy_id: "test".to_string(),
        };
        
        let fill = model.fill_order(&order_fill, &md).unwrap();
        assert_eq!(fill.fill_price, 95);
        
        // Buy Limit @ 85 (Low is 90, no fill)
        let order_no_fill = OrderEvent {
            timestamp: 100,
            order_id: 2,
            instrument_id: id,
            order_type: OrderType::Limit(85),
            side: Side::Buy,
            price: 85,
            quantity: 10,
            strategy_id: "test".to_string(),
        };
        
         assert!(model.fill_order(&order_no_fill, &md).is_none());
    }
}
