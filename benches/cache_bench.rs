use criterion::{criterion_group, criterion_main, Criterion};
use opt_bt::cache::store::CacheStore;

fn bench_cache_ensure_loaded(c: &mut Criterion) {
    let path = "sample_data/niftyIndex2024.sample.parquet";

    c.bench_function("cache_store_cold_load", |b| {
        b.iter(|| {
            let mut store = CacheStore::new(false);
            let _ = store
                .ensure_loaded(path)
                .expect("cold load should succeed");
        });
    });

    c.bench_function("cache_store_warm_hit", |b| {
        let mut store = CacheStore::new(false);
        let _ = store
            .ensure_loaded(path)
            .expect("setup cold load should succeed");

        b.iter(|| {
            let _ = store
                .ensure_loaded(path)
                .expect("warm hit should succeed");
        });
    });
}

criterion_group!(benches, bench_cache_ensure_loaded);
criterion_main!(benches);
