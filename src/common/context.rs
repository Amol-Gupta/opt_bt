use std::sync::Arc;
use std::collections::{BTreeSet, HashMap};
use crate::data::models::{Bar, MarketData};
use crate::common::types::{InstrumentId, OrderType, Side};
use crate::common::event::{Event, OrderEvent, FillEvent};
use crate::portfolio::manager::Account;
use crate::portfolio::allocator::PortfolioAllocator;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SubscriptionDiff {
    pub subscribed: Vec<InstrumentId>,
    pub unsubscribed: Vec<InstrumentId>,
    pub unchanged: Vec<InstrumentId>,
}

/// Context provides the Strategy with access to market data and execution capabilities.
/// It acts as a facade/gateway.
#[derive(Debug)]
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

    // Runtime diagnostics collected during simulation.
    pub warnings: Vec<String>,

    // Optional portfolio-level allocator guardrails.
    pub allocator: Option<PortfolioAllocator>,

    // Strategy-level live exposure state.
    pub strategy_open_notional: HashMap<String, i64>,
    strategy_positions: HashMap<String, HashMap<u32, i64>>,
    strategy_mark_prices: HashMap<String, HashMap<u32, i64>>,
    order_strategy_map: HashMap<u64, String>,
    desired_subscriptions: BTreeSet<InstrumentId>,
    active_subscriptions: BTreeSet<InstrumentId>,
}

impl Context {
    pub fn new(market_data: Arc<MarketData>, initial_capital: i64) -> Self {
        Self {
            market_data,
            current_timestamp: 0,
            event_buffer: Vec::new(),
            account: Account::new(initial_capital),
            active_strategy_id: "default".to_string(),
            warnings: Vec::new(),
            allocator: None,
            strategy_open_notional: HashMap::new(),
            strategy_positions: HashMap::new(),
            strategy_mark_prices: HashMap::new(),
            order_strategy_map: HashMap::new(),
            desired_subscriptions: BTreeSet::new(),
            active_subscriptions: BTreeSet::new(),
        }
    }

