use opt_bt::config::{Config, SweepConfig, StrategyConfig};
use opt_bt::data::loader::DataLoader;
use opt_bt::engine::runner::Engine;
use opt_bt::engine::sweep::run_sweep;
use opt_bt::strategy::examples::{
    AtmStraddleSellStrategy,
    NiftyNearestExpiryStraddleStrategy,
    RandomStrategy,
    SmaNifty50Strategy,
};
use opt_bt::strategy::portfolio::PortfolioStrategy;
use opt_bt::strategy::Strategy;
use opt_bt::reporting::json::generate_report_with_reproducibility;
use opt_bt::reporting::aggregator::AggregatedSummary;
use opt_bt::reporting::reproducibility::build_reproducibility;
use opt_bt::portfolio::allocator::PortfolioAllocator;
use opt_bt::common::logging;
use clap::{Parser, Subcommand};
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};

use opt_bt::common::types::PRICE_SCALE;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Single backtest run
    Run(Config),
    
    /// Parameter sweep optimization
    Sweep {
        /// Path to sweep configuration JSON file
        #[arg(long)]
        config: String,
    },
}

impl std::fmt::Display for Cli {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::fmt::Display for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run(raw_config) => {
            eprintln!("Running single backtest...");
            let config = raw_config
                .resolve()
                .unwrap_or_else(|err| panic!("Failed to resolve run config: {err}"));
            let _logger_guard = logging::init_with_time_mode_and_file(
                &config.log_level,
                Some(&config.log_time_mode),
                config.log_file.as_deref(),
            );

            let strategy_label = if let Some(strategy) = &config.strategy {
                strategy.clone()
            } else if let Some(portfolio) = &config.portfolio {
                let ids = portfolio
                    .strategies
                    .iter()
                    .map(|item| item.id.clone())
                    .collect::<Vec<_>>()
                    .join(",");
                format!("portfolio[{ids}]")
            } else {
                "random".to_string()
            };

            log::info!(
                "Backtest config: wall_start={} strategy={} start_date={} end_date={} data={} initial_capital={} log_level={} log_time_mode={}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                strategy_label,
                config.start_date.clone().unwrap_or_else(|| "<none>".to_string()),
                config.end_date.clone().unwrap_or_else(|| "<none>".to_string()),
                config
                    .data_dir
                    .clone()
                    .unwrap_or_else(|| "<auto-resolve>".to_string()),
                config.initial_capital,
                config.log_level,
                config.log_time_mode
            );
            let data_path = resolve_data_path(config.data_dir.as_deref())
                .unwrap_or_else(|err| panic!("Failed to resolve data path: {err}"));
            let market_data = DataLoader::load_parquet(data_path.to_string_lossy().as_ref())
                .unwrap_or_else(|err| panic!("Failed to load parquet data: {err}"));
            let strategy = build_portfolio_strategy(&config);
            let mut engine = Engine::new(strategy, market_data, config.initial_capital * PRICE_SCALE);
            if let Some(allocator) = build_allocator_from_config(&config, config.initial_capital * PRICE_SCALE) {
                engine.context.set_allocator(allocator);
            }
            engine.init();
            engine.run();

            let reproducibility = build_reproducibility(&config, data_path.to_string_lossy().as_ref())
                .unwrap_or_else(|err| panic!("Failed to build reproducibility metadata: {err}"));
            let report = generate_report_with_reproducibility(&engine, Some(reproducibility));
            let json = serde_json::to_string_pretty(&report).unwrap();
            if let Some(path) = &config.report_path {
                fs::write(path, &json).unwrap_or_else(|err| panic!("Failed to write report file: {err}"));
            }
            println!("{}", json);
        }
        Commands::Sweep { config } => {
            eprintln!("Running parameter sweep from config: {}", config);
            let sweep_cfg = SweepConfig::from_file(&config).expect("Failed to load sweep config");
            let _logger_guard = logging::init_with_time_mode_and_file(
                &sweep_cfg.base_config.log_level,
                Some(&sweep_cfg.base_config.log_time_mode),
                sweep_cfg.base_config.log_file.as_deref(),
            );
            let data_path = resolve_data_path(sweep_cfg.base_config.data_dir.as_deref())
                .unwrap_or_else(|err| panic!("Failed to resolve data path: {err}"));
            let market_data = DataLoader::load_parquet(data_path.to_string_lossy().as_ref())
                .unwrap_or_else(|err| panic!("Failed to load parquet data: {err}"));
            
            let results = run_sweep(
                &sweep_cfg,
                market_data,
                data_path.to_string_lossy().as_ref(),
                |cfg| {
                (
                    build_portfolio_strategy(cfg),
                    build_allocator_from_config(cfg, cfg.initial_capital * PRICE_SCALE),
                )
                },
            );
            
            let summary = AggregatedSummary::compute(&results);
            let json = serde_json::to_string_pretty(&summary).unwrap();
            println!("{}", json);
        }
    }
}

