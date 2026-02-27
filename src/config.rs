use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use anyhow::{Result, Context}; // Added Context for error message
use itertools::Itertools;

#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
#[command(author, version, about, long_about = None)]
pub struct Config {
    /// Path to data directory containing Parquet files
    #[arg(long, env = "OPT_BT_DATA_DIR")]
    pub data_dir: Option<String>,
    
    /// Path to JSON configuration file
    #[arg(short, long)]
    pub config_file: Option<String>,
    
    /// Start date for backtest (YYYY-MM-DD)
    #[arg(long)]
    pub start_date: Option<String>,
    
    /// End date for backtest (YYYY-MM-DD)
    #[arg(long)]
    pub end_date: Option<String>,

    /// Initial capital in account currency (e.g. INR)
    #[arg(long, default_value_t = 1000000)]
    pub initial_capital: i64,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Logger time mode (simulation, wall)
    #[arg(long, default_value = "simulation")]
    pub log_time_mode: String,

    /// Optional log file path for engine logger output
    #[arg(long)]
    pub log_file: Option<String>,

    /// Optional report output file path (JSON)
    #[arg(long)]
    pub report_path: Option<String>,

    /// Strategy parameters (key=value)
    #[arg(long, value_parser = parse_key_val)]
    pub params: Option<Vec<(String, String)>>,

    /// Strategy kind for single-strategy mode (e.g. random, atm_straddle)
    #[arg(long)]
    pub strategy: Option<String>,

    /// Optional portfolio composition loaded from JSON config.
    #[serde(default)]
    #[arg(skip)]
    pub portfolio: Option<PortfolioConfig>,

    /// Optional option-chain filter criteria for daily instrument subscription.
    #[serde(default)]
    #[arg(skip)]
    pub option_filter: Option<OptionFilterConfig>,
    
