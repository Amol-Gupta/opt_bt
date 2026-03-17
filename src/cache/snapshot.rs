use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use memmap2::MmapOptions;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::data::archived_view::ArchivedMarketDataView;
use crate::data::models::MarketData;
use crate::data::view::MarketDataView;

pub struct LoadedMarketDataView {
    pub market_data: Arc<dyn MarketDataView>,
    pub backend: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedSnapshotHandle {
    pub transport: String,
    pub location: String,
    pub format: String,
    pub generation: u64,
    pub byte_len: u64,
    pub checksum24: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub loaded_at_unix_secs: u64,
    pub instrument_count: usize,
    pub bar_count: usize,
}

fn snapshot_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("BT_CACHE_SNAPSHOT_DIR") {
        return PathBuf::from(dir);
    }

    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        return PathBuf::from(tmpdir).join("opt_bt_cache_snapshots");
    }

    PathBuf::from("/tmp/opt_bt_cache_snapshots")
}

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
    let mode = std::env::var("BT_CACHE_SNAPSHOT_READ_MODE")
        .unwrap_or_else(|_| "mmap".to_string())
        .to_ascii_lowercase();

    match mode.as_str() {
        "read" => load_market_data_snapshot_read(path),
        "mmap" => {
            load_market_data_snapshot_mmap(path).or_else(|_| load_market_data_snapshot_read(path))
        }
        _ => load_market_data_snapshot_mmap(path).or_else(|_| load_market_data_snapshot_read(path)),
    }
}

pub fn load_market_data_snapshot_view(path: &Path) -> Result<Arc<dyn MarketDataView>> {
    Ok(load_market_data_snapshot_view_with_backend(path)?.market_data)
}

pub fn load_market_data_snapshot_view_with_backend(path: &Path) -> Result<LoadedMarketDataView> {
    let mode = std::env::var("BT_CACHE_VIEW_MODE")
        .unwrap_or_else(|_| "archived".to_string())
        .to_ascii_lowercase();

    match mode.as_str() {
        "archived" => {
            if let Ok(view) = ArchivedMarketDataView::from_path(path) {
                return Ok(LoadedMarketDataView {
                    market_data: Arc::new(view),
                    backend: "archived",
                });
            }
            log::warn!(
                "archived snapshot view unavailable for {}, falling back to owned mode",
                path.display()
            );
        }
        "owned" => {
            let owned = load_market_data_snapshot(path)?;
            return Ok(LoadedMarketDataView {
                market_data: owned,
                backend: "owned",
            });
        }
        other => {
            log::warn!(
                "unknown BT_CACHE_VIEW_MODE='{}'; using archived mode with owned fallback",
                other
            );
            if let Ok(view) = ArchivedMarketDataView::from_path(path) {
                return Ok(LoadedMarketDataView {
                    market_data: Arc::new(view),
                    backend: "archived",
                });
            }
            log::warn!(
                "archived snapshot view unavailable for {}, falling back to owned mode",
                path.display()
            );
        }
    }

    let owned = load_market_data_snapshot(path)?;
    Ok(LoadedMarketDataView {
        market_data: owned,
        backend: "owned-fallback",
    })
}

pub fn build_shared_snapshot_handle(path: &Path, generation: u64) -> Result<SharedSnapshotHandle> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("failed to stat snapshot file {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(metadata.len().to_le_bytes());
    hasher.update(generation.to_le_bytes());
    let digest = hasher.finalize();
    let checksum24 = ((digest[0] as u32) << 16) | ((digest[1] as u32) << 8) | digest[2] as u32;

    Ok(SharedSnapshotHandle {
        transport: "file_mmap".to_string(),
        location: path.to_string_lossy().to_string(),
        format: "rkyv_market_data_v1".to_string(),
        generation,
        byte_len: metadata.len(),
        checksum24,
    })
}

