use std::sync::Arc;
use opt_bt::common::context::Context;
use opt_bt::common::event::{FillEvent, MarketEvent, SignalEvent, Event};
use opt_bt::common::types::{Side, Status, OrderType};
use opt_bt::strategy::Strategy;
use opt_bt::strategy::portfolio::PortfolioStrategy;
use opt_bt::data::models::{Bar, MarketData};
use std::collections::HashMap;

struct MockStrategy {
    id: String,
    target_qty: u32,
}

impl MockStrategy {
    fn new(id: &str, target_qty: u32) -> Self {
        Self {
            id: id.to_string(),
            target_qty,
        }
    }
}

impl Strategy for MockStrategy {
    fn on_market_event(&mut self, ctx: &mut Context, _event: &MarketEvent) {
        // Place initial order
        ctx.place_order(1, Side::Buy, self.target_qty, OrderType::Market);
    }
    
    fn on_signal(&mut self, _ctx: &mut Context, _event: &SignalEvent) {}
    
    fn on_fill(&mut self, ctx: &mut Context, event: &FillEvent) {
        // Confirm we received fill intended for us
        // Place a follow-up order with unique qty (target_qty * 10)
        // Only if the fill matches our initial order (which it should)
        
        if event.strategy_id == self.id {
             ctx.place_order(1, Side::Sell, self.target_qty * 10, OrderType::Market);
        }
    }
}

#[test]
fn test_portfolio_routing() {
    let mut portfolio = PortfolioStrategy::new();
    
    // Strategy A places qty 10, responds with 100
    // Strategy B places qty 20, responds with 200
    portfolio.add_strategy("A", Box::new(MockStrategy::new("A", 10)));
    portfolio.add_strategy("B", Box::new(MockStrategy::new("B", 20)));
    
    let market_data = Arc::new(MarketData {
        bars: HashMap::new(),
        instruments: HashMap::new(),
    });

    let mut context = Context::new(market_data.clone(), 100_000);
    
    // 1. Initial Market Event -> Each places 1 order
    // MarketEvent does not carry bar data directly, context would usually provide it via market_data if needed
    let market_event = MarketEvent {
        timestamp: 100,
        instrument_id: 1,
    };
    
    portfolio.on_market_event(&mut context, &market_event);
    
    // Check initial orders
    let events_1: Vec<_> = context.event_buffer.drain(..).collect();
    let orders_1: Vec<_> = events_1.iter().filter_map(|e| {
        if let Event::Order(o) = e { Some(o) } else { None }
    }).collect();

    assert_eq!(orders_1.len(), 2, "Expected 2 initial orders");
    
    let order_a = orders_1.iter().find(|o| o.strategy_id == "A").expect("Order A missing");
    assert_eq!(order_a.quantity, 10);
    
    let order_b = orders_1.iter().find(|o| o.strategy_id == "B").expect("Order B missing");
    assert_eq!(order_b.quantity, 20);
    
    // 2. Trigger Fill for Strategy A
    // We construct a FillEvent explicitly targeting "A"
    // The PortfolioStrategy should route this to Strat A, which places order qty 100.
    let fill_a = FillEvent {
        timestamp: 200,
        order_id: order_a.order_id,
        instrument_id: 1,
        side: Side::Buy,
        quantity: 10,
        fill_price: 10,
        fee: 0,
        status: Status::Filled,
        strategy_id: "A".to_string(),
    };
    
    portfolio.on_fill(&mut context, &fill_a);
    
    // Check for new order with qty 100
    let events_2: Vec<_> = context.event_buffer.drain(..).collect();
    let orders_2: Vec<_> = events_2.iter().filter_map(|e| {
        if let Event::Order(o) = e { Some(o) } else { None }
    }).collect();

    assert_eq!(orders_2.len(), 1, "Expected 1 new order from A");
    
    let response_a = orders_2.last().unwrap();
    assert_eq!(response_a.strategy_id, "A");
    assert_eq!(response_a.quantity, 100);
    
    // 3. Trigger Fill for Strategy B
    let fill_b = FillEvent {
        timestamp: 200,
        order_id: order_b.order_id,
        instrument_id: 1,
        side: Side::Buy,
        quantity: 20,
        fill_price: 10,
        fee: 0,
        status: Status::Filled,
        strategy_id: "B".to_string(),
    };
    
    portfolio.on_fill(&mut context, &fill_b);
    
    let events_3: Vec<_> = context.event_buffer.drain(..).collect();
    let orders_3: Vec<_> = events_3.iter().filter_map(|e| {
        if let Event::Order(o) = e { Some(o) } else { None }
    }).collect();

    assert_eq!(orders_3.len(), 1); 
    
    let response_b = orders_3.last().unwrap();
    assert_eq!(response_b.strategy_id, "B");
    assert_eq!(response_b.quantity, 200);
}
