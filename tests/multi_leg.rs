use opt_bt::data::models::{MarketData, Bar};
use opt_bt::engine::runner::Engine;
use opt_bt::common::context::Context;
use opt_bt::common::event::{MarketEvent, FillEvent};
use opt_bt::strategy::Strategy;
use opt_bt::common::types::{OrderType, Side};
use opt_bt::reporting::json::generate_report;
use std::sync::Arc;

// A simple Straddle Strategy for testing
// Buys 1 lot of CE and 1 lot of PE at the start
struct StraddleStrategy {
    ce_id: u32,
    pe_id: u32,
    quantity: i64,
    entered: bool,
}

impl StraddleStrategy {
    fn new(ce_id: u32, pe_id: u32, quantity: i64) -> Self {
        Self {
            ce_id,
            pe_id,
            quantity,
            entered: false,
        }
    }
}

impl Strategy for StraddleStrategy {
    fn on_start(&mut self, _ctx: &mut Context) {
        println!("StraddleStrategy: Started");
    }

    fn on_market_event(&mut self, ctx: &mut Context, _event: &MarketEvent) {
        if self.entered {
            return;
        }

        // We need pricing for both to enter a straddle properly (in reality), 
        // but for this test, we'll just fire market orders for both as soon as we see data for any.
        // Or better: Place orders for both legs.
        
        // Check if we have seen data for both? 
        // For simplicity, just place orders for both legs immediately on the first event.
        // In a real strategy, we'd wait for quote ticks for both.
        
        // Let's ensure we place orders for known instruments.
        // Since we know the IDs, we can just place orders.
        // However, the engine processes events sequentially. 
        // If we place an order for an instrument that hasn't had a market event yet (no price),
        // the fill model might reject it or it might sit pending until price arrives.
        // The DefaultFillModel needs a bar to fill.
        
        // So let's place orders.
        println!("Placing Straddle Orders");
        ctx.place_order(self.ce_id, Side::Buy, OrderType::Market, self.quantity);
        ctx.place_order(self.pe_id, Side::Buy, OrderType::Market, self.quantity);
        
        self.entered = true;
    }

    fn on_fill(&mut self, _ctx: &mut Context, event: &FillEvent) {
        println!("Filled: {:?}", event);
    }
}

#[test]
fn test_multi_leg_straddle() {
    // 1. Setup Market Data
    let mut market_data = MarketData::new();
    let symbol_ce = "NIFTY23JAN20000CE";
    let symbol_pe = "NIFTY23JAN20000PE";
    
    // Generate bars for CE
    let mut bars_ce = Vec::new();
    let start_ts = 1672564500; // 09:15:00 UTC
    for i in 0..10 {
        bars_ce.push(Bar {
            timestamp: start_ts + i * 60,
            open: 100 * 10000,
            high: 110 * 10000,
            low: 90 * 10000,
            close: 105 * 10000,
            volume: 100,
        });
    }
    
    // Generate bars for PE
    let mut bars_pe = Vec::new();
    for i in 0..10 {
        bars_pe.push(Bar {
            timestamp: start_ts + i * 60,
            open: 200 * 10000,
            high: 210 * 10000,
            low: 190 * 10000,
            close: 195 * 10000,
            volume: 100,
        });
    }

    // Add to market data
    // We need to grab the IDs
    // Since add_bar adds them if not present, let's just add one each first to get IDs
    market_data.add_bar(symbol_ce, bars_ce[0]); // ID 1
    market_data.add_bar(symbol_pe, bars_pe[0]); // ID 2
    
    let ce_id = market_data.get_id(symbol_ce).unwrap();
    let pe_id = market_data.get_id(symbol_pe).unwrap();
    
    // Add rest of bars
    for bar in bars_ce.iter().skip(1) {
        market_data.add_bar(symbol_ce, *bar);
    }
    for bar in bars_pe.iter().skip(1) {
        market_data.add_bar(symbol_pe, *bar);
    }

    let market_data = Arc::new(market_data);

    // 2. Setup Strategy
    let strategy = StraddleStrategy::new(ce_id, pe_id, 1);

    // 3. Setup Engine
    let initial_capital = 100_000 * 10000;
    let mut engine = Engine::new(strategy, market_data.clone(), initial_capital);

    // 4. Run
    engine.init();
    engine.run();

    // 5. Verify
    let report = generate_report(&engine);
    
    println!("Total Fills: {}", report.metrics.fill_count);
    
    // We expect at least 2 fills (1 Buy CE, 1 Buy PE)
    assert!(report.metrics.fill_count >= 2, "Expected at least 2 fills (straddle leg buys)");
    
    // Verify we have positions in both
    // note: generate_report doesn't expose current positions easily in the struct yet (it's in report.trades mostly)
    // iterate engine context directly
    let positions = &engine.context.account.positions;
    assert!(positions.contains_key(&ce_id), "Missing CE position");
    assert!(positions.contains_key(&pe_id), "Missing PE position");
    
    let pos_ce = positions.get(&ce_id).unwrap();
    let pos_pe = positions.get(&pe_id).unwrap();
    
    assert_eq!(pos_ce.quantity, 1, "CE Quantity mismatch");
    assert_eq!(pos_pe.quantity, 1, "PE Quantity mismatch");
}
