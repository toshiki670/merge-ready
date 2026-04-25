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

This installs two binaries: `merge-ready` (full CLI and daemon) and `merge-ready-prompt` (lightweight prompt binary).

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
merge-ready-prompt
```

Example output:

```text
⚠ review
```

`merge-ready-prompt` returns:

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

The daemon starts automatically the first time `merge-ready-prompt` runs. You can also manage it manually:

```bash
merge-ready daemon start   # start the background daemon (returns immediately)
merge-ready daemon stop    # stop the running daemon
merge-ready daemon status  # show pid, cache entries, and uptime
```

On the first query the daemon has no cache yet, so `? loading` is printed while it fetches in the background. Subsequent calls return the cached value instantly.

The daemon exits automatically after 30 minutes of inactivity.

## Starship Integration

Add merge status to your [Starship](https://starship.rs/) prompt by using a custom command module in `~/.config/starship.toml`:

```toml
[custom.merge_ready]
command = "merge-ready-prompt"
when = true
require_repo = true
shell = ["/bin/zsh"]
format = "[$output]($style) "
style = "bold yellow"
```

`require_repo = true` limits the module to git repositories without any shell command overhead. `merge-ready-prompt` itself returns `? ...` tokens when there is no associated PR, so no additional filtering is needed.

If your environment sets `STARSHIP_SHELL` to a slower shell (for example `fish`), custom modules can be noticeably slower due to shell startup cost. Pinning `shell = ["/bin/zsh"]` (or another lightweight shell on your system) keeps prompt latency low.

## Configuration

The configuration file is read from `$XDG_CONFIG_HOME/merge-ready.toml` (or `~/.config/merge-ready.toml` if `XDG_CONFIG_HOME` is not set).

Open the file in your editor:

```bash
merge-ready config
```

All fields are optional — omitting any field falls back to the default shown below.

```toml
[merge_ready]
symbol = "✓"
label = "merge-ready"
# format = "$symbol $label"

[conflict]
symbol = "✗"
label = "conflict"
# format = "$symbol $label"

[update_branch]
symbol = "✗"
label = "update-branch"
# format = "$symbol $label"

[sync_unknown]
symbol = "?"
label = "sync-unknown"
# format = "$symbol $label"

[ci_fail]
symbol = "✗"
label = "ci-fail"
# format = "$symbol $label"

[ci_action]
symbol = "⚠"
label = "ci-action"
# format = "$symbol $label"

[review]
symbol = "⚠"
label = "review"
# format = "$symbol $label"

[error.auth_required]
symbol = "!"
label = "gh auth login"
# format = "$symbol $label"

[error.rate_limited]
symbol = "✗"
label = "rate-limited"
# format = "$symbol $label"

[error.api_error]
symbol = "✗"
label = "api-error"
# format = "$symbol $label"
```

Each token supports three optional fields:

| Field | Description | Default |
|-------|-------------|---------|
| `symbol` | Leading symbol | see above |
| `label` | Status text | see above |
| `format` | Output template | `"$symbol $label"` |

## Requirements

- `gh` CLI installed and authenticated
- Current git branch linked to an existing GitHub pull request

## Features

- Minimal output focused on actionable blockers
- Prompt-friendly status token output
- Background daemon caches GitHub API results, eliminating per-prompt API calls
- Daemon auto-starts on first use; no manual setup required
