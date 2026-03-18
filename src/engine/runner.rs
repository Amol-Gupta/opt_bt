use std::sync::Arc;
use std::time::Instant;

use chrono::Timelike;

use crate::common::context::Context;
use crate::common::event::{
    AlarmEvent, CancelOrderEvent, Event, EventQueue, FillEvent, MarketEvent, OrderEvent,
    OrderRejectionEvent,
};
use crate::common::logging::set_simulation_time;
use crate::data::view::MarketDataView;
use crate::execution::fill::{DefaultFillModel, FillModel};
use crate::strategy::Strategy;

pub struct Engine<S: Strategy> {
    pub context: Context,
    pub strategy: S,
    pub event_queue: EventQueue,
    pub fill_model: Box<dyn FillModel>,
    pub market_data: Arc<dyn MarketDataView>,
    pub pending_orders: Vec<OrderEvent>,
    pub start_timestamp: Option<i64>,
    pub end_timestamp: Option<i64>,
}

#[derive(Debug, Default, Clone, Copy)]
struct EnginePhaseStats {
    timeline_steps: u64,
    timeline_build_ns: u128,
    loop_total_ns: u128,
    day_lifecycle_ns: u128,
    clock_update_ns: u128,
    start_stop_ns: u128,
    non_market_process_ns: u128,
    pending_fill_scan_ns: u128,
    strategy_market_cb_ns: u128,
    context_collect_ns: u128,
}

impl<S: Strategy> Engine<S> {
    pub fn new(strategy: S, market_data: Arc<dyn MarketDataView>, initial_capital: i64) -> Self {
        Self {
            context: Context::new(market_data.clone(), initial_capital),
            strategy,
            event_queue: EventQueue::new(),
            fill_model: Box::new(DefaultFillModel::new()),
            market_data,
            pending_orders: Vec::new(),
            start_timestamp: None,
            end_timestamp: None,
        }
    }

    pub fn set_date_bounds(&mut self, start_timestamp: i64, end_timestamp: i64) {
        self.start_timestamp = Some(start_timestamp);
        self.end_timestamp = Some(end_timestamp);
    }

    pub fn init(&mut self) {
        self.strategy.init(&mut self.context);
    }

    pub fn run(&mut self) {
        let profiling_enabled = std::env::var("BT_PROFILE")
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false);
        let mut stats = EnginePhaseStats::default();

        let started_timeline = Instant::now();
        let timeline = self.collect_market_timeline();
        stats.timeline_build_ns = started_timeline.elapsed().as_nanos();

        if let Some(first_timestamp) = timeline.first().copied().or(self.start_timestamp) {
            let started_clock = Instant::now();
            self.context.set_time(first_timestamp);
            set_simulation_time(first_timestamp);
            stats.clock_update_ns += started_clock.elapsed().as_nanos();
        }

        let started_lifecycle = Instant::now();
        self.on_start();
        stats.start_stop_ns += started_lifecycle.elapsed().as_nanos();

        let mut current_day: Option<i64> = None;
        let primary_instrument_id = self.primary_instrument_id();
        let started_loop_total = Instant::now();

        for timestamp in timeline {
            stats.timeline_steps += 1;

            let started_clock = Instant::now();
            self.context.set_time(timestamp);
            set_simulation_time(timestamp);
            stats.clock_update_ns += started_clock.elapsed().as_nanos();

            let event_day = timestamp.div_euclid(86_400);
            if current_day != Some(event_day) {
                let started_day_lifecycle = Instant::now();
                if current_day.is_some() {
                    self.strategy.after_close(&mut self.context);
                }
                self.strategy.on_date_change(&mut self.context);
                self.strategy.before_open(&mut self.context);
                stats.day_lifecycle_ns += started_day_lifecycle.elapsed().as_nanos();
                current_day = Some(event_day);
            }

            let started_non_market = Instant::now();
            self.process_due_non_market_events(timestamp);
            stats.non_market_process_ns += started_non_market.elapsed().as_nanos();

            let market_event = MarketEvent {
                timestamp,
                instrument_id: primary_instrument_id,
            };

            let started_market = Instant::now();
            self.handle_market_event(&market_event, &mut stats);
            stats.strategy_market_cb_ns += started_market.elapsed().as_nanos();

            let started_collect = Instant::now();
            let new_events = self.context.collect_events();
            stats.context_collect_ns += started_collect.elapsed().as_nanos();
            for event in new_events {
                self.event_queue.push(event);
            }

            let started_non_market_2 = Instant::now();
            self.process_due_non_market_events(timestamp);
            stats.non_market_process_ns += started_non_market_2.elapsed().as_nanos();
        }

