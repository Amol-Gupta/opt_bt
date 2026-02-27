use criterion::{criterion_group, criterion_main, Criterion};
use opt_bt::data::loader::DataLoader;
use opt_bt::engine::runner::Engine;
use opt_bt::strategy::RandomStrategy;

fn bench_engine_event_loop(c: &mut Criterion) {
    let market_data = DataLoader::load_parquet("sample_data/niftyIndex2024.sample.parquet")
        .expect("failed to load fixture parquet for benchmark");

    c.bench_function("engine_run_random_prob_0", |b| {
        b.iter(|| {
            let strategy = RandomStrategy::new(42, 0.0, 1);
            let mut engine = Engine::new(strategy, market_data.clone(), 1_000_000 * 10_000);
            engine.init();
            engine.run();
        });
    });
}

criterion_group!(benches, bench_engine_event_loop);
criterion_main!(benches);
