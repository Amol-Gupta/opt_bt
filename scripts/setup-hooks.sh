#!/bin/bash

# Setup pre-commit hooks for the repository
# Run this script once after cloning: ./scripts/setup-hooks.sh

set -e

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOOKS_DIR="$REPO_ROOT/.git/hooks"
SCRIPTS_DIR="$REPO_ROOT/scripts"

echo "🔧 Setting up git hooks..."

# Create pre-commit hook if it doesn't exist or update it
if [ -f "$SCRIPTS_DIR/hooks/pre-commit" ]; then
    cp "$SCRIPTS_DIR/hooks/pre-commit" "$HOOKS_DIR/pre-commit"
    chmod +x "$HOOKS_DIR/pre-commit"
    echo "✅ Pre-commit hook installed at $HOOKS_DIR/pre-commit"
else
    echo "⚠️  Hook template not found at $SCRIPTS_DIR/hooks/pre-commit"
    exit 1
fi

echo "🎉 Git hooks are ready! Formatting and linting will now run before commits."
