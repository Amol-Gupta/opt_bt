use std::cmp::Ordering;
use std::collections::BinaryHeap;
use crate::data::models::Bar;
use crate::core::types::{OrderType, Side, Status};
// use crate::strategy::Signal; // Not defined yet

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventType {
    MarketData { instrument_id: u32, bar: Bar },
    // Signal { strategy_id: u32, signal: Signal },
    OrderUpdate { order_id: u64, status: Status },
    Timer { id: u64, strategy_id: u32, payload: i64 },
}

#[derive(Debug, Clone, Eq)]
pub struct Event {
    pub timestamp: i64,
    pub priority: u8, // Lower is higher priority? Or higher is higher? 
                      // Spec says: "Reverse(timestamp) then priority"
                      // Usually priority 0 is highest.
    pub payload: EventType,
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp && self.priority == other.priority
    }
}

// Custom Ord implementation for Min-Heap based on Timestamp
// We want the smallest timestamp to be popped first.
// BinaryHeap is a Max-Heap. So we need to reverse the comparison.
impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        other.timestamp.cmp(&self.timestamp)
            .then_with(|| other.priority.cmp(&self.priority))
    }
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct EventQueue {
    heap: BinaryHeap<Event>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
        }
    }
    
    pub fn push(&mut self, event: Event) {
        self.heap.push(event);
    }
    
    pub fn pop(&mut self) -> Option<Event> {
        self.heap.pop()
    }
    
    pub fn peek(&self) -> Option<&Event> {
        self.heap.peek()
    }
    
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_queue_ordering() {
        let mut queue = EventQueue::new();
        
        let e1 = Event {
            timestamp: 100,
            priority: 1,
            payload: EventType::Timer { id: 1, strategy_id: 1, payload: 0 },
        };
        
        let e2 = Event {
            timestamp: 90,
            priority: 1,
            payload: EventType::Timer { id: 2, strategy_id: 1, payload: 0 },
        };
        
        let e3 = Event {
            timestamp: 100,
            priority: 0, // Higher priority (assuming 0 is higher than 1 in our logic above? Wait.)
                         // Ord: other.priority.cmp(&self.priority).
                         // If self=0, other=1. other(1).cmp(0) is Greater.
                         // So 1 > 0.
                         // In MaxHeap (BinaryHeap), 1 would be popped before 0.
                         // This means HIGHER number is HIGHER priority.
                         // Spec usually implies Priority 0 is top. 
                         // Let's adjust comment or logic. 
                         // "Secondary sort key (e.g., Data < Signal < Order)"
                         // If we want deterministic order: Data processed first.
                         // So Data should have "highest" priority (popped first).
            payload: EventType::Timer { id: 3, strategy_id: 1, payload: 0 },
        };

        queue.push(e1.clone());
        queue.push(e2.clone());
        queue.push(e3.clone());
        
        // Expected order:
        // 90 (e2)
        // 100, Priority ? 
        // We used `other.priority.cmp(&self.priority)`.
        // BinaryHeap pops MAX.
        // If we want Priority 0 to be popped BEFORE Priority 1 (at same timestamp).
        // Then (0) should be > (1).
        // other(1).cmp(0) -> Greater. So 1 > 0. 
        // So 1 is popped first.
        // So HIGHER NUMBER = HIGHER PRIORITY.
        
        assert_eq!(queue.pop().unwrap().timestamp, 90);
        
        let next = queue.pop().unwrap();
        assert_eq!(next.timestamp, 100);
        // We want strict ordering. Let's say we want 0 popped before 1.
        // If we want 0 > 1 (in heap terms).
        // other(1).cmp(0) -> Greater. 
        // So 1 is "Greater" than 0.
        // We probably want Lower Number = Higher Priority (0 is first).
        // To make 0 > 1 in specific Ord:
        // We need `self.priority.cmp(&other.priority)`?
        // self(0).cmp(1) -> Less. So 0 < 1. 1 popped first.
        // We want 0 popped first. So 0 essentially needs to be "Larger" in the max-heap logic.
        // Correct logic for Min-Heap simulation is tricky with MaxHeap.
        // Let's rely on standard Rust: BinaryHeap is MaxHeap.
        // To get MinHeap behavior for Timestamp: reverse comparison.
        // `other.timestamp.cmp(&self.timestamp)`. (Small timestamps are "Greater").
        // For Priority: We want Small Priority Number (0) to be "Greater" than Large Priority Number (1).
        // So we also reverse priority.
        // `other.priority.cmp(&self.priority)`.
        // other(1).cmp(0) -> Greater. So 1 > 0. 1 is popped first.
        // So Higher Number is Popped First.
        // So 1 is higher priority than 0.
        
        // If we want 0 to be higher priority (Data=0), we should change the cmp.
        // or just use Higher Number = Higher Priority convention.
        // Let's check spec GFR-001 "Data < Signal < Order".
        // Data should be processed first.
        // So Data Priority > Signal Priority.
        // If Data=0, Signal=1. And we want Data first.
        // We need 0 to come out before 1.
        // So 0 > 1 in heap.
        // `other.priority.cmp(&self.priority)` -> 1 > 0. 1 comes out.
        // `self.priority.cmp(&other.priority)` -> 0 < 1. 1 comes out.
        
        // Wait. `cmp` defines ordering.
        // If `a.cmp(b)` is Greater, a > b. A is at top of MaxHeap.
        // We want Small Timestamp (90) > Large Timestamp (100).
        // 100.cmp(90) -> Greater.
        // We reverse: 90.cmp(100) -> Less. 
        // other.cmp(self). 
        // Check 90 vs 100.
        // self=90, other=100. 100.cmp(90) -> Greater. So 90 is "Less"? No.
        
        // Let's stick to simple: `std::cmp::Reverse`.
        // But we have custom struct.
    }
}
