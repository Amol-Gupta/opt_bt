# QuickStart: Rust Options Backtest Engine

This quickstart has been merged into the root README to keep a single source of truth.

Use:
- [README](../../../README.md) for build/run commands, JSON config examples, lifecycle notes, and strategy tutorial.
- [README stage-1 bt workflow](../../../README.md#stage-1-bt-cli-workflow-alpha) for workspace/project CLI flow.

Minimal run command:
```bash
cargo run --release -- run \
  --data-dir ./sample_data/niftyIndex2024.sample.parquet \
  --strategy sma_nifty50 \
  --benchmark "NIFTY 50" \
  --params index_symbol=NIFTY\ 50 \
  --params qty=1 \
  --params short_period=20 \
  --params long_period=50
```

