use std::collections::BTreeMap;

use opt_bt::cache::ipc as cache_ipc;
use opt_bt::cache::snapshot::load_market_data_snapshot;
use my_strategy::create_strategy_by_id;
use opt_bt::common::logging;
use opt_bt::common::types::PRICE_SCALE;
use opt_bt::engine::runner::Engine;
use opt_bt::reporting::json::generate_report;
use opt_bt::strategy::portfolio::PortfolioStrategy;

fn parse_params(values: &[String]) -> Result<BTreeMap<String, String>, String> {
    let mut params = BTreeMap::new();
    for token in values {
        let Some((key, value)) = token.split_once('=') else {
            return Err(format!("invalid --params token '{}': expected key=value", token));
        };
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() {
            return Err(format!("invalid --params token '{}': empty key", token));
        }
        params.insert(key.to_string(), value.to_string());
    }
    Ok(params)
}

fn load_market_data(
    data_path: &str,
    start_ts: i64,
    end_ts: i64,
) -> std::sync::Arc<opt_bt::data::models::MarketData> {
    let addr = std::env::var("BT_CACHE_ADDR")
        .unwrap_or_else(|_| panic!("BT_CACHE_ADDR is required: backtest runs in cache-only mode"));
    let ensured = cache_ipc::ensure_loaded(&addr, data_path, Some((start_ts, end_ts)))
        .unwrap_or_else(|err| panic!("Cache ENSURE failed for {data_path} via {addr}: {err}"));
    let snapshot_path = std::path::Path::new(&ensured.entry.snapshot_path);
    let md = load_market_data_snapshot(snapshot_path).unwrap_or_else(|err| {
        panic!(
            "Failed to load cache snapshot {}: {err}",
            ensured.entry.snapshot_path
        )
    });
    log::info!(
        "Loaded market data via cache snapshot: cache_hit={} snapshot={}",
        ensured.cache_hit,
        ensured.entry.snapshot_path
    );
    md
}

fn main() {
    let mut strategy_id: Option<String> = None;
    let mut data: Option<String> = None;
    let mut start_date: Option<String> = None;
    let mut end_date: Option<String> = None;
    let mut initial_capital: i64 = 1_000_000;
    let mut log_level = "info".to_string();
    let mut log_time_mode = "simulation".to_string();
    let mut log_file: Option<String> = None;
    let mut report_path: Option<String> = None;
    let mut raw_params: Vec<String> = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--strategy-id" => strategy_id = args.next(),
            "--data" => data = args.next(),
            "--initial-capital" => {
                if let Some(value) = args.next() {
                    initial_capital = value.parse::<i64>().unwrap_or(1_000_000);
                }
            }
            "--log-level" => {
                if let Some(value) = args.next() {
                    log_level = value;
                }
            }
            "--log-time-mode" => {
                if let Some(value) = args.next() {
                    log_time_mode = value;
                }
            }
            "--log-file" => log_file = args.next(),
            "--report-path" => report_path = args.next(),
            "--params" => {
                if let Some(value) = args.next() {
                    raw_params.push(value);
                }
            }
            "--start-date" => start_date = args.next(),
            "--end-date" => end_date = args.next(),
            _ => {}
        }
    }

    let strategy_id = strategy_id.unwrap_or_else(|| "my_strategy".to_string());
    let resolved_start_date = start_date
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| panic!("missing required backtest date range field(s): start_date"));
    let resolved_end_date = end_date
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| panic!("missing required backtest date range field(s): end_date"));
    let (start_ts, end_ts) = opt_bt::config::parse_date_range_to_epoch(resolved_start_date, resolved_end_date)
        .unwrap_or_else(|err| panic!("{}", err));
    let data = data.unwrap_or_else(|| "./sample_data/niftyIndex2024.sample.parquet".to_string());

    let params = parse_params(&raw_params).unwrap_or_else(|err| panic!("Failed to parse params: {}", err));
    let _logger_guard = logging::init_with_time_mode_and_file(
        &log_level,
        Some(&log_time_mode),
        log_file.as_deref(),
    );
    if log_time_mode.eq_ignore_ascii_case("simulation") {
        logging::set_simulation_time(start_ts);
    }
    log::info!(
        "Backtest config: strategy={} start_date={} end_date={} data={} initial_capital={} log_level={} log_time_mode={}",
        strategy_id,
        start_date.clone().unwrap_or_else(|| "<none>".to_string()),
        end_date.clone().unwrap_or_else(|| "<none>".to_string()),
        data,
        initial_capital,
        log_level,
        log_time_mode
    );
    let market_data = load_market_data(&data, start_ts, end_ts);

    let strategy = create_strategy_by_id(&strategy_id, &params)
        .unwrap_or_else(|| panic!("Unknown project strategy id: {}", strategy_id));

    let mut portfolio = PortfolioStrategy::new();
    portfolio.add_strategy("default", strategy);

    let mut engine = Engine::new(portfolio, market_data, initial_capital * PRICE_SCALE);
    engine.set_date_bounds(start_ts, end_ts);
    engine.init();
    engine.run();

    let report = generate_report(&engine);
    let json = serde_json::to_string_pretty(&report).expect("serialize report");
    if let Some(path) = report_path {
        std::fs::write(path, &json).expect("write report");
    }
    println!("{}", json);
}
