use std::time::Duration;

use opt_bt::cache::store::CacheStore;

fn fixture_path() -> &'static str {
    "sample_data/niftyIndex2024.sample.parquet"
}

#[test]
fn ensure_loaded_reports_cold_then_warm_hit() {
    let mut store = CacheStore::new(false);

    let first = store
        .ensure_loaded(fixture_path(), None, None)
        .expect("cold ensure should load");
    assert!(!first.cache_hit, "first ensure should be a miss");
    assert_eq!(first.shared_handle.transport, "file_mmap");
    assert_eq!(first.shared_handle.format, "rkyv_market_data_v1");
    assert!(first.shared_handle.byte_len > 0);

    let second = store
        .ensure_loaded(fixture_path(), None, None)
        .expect("warm ensure should hit");
    assert!(second.cache_hit, "second ensure should be a cache hit");
    assert_eq!(
        first.shared_handle.generation,
        second.shared_handle.generation
    );
}

#[test]
fn ensure_loaded_invalidates_on_file_change() {
    let temp = std::env::temp_dir().join("opt_bt_cache_invalidation.parquet");
    std::fs::copy(fixture_path(), &temp).expect("copy fixture");

    let mut store = CacheStore::new(false);
    let cold = store
        .ensure_loaded(temp.to_string_lossy().as_ref(), None, None)
        .expect("cold ensure should load");
    assert!(!cold.cache_hit);

    std::thread::sleep(Duration::from_secs(1));
    std::fs::copy(fixture_path(), &temp).expect("rewrite fixture to bump mtime");

    let reloaded = store
        .ensure_loaded(temp.to_string_lossy().as_ref(), None, None)
        .expect("ensure after file change should reload");
    assert!(
        !reloaded.cache_hit,
        "mtime change should produce a new fingerprint key"
    );
    assert!(reloaded.shared_handle.generation > cold.shared_handle.generation);

    let _ = std::fs::remove_file(temp);
}

#[test]
fn status_includes_entry_summaries() {
    let mut store = CacheStore::new(false);
    let loaded = store
        .ensure_loaded(fixture_path(), None, None)
        .expect("ensure should load fixture");

    let status = store.status();
    assert_eq!(status.entry_count, 1);
    assert_eq!(status.keys.len(), 1);
    assert_eq!(status.entries.len(), 1);

    let summary = &status.entries[0];
    assert_eq!(summary.key, status.keys[0]);
    assert_eq!(
        summary.entry.fingerprint.canonical_path,
        loaded.entry.fingerprint.canonical_path
    );
    assert_eq!(summary.entry.snapshot_path, loaded.entry.snapshot_path);
    assert_eq!(
        summary.shared_handle.generation,
        loaded.shared_handle.generation
    );
}
