#!/bin/bash

# Setup pre-commit hooks for the repository
# Run this script once after cloning: ./scripts/setup-hooks.sh

set -e

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOOKS_DIR="$REPO_ROOT/.git/hooks"
VERSIONED_HOOK="$REPO_ROOT/.githooks/pre-commit"

echo "Setting up git hooks..."

# Create pre-commit hook if it doesn't exist or update it
if [ -f "$VERSIONED_HOOK" ]; then
    cp "$VERSIONED_HOOK" "$HOOKS_DIR/pre-commit"
    chmod +x "$HOOKS_DIR/pre-commit"
    git config core.hooksPath .githooks
    echo "Pre-commit hook installed at $HOOKS_DIR/pre-commit"
    echo "Configured git hooks path: .githooks"
else
    echo "Hook template not found at $VERSIONED_HOOK"
    exit 1
fi

echo "Git hooks are ready! Formatting, linting, and tests will now run before commits."
