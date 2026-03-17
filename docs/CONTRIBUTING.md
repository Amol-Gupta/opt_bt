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
