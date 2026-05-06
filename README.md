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
⚠ Resolve review
```

`merge-ready-prompt` prints a single status token to stdout and always exits with code `0`. Use the printed token text for conditional logic in shell scripts and prompt hooks.

## Output Tokens

- `✓ Ready for merge` - ready to merge
- `✎ Ready for review` - pull request is in draft state
- `+ Create PR` - branch exists but no pull request has been created yet
- `⚠ Resolve review` - changes were requested in review
- `@ Assign reviewer` - no reviewer assigned yet
- `⚠ Run CI action` - CI checks require manual action
- `⧖ Wait for CI` - CI checks are pending
- `✗ Fix CI failure` - CI checks failed
- `✗ Resolve conflict` - merge conflicts exist
- `✗ Update branch` - branch is behind base branch
- `? Check branch sync` - branch sync status is unknown
- `? Check merge blocker` - PR is blocked for an unknown reason
- `⧖ Wait for status` - GitHub is calculating merge status
- `? loading` - cache miss; daemon is fetching in the background

### Multiple Pull Requests

When a branch has **multiple open pull requests**, all statuses are displayed in PR-number ascending order, each suffixed with `#<number>`:

```text
✓ Ready for merge #200 ✎ Ready for review #201
```

When there is only one open pull request, the `#<number>` suffix is omitted:

```text
✓ Ready for merge
```

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

### Watch Mode

To inspect all cached entries in real time, use the `watch` subcommand:

```bash
merge-ready watch   # refresh every second; Ctrl+C to stop
```

Example output:

```text
CWD              BRANCH          STATUS             CACHED AT
/home/user/myapp feat/42-login   ✓ Ready for merge  5s ago
/home/user/myapp main            ⧖ Wait for CI      23s ago
```

`watch` polls the daemon once per second and redraws the table in place. If the daemon is not running it prints `daemon is not running` and exits immediately.

## Starship Integration

