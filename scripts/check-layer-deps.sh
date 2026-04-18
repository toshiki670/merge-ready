#!/usr/bin/env bash
# Verify DDD layer dependency rules.
#
# Allowed dependency matrix (✅ = allowed, ❌ = forbidden):
#
#   from \ to      domain  application  infrastructure  interface
#   domain           –        ❌            ❌             ❌
#   application      ✅        –            ❌             ❌
#   infrastructure   ✅       ❌            –              ❌
#   interface        ❌       ✅            ❌             –
#   bin              ❌       ✅            ✅             ✅

set -uo pipefail

cd "$(dirname "$0")/.."

FAIL=0

# Check that none of the target paths contain lines matching pattern.
# Prints matching lines and sets FAIL=1 if any match is found.
check_forbidden() {
  local from_label="$1"
  local to_label="$2"
  local pattern="$3"
  shift 3

  local existing=()
  for p in "$@"; do
    [ -e "$p" ] && existing+=("$p")
  done
  [ "${#existing[@]}" -eq 0 ] && return 0

  local hits
  if hits=$(grep -rEn "$pattern" "${existing[@]}" 2>/dev/null) && [ -n "$hits" ]; then
    printf '%s\n' "$hits"
    printf 'ERROR: [%s] must not import [%s]\n\n' "$from_label" "$to_label" >&2
    FAIL=1
  fi
  return 0
}

MR="src/contexts/merge_readiness"
CF="src/contexts/configuration"

# ── merge_readiness::domain ───────────────────────────────────────────────────
check_forbidden "merge_readiness::domain" "::application" \
  "use (crate::)?contexts::merge_readiness::application" \
  "$MR/domain.rs" "$MR/domain"

check_forbidden "merge_readiness::domain" "::infrastructure" \
  "use (crate::)?contexts::merge_readiness::infrastructure" \
  "$MR/domain.rs" "$MR/domain"

check_forbidden "merge_readiness::domain" "::interface" \
  "use (crate::)?contexts::merge_readiness::interface" \
  "$MR/domain.rs" "$MR/domain"

# ── merge_readiness::application ─────────────────────────────────────────────
check_forbidden "merge_readiness::application" "::infrastructure" \
  "use (crate::)?contexts::merge_readiness::infrastructure" \
  "$MR/application.rs" "$MR/application"

check_forbidden "merge_readiness::application" "::interface" \
  "use (crate::)?contexts::merge_readiness::interface" \
  "$MR/application.rs" "$MR/application"

# ── merge_readiness::infrastructure ──────────────────────────────────────────
check_forbidden "merge_readiness::infrastructure" "::application" \
  "use (crate::)?contexts::merge_readiness::application" \
  "$MR/infrastructure.rs" "$MR/infrastructure"

check_forbidden "merge_readiness::infrastructure" "::interface" \
  "use (crate::)?contexts::merge_readiness::interface" \
  "$MR/infrastructure.rs" "$MR/infrastructure"

# ── merge_readiness::interface ────────────────────────────────────────────────
check_forbidden "merge_readiness::interface" "::domain" \
  "use (crate::)?contexts::merge_readiness::domain" \
  "$MR/interface.rs" "$MR/interface"

check_forbidden "merge_readiness::interface" "::infrastructure" \
  "use (crate::)?contexts::merge_readiness::infrastructure" \
  "$MR/interface.rs" "$MR/interface"

# ── configuration::domain ─────────────────────────────────────────────────────
check_forbidden "configuration::domain" "::infrastructure" \
  "use (crate::)?contexts::configuration::infrastructure" \
  "$CF/domain.rs" "$CF/domain"

# ── bin (main.rs / cached.rs / refresh.rs) ───────────────────────────────────
# bin may only import application / infrastructure / interface — not domain directly
check_forbidden "bin" "any ::domain" \
  "use (crate::)?contexts::[a-z_]+::domain" \
  "src/main.rs" "src/cached.rs" "src/refresh.rs"

# ── Result ────────────────────────────────────────────────────────────────────
if [ "$FAIL" -eq 1 ]; then
  echo "Layer dependency rule violations found." >&2
  exit 1
fi

echo "All layer dependency rules OK"
