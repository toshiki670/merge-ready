#!/usr/bin/env bash
set -euo pipefail

git config --local hook.rust-fmt.event pre-commit
git config --local hook.rust-fmt.command "cargo fmt --all -- --check"

git config --local hook.rust-clippy.event pre-push
git config --local hook.rust-clippy.command "cargo clippy --workspace --all-targets --all-features -- -D warnings"

echo "Configured local hooks:"
git config --local --get-regexp '^hook\.'