    pub fn set_allocator(&mut self, allocator: PortfolioAllocator) {
        self.allocator = Some(allocator);
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

    pub fn warn(&mut self, message: String) {
        log::warn!("{}", message);
        self.warnings.push(message);
    }

    pub fn set_desired_subscriptions<I>(&mut self, instrument_ids: I)
    where
        I: IntoIterator<Item = InstrumentId>,
    {
        self.desired_subscriptions = instrument_ids.into_iter().collect();
    }

    pub fn clear_desired_subscriptions(&mut self) {
        self.desired_subscriptions.clear();
    }

    pub fn apply_subscription_diff(&mut self) -> SubscriptionDiff {
        let subscribed: Vec<InstrumentId> = self
            .desired_subscriptions
            .difference(&self.active_subscriptions)
            .copied()
            .collect();
        let unsubscribed: Vec<InstrumentId> = self
            .active_subscriptions
            .difference(&self.desired_subscriptions)
            .copied()
            .collect();
        let unchanged: Vec<InstrumentId> = self
            .active_subscriptions
            .intersection(&self.desired_subscriptions)
            .copied()
            .collect();

        self.active_subscriptions = self.desired_subscriptions.clone();

        SubscriptionDiff {
            subscribed,
            unsubscribed,
            unchanged,
        }
    }

    pub fn active_subscriptions(&self) -> Vec<InstrumentId> {
        self.active_subscriptions.iter().copied().collect()
    }

    pub fn desired_subscriptions(&self) -> Vec<InstrumentId> {
        self.desired_subscriptions.iter().copied().collect()
    }
    
    pub fn get_bar(&self, instrument_id: u32) -> Option<&Bar> {
        self.market_data
            .get_bar_at_or_before(instrument_id, self.current_timestamp)
    }

    /// Place a new order
    pub fn place_order(&mut self, instrument_id: u32, side: Side, order_type: OrderType, quantity: i64) -> u64 {
        if quantity <= 0 {
            self.warn(format!(
                "rejected order: non-positive quantity={} strategy_id='{}' instrument_id={}",
                quantity, self.active_strategy_id, instrument_id
            ));
            return 0;
        }

        if side == Side::Buy {
            let required_cash = self.estimate_order_notional(instrument_id, &order_type, quantity);
            if required_cash > self.account.cash {
                self.warn(format!(
                    "rejected order: insufficient capital strategy_id='{}' instrument_id={} required_cash={} available_cash={}",
                    self.active_strategy_id,
                    instrument_id,
                    required_cash,
                    self.account.cash
                ));
                return 0;
            }
        }

        let additional_notional = self.estimate_order_notional(instrument_id, &order_type, quantity);
        let current_open_notional = *self
            .strategy_open_notional
            .get(&self.active_strategy_id)
            .unwrap_or(&0);

        if let Some(allocator) = &self.allocator {
            if !allocator.can_accept_notional(
                &self.active_strategy_id,
                current_open_notional,
                additional_notional,
            ) {
                self.warn(format!(
                    "allocator rejected order: strategy_id='{}' instrument_id={} current_open_notional={} additional_notional={}",
                    self.active_strategy_id, instrument_id, current_open_notional, additional_notional
                ));
                return 0;
            }
        }

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
        self.order_strategy_map.insert(order_id, self.active_strategy_id.clone());
        self.event_buffer.push(event);
        order_id
    }

    pub fn on_fill_exposure(&mut self, fill: &FillEvent) {
        let strategy_id = if fill.strategy_id.is_empty() {
            self.order_strategy_map
                .get(&fill.order_id)
                .cloned()
                .unwrap_or_else(|| "unknown".to_string())
        } else {
            fill.strategy_id.clone()
        };

        let signed_delta = match fill.side {
            Side::Buy => fill.quantity,
            Side::Sell => -fill.quantity,
        };

        {
            let positions = self
                .strategy_positions
                .entry(strategy_id.clone())
                .or_insert_with(HashMap::new);
            let qty = positions.entry(fill.instrument_id).or_insert(0);
            *qty += signed_delta;
        }

        self.strategy_mark_prices
            .entry(strategy_id.clone())
            .or_insert_with(HashMap::new)
            .insert(fill.instrument_id, fill.fill_price);

        let mut exposure_total: i128 = 0;
        if let Some(positions) = self.strategy_positions.get(&strategy_id) {
            let marks = self.strategy_mark_prices.get(&strategy_id);
            for (instrument_id, qty) in positions {
                if *qty == 0 {
                    continue;
                }
                let mark = marks
                    .and_then(|m| m.get(instrument_id))
                    .copied()
                    .unwrap_or(fill.fill_price);
                exposure_total += (*qty as i128).abs() * (mark as i128);
            }
        }

        let exposure_total = exposure_total.clamp(i64::MIN as i128, i64::MAX as i128) as i64;
        self.strategy_open_notional.insert(strategy_id, exposure_total);
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

    fn estimate_order_notional(&self, instrument_id: u32, order_type: &OrderType, quantity: i64) -> i64 {
        let price = match order_type {
            OrderType::Limit(price) | OrderType::Stop(price) => *price,
            OrderType::Market => self.get_bar(instrument_id).map(|bar| bar.close).unwrap_or(0),
        };

        ((quantity as i128).abs() * (price as i128))
            .clamp(i64::MIN as i128, i64::MAX as i128) as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::event::FillEvent;
    use crate::common::types::Status;
    use crate::portfolio::allocator::PortfolioAllocator;
    use crate::common::types::PRICE_SCALE;

    #[test]
    fn test_context_place_order() {
        let mut md = MarketData::new();
        md.add_bar(
            "NIFTY",
            Bar {
                timestamp: 100,
                open: 100 * PRICE_SCALE,
                high: 100 * PRICE_SCALE,
                low: 100 * PRICE_SCALE,
                close: 100 * PRICE_SCALE,
                volume: 1,
            },
        );
        let md = Arc::new(md);
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

    #[test]
    fn test_allocator_rejects_excess_notional() {
        let mut md = MarketData::new();
        md.add_bar(
            "NIFTY",
            Bar {
                timestamp: 100,
                open: 100 * PRICE_SCALE,
                high: 100 * PRICE_SCALE,
                low: 100 * PRICE_SCALE,
                close: 100 * PRICE_SCALE,
                volume: 1,
            },
        );
        let instrument_id = md.get_id("NIFTY").expect("instrument missing");
        let mut ctx = Context::new(Arc::new(md), 1_000_000 * PRICE_SCALE);
        ctx.set_time(100);
        ctx.set_strategy_id("s1");

        let mut allocator = PortfolioAllocator::new(1_000_000 * PRICE_SCALE);
        allocator.register_strategy("s1", 100_000 * PRICE_SCALE, 0.5);
        ctx.set_allocator(allocator);

        let order_id = ctx.place_order(instrument_id, Side::Buy, OrderType::Market, 1_000);
        assert_eq!(order_id, 0);
        assert!(ctx.event_buffer.is_empty());
        assert!(ctx
            .warnings
            .iter()
            .any(|w| w.contains("allocator rejected order")));
    }

    #[test]
    fn test_strategy_exposure_updates_from_fills() {
        let md = Arc::new(MarketData::new());
        let mut ctx = Context::new(md, 1_000_000 * PRICE_SCALE);

        let buy_fill = FillEvent {
            timestamp: 100,
            order_id: 1,
            instrument_id: 7,
            side: Side::Buy,
            quantity: 2,
            fill_price: 100 * PRICE_SCALE,
            fee: 0,
            status: Status::Filled,
            strategy_id: "s1".to_string(),
        };
        ctx.on_fill_exposure(&buy_fill);
        assert_eq!(ctx.strategy_open_notional.get("s1").copied(), Some(200 * PRICE_SCALE));

        let sell_fill = FillEvent {
            timestamp: 101,
            order_id: 2,
            instrument_id: 7,
            side: Side::Sell,
            quantity: 1,
            fill_price: 100 * PRICE_SCALE,
            fee: 0,
            status: Status::Filled,
            strategy_id: "s1".to_string(),
        };
        ctx.on_fill_exposure(&sell_fill);
        assert_eq!(ctx.strategy_open_notional.get("s1").copied(), Some(100 * PRICE_SCALE));
    }

    #[test]
    fn test_subscription_diff_is_deterministic() {
        let md = Arc::new(MarketData::new());
        let mut ctx = Context::new(md, 1_000_000 * PRICE_SCALE);

        ctx.set_desired_subscriptions(vec![3, 1, 2]);
        let first = ctx.apply_subscription_diff();
        assert_eq!(first.subscribed, vec![1, 2, 3]);
        assert!(first.unsubscribed.is_empty());
        assert!(first.unchanged.is_empty());

        ctx.set_desired_subscriptions(vec![2, 4]);
        let second = ctx.apply_subscription_diff();
        assert_eq!(second.subscribed, vec![4]);
        assert_eq!(second.unsubscribed, vec![1, 3]);
        assert_eq!(second.unchanged, vec![2]);
        assert_eq!(ctx.active_subscriptions(), vec![2, 4]);
    }

    #[test]
    fn test_rejects_buy_order_on_insufficient_capital() {
        let mut md = MarketData::new();
        md.add_bar(
            "NIFTY",
            Bar {
                timestamp: 100,
                open: 100 * PRICE_SCALE,
                high: 100 * PRICE_SCALE,
                low: 100 * PRICE_SCALE,
                close: 100 * PRICE_SCALE,
                volume: 1,
            },
        );
        let instrument_id = md.get_id("NIFTY").expect("instrument missing");
        let mut ctx = Context::new(Arc::new(md), 10 * PRICE_SCALE);
        ctx.set_time(100);

        let order_id = ctx.place_order(instrument_id, Side::Buy, OrderType::Market, 1);
        assert_eq!(order_id, 0);
        assert!(ctx.event_buffer.is_empty());
        assert!(ctx
            .warnings
            .iter()
            .any(|w| w.contains("insufficient capital")));
    }
}

