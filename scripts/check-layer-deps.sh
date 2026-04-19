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
ALL_LAYERS=(domain application infrastructure interface)
ALL_SOURCES=(domain application infrastructure interface bin)

# src/contexts/<ctx>/<layer>.rs or src/contexts/<ctx>/<layer>/... -> <layer>
get_layer() {
  printf '%s' "$1" | sed -E 's|src/contexts/[^/]+/([^/.]+).*|\1|'
}

# src/contexts/<ctx>/... -> <ctx>
get_context() {
  printf '%s' "$1" | sed -E 's|src/contexts/([^/]+)/.*|\1|'
}

is_forbidden_dependency() {
  local from="$1"
  local to="$2"

  for rule in "${FORBIDDEN_LAYERS[@]}"; do
    [ "$rule" = "${from}:${to}" ] && return 0
  done
  # bin rule is defined separately in this script, but it is part of the matrix.
  [ "$from" = "bin" ] && [ "$to" = "domain" ] && return 0
  return 1
}

can_depend_on() {
  local from="$1"
  local to="$2"

  [ "$from" = "$to" ] && return 0
  is_forbidden_dependency "$from" "$to" && return 1
  return 0
}

extract_reexport_target_layer() {
  local line="$1"
  local layer

  for layer in "${ALL_LAYERS[@]}"; do
    if [[ "$line" =~ contexts::[a-z_]+::${layer}([^[:alnum:]_]|$) ]]; then
      printf '%s' "$layer"
      return 0
    fi
    if [[ "$line" =~ (super::)+${layer}([^[:alnum:]_]|$) ]]; then
      printf '%s' "$layer"
      return 0
    fi
    if [[ "$line" =~ self::${layer}([^[:alnum:]_]|$) ]]; then
      printf '%s' "$layer"
      return 0
    fi
  done

  return 1
}

# ── Layer dependency rules ────────────────────────────────────────────────────
while IFS= read -r file; do
  layer=$(get_layer "$file")

  for rule in "${FORBIDDEN_LAYERS[@]}"; do
    from_layer="${rule%%:*}"
    to_layer="${rule##*:}"
    [ "$layer" = "$from_layer" ] || continue

    # Detect both direct imports and re-exports (`use` / `pub use`) that reference
    # forbidden layers via absolute paths or relative paths.
    # Examples:
    # - use crate::contexts::<ctx>::domain::...
    # - pub use crate::contexts::<ctx>::domain::...
    # - use super::domain::...
    # - pub use super::super::domain::...
    hits=$(grep -En "^[[:space:]]*(pub([[:space:]]*\\([^)]*\\))?[[:space:]]+)?use[[:space:]]+[^;]*((crate::)?contexts::[a-z_]+::${to_layer}|(super::)+${to_layer}|self::${to_layer})(::|\\{|;|[[:space:]])" "$file" 2>/dev/null) || true
    if [ -n "$hits" ]; then
      printf '%s\n' "$hits"
      printf 'ERROR: [%s] %s must not depend on [%s] (including re-export)\n\n' "$from_layer" "$file" "$to_layer" >&2
      FAIL=1
    fi
  done
done < <(find src/contexts -name "*.rs" | sort)

# ── Re-export bypass rules ────────────────────────────────────────────────────
# A `pub use` from layer A to layer B can bypass the dependency matrix when
# some source layer S may depend on A but must not depend on B.
while IFS= read -r file; do
  from_layer=$(get_layer "$file")

  while IFS= read -r hit; do
    [ -n "$hit" ] || continue
    line_no="${hit%%:*}"
    line="${hit#*:}"

    target_layer="$(extract_reexport_target_layer "$line")" || continue
    [ "$target_layer" = "$from_layer" ] && continue

    bypass_sources=()
    for source in "${ALL_SOURCES[@]}"; do
      if can_depend_on "$source" "$from_layer" && ! can_depend_on "$source" "$target_layer"; then
        bypass_sources+=("$source")
      fi
    done

    if [ "${#bypass_sources[@]}" -gt 0 ]; then
      printf '%s:%s\n' "$line_no" "$line"
      printf 'ERROR: [re-export-bypass] %s (%s -> %s) can bypass forbidden rules for [%s]\n\n' \
        "$file" "$from_layer" "$target_layer" "$(IFS=,; echo "${bypass_sources[*]}")" >&2
      FAIL=1
    fi
  done < <(grep -En "^[[:space:]]*pub([[:space:]]*\\([^)]*\\))?[[:space:]]+use[[:space:]]+" "$file" 2>/dev/null || true)
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
