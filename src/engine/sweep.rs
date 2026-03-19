use crate::common::types::PRICE_SCALE;
use crate::config::{Config, SweepConfig};
use crate::data::view::MarketDataView;
use crate::engine::runner::Engine;
use crate::portfolio::allocator::PortfolioAllocator;
use crate::reporting::json::{generate_report_with_reproducibility, BacktestReport};
use crate::reporting::reproducibility::build_reproducibility;
use crate::strategy::Strategy;

use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SweepResult {
    pub params: HashMap<String, String>,
    pub report: BacktestReport,
}

/// Run a parameter sweep with the given base strategy factory and sweep configuration.
///
/// F: Factory function: Fn(&Config) -> S
pub fn run_sweep<S, F>(
    sweep_config: &SweepConfig,
    market_data: Arc<dyn MarketDataView>,
    data_path: &str,
    strategy_factory: F,
) -> Vec<SweepResult>
where
    S: Strategy + Send,
    F: Fn(&Config) -> (S, Option<PortfolioAllocator>) + Sync + Send,
{
    // 1. Generate all configurations
    let configs = sweep_config.generate_permutations();

    // println!("Starting sweep with {} variations...", configs.len());

    // 2. Run in parallel using Rayon
    configs
        .into_par_iter()
        .map(|config| {
            // Create Strategy instance with specific params
            let (strategy, allocator) = strategy_factory(&config);

            // Create Engine
            let mut engine = Engine::new(
                strategy,
                market_data.clone(),
                config.initial_capital * PRICE_SCALE,
            );
            if let Some(allocator) = allocator {
                engine.context.set_allocator(allocator);
            }

            // Initialize
            engine.init();

            // Run
            engine.run();

            // Generate Report
            let reproducibility = build_reproducibility(&config, data_path)
                .expect("failed to build reproducibility metadata for sweep run");
            let report = generate_report_with_reproducibility(&engine, Some(reproducibility));

            SweepResult {
                params: config.merged_params,
                report,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::run_sweep;
    use crate::common::types::SimTime;
    use crate::config::{Config, SweepConfig};
    use crate::data::models::{Bar, MarketData};
    use crate::strategy::examples::RandomStrategy;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;

    // Helper to create dummy market data
    fn setup_market_data() -> Arc<MarketData> {
        let mut md = MarketData::new();
        // Add minimal data (reuse logic from single_run or just basic)
        let bar = Bar {
            timestamp: SimTime::utc(1000),
            open: 1000000,
            high: 1010000,
            low: 990000,
            close: 1005000,
            volume: 100,
        };
        md.add_bar("TEST", bar);
        Arc::new(md)
    }

    #[test]
    fn test_sweep_execution() {
        // Mock Config
        // Config::parse requires args, but we can verify logic using struct setup
        let base_config = Config {
            data_dir: None,
            config_file: None,
            start_date: None,
            end_date: None,
            initial_capital: 100000,
            log_level: "info".to_string(),
            log_time_mode: "simulation".to_string(),
            log_file: None,
            report_path: None,
            benchmark: "NIFTY 50".to_string(),
            params: None,
            strategy: None,
            portfolio: None,
            option_filter: None,
            merged_params: HashMap::new(),
        };

        let mut ranges = HashMap::new();
        // Variate probability
        ranges.insert("prob".to_string(), vec![json!(0.1), json!(0.9)]);

        let sweep_config = SweepConfig {
            base_config,
            param_ranges: ranges,
        };

        let md = setup_market_data();

        // Assuming RandomStrategy is Send
        let results = run_sweep(
            &sweep_config,
            md,
            "sample_data/niftyIndex2024.sample.parquet",
            |cfg| {
                let prob = cfg
                    .merged_params
                    .get("prob")
                    .unwrap()
                    .parse::<f64>()
                    .unwrap_or(0.5);
                (RandomStrategy::new(42, prob, 1), None)
            },
        );

        assert_eq!(results.len(), 2);

        // Check parameters in results
        let p1 = results
            .iter()
            .find(|r| r.params.get("prob") == Some(&"0.1".to_string()))
            .is_some();
        let p2 = results
            .iter()
            .find(|r| r.params.get("prob") == Some(&"0.9".to_string()))
            .is_some();

        assert!(p1);
        assert!(p2);
    }
}
