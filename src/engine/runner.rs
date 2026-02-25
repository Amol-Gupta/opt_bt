use std::sync::Arc;
use crate::common::context::Context;
use crate::common::event::{Event, EventQueue, MarketEvent, OrderEvent, FillEvent};
use crate::strategy::Strategy;
use crate::execution::fill::{FillModel, DefaultFillModel};
use crate::portfolio::manager::Account;
use crate::data::models::MarketData;

pub struct Engine<S: Strategy> {
    pub context: Context,
    pub strategy: S,
    pub event_queue: EventQueue,
    // pub account: Account, // Moved to Context
    pub fill_model: Box<dyn FillModel>,
    pub market_data: Arc<MarketData>,
    pub pending_orders: Vec<OrderEvent>,
}

impl<S: Strategy> Engine<S> {
    pub fn new(strategy: S, market_data: Arc<MarketData>, initial_capital: i64) -> Self {
        Self {
            context: Context::new(market_data.clone(), initial_capital),
            strategy,
            event_queue: EventQueue::new(),
            // account: Account::new(initial_capital), // Removed
            fill_model: Box::new(DefaultFillModel::new()),
            market_data,
            pending_orders: Vec::new(),
        }
    }
    
    // Initialize simulation: Load first batch of events
    pub fn init(&mut self) {
        // Iterate over all instruments and their bars in MarketData
        for (instrument_id, bars) in &self.market_data.bars {
            for bar in bars {
                let event = MarketEvent {
                    timestamp: bar.timestamp,
                    instrument_id: instrument_id.clone(),
                };
                self.event_queue.push(Event::Market(event));
            }
        }
        
        self.strategy.init(&mut self.context);
    }
    
    pub fn run(&mut self) {
        self.on_start();
        
        while let Some(event) = self.event_queue.pop() {
            // Update Context Time
             self.context.set_time(event.timestamp());
             
             // Process Event
             match event {
                 Event::Market(e) => self.handle_market_event(&e),
                 Event::Signal(e) => self.strategy.on_signal(&mut self.context, &e),
                 Event::Order(e) => self.handle_order_event(&e),
                 Event::Fill(e) => self.handle_fill_event(&e),
             }
             
             // After processing, collect new events from Context
             let new_events = self.context.collect_events();
             for e in new_events {
                 self.event_queue.push(e);
             }
        }
        
        self.on_stop();
    }
    
    fn on_start(&mut self) {
        self.strategy.on_start(&mut self.context);
        println!("Backtest started.");
    }
    
    fn on_stop(&mut self) {
        self.strategy.on_stop(&mut self.context);
        println!("Backtest finished. Final Portfolio Cash: {}", self.context.account.cash);
    }
    
    fn handle_market_event(&mut self, event: &MarketEvent) {
        // 1. Check Pending Orders for Fills BEFORE Strategy sees new bar
        // (Assuming Limit orders work on this bar's High/Low)
        
        let mut remaining_orders = Vec::new();
        let mut fills = Vec::new();

        for order in &self.pending_orders {
            if let Some(fill_event) = self.fill_model.fill_order(order, &self.market_data) {
                fills.push(fill_event);
            } else {
                remaining_orders.push(order.clone());
            }
        }
        
        self.pending_orders = remaining_orders;
        
        for fill in fills {
            self.event_queue.push(Event::Fill(fill));
        }

        // 2. Notify Strategy
        self.strategy.on_market_event(&mut self.context, event);
    }
    
    fn handle_order_event(&mut self, event: &OrderEvent) {
        // 1. Notify Strategy (Lifecycle)
        self.strategy.on_order_event(&mut self.context, event);
        
        // 2. Try to fill immediately
        if let Some(fill) = self.fill_model.fill_order(event, &self.market_data) {
             self.event_queue.push(Event::Fill(fill));
        } else {
             self.pending_orders.push(event.clone());
        }
    }
    
    fn handle_fill_event(&mut self, event: &FillEvent) {
        // 1. Update Portfolio/Account
        self.context.account.on_fill(event);
        
        // 2. Notify Strategy
        self.strategy.on_fill(&mut self.context, event);
        println!("Filled: {:?} {} @ {}", event.side, event.quantity, event.fill_price);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::Strategy;
    use crate::common::context::Context; // Explicitly verify context import
    
    struct TestStrategy {
        pub start_called: bool,
        pub stop_called: bool,
    }
    
    impl TestStrategy {
        fn new() -> Self {
            Self { start_called: false, stop_called: false }
        }
    }
    
    impl Strategy for TestStrategy {
        fn init(&mut self, _ctx: &mut Context) {}
        fn on_start(&mut self, _ctx: &mut Context) { self.start_called = true; }
        fn on_stop(&mut self, _ctx: &mut Context) { self.stop_called = true; }
        fn on_market_event(&mut self, _ctx: &mut Context, _event: &MarketEvent) {}
        fn on_signal(&mut self, _ctx: &mut Context, _event: &crate::common::event::SignalEvent) {}
        fn on_order_event(&mut self, _ctx: &mut Context, _event: &OrderEvent) {}
        fn on_fill(&mut self, _ctx: &mut Context, _event: &FillEvent) {}
    }
    
    #[test]
    fn test_engine_lifecycle() {
        let strategy = TestStrategy::new();
        let market_data = Arc::new(MarketData::new());
        let mut engine = Engine::new(strategy, market_data, 10000);
        
        engine.init();
        engine.run();
        
        // Strategy is moved into Engine. We can't inspect it directly unless we extract it or inspect side effects.
        // Wait, Engine owns Strategy. We need to check if methods were called.
        // A common pattern is using Arc<Mutex<State>> in Strategy to verify side effects.
        // Or inspect engine.strategy if it's public.
        
        assert!(engine.strategy.start_called);
        assert!(engine.strategy.stop_called);
    }
}
