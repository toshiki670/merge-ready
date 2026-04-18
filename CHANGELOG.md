# Changelog

All notable changes to this project will be documented in this file.

## [0.1.0] - 2026-04-15
### Bug Fixes
- Treat "no checks reported" as empty CI checks instead of api-error ([`bfebf5d`](https://github.com/toshiki670/merge-ready/commit/bfebf5d6f9996e6ca5db335bc04a7ac422c762ce))
- Remove #[allow(dead_code)] from GhCheckItem.state ([`604750e`](https://github.com/toshiki670/merge-ready/commit/604750e4b587caa7bf7d34b293bac0ce62a0e8d8))
- No-args shows help; prompt subcommand required for PR status ([`98374e7`](https://github.com/toshiki670/merge-ready/commit/98374e7a0a42f54ad2135dd50f4b653a41908828))
- Use worktree path as cache key, flatten cache file structure ([`36a58c7`](https://github.com/toshiki670/merge-ready/commit/36a58c733147fa7ec1a3c34967adee3dacc7efcb))
- Pass PromptArgs by reference to satisfy clippy::needless_pass_by_value ([`eda26b3`](https://github.com/toshiki670/merge-ready/commit/eda26b36148fbb7c4ec48e5abef45f922426ae51))
- Prevent cache corruption on refresh error; deduplicate background spawns ([`f7f722a`](https://github.com/toshiki670/merge-ready/commit/f7f722aca5919ccf4f74694fa3b113eba434f8bc))
- Show nothing (not "? loading") when outside a git repository ([`e485791`](https://github.com/toshiki670/merge-ready/commit/e48579160b4014ba94a94ded3dcd01800ac76abd))
- Address PR review issues 1-3 ([`96d601e`](https://github.com/toshiki670/merge-ready/commit/96d601ec0e3b32250b49d8e673e438f9b31fce47))
- Address PR review issues 4-6 ([`625d7e7`](https://github.com/toshiki670/merge-ready/commit/625d7e74e25753d77a87ae186d7113871b596015))
- Simplify lock, include branch in cache key ([`2b44aa2`](https://github.com/toshiki670/merge-ready/commit/2b44aa2068d1f5964b372c478621089fd21edab4))
- Address PR review issues (PID reuse, atomic write, DDD layers) ([`165a3d8`](https://github.com/toshiki670/merge-ready/commit/165a3d83ceaa6a0fd93cd74dceb2d1ba09af9995))
- Eliminate empty-lock window and handle write failure in create_with_pid ([`9ae6f7b`](https://github.com/toshiki670/merge-ready/commit/9ae6f7b178907b04c3958e2f3e478ee1fc5c1fe8))
- Pass --repo-id to child, move spawn to CLI, use PID-based tmp ([`75d5aae`](https://github.com/toshiki670/merge-ready/commit/75d5aae77d9be695de35c4e63a18da7484be1f3b))
- Prevent code injection in dependabot-security workflow ([`54bf9e4`](https://github.com/toshiki670/merge-ready/commit/54bf9e4833281a0964f763839129a3f090aa9c57))
- Use GitHub App token in release-prepare to allow PR creation ([`d5e56c2`](https://github.com/toshiki670/merge-ready/commit/d5e56c2bfd3e336dcf84dbbb9fee2df6eddcf117))
- Align release-publish trigger with actual branch prefix ([`a5a5cb2`](https://github.com/toshiki670/merge-ready/commit/a5a5cb20cda2d321f14b40528af38a4d080cf882))
### Features
- Add E2E red tests and introduce rustfmt/clippy ([`011dfa1`](https://github.com/toshiki670/merge-ready/commit/011dfa160d2f30f12c256f522090e41e829e4ca8))
- Implement core PR merge readiness evaluation logic ([`e0d1a9d`](https://github.com/toshiki670/merge-ready/commit/e0d1a9d4e5998771da9e3b83a6b3eccca416b4c3))
- Detect update-branch via GitHub Compare API ([`20db7dd`](https://github.com/toshiki670/merge-ready/commit/20db7dd14ece9155b1aa69b9b597bc95cdc00caa))
- Sub-40ms latency via cache-first architecture (closes #7) ([`4216563`](https://github.com/toshiki670/merge-ready/commit/4216563356cb7fb753d1438f793711a49873800f))
- Add clap-based CLI with help and prompt subcommands (Issue #9) ([`a835a56`](https://github.com/toshiki670/merge-ready/commit/a835a560621c3c80318dc4528c1e33094f7a4946))
- Implement --refresh and --no-cache for prompt subcommand ([`a9992c5`](https://github.com/toshiki670/merge-ready/commit/a9992c5b0508eb95baa8a84e980df02153f8a778))
