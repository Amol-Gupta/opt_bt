use opt_bt::common::context::Context;
use opt_bt::common::event::{Event, FillEvent, MarketEvent, OrderEvent, SignalEvent};
use opt_bt::common::types::{OrderType, Side, SimTime, Status};
use opt_bt::data::models::MarketData;
use opt_bt::strategy::portfolio::PortfolioStrategy;
use opt_bt::strategy::Strategy;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Default)]
struct StrategyState {
    fills_seen: usize,
}

struct MockStrategy {
    id: String,
    target_qty: i64,
    emitted_initial_order: bool,
    state: Arc<Mutex<StrategyState>>,
}

impl MockStrategy {
    fn new(id: &str, target_qty: i64, state: Arc<Mutex<StrategyState>>) -> Self {
        Self {
            id: id.to_string(),
            target_qty,
            emitted_initial_order: false,
            state,
        }
    }
}

impl Strategy for MockStrategy {
    fn on_market_event(&mut self, ctx: &mut Context, _event: &MarketEvent) {
        if !self.emitted_initial_order {
            ctx.place_order(1, Side::Buy, OrderType::Market, self.target_qty);
            self.emitted_initial_order = true;
        }
    }

    fn on_signal(&mut self, _ctx: &mut Context, _event: &SignalEvent) {}

    fn on_fill(&mut self, ctx: &mut Context, event: &FillEvent) {
        if event.strategy_id == self.id || ctx.active_strategy_id == self.id {
            {
                let mut state = self.state.lock().expect("state mutex poisoned");
                state.fills_seen += 1;
            }
            ctx.place_order(1, Side::Sell, OrderType::Market, self.target_qty * 10);
        }
    }
}

fn drain_orders(context: &mut Context) -> Vec<OrderEvent> {
    context
        .event_buffer
        .drain(..)
        .filter_map(|e| match e {
            Event::Order(o) => Some(o),
            _ => None,
        })
        .collect()
}

