use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::data::models::MarketData;

const SNAPSHOT_DIR: &str = "/tmp/opt_bt_cache_snapshots";

pub fn write_market_data_snapshot(cache_key: &str, market_data: &MarketData) -> Result<PathBuf> {
    let snapshot_path = snapshot_path_for_key(cache_key);
    if let Some(parent) = snapshot_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(&snapshot_path)
        .with_context(|| format!("failed to create snapshot file {}", snapshot_path.display()))?;
    let mut writer = file;
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(market_data)
        .with_context(|| format!("failed to archive snapshot {}", snapshot_path.display()))?;
    writer
        .write_all(&bytes)
        .with_context(|| format!("failed to write snapshot {}", snapshot_path.display()))?;

    Ok(snapshot_path)
}

pub fn load_market_data_snapshot(path: &Path) -> Result<Arc<MarketData>> {
    let data = std::fs::read(path)
        .with_context(|| format!("failed to open snapshot file {}", path.display()))?;
    let market_data = rkyv::from_bytes::<MarketData, rkyv::rancor::Error>(&data)
        .with_context(|| format!("failed to deserialize snapshot {}", path.display()))?;
    Ok(Arc::new(market_data))
}

pub fn snapshot_path_for_key(cache_key: &str) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(cache_key.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    Path::new(SNAPSHOT_DIR).join(format!("{hash}.rkyv"))
}
