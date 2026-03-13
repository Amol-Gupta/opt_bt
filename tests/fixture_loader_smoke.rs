use opt_bt::data::loader::DataLoader;
use opt_bt::engine::runner::Engine;
use opt_bt::reporting::json::generate_report;
use opt_bt::strategy::NiftyNearestExpiryStraddleStrategy;
use opt_bt::strategy::RandomStrategy;

#[test]
fn test_load_fixture_parquet_files() {
    let merged = DataLoader::load_parquet("sample_data/merged_data.sample.parquet")
        .expect("failed to load merged fixture parquet");
    assert!(
        !merged.instruments.is_empty(),
        "expected merged fixture instruments"
    );

    let total_bars: usize = merged.bars.values().map(|bars| bars.len()).sum();
    assert!(total_bars > 0, "expected merged fixture bars");

    let has_option_symbol = merged
        .ids
        .values()
        .any(|sym| sym.ends_with("CE") || sym.ends_with("PE"));
    assert!(
        has_option_symbol,
        "expected CE/PE symbols in merged fixture"
    );

    let index = DataLoader::load_parquet("sample_data/niftyIndex2024.sample.parquet")
        .expect("failed to load index fixture parquet");
    assert!(
        !index.instruments.is_empty(),
        "expected index fixture instruments"
    );
    assert!(
        index.ids.values().any(|sym| sym.contains("NIFTY")),
        "expected NIFTY symbol in index fixture"
    );
}

#[test]
fn test_engine_smoke_with_fixture_loader_data() {
    let market_data = DataLoader::load_parquet("sample_data/niftyIndex2024.sample.parquet")
        .expect("failed to load index fixture parquet");

    let strategy = RandomStrategy::new(42, 0.0, 1);
    let mut engine = Engine::new(strategy, market_data, 1_000_000 * 10_000);

    let timeline_len = engine.market_data.market_timeline().len();
    assert!(timeline_len > 0, "expected non-empty market timeline");

    engine.init();

    engine.run();
    assert!(engine.context.account.cash > 0);
}

#[test]
fn test_nearest_expiry_straddle_trades_on_combined_fixture() {
    let market_data = DataLoader::load_parquet("sample_data/nifty_with_options.sample.parquet")
        .expect("failed to load combined fixture parquet");

    let option_count = market_data
        .instrument_meta
        .values()
        .filter(|instrument| instrument.option.is_some())
        .count();
    assert!(
        option_count > 0,
        "expected parsed option instruments in combined fixture"
    );

    let strategy = NiftyNearestExpiryStraddleStrategy::new("NIFTY 50", 1);
    let mut engine = Engine::new(strategy, market_data, 1_000_000 * 10_000);

    engine.init();
    engine.run();

    let report = generate_report(&engine);
    assert!(
        report.metrics.fill_count > 0,
        "expected nearest-expiry straddle to execute fills on combined fixture"
    );
    assert!(
        report.portfolio.total_trade_count > 0,
        "expected non-zero portfolio trade count"
    );
}