fn build_portfolio_strategy(config: &Config) -> PortfolioStrategy {
    let mut portfolio = PortfolioStrategy::new();

    if let Some(portfolio_cfg) = &config.portfolio {
        for strategy_cfg in &portfolio_cfg.strategies {
            let strategy = build_child_strategy(strategy_cfg)
                .unwrap_or_else(|| panic!("unknown strategy kind '{}' for id '{}'", strategy_cfg.kind, strategy_cfg.id));
            portfolio.add_strategy(&strategy_cfg.id, strategy);
        }
        return portfolio;
    }

    let kind = config.strategy.clone().unwrap_or_else(|| "random".to_string());
    let id = "default".to_string();
    let spec = StrategyConfig {
        id: id.clone(),
        kind,
        params: config.merged_params.clone(),
        capital_limit: None,
        max_exposure_ratio: None,
    };
    let strategy = build_child_strategy(&spec)
        .unwrap_or_else(|| panic!("unknown strategy kind '{}'", spec.kind));
    portfolio.add_strategy(&id, strategy);
    portfolio
}

fn build_child_strategy(spec: &StrategyConfig) -> Option<Box<dyn Strategy + Send>> {
    match spec.kind.as_str() {
        "random" => {
            let probability = spec
                .params
                .get("prob")
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.5);
            let quantity = spec
                .params
                .get("qty")
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(1);
            let seed = spec
                .params
                .get("seed")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(42);
            Some(Box::new(RandomStrategy::new(seed, probability, quantity)))
        }
        "atm_straddle" | "atm_straddle_sell" => {
            let index_symbol = spec
                .params
                .get("index_symbol")
                .cloned()
                .unwrap_or_else(|| "NIFTY 50".to_string());
            let quantity = spec
                .params
                .get("qty")
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(1);
            Some(Box::new(AtmStraddleSellStrategy::new(&index_symbol, quantity)))
        }
        "nifty_nearest_expiry_straddle" => {
            let index_symbol = spec
                .params
                .get("index_symbol")
                .cloned()
                .unwrap_or_else(|| "NIFTY 50".to_string());
            let quantity = spec
                .params
                .get("qty")
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(1);
            Some(Box::new(NiftyNearestExpiryStraddleStrategy::new(&index_symbol, quantity)))
        }
        "sma_nifty50" => {
            let index_symbol = spec
                .params
                .get("index_symbol")
                .cloned()
                .unwrap_or_else(|| "NIFTY 50".to_string());
            let quantity = spec
                .params
                .get("qty")
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(1);
            let short_period = spec
                .params
                .get("short_period")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(20);
            let long_period = spec
                .params
                .get("long_period")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(50);
            Some(Box::new(SmaNifty50Strategy::new(
                &index_symbol,
                quantity,
                short_period,
                long_period,
            )))
        }
        _ => None,
    }
}

fn build_allocator_from_config(config: &Config, total_capital_scaled: i64) -> Option<PortfolioAllocator> {
    let portfolio_cfg = config.portfolio.as_ref()?;
    let mut allocator = PortfolioAllocator::new(total_capital_scaled);
    let mut has_rules = false;

    for strategy in &portfolio_cfg.strategies {
        if let Some(capital_limit) = strategy.capital_limit {
            allocator.register_strategy(
                &strategy.id,
                capital_limit * PRICE_SCALE,
                strategy.max_exposure_ratio.unwrap_or(1.0),
            );
            has_rules = true;
        }
    }

    if has_rules {
        Some(allocator)
    } else {
        None
    }
}

fn resolve_data_path(configured: Option<&str>) -> Result<PathBuf, String> {
    if let Some(path) = configured {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Ok(candidate);
        }

        return Err(format!("configured data path does not exist: {path}"));
    }

    let fallback = Path::new("sample_data/niftyIndex2024.sample.parquet");
    if fallback.exists() {
        return Ok(fallback.to_path_buf());
    }

    Err("no data path configured; pass --data-dir <parquet-path> or add sample fixtures".to_string())
}
