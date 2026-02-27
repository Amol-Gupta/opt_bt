use std::sync::Arc;
use crate::common::context::Context;
use crate::common::event::{Event, EventQueue, MarketEvent, OrderEvent, FillEvent};
use crate::strategy::Strategy;
use crate::execution::fill::{FillModel, DefaultFillModel};
use crate::data::models::MarketData;
use crate::common::logging::set_simulation_time;

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
                    instrument_id: *instrument_id,
                };
                self.event_queue.push(Event::Market(event));
            }
        }
        
        self.strategy.init(&mut self.context);
    }
    
    pub fn run(&mut self) {
        self.on_start();
        let mut current_day: Option<i64> = None;
        
        while let Some(event) = self.event_queue.pop() {
            let event_day = event.timestamp().div_euclid(86_400);
            if current_day != Some(event_day) {
                if current_day.is_some() {
                    self.strategy.after_close(&mut self.context);
                }
                self.strategy.on_date_change(&mut self.context);
                self.strategy.before_open(&mut self.context);
                current_day = Some(event_day);
            }

            // Update Context Time
             self.context.set_time(event.timestamp());
             set_simulation_time(event.timestamp());
             
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

        if current_day.is_some() {
            self.strategy.after_close(&mut self.context);
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
        
        let mut remaining_orders = Vec::with_capacity(self.pending_orders.len());
        let mut fills = Vec::with_capacity(self.pending_orders.len());

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
        self.context.on_fill_exposure(event);
        self.context.account.on_fill(event);
        
        // 2. Notify Strategy
        self.strategy.on_fill(&mut self.context, event);
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
        pub date_change_count: usize,
        pub before_open_count: usize,
        pub after_close_count: usize,
    }
    
    impl TestStrategy {
        fn new() -> Self {
            Self {
                start_called: false,
                stop_called: false,
                date_change_count: 0,
                before_open_count: 0,
                after_close_count: 0,
            }
        }
    }
    
    impl Strategy for TestStrategy {
        fn init(&mut self, _ctx: &mut Context) {}
        fn before_open(&mut self, _ctx: &mut Context) { self.before_open_count += 1; }
        fn after_close(&mut self, _ctx: &mut Context) { self.after_close_count += 1; }
        fn on_start(&mut self, _ctx: &mut Context) { self.start_called = true; }
        fn on_stop(&mut self, _ctx: &mut Context) { self.stop_called = true; }
        fn on_date_change(&mut self, _ctx: &mut Context) { self.date_change_count += 1; }
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

    #[test]
    fn test_day_boundary_hooks_are_called_in_sequence() {
        let strategy = TestStrategy::new();
        let mut market_data = MarketData::new();
        market_data.add_bar(
            "TEST",
            crate::data::models::Bar {
                timestamp: 100,
                open: 1_000_000,
                high: 1_000_000,
                low: 1_000_000,
                close: 1_000_000,
                volume: 1,
            },
        );
        market_data.add_bar(
            "TEST",
            crate::data::models::Bar {
                timestamp: 86_500,
                open: 1_000_000,
                high: 1_000_000,
                low: 1_000_000,
                close: 1_000_000,
                volume: 1,
            },
        );

        let mut engine = Engine::new(strategy, Arc::new(market_data), 10_000);
        engine.init();
        engine.run();

        assert_eq!(engine.strategy.date_change_count, 2);
        assert_eq!(engine.strategy.before_open_count, 2);
        assert_eq!(engine.strategy.after_close_count, 2);
    }
}
