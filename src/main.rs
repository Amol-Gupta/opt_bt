use opt_bt::config::{Config, SweepConfig};
use opt_bt::engine::runner::Engine;
use opt_bt::engine::sweep::run_sweep;
use opt_bt::strategy::examples::RandomStrategy;
use opt_bt::reporting::json::generate_report;
use opt_bt::reporting::aggregator::AggregatedSummary;
use opt_bt::data::models::MarketData;
use std::sync::Arc;
use clap::{Parser, Subcommand};
use std::fs;

use opt_bt::common::types::PRICE_SCALE;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run(config) => {
            println!("Running single backtest...");
            // TODO: Load market data properly
            let market_data = Arc::new(MarketData::new()); // Empty for now as loader is stub
            
            // Strategy factory based on config params
            // For MVP, we currently hardcode RandomStrategy or use config params to pick strategy.
            // Let's assume RandomStrategy with default params unless parsed.
            let probability = config.merged_params.get("prob").and_then(|s| s.parse().ok()).unwrap_or(0.5);
            let quantity = config.merged_params.get("qty").and_then(|s| s.parse().ok()).unwrap_or(1);
            
            let strategy = RandomStrategy::new(42, probability, quantity);
            
            let mut engine = Engine::new(strategy, market_data, config.initial_capital * PRICE_SCALE);
            engine.init();
            engine.run();
            
            let report = generate_report(&engine);
            let json = serde_json::to_string_pretty(&report).unwrap();
            println!("{}", json);
        }
        Commands::Sweep { config } => {
            println!("Running parameter sweep from config: {}", config);
            let sweep_cfg = SweepConfig::from_file(&config).expect("Failed to load sweep config");
            
            let market_data = Arc::new(MarketData::new()); // Stub data
            
            let results = run_sweep(&sweep_cfg, market_data, |cfg| {
                let probability = cfg.merged_params.get("prob").and_then(|s| s.parse().ok()).unwrap_or(0.5);
                let quantity = cfg.merged_params.get("qty").and_then(|s| s.parse().ok()).unwrap_or(1);
                RandomStrategy::new(42, probability, quantity)
            });
            
            let summary = AggregatedSummary::compute(&results);
            let json = serde_json::to_string_pretty(&summary).unwrap();
            println!("{}", json);
        }
    }
}
