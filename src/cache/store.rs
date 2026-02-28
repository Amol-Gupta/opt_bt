use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::cache::snapshot::write_market_data_snapshot;
use crate::data::fingerprint::{build_dataset_fingerprint, DatasetFingerprint};
use crate::data::loader::DataLoader;
use crate::data::models::MarketData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub fingerprint: DatasetFingerprint,
    pub start_ts: Option<i64>,
    pub end_ts: Option<i64>,
    pub loaded_at_unix_secs: u64,
    pub instrument_count: usize,
    pub bar_count: usize,
    pub snapshot_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsureLoadedResult {
    pub entry: CacheEntry,
    pub cache_hit: bool,
    pub load_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStatus {
    pub entry_count: usize,
    pub keys: Vec<String>,
}

#[derive(Debug)]
struct CachedDataset {
    entry: CacheEntry,
    #[allow(dead_code)]
    market_data: Arc<MarketData>,
}

#[derive(Debug, Default)]
pub struct CacheStore {
    include_sha256: bool,
    entries: HashMap<String, CachedDataset>,
}

impl CacheStore {
    pub fn new(include_sha256: bool) -> Self {
        Self {
            include_sha256,
            entries: HashMap::new(),
        }
    }

    pub fn ensure_loaded(
        &mut self,
        path: &str,
        start_ts: Option<i64>,
        end_ts: Option<i64>,
    ) -> Result<EnsureLoadedResult> {
        let fingerprint = build_dataset_fingerprint(std::path::Path::new(path), self.include_sha256)?;
        let key = if let (Some(start), Some(end)) = (start_ts, end_ts) {
            format!("{}:{}:{}", fingerprint.key(), start, end)
        } else {
            fingerprint.key()
        };

        if let Some(existing) = self.entries.get(&key) {
            return Ok(EnsureLoadedResult {
                entry: existing.entry.clone(),
                cache_hit: true,
                load_ms: 0,
            });
        }

        let started = Instant::now();
        let market_data = if let (Some(start), Some(end)) = (start_ts, end_ts) {
            DataLoader::load_parquet_range(path, start, end)?
        } else {
            DataLoader::load_parquet(path)?
        };
        let load_ms = started.elapsed().as_millis();

        let bar_count = market_data
            .bars
            .values()
            .map(|bars| bars.len())
            .sum::<usize>();

        let entry = CacheEntry {
            fingerprint,
            start_ts,
            end_ts,
            loaded_at_unix_secs: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            instrument_count: market_data.instruments.len(),
            bar_count,
            snapshot_path: write_market_data_snapshot(&key, market_data.as_ref())?
                .to_string_lossy()
                .to_string(),
        };

        self.entries.insert(
            key,
            CachedDataset {
                entry: entry.clone(),
                market_data,
            },
        );

        Ok(EnsureLoadedResult {
            entry,
            cache_hit: false,
            load_ms,
        })
    }

    pub fn evict(&mut self, path: &str) -> Result<bool> {
        let target_path = std::fs::canonicalize(path)
            .unwrap_or_else(|_| std::path::PathBuf::from(path))
            .to_string_lossy()
            .to_string();

        let mut removed_any = false;
        self.entries.retain(|_, value| {
            let keep = value.entry.fingerprint.canonical_path != target_path;
            if !keep {
                removed_any = true;
            }
            keep
        });

        Ok(removed_any)
    }

    pub fn status(&self) -> CacheStatus {
        let mut keys: Vec<String> = self.entries.keys().cloned().collect();
        keys.sort();

        CacheStatus {
            entry_count: keys.len(),
            keys,
        }
    }
}
