#!/usr/bin/env bash
# Forbid masking Clippy warnings via #[allow(clippy::...)] in Rust files.

set -euo pipefail

cd "${ROOT_DIR:-$(dirname "$0")/..}"

hits="$(rg --line-number --glob '*.rs' '#\[allow\(clippy::[^)]*\)\]' . \
  --glob '!target/**' \
  --glob '!.git/**' || true)"

if [ -n "$hits" ]; then
  printf '%s\n' "$hits"
  cat >&2 <<'MSG'

ERROR: #[allow(clippy::...)] is forbidden in Rust files (*.rs).

Fix guidance:
  - Remove the allow attribute.
  - Refactor code so Clippy warnings are resolved without suppression.

MSG
  exit 1
fi

echo "No #[allow(clippy::...)] in Rust files. OK"