Add merge status to your [Starship](https://starship.rs/) prompt by using a custom command module in `~/.config/starship.toml`:

```toml
[custom.merge_ready]
command = "merge-ready-prompt"
when = true
require_repo = true
shell = ["/bin/zsh"]
format = "($output )"
```

`require_repo = true` limits the module to git repositories without any shell command overhead. `merge-ready-prompt` itself returns `+ Create PR` when no pull request exists for the branch, so no additional filtering is needed.

If your environment sets `STARSHIP_SHELL` to a slower shell (for example `fish`), custom modules can be noticeably slower due to shell startup cost. Pinning `shell = ["/bin/zsh"]` (or another lightweight shell on your system) keeps prompt latency low.

> **Note:** Do not set `style` in the Starship custom module when using the `[text](style)` syntax in merge-ready's `format` field. Starship's `style` wraps the entire output in its own ANSI codes, and merge-ready's internal reset sequences will break that outer styling, causing subsequent prompt modules (e.g. `cmd_duration`) to lose their color.

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
label = "Ready for merge"
format = "[$symbol $label( #$pr_id)](bold green)"

[no_pull_request]
symbol = "+"
label = "Create PR"
format = "[$symbol $label](cyan)"

[conflict]
symbol = "✗"
label = "Resolve conflict"
format = "[$symbol $label( #$pr_id)](bold red)"

[update_branch]
symbol = "✗"
label = "Update branch"
format = "[$symbol $label( #$pr_id)](yellow)"

# [sync_unknown]
# symbol = "?"
# label = "Check branch sync"
# format = "[$symbol $label( #$pr_id)](yellow)"

[ci_fail]
symbol = "✗"
label = "Fix CI failure"
format = "[$symbol $label( #$pr_id)](bold red)"

# [ci_action]
# symbol = "⚠"
# label = "Run CI action"
# format = "[$symbol $label( #$pr_id)](yellow)"

[ci_pending]
symbol = "⧖"
label = "Wait for CI"
format = "[$symbol $label( #$pr_id)](cyan)"

[changes_requested]
symbol = "⚠"
label = "Resolve review"
format = "[$symbol $label( #$pr_id)](yellow)"

[review_required]
symbol = "@"
label = "Assign reviewer"
format = "[$symbol $label( #$pr_id)](cyan)"

[draft]
symbol = "✎"
label = "Ready for review"
format = "[$symbol $label( #$pr_id)](dimmed)"

# [status_calculating]
# symbol = "⧖"
# label = "Wait for status"
# format = "[$symbol $label( #$pr_id)](dimmed)"

# [blocked_unknown]
# symbol = "?"
# label = "Check merge blocker"
# format = "[$symbol $label( #$pr_id)](yellow)"

[error]
symbol = "✗"
format = "[$symbol $message](bold red)"
```

Each token supports three optional fields:

| Field | Description | Default |
|-------|-------------|---------|
| `symbol` | Leading symbol | see above |
| `label` | Status text | see above |
| `format` | Output template | `"$symbol $label"` |

The `[error]` section uses `$message` instead of `$label`. The message is set automatically from the error that occurred (e.g. `authentication required`, `rate limited`, or the raw API error message).

| Field | Description | Default |
|-------|-------------|---------|
| `symbol` | Leading symbol | `"✗"` |
| `format` | Output template | `"$symbol $message"` |

### Format Variables

The following variables are available in `format` templates:

| Variable | Description | Available in |
|----------|-------------|--------------|
| `$symbol` | Leading symbol | all tokens |
| `$label` | Status text | PR state tokens, `no_pull_request` |
| `$pr_id` | PR number string; empty `""` when only one open PR exists | PR state tokens (12 types) |
| `$message` | Error message | `[error]` only |

`$pr_id` is not present in `no_pull_request` or `[error]` tokens. Writing `$pr_id` in those format strings leaves the literal text `$pr_id` in the output.

### Conditional Format Strings

The `format` field supports a `(...)` syntax that hides the block when all variables inside are empty. This follows [Starship's conditional format strings](https://starship.rs/config/#conditional-format-strings).

```toml
# `( #$pr_id)` is shown only when $pr_id is non-empty (i.e., multiple PRs on the branch)
[merge_ready]
format = "$symbol $label( #$pr_id)"
```

| `$pr_id` value | Output |
|----------------|--------|
| `"200"` (multiple PRs) | `✓ Ready for merge #200` |
| `""` (single PR) | `✓ Ready for merge` |

Rules:

- All variables in `(...)` are empty → block is hidden
- At least one variable is non-empty → block is rendered normally
- No variables in `(...)` → block is always hidden

Conditional blocks can also appear inside a `[text](style)` block:

```toml
[merge_ready]
format = "[$symbol $label( #$pr_id)](bold green)"
```

This applies the style to the whole output while still hiding `( #$pr_id)` when `$pr_id` is empty.

### Style Strings

The `format` field supports a [Starship-inspired](https://starship.rs/config/#style-strings) `[text](style)` syntax to apply ANSI colors and attributes:

```toml
[merge_ready]
format = "[$symbol $label](bold green)"

[ci_fail]
format = "[$symbol](bold red) $label"

[changes_requested]
format = "[$symbol](yellow) $label"
```

Supported style specifiers:

| Specifier | Examples |
|-----------|---------|
| Color names | `red`, `green`, `yellow`, `blue`, `cyan`, `purple`, `white`, `black` |
| Bright colors | `bright-red`, `bright-green`, … |
| Attributes | `bold`, `italic`, `underline`, `dimmed`, `inverted`, `blink`, `hidden`, `strikethrough` |
| 256-color / truecolor | `fg:123`, `bg:255`, `fg:#ff8700` |
| Disable all styles | `none` |

Specifiers are case-insensitive and order-independent. Multiple specifiers are separated by spaces (e.g. `bold bright-green`).

## Requirements

- `gh` CLI installed and authenticated
- Current git branch linked to an existing GitHub pull request

## Features

- Minimal output focused on actionable blockers
- Prompt-friendly status token output
- Background daemon caches GitHub API results, eliminating per-prompt API calls
- Daemon auto-starts on first use; no manual setup required
