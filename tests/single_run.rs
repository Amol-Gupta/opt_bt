use opt_bt::common::context::Context;
use opt_bt::common::event::{FillEvent, MarketEvent, OrderEvent, SignalEvent};
use opt_bt::common::types::SimTime;
use opt_bt::data::models::{Bar, MarketData};
use opt_bt::engine::runner::Engine;
use opt_bt::reporting::json::generate_report;
use opt_bt::strategy::examples::RandomStrategy;
use opt_bt::strategy::Strategy;
use std::sync::Arc;

#[test]
fn test_single_strategy_run() {
    // 1. Setup Market Data (Synthetic)
    let mut market_data = MarketData::new();
    let symbol = "NIFTY23JAN20000CE";

    // Generate 100 bars of data
    // Price starts at 100, walks randomly
    let mut price = 100.0 * 10000.0;
    let mut bars = Vec::new();
    let start_ts = 1672564500; // 09:15:00 UTC

    for i in 0..100 {
        let open = price as i64;
        let close = (price + (if i % 2 == 0 { 5000.0 } else { -5000.0 })) as i64; // Alternating up/down
        let high = open.max(close) + 1000;
        let low = open.min(close) - 1000;

        bars.push(Bar {
            timestamp: SimTime::utc(start_ts + i * 60),
            open,
            high,
            low,
            close,
            volume: 100,
        });
        price = close as f64;
    }

    // Manually add bars (since market_data.add_bar adds one by one, we can use that)
    for bar in bars {
        market_data.add_bar(symbol, bar);
    }

    let market_data = Arc::new(market_data);

    // 2. Setup Strategy
    // Prob 0.5 means ~50 orders
    let strategy = RandomStrategy::new(42, 0.5, 1);

    // 3. Setup Engine
    let initial_capital = 1_000_000 * 10000; // 1M scaled
    let mut engine = Engine::new(strategy, market_data, initial_capital);

    // 4. Run Simulation
    engine.init();
    engine.run();

    // 5. Generate Report & Verify
    let report = generate_report(&engine);

    println!("Total Fills: {}", report.metrics.fill_count);
    println!("Final Balance: {}", report.metrics.final_cash_balance);

    assert!(
        report.metrics.fill_count > 0,
        "Expected some fills to occur"
    );
    assert!(report.fills.len() > 0);

    // Check if we didn't crash and money changed
    assert_ne!(report.metrics.final_cash_balance, 1_000_000.0);
}

struct DayHookStrategy {
    pub before_open_count: usize,
    pub after_close_count: usize,
}

impl DayHookStrategy {
    fn new() -> Self {
        Self {
            before_open_count: 0,
            after_close_count: 0,
        }
    }
}

impl Strategy for DayHookStrategy {
    fn before_open(&mut self, _ctx: &mut Context) {
        self.before_open_count += 1;
    }

    fn after_close(&mut self, _ctx: &mut Context) {
        self.after_close_count += 1;
    }

    fn on_market_event(&mut self, _ctx: &mut Context, _event: &MarketEvent) {}
    fn on_signal(&mut self, _ctx: &mut Context, _event: &SignalEvent) {}
    fn on_order_event(&mut self, _ctx: &mut Context, _event: &OrderEvent) {}
    fn on_fill(&mut self, _ctx: &mut Context, _event: &FillEvent) {}
}

#[test]
fn test_day_open_close_hooks_once_per_day() {
    let mut market_data = MarketData::new();
    let symbol = "NIFTY24APR22000CE";

    market_data.add_bar(
        symbol,
        Bar {
            timestamp: SimTime::utc(1_672_564_500),
            open: 1_000_000,
            high: 1_001_000,
            low: 999_000,
            close: 1_000_500,
            volume: 100,
        },
    );
    market_data.add_bar(
        symbol,
        Bar {
            timestamp: SimTime::utc(1_672_564_500 + 86_400),
            open: 1_000_000,
            high: 1_001_000,
            low: 999_000,
            close: 1_000_500,
            volume: 100,
        },
    );

    let strategy = DayHookStrategy::new();
    let mut engine = Engine::new(strategy, Arc::new(market_data), 1_000_000 * 10_000);

    engine.init();
    engine.run();

    assert_eq!(engine.strategy.before_open_count, 2);
    assert_eq!(engine.strategy.after_close_count, 2);
}
