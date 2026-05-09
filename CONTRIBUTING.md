# Contributing to merge-ready

## Development Philosophy

This project follows TDD as a **design practice**, not merely a testing strategy.

**Red → Green → Refactor**

1. **Red** — Write a failing test that expresses the intended behaviour as a specification.
2. **Green** — Make it pass with the minimum implementation necessary.
3. **Refactor** — Improve the design without changing observable behaviour.

Key principles:

- Tests are specifications. They document *what* the code must do, not *how*.
- Testability reflects design quality. Hard-to-test code signals tight coupling, unclear responsibilities, or hidden side effects. Let that friction guide you toward smaller, well-bounded units.
- Take small steps. Keep the codebase in a working state at every commit. Prefer fast feedback over large upfront design.
- Design evolves. Start with what is needed now. Refactor toward clarity once behaviour is pinned by tests. Avoid speculative abstractions.
- When touching untested code, write characterisation tests first to fix existing behaviour before making changes.

## Prerequisites

| Tool | Purpose |
|------|---------|
| [Rust](https://www.rust-lang.org/tools/install) (stable) | Build and test |
| [mise](https://mise.jdx.dev/) | Toolchain version management |
| [gh](https://cli.github.com/) CLI (authenticated) | Required at runtime |

```bash
mise install   # installs Rust toolchain declared in mise.toml
mise run setup-hooks  # sets local git hooks (pre-commit: fmt, pre-push: clippy)
```

This repository uses local Git hooks configured via `mise` task:

- `pre-commit`: `cargo fmt --all -- --check`
- `pre-commit`: `bash scripts/check-no-clippy-allow.sh` (forbid `#[allow(clippy::...)]` in Rust files)
- `pre-push`: `cargo clippy --workspace --all-targets --all-features -- -D warnings`

Re-run after cloning the repository:

```bash
mise run setup-hooks
```

## Build & Test

```bash
cargo build                          # compile
cargo test --workspace               # run all tests
cargo clippy --workspace -- -D warnings  # lints (must pass)
cargo fmt --check --all              # formatting check
cargo deny check                     # dependency audit
bash scripts/check-layer-deps.sh     # DDD layer dependency rules
bash scripts/check-no-mod-rs.sh      # forbid mod.rs (use Rust 2018+ style)
bash scripts/check-no-clippy-allow.sh # forbid #[allow(clippy::...)] in Rust files
bash scripts/check-pr-title.sh       # require Conventional Commit PR titles (set PR_TITLE)
bash scripts/check-e2e.sh           # feat/fix PRs must include changes under tests/e2e/ (set PR_TITLE and BASE_REF)
```

## File Naming

Use Rust 2018+ module naming: `foo.rs` instead of `foo/mod.rs`.
CI will reject new `mod.rs` files (exception: `tests/e2e/` is allowed).

To fix a violation:

```
foo/bar/mod.rs  →  rename to foo/bar.rs  (keep the content as-is)
```

## Commit Convention

Follow [Conventional Commits](https://www.conventionalcommits.org/). Only `feat`, `fix`, and `perf` commits appear in the changelog (`cliff.toml`).

**Breaking changes:** append `!` after the prefix and include a `BREAKING CHANGE:` footer.

```
feat!: remove --legacy flag

BREAKING CHANGE: --legacy flag has been removed.
```

## Pull Request Flow

Branch naming:

```
<type>/[<issue-id>-]<short-description>
# examples:
feat/42-add-login
fix/PROJECT-8-null-check
chore/update-deps
```

PR checklist:

- [ ] All CI checks (`cargo test`, `clippy`, `fmt`, `deny`, layer deps) pass locally before pushing
- [ ] Each commit is atomic and passes tests on its own
- [ ] Commit messages follow the convention above
- [ ] New behaviour is covered by tests written before the implementation
- [ ] `feat:` / `fix:` PRs include changes under `tests/e2e/`

## Release

Releases are automated via [release-plz](https://release-plz.unksolid.dev/).

1. Run `mise run release-prepare` — creates a Release PR with version bump and updated `CHANGELOG.md`
2. Review and merge the Release PR
3. Merging triggers automatic publication to [crates.io](https://crates.io/crates/merge-ready)
