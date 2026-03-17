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
