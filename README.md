# merge-ready

Instantly see whether your PR is ready to merge. A GitHub-aware tool that displays only actionable merge blockers.

## Problem

Developers working with GitHub PRs face a common friction:

- You need to know: "Can I merge this PR right now?"
- But checking requires running `gh pr view`, context-switching to GitHub, or parsing verbose output
- PR checks show too much information — conflict details, CI logs, review comments — making it hard to know what action to take next

## Solution

`merge-ready` analyzes your current branch's PR state and outputs **only what matters for merging**:

✗ conflict          → Merge is blocked by conflicts
⚠ update-branch     → Branch needs rebasing
✗ ci-fail           → CI checks failed
⚠ ci-action         → CI requires your action
⚠ review            → Review changes requested
✓ merge-ready       → Ready to merge!

No noise. No context switching. One glance at your prompt tells you exactly what to do next.

## Features

- **Minimal output** — only blockers that prevent merging
- **Smart caching** — reduces GitHub API calls (10s TTL per repo/branch)
- **GitHub-native** — works with `gh` CLI, no extra auth needed
- **Language-agnostic** — written in Python, works across shells
