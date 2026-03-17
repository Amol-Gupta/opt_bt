# opt_bt

High-performance event-driven options backtesting engine in Rust.

For detailed documentation, see:
- **[User Guide](docs/USER_GUIDE.md)** – Overview, features, and installation
- **[Contributing Guide](docs/CONTRIBUTING.md)** – Local setup, development workflow, and guidelines

## Quick Start

### Install from GitHub

```bash
export OPT_BT_RELEASE_REPO="https://github.com/Amol-Gupta/opt_bt"
export OPT_BT_RELEASE_REV="<tag-or-commit>"

cargo install \
  --git https://github.com/Amol-Gupta/opt_bt \
  --rev "$OPT_BT_RELEASE_REV" \
  --bin bt \
  --bin opt_bt \
  --force
```

### Develop Locally

```bash
# Clone and setup hooks
git clone https://github.com/Amol-Gupta/opt_bt.git
cd opt_bt
./scripts/setup-hooks.sh

# Build and test
cargo build --release
cargo test
```

See [Contributing Guide](docs/CONTRIBUTING.md) for full setup instructions.
