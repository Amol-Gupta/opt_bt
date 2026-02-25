# QuickStart: Rust Options Backtest Engine

## Requirements
- Rust stable (`cargo --version`)
- Historical data in **Parquet** format (`.parquet`).

## Setup

1.  **Clone & Build**:
    ```bash
    git clone https://github.com/your-org/opt_bt.git
    cd opt_bt
    cargo build --release
    ```

2.  **Prepare Data**:
    - Ensure your data is in Parquet format.
    - Schema: `[timestamp: i64, symbol_id: u32, price: i64, size: u32]`
    - Example tool: `tools/convert_csv_parquet` provided in repo.

3.  **Implement a Strategy**:
    Create a new strategy struct implementing the `Strategy` trait:
    ```rust
    // src/strategies/sma_cross.rs
    use crate::strategy::{Strategy, Context};
    use crate::data::Bar;

    pub struct SmaCross {
        period: usize,
    }

    impl Strategy for SmaCross {
        fn on_data(&mut self, ctx: &mut Context, bar: &Bar) {
            // Your logic here
            if bar.close > self.sma(ctx) {
                ctx.buy(bar.instrument_id, 1);
            }
        }
    }
    ```

4.  **Run Backtest**:
    Update `src/main.rs` to use your strategy and run:
    ```rust
    fn main() {
        let engine = Engine::new(Config::default());
        let strategy = SmaCross { period: 20 };
        engine.run(strategy);
    }
    ```
    ```bash
    cargo run --release
    ```

## CLI Commands

The engine provides several commands for data inspection and execution:

### 1. Inspect Data
Loads the Parquet file into RAM and prints metadata (instruments, date range) to verify data integrity and measuring loading speed.
```bash
cargo run --release -- inspect --data ./data/nifty_options.parquet
# Output:
# Loaded 1,500,000 bars in 350ms.
# Instruments: 154 (NIFTY*, BANKNIFTY*)
# Range: 2021-01-01 to 2023-12-31
# Memory Usage: ~450MB
```

### 2. Run Single Backtest
Runs a specific strategy with defined parameters.
```bash
cargo run --release -- run --strategy sma-cross --params "period=20" --data ./data.parquet
```

### 3. Parameter Sweep (Optimization)
Runs a parameter sweep using all CPU cores.
```bash
# Example: Run sweep across SMA periods 10-50 in steps of 5
cargo run --release -- sweep --strategy sma-cross --start 10 --end 50 --step 5
```

The CLI (`clap`) handles argument parsing, and `lib.rs` executes the logic.