        stats.loop_total_ns = started_loop_total.elapsed().as_nanos();

        if current_day.is_some() {
            let started_day_lifecycle = Instant::now();
            self.strategy.after_close(&mut self.context);
            stats.day_lifecycle_ns += started_day_lifecycle.elapsed().as_nanos();
        }

        let started_lifecycle = Instant::now();
        self.on_stop();
        stats.start_stop_ns += started_lifecycle.elapsed().as_nanos();

        if profiling_enabled {
            log::info!(
                "engine_profile steps={} timeline_build_ms={} loop_total_ms={} day_lifecycle_ms={} clock_update_ms={} start_stop_ms={} non_market_process_ms={} pending_fill_scan_ms={} strategy_market_cb_ms={} context_collect_ms={}",
                stats.timeline_steps,
                stats.timeline_build_ns / 1_000_000,
                stats.loop_total_ns / 1_000_000,
                stats.day_lifecycle_ns / 1_000_000,
                stats.clock_update_ns / 1_000_000,
                stats.start_stop_ns / 1_000_000,
                stats.non_market_process_ns / 1_000_000,
                stats.pending_fill_scan_ns / 1_000_000,
                stats.strategy_market_cb_ns / 1_000_000,
                stats.context_collect_ns / 1_000_000
            );
            log::info!(
                "engine_profile_ns steps={} timeline_build_ns={} loop_total_ns={} day_lifecycle_ns={} clock_update_ns={} start_stop_ns={} non_market_process_ns={} pending_fill_scan_ns={} strategy_market_cb_ns={} context_collect_ns={}",
                stats.timeline_steps,
                stats.timeline_build_ns,
                stats.loop_total_ns,
                stats.day_lifecycle_ns,
                stats.clock_update_ns,
                stats.start_stop_ns,
                stats.non_market_process_ns,
                stats.pending_fill_scan_ns,
                stats.strategy_market_cb_ns,
                stats.context_collect_ns
            );
        }
    }

    fn on_start(&mut self) {
        self.strategy.on_start(&mut self.context);
        println!("Backtest started.");
    }

    fn on_stop(&mut self) {
        self.strategy.on_stop(&mut self.context);
        println!(
            "Backtest finished. Final Portfolio Cash: {}",
            self.context.account.cash
        );
    }

    fn handle_market_event(&mut self, event: &MarketEvent, stats: &mut EnginePhaseStats) {
        let mut remaining_orders = Vec::with_capacity(self.pending_orders.len());
        let mut fills = Vec::with_capacity(self.pending_orders.len());

        let started_fill_scan = Instant::now();
        for order in &self.pending_orders {
            if let Some(fill_event) =
                self.fill_model
                    .fill_order(order, self.market_data.as_ref(), event.timestamp)
            {
                fills.push(fill_event);
            } else {
                remaining_orders.push(order.clone());
            }
        }
        stats.pending_fill_scan_ns += started_fill_scan.elapsed().as_nanos();

        self.pending_orders = remaining_orders;

        for fill in fills {
            self.event_queue.push(Event::Fill(fill));
        }

        self.strategy.on_market_event(&mut self.context, event);
    }

    fn handle_order_event(&mut self, event: &OrderEvent) {
        self.strategy.on_order_event(&mut self.context, event);

        if let Some(fill) =
            self.fill_model
                .fill_order(event, self.market_data.as_ref(), event.timestamp)
        {
            self.event_queue.push(Event::Fill(fill));
        } else {
            self.pending_orders.push(event.clone());
        }
    }

    fn handle_fill_event(&mut self, event: &FillEvent) {
        self.context.on_fill_exposure(event);
        self.context.account.on_fill(event);

        self.strategy.on_fill(&mut self.context, event);
    }

    fn handle_cancel_order_event(&mut self, event: &CancelOrderEvent) {
        self.pending_orders.retain(|order| order.order_id != event.order_id);
    }

    fn handle_order_rejection_event(&mut self, event: &OrderRejectionEvent) {
        self.context.set_strategy_id(&event.strategy_id);
        self.strategy.on_order_rejected(&mut self.context, event);
    }

    fn handle_alarm_event(&mut self, event: &AlarmEvent) {
        self.context.set_strategy_id(&event.strategy_id);
        self.strategy.on_alarm(&mut self.context, event);
    }

    fn process_due_non_market_events(&mut self, current_timestamp: i64) {
        loop {
            let due = matches!(
                self.event_queue.peek(),
                Some(event) if event.timestamp() <= current_timestamp
            );
            if !due {
                break;
            }

            let Some(event) = self.event_queue.pop() else {
                break;
            };

            match event {
                Event::Market(_) => {}
                Event::Alarm(alarm) => self.handle_alarm_event(&alarm),
                Event::Signal(signal) => self.strategy.on_signal(&mut self.context, &signal),
                Event::Order(order) => self.handle_order_event(&order),
                Event::OrderRejection(rejection) => self.handle_order_rejection_event(&rejection),
                Event::CancelOrder(cancel) => self.handle_cancel_order_event(&cancel),
                Event::Fill(fill) => self.handle_fill_event(&fill),
            }

            let new_events = self.context.collect_events();
            for generated in new_events {
                self.event_queue.push(generated);
            }
        }
    }

    fn collect_market_timeline(&self) -> Vec<i64> {
        let start_timestamp = self.start_timestamp;
        let end_timestamp = self.end_timestamp;

        self.market_data
            .market_timeline()
            .into_iter()
            .filter(|timestamp| is_market_hour(*timestamp))
            .filter(|timestamp| {
                let after_start = start_timestamp
                    .map(|start| *timestamp >= start)
                    .unwrap_or(true);
                let before_end = end_timestamp.map(|end| *timestamp <= end).unwrap_or(true);
                after_start && before_end
            })
            .collect()
    }

    fn primary_instrument_id(&self) -> u32 {
        if let Some(index_id) = self.market_data.get_id("NIFTY 50") {
            return index_id;
        }

        self.market_data.min_instrument_id().unwrap_or(0)
    }
}

