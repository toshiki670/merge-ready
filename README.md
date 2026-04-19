# merge-ready

[![Crates.io](https://img.shields.io/crates/v/merge-ready)](https://crates.io/crates/merge-ready)
[![Downloads](https://img.shields.io/crates/d/merge-ready)](https://crates.io/crates/merge-ready)
[![Docs.rs](https://img.shields.io/docsrs/merge-ready)](https://docs.rs/merge-ready)
[![CI](https://img.shields.io/github/actions/workflow/status/toshiki670/merge-ready/ci.yml?branch=main&label=ci)](https://github.com/toshiki670/merge-ready/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/merge-ready)](https://github.com/toshiki670/merge-ready/blob/main/LICENSE)

`merge-ready` is a Rust CLI that reports whether the pull request for your current branch is mergeable. It prints concise status tokens designed for shell prompt integration and automation scripts.

## Install

```bash
cargo install merge-ready
```

For development builds:

```bash
cargo install --path .
```

## Usage

Show top-level help:

```bash
merge-ready --help
```

Show merge status tokens for prompt integration:

```bash
merge-ready prompt
```

Bypass cache and fetch fresh state:

```bash
merge-ready prompt --no-cache
```

Example output:

```text
⚠ review
```

`merge-ready prompt` returns:

- `0` when mergeable (`✓ merge-ready`)
- `1` when blocked (`⚠ ...` or `✗ ...`)
- `2` when state cannot be determined (`? ...`)

This makes it easy to use from shell scripts and prompt hooks.

## Output Tokens

- `✓ merge-ready` - ready to merge
- `⚠ review` - changes were requested in review
- `⚠ ci-action` - CI checks are still in progress
- `✗ ci-fail` - CI checks failed
- `✗ conflict` - merge conflicts exist
- `✗ update-branch` - branch is behind base branch
- `? sync-unknown` - branch sync status is unknown
- `? loading` - cache miss; daemon is fetching in the background

## Background Daemon

`merge-ready` uses a background daemon to cache GitHub API results and serve prompt queries with near-zero latency.

The daemon starts automatically the first time `merge-ready prompt` runs. You can also manage it manually:

```bash
merge-ready daemon start   # start the background daemon (returns immediately)
merge-ready daemon stop    # stop the running daemon
merge-ready daemon status  # show pid, cache entries, and uptime
```

On the first query the daemon has no cache yet, so `? loading` is printed while it fetches in the background. Subsequent calls return the cached value instantly.

The daemon exits automatically after 30 minutes of inactivity.

## Requirements

- `gh` CLI installed and authenticated
- Current git branch linked to an existing GitHub pull request

## Features

- Minimal output focused on actionable blockers
- Prompt-friendly status token output
- Background daemon caches GitHub API results, eliminating per-prompt API calls
- Daemon auto-starts on first use; no manual setup required
