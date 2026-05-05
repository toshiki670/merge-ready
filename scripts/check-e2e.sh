#!/usr/bin/env bash
set -uo pipefail

cd "${ROOT_DIR:-$(dirname "$0")/..}"

if echo "$PR_TITLE" | grep -qE '^(feat|fix)(!)?:'; then
  changed=$(git diff --name-only "origin/${BASE_REF}...HEAD" -- tests/e2e/)
  if [ -z "$changed" ]; then
    printf "ERROR: PR title '%s' requires changes in tests/e2e/ but none were found.\n" "$PR_TITLE" >&2
    exit 1
  fi
  echo "E2E changes detected:"
  echo "$changed"
  echo "OK"
  exit 0
fi

if echo "$PR_TITLE" | grep -qE '^test(!)?:'; then
  changed=$(git diff --name-only "origin/${BASE_REF}...HEAD" -- tests/)
  if [ -z "$changed" ]; then
    printf "ERROR: PR title '%s' requires changes in tests/ but none were found.\n" "$PR_TITLE" >&2
    exit 1
  fi
  echo "Test changes detected:"
  echo "$changed"
  echo "OK"
  exit 0
fi

changed=$(git diff --name-only "origin/${BASE_REF}...HEAD" -- tests/)
if [ -n "$changed" ]; then
  printf "WARN: PR title '%s' is not feat/fix/test but test files were modified:\n" "$PR_TITLE"
  echo "$changed"
fi

exit 0
