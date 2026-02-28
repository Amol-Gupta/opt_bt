use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatasetFingerprint {
    pub canonical_path: String,
    pub size_bytes: u64,
    pub modified_unix_secs: u64,
    pub sha256: Option<String>,
}

impl DatasetFingerprint {
    pub fn key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.canonical_path, self.size_bytes, self.modified_unix_secs
        )
    }
}

pub fn build_dataset_fingerprint(path: &Path, include_sha256: bool) -> Result<DatasetFingerprint> {
    let canonical = canonical_or_original(path)?;
    let metadata = std::fs::metadata(&canonical)
        .with_context(|| format!("failed to read metadata for {}", canonical.display()))?;
    let modified = metadata
        .modified()
        .with_context(|| format!("failed to read mtime for {}", canonical.display()))?;
    let modified_unix_secs = modified
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let sha256 = if include_sha256 {
        Some(compute_file_sha256(&canonical)?)
    } else {
        None
    };

    Ok(DatasetFingerprint {
        canonical_path: canonical.to_string_lossy().to_string(),
        size_bytes: metadata.len(),
        modified_unix_secs,
        sha256,
    })
}

fn canonical_or_original(path: &Path) -> Result<PathBuf> {
    match std::fs::canonicalize(path) {
        Ok(canonical) => Ok(canonical),
        Err(_) => {
            if path.exists() {
                Ok(path.to_path_buf())
            } else {
                Err(anyhow::anyhow!("dataset path does not exist: {}", path.display()))
            }
        }
    }
}

fn compute_file_sha256(path: &Path) -> Result<String> {
    let mut file = File::open(path)
        .with_context(|| format!("failed to open {} for hashing", path.display()))?;
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

    #[test]
    fn builds_fingerprint_without_hash() {
        let tmp = std::env::temp_dir().join("opt_bt_fp_no_hash.txt");
        std::fs::write(&tmp, b"hello").expect("write temp");

        let fp = build_dataset_fingerprint(&tmp, false).expect("fingerprint");
        assert!(fp.size_bytes > 0);
        assert!(fp.sha256.is_none());

        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn builds_fingerprint_with_hash() {
        let tmp = std::env::temp_dir().join("opt_bt_fp_hash.txt");
        std::fs::write(&tmp, b"abc").expect("write temp");

        let fp = build_dataset_fingerprint(&tmp, true).expect("fingerprint");
        assert_eq!(
            fp.sha256.expect("hash"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );

        let _ = std::fs::remove_file(tmp);
    }
}
