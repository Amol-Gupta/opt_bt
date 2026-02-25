use opt_bt::data::models::{MarketData, Bar};
use opt_bt::strategy::examples::RandomStrategy;
use opt_bt::engine::runner::Engine;
use opt_bt::reporting::json::generate_report;
use std::sync::Arc;
use std::collections::HashMap;

#[test]
fn test_single_strategy_run() {
    // 1. Setup Market Data (Synthetic)
    let mut market_data = MarketData::new();
    let symbol = "NIFTY23JAN20000CE";
    
    // Generate 100 bars of data
    // Price starts at 100, walks randomly
    let mut price = 100.0 * 10000.0;
    let mut bars = Vec::new();
    let start_ts = 1672531200; // 2023-01-01
    
    for i in 0..100 {
        let open = price as i64;
        let close = (price + (if i % 2 == 0 { 5000.0 } else { -5000.0 })) as i64; // Alternating up/down
        let high = open.max(close) + 1000;
        let low = open.min(close) - 1000;
        
        bars.push(Bar {
            timestamp: start_ts + i * 60,
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
    
    println!("Total Trades: {}", report.metrics.trade_count);
    println!("Final Balance: {}", report.metrics.final_cash_balance);
    
    assert!(report.metrics.trade_count > 0, "Expected some trades to occur");
    assert!(report.trades.len() > 0);
    
    // Check if we didn't crash and money changed
    assert_ne!(report.metrics.final_cash_balance, 1_000_000.0);
}
