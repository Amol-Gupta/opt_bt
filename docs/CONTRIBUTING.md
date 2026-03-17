# Contributing to opt_bt

Welcome! Here's how to set up your local development environment.

## Prerequisites

- Rust (install via [rustup](https://rustup.rs/))
- Git

## Local Setup

### 1. Clone the Repository

```bash
git clone https://github.com/Amol-Gupta/opt_bt.git
cd opt_bt
```

### 2. Install Pre-commit Hooks (Recommended)

Pre-commit hooks enforce formatting and linting standards locally before you push to GitHub:

```bash
./scripts/setup-hooks.sh
```

This installs a pre-commit hook that runs:
- `cargo fmt --check` (formatting check)
- `cargo clippy -- -D warnings` (linting)

If a commit fails the checks:
- **Formatting**: Run `cargo fmt` to auto-fix, then retry the commit.
- **Clippy warnings**: Fix the warnings or use `git commit --no-verify` to skip (not recommended).

### 3. Build and Test

```bash
cargo build
cargo test
```

## Development Mode & Binary Deployment

When developing locally, you have several deployment options:

### Source/Development Mode (from this repository)

Build the binaries:
```bash
cargo build --release
cargo build --release --bin bt
```

Run directly from source:
```bash
cargo run --bin bt -- workspace init --path ./demo_ws
cargo run --bin bt -- run --workspace ./demo_ws --project my_strategy --strategy demo
```

### Installed Mode (from ~/.cargo/bin)

Install the binaries globally:
```bash
cargo install --path . --bin bt --bin opt_bt --force
```

Run from any directory:
```bash
bt workspace init --path ~/my_workspace
bt run --workspace ~/my_workspace --project my_strategy --strategy demo
```

### Key Differences

- **Source mode** generates projects with `path = ...` dependencies pointing to this checkout
- **Installed mode** generates projects with pinned `git = ...` dependencies from GitHub
- Both modes can coexist; be explicit about which binary you're invoking:
  ```bash
  /home/amol/opt_bt/target/release/bt --help      # source/dev mode
  ~/.cargo/bin/bt --help                          # installed mode
  ```

### Example: Test Both Modes

```bash
# source/dev mode from this checkout
cd /home/amol/opt_bt
cargo run --bin bt -- workspace init --path ./demo_ws_dev

# installed mode in a fresh location
mkdir -p /tmp/opt_bt_install_test && cd /tmp/opt_bt_install_test
bt workspace init --path ./ws
bt project init --workspace ./ws my_strategy
```

## Broad Architecture

At a high level, `opt_bt` is an event-driven pipeline:

1. **Data** loads market bars/ticks from parquet.
2. **Engine** replays timeline events in simulation order.
3. **Strategy** emits signals/orders.
4. **Execution** applies fills/slippage/stale checks.
5. **Portfolio/Account** updates positions, cash, and attribution.
6. **Reporting** builds JSON/HTML outputs with reproducibility metadata.

### Module Organization

Core modules in `src/` are organized by responsibility:

- `src/config.rs` – config models and resolution
- `src/data/` – dataset loading, models, and views
- `src/engine/` – run loop and sweep orchestration
- `src/execution/` – fill/slippage/stale execution policies
- `src/portfolio/` – account state, routing, and strategy attribution
- `src/strategy/` – strategy interfaces and portfolio wrappers
- `src/reporting/` – metrics, reproducibility, JSON/HTML report generation
- `src/bin/bt.rs` – workspace/project CLI orchestration
- `src/main.rs` – `opt_bt` executable entrypoint

## Integration Testing

When adding new strategies or features to the engine directly, use integration tests under `tests/`:

### Test structure
- Create synthetic `MarketData` fixtures
- Instantiate `Engine` and run: `engine.init(); engine.run();`
- Assert trades/positions and lifecycle behavior

### Reference examples
- [`tests/single_run.rs`](../tests/single_run.rs) – basic engine lifecycle
- [`tests/atm_straddle.rs`](../tests/atm_straddle.rs) – strategy-specific validation
- [`tests/portfolio_test.rs`](../tests/portfolio_test.rs) – portfolio composition

## Runtime Usage Docs

The following runtime topics are intentionally documented in [USER_GUIDE.md](USER_GUIDE.md):

- binaries and command surface (`bt`, `opt_bt`, project runners)
- cache server behavior and warm/cold run flow (including `rkyv` snapshot format)
- `bt data` quick lookup workflows for post-run analysis
- workspace structure (`.bt/`, `projects/`, `backtests/`)
- `bt.toml` and `generated/strategy_registry.rs` runtime details
- strategy compile/link/discovery execution path in [CompileFlow.md](CompileFlow.md)
- portfolio usage examples

Use this Contributing guide for developer workflow and repository architecture.

## Development Workflow

1. Create a feature branch: `git checkout -b feature/your-feature`
2. Make changes and test locally
3. Commit (hooks will run automatically): `git commit -m "your message"`
4. Push: `git push origin feature/your-feature`
5. Open a pull request

## CI Checks

All pushes to `001-options-backtest-engine` and PRs to `main` run:
- Build: `cargo build --verbose`
- Tests: `cargo test --verbose`
- Linting: `cargo clippy -- -D warnings`
- Formatting: `cargo fmt -- --check`

## Tips

- Run `cargo fmt` to auto-format code
- Run `cargo clippy --fix` for auto-fixable clippy suggestions
- Run `cargo test` frequently during development