fn is_market_hour(timestamp: i64) -> bool {
    let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp, 0) else {
        return false;
    };
    let minutes = dt.hour() * 60 + dt.minute();
    let market_open = 9 * 60 + 15;
    let market_close = 15 * 60 + 30;
    minutes >= market_open && minutes <= market_close
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::context::Context;
    use crate::data::models::MarketData;
    use crate::strategy::Strategy;

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
        fn before_open(&mut self, _ctx: &mut Context) {
            self.before_open_count += 1;
        }
        fn after_close(&mut self, _ctx: &mut Context) {
            self.after_close_count += 1;
        }
        fn on_start(&mut self, _ctx: &mut Context) {
            self.start_called = true;
        }
        fn on_stop(&mut self, _ctx: &mut Context) {
            self.stop_called = true;
        }
        fn on_date_change(&mut self, _ctx: &mut Context) {
            self.date_change_count += 1;
        }
        fn on_market_event(&mut self, _ctx: &mut Context, _event: &MarketEvent) {}
        fn on_signal(&mut self, _ctx: &mut Context, _event: &crate::common::event::SignalEvent) {}
        fn on_order_event(&mut self, _ctx: &mut Context, _event: &OrderEvent) {}
        fn on_fill(&mut self, _ctx: &mut Context, _event: &FillEvent) {}
    }

    #[test]
    fn test_engine_lifecycle() {
        let strategy = TestStrategy::new();
        let market_data = Arc::new(MarketData::new());
        let mut engine = Engine::new(strategy, market_data, 10_000);

        engine.init();
        engine.run();

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
                timestamp: 34_200,
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
                timestamp: 120_600,
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