#[test]
fn test_portfolio_routing() {
    let mut portfolio = PortfolioStrategy::new();
    let a_state = Arc::new(Mutex::new(StrategyState::default()));
    let b_state = Arc::new(Mutex::new(StrategyState::default()));

    portfolio.add_strategy("A", Box::new(MockStrategy::new("A", 10, a_state.clone())));
    portfolio.add_strategy("B", Box::new(MockStrategy::new("B", 20, b_state.clone())));

    let market_data = Arc::new(MarketData::new());

    let mut context = Context::new(market_data.clone(), 100_000);

    let market_event = MarketEvent {
        timestamp: SimTime::utc(100),
        instrument_id: 1,
    };

    portfolio.on_market_event(&mut context, &market_event);
    let orders_1 = drain_orders(&mut context);

    assert_eq!(orders_1.len(), 2, "expected 2 initial orders");

    let order_a = orders_1
        .iter()
        .find(|o| o.strategy_id == "A")
        .expect("order A missing")
        .clone();
    assert_eq!(order_a.quantity, 10);

    let order_b = orders_1
        .iter()
        .find(|o| o.strategy_id == "B")
        .expect("order B missing")
        .clone();
    assert_eq!(order_b.quantity, 20);

    portfolio.on_order_event(&mut context, &order_a);
    portfolio.on_order_event(&mut context, &order_b);

    let fill_a = FillEvent {
        timestamp: SimTime::utc(200),
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

    let orders_2 = drain_orders(&mut context);
    assert_eq!(orders_2.len(), 1, "expected 1 response order from A");
    let response_a = orders_2.last().expect("missing A response order");
    assert_eq!(response_a.strategy_id, "A");
    assert_eq!(response_a.quantity, 100);

    let fill_b = FillEvent {
        timestamp: SimTime::utc(200),
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

    let orders_3 = drain_orders(&mut context);
    assert_eq!(orders_3.len(), 1, "expected 1 response order from B");
    let response_b = orders_3.last().expect("missing B response order");
    assert_eq!(response_b.strategy_id, "B");
    assert_eq!(response_b.quantity, 200);

    assert!(
        context.warnings.is_empty(),
        "unexpected warnings: {:?}",
        context.warnings
    );
    assert_eq!(a_state.lock().expect("state mutex poisoned").fills_seen, 1);
    assert_eq!(b_state.lock().expect("state mutex poisoned").fills_seen, 1);
}

#[test]
fn test_unroutable_signal_order_fill_generate_warnings() {
    let mut portfolio = PortfolioStrategy::new();
    let market_data = Arc::new(MarketData::new());
    let mut context = Context::new(market_data, 100_000);

    let signal = SignalEvent {
        timestamp: SimTime::utc(100),
        instrument_id: 1,
        side: Side::Buy,
        price: 10,
        quantity: 1,
        strategy_id: "missing".to_string(),
    };
    portfolio.on_signal(&mut context, &signal);

    let order = OrderEvent {
        timestamp: SimTime::utc(101),
        order_id: 999,
        instrument_id: 1,
        order_type: OrderType::Market,
        side: Side::Buy,
        price: 0,
        quantity: 1,
        strategy_id: "missing".to_string(),
    };
    portfolio.on_order_event(&mut context, &order);

    let fill = FillEvent {
        timestamp: SimTime::utc(102),
        order_id: 998,
        instrument_id: 1,
        side: Side::Buy,
        quantity: 1,
        fill_price: 10,
        fee: 0,
        status: Status::Filled,
        strategy_id: "missing".to_string(),
    };
    portfolio.on_fill(&mut context, &fill);

    assert_eq!(
        context.warnings.len(),
        3,
        "expected three unroutable warnings"
    );
    assert!(context
        .warnings
        .iter()
        .any(|w| w.contains("unroutable signal event")));
    assert!(context
        .warnings
        .iter()
        .any(|w| w.contains("unroutable order event")));
    assert!(context
        .warnings
        .iter()
        .any(|w| w.contains("unroutable fill event")));
}

#[test]
fn test_fill_fallback_routes_after_order_mapping_and_handles_out_of_order() {
    let mut portfolio = PortfolioStrategy::new();
    let a_state = Arc::new(Mutex::new(StrategyState::default()));
    portfolio.add_strategy("A", Box::new(MockStrategy::new("A", 10, a_state.clone())));

    let market_data = Arc::new(MarketData::new());
    let mut context = Context::new(market_data, 100_000);

    let market_event = MarketEvent {
        timestamp: SimTime::utc(200),
        instrument_id: 1,
    };
    portfolio.on_market_event(&mut context, &market_event);
    let initial_orders = drain_orders(&mut context);
    assert_eq!(initial_orders.len(), 1, "expected single initial order");
    let mapped_order = initial_orders[0].clone();

    let unknown_fill_before_map = FillEvent {
        timestamp: SimTime::utc(201),
        order_id: mapped_order.order_id,
        instrument_id: 1,
        side: Side::Buy,
        quantity: 10,
        fill_price: 10,
        fee: 0,
        status: Status::Filled,
        strategy_id: "missing".to_string(),
    };
    portfolio.on_fill(&mut context, &unknown_fill_before_map);
    assert!(
        drain_orders(&mut context).is_empty(),
        "no routed order expected before map"
    );
    assert_eq!(
        context.warnings.len(),
        1,
        "expected unroutable warning before mapping"
    );

    portfolio.on_order_event(&mut context, &mapped_order);

    let unknown_fill_after_map = FillEvent {
        timestamp: SimTime::utc(202),
        order_id: mapped_order.order_id,
        instrument_id: 1,
        side: Side::Buy,
        quantity: 10,
        fill_price: 10,
        fee: 0,
        status: Status::Filled,
        strategy_id: "missing".to_string(),
    };
    portfolio.on_fill(&mut context, &unknown_fill_after_map);

    let routed = drain_orders(&mut context);
    assert_eq!(routed.len(), 1, "expected fallback-routed response order");
    assert_eq!(routed[0].strategy_id, "A");
    assert_eq!(routed[0].quantity, 100);
    assert_eq!(a_state.lock().expect("state mutex poisoned").fills_seen, 1);
}

#[derive(Default)]
struct LifecycleState {
    before_open: usize,
    after_close: usize,
}

struct LifecycleStrategy {
    id: String,
    state: Arc<Mutex<HashMap<String, LifecycleState>>>,
}

impl LifecycleStrategy {
    fn new(id: &str, state: Arc<Mutex<HashMap<String, LifecycleState>>>) -> Self {
        Self {
            id: id.to_string(),
            state,
        }
    }
}

impl Strategy for LifecycleStrategy {
    fn before_open(&mut self, _ctx: &mut Context) {
        let mut map = self.state.lock().expect("lifecycle state mutex poisoned");
        let entry = map.entry(self.id.clone()).or_default();
        entry.before_open += 1;
    }

    fn after_close(&mut self, _ctx: &mut Context) {
        let mut map = self.state.lock().expect("lifecycle state mutex poisoned");
        let entry = map.entry(self.id.clone()).or_default();
        entry.after_close += 1;
    }
}

#[test]
fn test_portfolio_before_open_after_close_fanout() {
    let mut portfolio = PortfolioStrategy::new();
    let lifecycle = Arc::new(Mutex::new(HashMap::<String, LifecycleState>::new()));

    portfolio.add_strategy(
        "A",
        Box::new(LifecycleStrategy::new("A", lifecycle.clone())),
    );
    portfolio.add_strategy(
        "B",
        Box::new(LifecycleStrategy::new("B", lifecycle.clone())),
    );

    let market_data = Arc::new(MarketData::new());
    let mut context = Context::new(market_data, 100_000);

    portfolio.before_open(&mut context);
    portfolio.after_close(&mut context);

    let state = lifecycle.lock().expect("lifecycle state mutex poisoned");
    let a = state.get("A").expect("state for A missing");
    let b = state.get("B").expect("state for B missing");

    assert_eq!(a.before_open, 1);
    assert_eq!(a.after_close, 1);
    assert_eq!(b.before_open, 1);
    assert_eq!(b.after_close, 1);
}
