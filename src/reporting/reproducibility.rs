use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::Result;
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::reporting::json::{DatasetMetadata, Reproducibility};

pub fn build_reproducibility(config: &Config, data_path: &str) -> Result<Reproducibility> {
    let dataset_sha = if include_dataset_sha(config) {
        Some(compute_file_sha256(data_path)?)
    } else {
        None
    };

    let mut config_map = HashMap::new();
    config_map.insert(
        "data_dir".to_string(),
        config
            .data_dir
            .clone()
            .unwrap_or_else(|| data_path.to_string()),
    );
    config_map.insert(
        "start_date".to_string(),
        config
            .start_date
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
    );
    config_map.insert(
        "end_date".to_string(),
        config
            .end_date
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
    );
    config_map.insert(
        "initial_capital".to_string(),
        config.initial_capital.to_string(),
    );
    config_map.insert("log_level".to_string(), config.log_level.clone());

    let strategy_name = if config.portfolio.is_some() {
        "portfolio".to_string()
    } else {
        config
            .strategy
            .clone()
            .unwrap_or_else(|| "unknown".to_string())
    };

    let strategy_parameters = if let Some(portfolio_cfg) = &config.portfolio {
        portfolio_cfg
            .strategies
            .iter()
            .map(|strategy| (strategy.id.clone(), strategy.params.clone()))
            .collect::<HashMap<String, HashMap<String, String>>>()
    } else {
        let mut map = HashMap::new();
        map.insert(strategy_name.clone(), config.merged_params.clone());
        map
    };

    let strategy_version = std::env::var("OPT_BT_STRATEGY_VERSION")
        .or_else(|_| std::env::var("GIT_COMMIT"))
        .unwrap_or_else(|_| "unknown".to_string());

    Ok(Reproducibility {
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        strategy_version,
        strategy_name,
        parameters: config.merged_params.clone(),
        strategy_parameters,
        config: config_map,
        dataset: DatasetMetadata {
            source: data_path.to_string(),
            sha256: dataset_sha,
            granularity: "1m".to_string(),
            start_date: config
                .start_date
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            end_date: config
                .end_date
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
        },
    })
}

fn include_dataset_sha(config: &Config) -> bool {
    if let Ok(value) = std::env::var("OPT_BT_REPORT_SHA256") {
        if parse_bool_flag(&value) {
            return true;
        }
        if !value.trim().is_empty() {
            return false;
        }
    }

    if let Some(value) = config.merged_params.get("dataset_sha256") {
        return parse_bool_flag(value);
    }

    if let Some(value) = config.merged_params.get("include_dataset_sha") {
        return parse_bool_flag(value);
    }

    false
}

fn parse_bool_flag(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "on"
    )
}

fn compute_file_sha256(path: &str) -> Result<String> {
    let mut file = File::open(Path::new(path))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_compute_file_sha256_known_value() {
        let temp_path = std::env::temp_dir().join("opt_bt_repro_sha_test.txt");
        fs::write(&temp_path, b"abc").expect("failed to write temp file");

        let hash = compute_file_sha256(temp_path.to_string_lossy().as_ref())
            .expect("failed to compute hash");
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );

        let _ = fs::remove_file(temp_path);
    }
}