pub fn validate_shared_snapshot_handle(path: &Path, handle: &SharedSnapshotHandle) -> Result<()> {
    if handle.transport != "file_mmap" {
        anyhow::bail!(
            "unsupported shared snapshot transport: {}",
            handle.transport
        );
    }

    if Path::new(&handle.location) != path {
        anyhow::bail!(
            "snapshot location mismatch: expected {} got {}",
            path.display(),
            handle.location
        );
    }

    let metadata = std::fs::metadata(path)
        .with_context(|| format!("failed to stat snapshot file {}", path.display()))?;
    if metadata.len() != handle.byte_len {
        anyhow::bail!(
            "snapshot byte length mismatch: expected {} got {}",
            handle.byte_len,
            metadata.len()
        );
    }

    let validate_checksum = std::env::var("BT_CACHE_VALIDATE_CHECKSUM")
        .ok()
        .map(|raw| raw.eq_ignore_ascii_case("1") || raw.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if validate_checksum {
        let bytes = std::fs::read(path)
            .with_context(|| format!("failed to open snapshot file {}", path.display()))?;
        let digest = Sha256::digest(&bytes);
        let checksum24 = ((digest[0] as u32) << 16) | ((digest[1] as u32) << 8) | digest[2] as u32;
        if checksum24 != handle.checksum24 {
            anyhow::bail!(
                "snapshot checksum mismatch: expected {} got {}",
                handle.checksum24,
                checksum24
            );
        }
    }

    Ok(())
}

fn load_market_data_snapshot_read(path: &Path) -> Result<Arc<MarketData>> {
    let data = std::fs::read(path)
        .with_context(|| format!("failed to open snapshot file {}", path.display()))?;
    deserialize_market_data(&data, path)
}

fn load_market_data_snapshot_mmap(path: &Path) -> Result<Arc<MarketData>> {
    let file = File::open(path)
        .with_context(|| format!("failed to open snapshot file {}", path.display()))?;
    let mmap = unsafe { MmapOptions::new().map(&file) }
        .with_context(|| format!("failed to mmap snapshot file {}", path.display()))?;
    deserialize_market_data(&mmap, path)
}

fn deserialize_market_data(bytes: &[u8], path: &Path) -> Result<Arc<MarketData>> {
    let market_data = rkyv::from_bytes::<MarketData, rkyv::rancor::Error>(bytes)
        .with_context(|| format!("failed to deserialize snapshot {}", path.display()))?;
    Ok(Arc::new(market_data))
}

pub fn snapshot_path_for_key(cache_key: &str) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(cache_key.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    snapshot_dir().join(format!("{hash}.rkyv"))
}

fn snapshot_metadata_path(snapshot_path: &Path) -> PathBuf {
    let mut value = snapshot_path.as_os_str().to_os_string();
    value.push(".meta.json");
    PathBuf::from(value)
}

pub fn write_snapshot_metadata(snapshot_path: &Path, metadata: &SnapshotMetadata) -> Result<()> {
    let metadata_path = snapshot_metadata_path(snapshot_path);
    if let Some(parent) = metadata_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_vec_pretty(metadata)?;
    std::fs::write(&metadata_path, payload).with_context(|| {
        format!(
            "failed to write snapshot metadata {}",
            metadata_path.display()
        )
    })?;
    Ok(())
}

pub fn read_snapshot_metadata(snapshot_path: &Path) -> Result<Option<SnapshotMetadata>> {
    let metadata_path = snapshot_metadata_path(snapshot_path);
    if !metadata_path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read(&metadata_path).with_context(|| {
        format!(
            "failed to read snapshot metadata {}",
            metadata_path.display()
        )
    })?;
    let parsed = serde_json::from_slice::<SnapshotMetadata>(&raw).with_context(|| {
        format!(
            "failed to parse snapshot metadata {}",
            metadata_path.display()
        )
    })?;
    Ok(Some(parsed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_snapshot_path() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "opt_bt_snapshot_test_{}_{}.rkyv",
            std::process::id(),
            nanos
        ))
    }

    fn write_fixture_snapshot(path: &Path) {
        let market_data = MarketData::new();
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&market_data).unwrap();
        std::fs::write(path, &bytes).unwrap();
    }

    #[test]
    fn loads_snapshot_from_read_mode() {
        let path = fixture_snapshot_path();
        write_fixture_snapshot(&path);

        let loaded = load_market_data_snapshot_read(&path).unwrap();
        assert_eq!(loaded.bars.len(), 0);
        assert_eq!(loaded.bars_by_time.len(), 0);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn loads_snapshot_from_mmap_mode() {
        let path = fixture_snapshot_path();
        write_fixture_snapshot(&path);

        let loaded = load_market_data_snapshot_mmap(&path).unwrap();
        assert_eq!(loaded.bars.len(), 0);
        assert_eq!(loaded.bars_by_time.len(), 0);

        let _ = std::fs::remove_file(&path);
    }
}
