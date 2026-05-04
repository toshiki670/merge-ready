#!/usr/bin/env bash
set -uo pipefail

cd "${ROOT_DIR:-$(dirname "$0")/..}"

if ! echo "$PR_TITLE" | grep -qE '^(feat|fix)(!)?:'; then
  echo "PR title does not start with feat/fix; skipping E2E check."
  exit 0
fi

changed=$(git diff --name-only "origin/${BASE_REF}...HEAD" -- tests/e2e/)

if [ -z "$changed" ]; then
  printf "ERROR: PR title '%s' requires changes in tests/e2e/ but none were found.\n" "$PR_TITLE" >&2
  exit 1
fi

echo "E2E changes detected:"
echo "$changed"
echo "OK"
