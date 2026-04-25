#!/usr/bin/env bash
# Test suite for check-layer-deps.sh
#
# Usage: bash scripts/test-check-layer-deps.sh

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CHECK_SCRIPT="$SCRIPT_DIR/check-layer-deps.sh"

PASS=0
FAIL=0

# ── Helpers ──────────────────────────────────────────────────────────────────

make_root() {
    local dir
    dir=$(mktemp -d)
    mkdir -p "$dir/src/contexts/prompt/infrastructure"
    echo "$dir"
}

make_bin_root() {
    local dir
    dir=$(mktemp -d)
    mkdir -p "$dir/src/contexts/prompt/infrastructure"
    touch "$dir/src/main.rs"
    echo "$dir"
}

run_check() {
    local dir="$1"
    ROOT_DIR="$dir" bash "$CHECK_SCRIPT" > /dev/null 2>&1
    echo $?
}

expect_pass() {
    local name="$1"
    local dir="$2"
    if [ "$(run_check "$dir")" -eq 0 ]; then
        echo "PASS: $name"
        PASS=$((PASS + 1))
    else
        echo "FAIL: $name (expected check to pass, but it failed)"
        FAIL=$((FAIL + 1))
    fi
}

expect_fail() {
    local name="$1"
    local dir="$2"
    if [ "$(run_check "$dir")" -ne 0 ]; then
        echo "PASS: $name"
        PASS=$((PASS + 1))
    else
        echo "FAIL: $name (expected check to fail, but it passed)"
        FAIL=$((FAIL + 1))
    fi
}

# ── Test cases ────────────────────────────────────────────────────────────────

# infrastructure → application::port  (ALLOWED: hexagonal port implementation)
DIR=$(make_root)
cat > "$DIR/src/contexts/prompt/infrastructure/gh.rs" << 'EOF'
use crate::contexts::prompt::application::port::PromptStatusPort;
EOF
expect_pass "infrastructure → application::port via absolute path (allowed)" "$DIR"
rm -rf "$DIR"

# infrastructure → application::port via super::  (ALLOWED)
DIR=$(make_root)
cat > "$DIR/src/contexts/prompt/infrastructure/gh.rs" << 'EOF'
use super::super::application::port::PromptStatusPort;
EOF
expect_pass "infrastructure → application::port via super:: (allowed)" "$DIR"
rm -rf "$DIR"

# infrastructure → application::port multi-import  (ALLOWED)
DIR=$(make_root)
cat > "$DIR/src/contexts/prompt/infrastructure/gh.rs" << 'EOF'
use crate::contexts::prompt::application::port::{LoadConfigPort, UpdateConfigPort};
EOF
expect_pass "infrastructure → application::port multi-import (allowed)" "$DIR"
rm -rf "$DIR"

# infrastructure → application service  (FORBIDDEN)
DIR=$(make_root)
cat > "$DIR/src/contexts/prompt/infrastructure/bad.rs" << 'EOF'
use crate::contexts::prompt::application::prompt::fetch_output;
EOF
expect_fail "infrastructure → application::prompt service (forbidden)" "$DIR"
rm -rf "$DIR"

# infrastructure → application config_service  (FORBIDDEN)
DIR=$(make_root)
cat > "$DIR/src/contexts/prompt/infrastructure/bad.rs" << 'EOF'
use crate::contexts::prompt::application::config_service::ConfigService;
EOF
expect_fail "infrastructure → application::config_service (forbidden)" "$DIR"
rm -rf "$DIR"

# bin → domain  (ALLOWED: Composition Root can wire domain types)
DIR=$(make_bin_root)
cat > "$DIR/src/main.rs" << 'EOF'
use crate::contexts::prompt::domain::pr_state::PrLifecycle;
EOF
expect_pass "bin → domain (allowed)" "$DIR"
rm -rf "$DIR"

# ── Result ────────────────────────────────────────────────────────────────────

echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
