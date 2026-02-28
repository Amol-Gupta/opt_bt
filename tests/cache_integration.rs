use std::time::Duration;

use opt_bt::cache::store::CacheStore;

fn fixture_path() -> &'static str {
    "sample_data/niftyIndex2024.sample.parquet"
}

#[test]
fn ensure_loaded_reports_cold_then_warm_hit() {
    let mut store = CacheStore::new(false);

    let first = store
        .ensure_loaded(fixture_path())
        .expect("cold ensure should load");
    assert!(!first.cache_hit, "first ensure should be a miss");

    let second = store
        .ensure_loaded(fixture_path())
        .expect("warm ensure should hit");
    assert!(second.cache_hit, "second ensure should be a cache hit");
}

#[test]
fn ensure_loaded_invalidates_on_file_change() {
    let temp = std::env::temp_dir().join("opt_bt_cache_invalidation.parquet");
    std::fs::copy(fixture_path(), &temp).expect("copy fixture");

    let mut store = CacheStore::new(false);
    let cold = store
        .ensure_loaded(temp.to_string_lossy().as_ref())
        .expect("cold ensure should load");
    assert!(!cold.cache_hit);

    std::thread::sleep(Duration::from_secs(1));
    std::fs::copy(fixture_path(), &temp).expect("rewrite fixture to bump mtime");

    let reloaded = store
        .ensure_loaded(temp.to_string_lossy().as_ref())
        .expect("ensure after file change should reload");
    assert!(
        !reloaded.cache_hit,
        "mtime change should produce a new fingerprint key"
    );

    let _ = std::fs::remove_file(temp);
}