    // Internal use: computed or merged parameters
    #[serde(skip)]
    #[arg(skip)]
    pub merged_params: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioConfig {
    pub strategies: Vec<StrategyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub params: HashMap<String, String>,
    pub capital_limit: Option<i64>,
    pub max_exposure_ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionFilterConfig {
    pub underlying: String,
    #[serde(default)]
    pub nearest_expiry_only: bool,
    #[serde(default)]
    pub min_days_to_expiry: Option<i32>,
    #[serde(default)]
    pub max_days_to_expiry: Option<i32>,
    #[serde(default)]
    pub strike_step: Option<i64>,
    #[serde(default)]
    pub strike_band_steps: Option<i32>,
    #[serde(default = "default_include_calls")]
    pub include_calls: bool,
    #[serde(default = "default_include_puts")]
    pub include_puts: bool,
}

fn default_include_calls() -> bool {
    true
}

fn default_include_puts() -> bool {
    true
}

/// Parse a single key-value pair
fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{}`", s))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

impl Config {
    /// Load configuration with priority: CLI > Env (handled by clap) > JSON > Defaults
    pub fn load() -> Result<Self> {
        // 1. Parse CLI args (updates Env vars automatically via clap features)
        let mut cli_config = Config::parse();

        // 2. Load from JSON file if specified
        if let Some(path) = &cli_config.config_file {
            if Path::new(path).exists() {
                let content = fs::read_to_string(path)?;
                let file_config: Config = serde_json::from_str(&content)?;
                
                // Merge JSON into CLI (CLI takes precedence if set, otherwise use JSON)
                // Note: Option fields are easy, primitives need checking against defaults or logical implications
                if cli_config.data_dir.is_none() { cli_config.data_dir = file_config.data_dir; }
                if cli_config.start_date.is_none() { cli_config.start_date = file_config.start_date; }
                if cli_config.end_date.is_none() { cli_config.end_date = file_config.end_date; }
                if cli_config.option_filter.is_none() { cli_config.option_filter = file_config.option_filter; }
                // For params, we might want to merge map
            }
        }
        
        // 3. Merge parameters
        let mut final_params = HashMap::new();
        if let Some(cli_params) = &cli_config.params {
            for (k, v) in cli_params {
                final_params.insert(k.clone(), v.clone());
            }
        }
        cli_config.merged_params = final_params;

        Ok(cli_config)
    }

    /// Merge optional config_file values into this config, keeping explicit CLI values as highest priority.
    pub fn resolve(mut self) -> Result<Self> {
        if let Some(path) = &self.config_file {
            if Path::new(path).exists() {
                let content = fs::read_to_string(path)?;
                let file_config: Config = serde_json::from_str(&content)?;

                if self.data_dir.is_none() {
                    self.data_dir = file_config.data_dir;
                }
                if self.start_date.is_none() {
                    self.start_date = file_config.start_date;
                }
                if self.end_date.is_none() {
                    self.end_date = file_config.end_date;
                }
                if self.strategy.is_none() {
                    self.strategy = file_config.strategy;
                }
                if self.portfolio.is_none() {
                    self.portfolio = file_config.portfolio;
                }
                if self.option_filter.is_none() {
                    self.option_filter = file_config.option_filter;
                }
                if self.params.is_none() {
                    self.params = file_config.params;
                }
                if self.initial_capital == 1_000_000 {
                    self.initial_capital = file_config.initial_capital;
                }
                if self.log_level == "info" {
                    self.log_level = file_config.log_level;
                }
                if self.log_time_mode == "simulation" {
                    self.log_time_mode = file_config.log_time_mode;
                }
                if self.log_file.is_none() {
                    self.log_file = file_config.log_file;
                }
                if self.report_path.is_none() {
                    self.report_path = file_config.report_path;
                }
            }
        }

        let mut final_params = HashMap::new();
        if let Some(params) = &self.params {
            for (key, value) in params {
                final_params.insert(key.clone(), value.clone());
            }
        }
        self.merged_params = final_params;

        Ok(self)
    }
}

// SweepConfig and Permutation logic
#[derive(Debug, Serialize, Deserialize)]
pub struct SweepConfig {
    #[serde(flatten)]
    pub base_config: Config,
    
    // Parameter ranges for sweep: "param_name" -> [val1, val2, val3]
    #[serde(default)] // Allow missing field
    pub param_ranges: HashMap<String, Vec<serde_json::Value>>,
}

impl SweepConfig {
    pub fn new(base_config: Config) -> Self {
        Self {
            base_config,
            param_ranges: HashMap::new(),
        }
    }
    
    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path).context("Failed to read sweep config file")?;
        let config: SweepConfig = serde_json::from_str(&content).context("Failed to parse sweep config JSON")?;
        Ok(config)
    }

    /// Generate all permutations of Config based on param_ranges
    pub fn generate_permutations(&self) -> Vec<Config> {
        if self.param_ranges.is_empty() {
             return vec![self.base_config.clone()];
        }

        // Sort keys to ensure deterministic order of permutations
        let keys: Vec<&String> = self.param_ranges.keys().sorted().collect();
        // Create iterators for each range, matching the sorted keys order
        let values: Vec<&Vec<serde_json::Value>> = keys.iter()
            .map(|k| self.param_ranges.get(*k).unwrap())
            .collect();

        // Multi-cartesian product
        let combinations = values.into_iter().multi_cartesian_product();

        let mut configs = Vec::new();

        for combination in combinations {
             let mut new_config = self.base_config.clone();
             
             // Update base merged_params
             for (i, val) in combination.into_iter().enumerate() {
                 let key = keys[i];
                 let val_str = match val {
                     serde_json::Value::String(s) => s.clone(),
                     serde_json::Value::Number(n) => n.to_string(),
                     serde_json::Value::Bool(b) => b.to_string(),
                     _ => val.to_string(), // For arrays/objects, define if needed. Strategy expects primitives mostly.
                 };
                 new_config.merged_params.insert(key.clone(), val_str);
             }
             
             configs.push(new_config);
        }
        
        configs
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_permute() {
        // Mock Config
        // Config::parse requires args, but we can verify logic using struct setup
        let mut base = Config {
            data_dir: None,
            config_file: None,
            start_date: None,
            end_date: None,
            initial_capital: 100000,
            log_level: "info".to_string(),
            log_time_mode: "simulation".to_string(),
            log_file: None,
            report_path: None,
            params: None,
            strategy: None,
            portfolio: None,
            option_filter: None,
            merged_params: HashMap::new(),
        };
        base.merged_params.insert("p1".to_string(), "base".to_string());
        
        let mut ranges = HashMap::new();
        ranges.insert("p1".to_string(), vec![serde_json::json!(1), serde_json::json!(2)]);
        ranges.insert("p2".to_string(), vec![serde_json::json!("A"), serde_json::json!("B")]);
        
        let sweep = SweepConfig {
            base_config: base,
            param_ranges: ranges,
        };
        
        let permutations = sweep.generate_permutations();
        assert_eq!(permutations.len(), 4); // 2 * 2
        
        // Verify contents
        let p1_vals: Vec<_> = permutations.iter().map(|c| c.merged_params.get("p1").unwrap()).sorted().collect();
        let p2_vals: Vec<_> = permutations.iter().map(|c| c.merged_params.get("p2").unwrap()).sorted().collect();
        
        // p1 values: 1, 1, 2, 2 (as strings)
        assert_eq!(p1_vals, vec!["1", "1", "2", "2"]);
        
        // p2 values: A, A, B, B
        assert_eq!(p2_vals, vec!["A", "A", "B", "B"]);
    }
}
