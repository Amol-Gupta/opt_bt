use opt_bt::common::context::Context;
use opt_bt::common::event::FillEvent;
use opt_bt::common::types::{Side, Status, PRICE_SCALE};
use opt_bt::data::models::{Bar, MarketData};
use opt_bt::strategy::examples::NiftyNearestExpiryStraddleStrategy;
use opt_bt::strategy::Strategy;
use std::sync::Arc;

#[test]
fn test_after_close_logs_position_pnl_when_populated() {
    let mut market_data = MarketData::new();
    market_data.add_bar(
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
    market_data.add_bar(
        "NIFTY",
        Bar {
            timestamp: 101,
            open: 110 * PRICE_SCALE,
            high: 110 * PRICE_SCALE,
            low: 110 * PRICE_SCALE,
            close: 110 * PRICE_SCALE,
            volume: 1,
        },
    );
    let instrument_id = market_data.get_id("NIFTY").expect("missing instrument");

    let mut ctx = Context::new(Arc::new(market_data), 1_000_000 * PRICE_SCALE);
    ctx.set_time(101);

    let fill = FillEvent {
        timestamp: 100,
        order_id: 1,
        instrument_id,
        side: Side::Buy,
        quantity: 2,
        fill_price: 100 * PRICE_SCALE,
        fee: 0,
        status: Status::Filled,
        strategy_id: "default".to_string(),
    };
    ctx.on_fill_exposure(&fill);
    ctx.account.on_fill(&fill);

    let mut strategy = NiftyNearestExpiryStraddleStrategy::new("NIFTY", 1);
    strategy.after_close(&mut ctx);

    let has_populated_position_log = strategy.event_log().iter().any(|line| {
        line.contains("after_close position_pnl")
            && line.contains("instrument_id=")
            && line.contains("qty=2")
            && line.contains("unrealized=")
    });
    assert!(
        has_populated_position_log,
        "expected populated after_close position_pnl log entry"
    );
}
