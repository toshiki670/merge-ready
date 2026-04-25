#!/usr/bin/env bash
# src/protocol/protocol.rs に std 以外の use がないことを確認する。
# std:: / core:: / self:: / super:: / crate:: 以外の use 文があればエラーにする。

set -euo pipefail

FILE="src/protocol/protocol.rs"

if [ ! -f "$FILE" ]; then
  echo "ERROR: $FILE not found" >&2
  exit 1
fi

# use 文を探し、許可されたプレフィックス以外のものがあればエラー
HITS=$(grep -En "^[[:space:]]*use[[:space:]]" "$FILE" \
  | grep -Ev "(std|core|self|super|crate)::" \
  | grep -v "^[[:space:]]*//" \
  || true)

if [ -n "$HITS" ]; then
  echo "$HITS"
  echo "ERROR: $FILE contains non-std use statements" >&2
  exit 1
fi

echo "Protocol deps OK: no external crates in $FILE"
