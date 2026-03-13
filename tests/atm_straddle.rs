use std::sync::Arc;

use opt_bt::common::types::{InstrumentKind, OptionType};
use opt_bt::data::models::{Bar, Instrument, MarketData, OptionSpec};
use opt_bt::engine::runner::Engine;
use opt_bt::strategy::{AtmStraddleSellStrategy, NiftyNearestExpiryStraddleStrategy};

#[test]
fn test_atm_straddle_sells_at_10_exits_at_11() {
    let mut market_data = MarketData::new();

    let day_start = 1_704_067_200; // 2024-01-01 00:00:00 UTC
    let t_10 = day_start + 10 * 60 * 60;
    let t_11 = day_start + 11 * 60 * 60;

    let index_symbol = "NIFTY 50";
    let ce_symbol = "NIFTY11JAN2420000CE";
    let pe_symbol = "NIFTY11JAN2420000PE";

    for timestamp in [t_10, t_11] {
        market_data.add_bar(
            index_symbol,
            Bar {
                timestamp,
                open: 20_000 * 10_000,
                high: 20_020 * 10_000,
                low: 19_980 * 10_000,
                close: 20_000 * 10_000,
                volume: 1_000,
            },
        );

        market_data.add_bar(
            ce_symbol,
            Bar {
                timestamp,
                open: 120 * 10_000,
                high: 130 * 10_000,
                low: 110 * 10_000,
                close: 125 * 10_000,
                volume: 1_000,
            },
        );

        market_data.add_bar(
            pe_symbol,
            Bar {
                timestamp,
                open: 115 * 10_000,
                high: 125 * 10_000,
                low: 105 * 10_000,
                close: 118 * 10_000,
                volume: 1_000,
            },
        );
    }

    let ce_id = market_data
        .get_id(ce_symbol)
        .expect("CE instrument missing");
    let pe_id = market_data
        .get_id(pe_symbol)
        .expect("PE instrument missing");

    let strategy = AtmStraddleSellStrategy::new(index_symbol, 1);
    let mut engine = Engine::new(strategy, Arc::new(market_data), 1_000_000 * 10_000);

    engine.init();
    engine.run();

    let account = &engine.context.account;

    assert_eq!(account.trades.len(), 4, "expected 2 sell + 2 buy trades");

    let ce_pos = account.positions.get(&ce_id).expect("CE position missing");
    let pe_pos = account.positions.get(&pe_id).expect("PE position missing");

    assert_eq!(ce_pos.quantity, 0, "CE should be squared off by 11:00");
    assert_eq!(pe_pos.quantity, 0, "PE should be squared off by 11:00");
}

#[test]
fn test_nifty_nearest_expiry_straddle_logs_events_and_manages_subscriptions() {
    let mut market_data = MarketData::new();

    let day_start = 1_711_929_600; // 2024-04-01 00:00:00 UTC
    let t_10 = day_start + 10 * 60 * 60;
    let t_11 = day_start + 11 * 60 * 60;

    let index_symbol = "NIFTY 50";
    let near_ce_symbol = "NIFTY04APR2422000CE";
    let near_pe_symbol = "NIFTY04APR2422000PE";
    let far_ce_symbol = "NIFTY11APR2422000CE";

    for timestamp in [t_10, t_11] {
        market_data.add_bar(
            index_symbol,
            Bar {
                timestamp,
                open: 22_000 * 10_000,
                high: 22_020 * 10_000,
                low: 21_980 * 10_000,
                close: 22_000 * 10_000,
                volume: 1_000,
            },
        );

        market_data.add_bar(
            near_ce_symbol,
            Bar {
                timestamp,
                open: 120 * 10_000,
                high: 130 * 10_000,
                low: 110 * 10_000,
                close: 125 * 10_000,
                volume: 1_000,
            },
        );

        market_data.add_bar(
            near_pe_symbol,
            Bar {
                timestamp,
                open: 118 * 10_000,
                high: 128 * 10_000,
                low: 108 * 10_000,
                close: 121 * 10_000,
                volume: 1_000,
            },
        );

        market_data.add_bar(
            far_ce_symbol,
            Bar {
                timestamp,
                open: 140 * 10_000,
                high: 150 * 10_000,
                low: 130 * 10_000,
                close: 145 * 10_000,
                volume: 500,
            },
        );
    }

    let near_ce_id = market_data
        .get_id(near_ce_symbol)
        .expect("near CE instrument missing");
    let near_pe_id = market_data
        .get_id(near_pe_symbol)
        .expect("near PE instrument missing");
    let far_ce_id = market_data
        .get_id(far_ce_symbol)
        .expect("far CE instrument missing");

    market_data.upsert_instrument(Instrument {
        id: near_ce_id,
        symbol: near_ce_symbol.to_string(),
        kind: InstrumentKind::Option,
        option: Some(OptionSpec {
            underlying: "NIFTY".to_string(),
            expiry_yyyymmdd: 20240404,
            strike: 22_000 * 10_000,
            option_type: OptionType::Call,
        }),
    });
    market_data.upsert_instrument(Instrument {
        id: near_pe_id,
        symbol: near_pe_symbol.to_string(),
        kind: InstrumentKind::Option,
        option: Some(OptionSpec {
            underlying: "NIFTY".to_string(),
            expiry_yyyymmdd: 20240404,
            strike: 22_000 * 10_000,
            option_type: OptionType::Put,
        }),
    });
    market_data.upsert_instrument(Instrument {
        id: far_ce_id,
        symbol: far_ce_symbol.to_string(),
        kind: InstrumentKind::Option,
        option: Some(OptionSpec {
            underlying: "NIFTY".to_string(),
            expiry_yyyymmdd: 20240411,
            strike: 22_000 * 10_000,
            option_type: OptionType::Call,
        }),
    });

    let strategy = NiftyNearestExpiryStraddleStrategy::new(index_symbol, 1);
    let mut engine = Engine::new(strategy, Arc::new(market_data), 1_000_000 * 10_000);

    engine.init();
    engine.run();

    let log = engine.strategy.event_log();
    assert!(log.iter().any(|line| line.contains("on_start")));
    assert!(log.iter().any(|line| line.contains("on_date_change")));
    assert!(log.iter().any(|line| line.contains("before_open")));
    assert!(log.iter().any(|line| line.contains("on_market_event")));
    assert!(log.iter().any(|line| line.contains("on_order_event")));
    assert!(log.iter().any(|line| line.contains("on_fill")));
    assert!(log.iter().any(|line| line.contains("after_close")));
    assert!(log.iter().any(|line| line.contains("on_stop")));

    assert!(
        !log.iter()
            .any(|line| line.contains(&format!("ce_id={}", far_ce_id))),
        "far expiry contract should not be selected for ATM straddle"
    );

    assert!(
        engine.context.active_subscriptions().is_empty(),
        "subscriptions should be cleared after close"
    );
}
