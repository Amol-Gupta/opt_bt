use crate::common::types::{OrderType, Side, Status};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
// use crate::strategy::Signal; // Not defined yet

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Market(MarketEvent),
    Alarm(AlarmEvent),
    Signal(SignalEvent),
    Order(OrderEvent),
    OrderRejection(OrderRejectionEvent),
    CancelOrder(CancelOrderEvent),
    Fill(FillEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketEvent {
    pub timestamp: i64,
    pub instrument_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalEvent {
    pub timestamp: i64,
    pub instrument_id: u32,
    pub side: Side,
    pub price: i64,
    pub quantity: i64,
    pub strategy_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlarmEvent {
    pub timestamp: i64,
    pub alarm_id: u64,
    pub key: String,
    pub strategy_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderEvent {
    pub timestamp: i64,
    pub order_id: u64,
    pub instrument_id: u32,
    pub order_type: OrderType,
    pub side: Side,
    pub price: i64,
    pub quantity: i64,
    pub strategy_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelOrderEvent {
    pub timestamp: i64,
    pub order_id: u64,
    pub strategy_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderRejectionEvent {
    pub timestamp: i64,
    pub instrument_id: u32,
    pub order_type: OrderType,
    pub side: Side,
    pub quantity: i64,
    pub reason: String,
    pub strategy_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillEvent {
    pub timestamp: i64,
    pub order_id: u64,
    pub instrument_id: u32,
    pub side: Side,
    pub quantity: i64,
    pub fill_price: i64,
    pub fee: i64,
    pub status: Status,
    pub strategy_id: String,
}

impl Event {
    pub fn timestamp(&self) -> i64 {
        match self {
            Event::Market(e) => e.timestamp,
            Event::Alarm(e) => e.timestamp,
            Event::Signal(e) => e.timestamp,
            Event::Order(e) => e.timestamp,
            Event::OrderRejection(e) => e.timestamp,
            Event::CancelOrder(e) => e.timestamp,
            Event::Fill(e) => e.timestamp,
        }
    }
}

// Custom Ord implementation for Min-Heap based on Timestamp
// We want the smallest timestamp to be popped first.
// BinaryHeap is a Max-Heap. So we need to reverse the comparison.
impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        // We want the smallest timestamp to be "Greatest" so it pops first.
        other.timestamp().cmp(&self.timestamp())
    }
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug)]
pub struct EventQueue {
    queue: BinaryHeap<Event>,
}

impl EventQueue {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, event: Event) {
        self.queue.push(event);
    }

    pub fn pop(&mut self) -> Option<Event> {
        self.queue.pop()
    }

    pub fn peek(&self) -> Option<&Event> {
        self.queue.peek()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_ordering() {
        let mut queue = EventQueue::new();

        let e1 = Event::Market(MarketEvent {
            timestamp: 100,
            instrument_id: 1,
        });
        let e2 = Event::Market(MarketEvent {
            timestamp: 50,
            instrument_id: 1,
        });
        let e3 = Event::Market(MarketEvent {
            timestamp: 150,
            instrument_id: 1,
        });

        queue.push(e1);
        queue.push(e2);
        queue.push(e3);

        // Should pop e2 (50), then e1 (100), then e3 (150)
        assert_eq!(queue.pop().unwrap().timestamp(), 50);
        assert_eq!(queue.pop().unwrap().timestamp(), 100);
        assert_eq!(queue.pop().unwrap().timestamp(), 150);
        assert!(queue.pop().is_none());
    }
}
