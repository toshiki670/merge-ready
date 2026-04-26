#!/usr/bin/env bash
# Forbid mod.rs files outside of allowed paths (use Rust 2018+ style instead).
#
# Allowed exceptions:
#   tests/e2e/  — migration cost is too high; excluded from this check

set -uo pipefail

cd "${ROOT_DIR:-$(dirname "$0")/..}"

hits=$(find . -name "mod.rs" \
  -not -path "./.git/*" \
  -not -path "./target/*" \
  -not -path "./tests/e2e/*" \
  | sort)

if [ -n "$hits" ]; then
  printf '%s\n' "$hits"
  cat >&2 <<'MSG'

ERROR: mod.rs files are forbidden. Use Rust 2018+ style instead.

  How to fix:
    foo/bar/mod.rs  →  rename to foo/bar.rs  (keep the content as-is)

  Exception: tests/e2e/ is allowed.

MSG
  exit 1
fi

echo "No forbidden mod.rs files found. OK"
