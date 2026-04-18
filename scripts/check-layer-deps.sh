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
#
# Cross-context rule:
#   Files under src/contexts/A/ must not reference contexts::B:: (bin is exempt)

set -uo pipefail

cd "$(dirname "$0")/.."

FAIL=0

# Forbidden layer-to-layer dependencies: "from_layer:to_layer"
FORBIDDEN_LAYERS=(
  "domain:application"
  "domain:infrastructure"
  "domain:interface"
  "application:infrastructure"
  "application:interface"
  "infrastructure:application"
  "infrastructure:interface"
  "interface:domain"
  "interface:infrastructure"
)

# src/contexts/<ctx>/<layer>.rs or src/contexts/<ctx>/<layer>/... -> <layer>
get_layer() {
  printf '%s' "$1" | sed -E 's|src/contexts/[^/]+/([^/.]+).*|\1|'
}

# src/contexts/<ctx>/... -> <ctx>
get_context() {
  printf '%s' "$1" | sed -E 's|src/contexts/([^/]+)/.*|\1|'
}

# ── Layer dependency rules ────────────────────────────────────────────────────
while IFS= read -r file; do
  layer=$(get_layer "$file")

  for rule in "${FORBIDDEN_LAYERS[@]}"; do
    from_layer="${rule%%:*}"
    to_layer="${rule##*:}"
    [ "$layer" = "$from_layer" ] || continue

    hits=$(grep -En "use (crate::)?contexts::[a-z_]+::${to_layer}" "$file" 2>/dev/null) || true
    if [ -n "$hits" ]; then
      printf '%s\n' "$hits"
      printf 'ERROR: [%s] %s must not import [%s]\n\n' "$from_layer" "$file" "$to_layer" >&2
      FAIL=1
    fi
  done
done < <(find src/contexts -name "*.rs" | sort)

# ── Bin: must not import domain directly ─────────────────────────────────────
for file in src/main.rs src/cached.rs src/refresh.rs; do
  [ -f "$file" ] || continue

  hits=$(grep -En "use (crate::)?contexts::[a-z_]+::domain" "$file" 2>/dev/null) || true
  if [ -n "$hits" ]; then
    printf '%s\n' "$hits"
    printf 'ERROR: [bin] %s must not import domain directly\n\n' "$file" >&2
    FAIL=1
  fi
done

# ── Cross-context dependency rules ────────────────────────────────────────────
while IFS= read -r file; do
  ctx=$(get_context "$file")

  hits=$(grep -En "use (crate::)?contexts::[a-z_]+" "$file" 2>/dev/null \
    | grep -v "contexts::${ctx}::") || true
  if [ -n "$hits" ]; then
    printf '%s\n' "$hits"
    printf 'ERROR: [cross-context] %s (%s) must not reference other contexts\n\n' "$file" "$ctx" >&2
    FAIL=1
  fi
done < <(find src/contexts -name "*.rs" | sort)

# ── Result ────────────────────────────────────────────────────────────────────
if [ "$FAIL" -eq 1 ]; then
  echo "Layer dependency rule violations found." >&2
  exit 1
fi

echo "All layer dependency rules OK"
