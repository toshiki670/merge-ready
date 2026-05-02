# Changelog

All notable changes to this project will be documented in this file.

## [0.6.0] - 2026-05-02
### Bug Fixes
- Remove needless pass-by-value allow in cli::run ([#226](https://github.com/toshiki670/merge-ready/pull/226)) ([`2ed55c9`](https://github.com/toshiki670/merge-ready/commit/2ed55c910d6f9a7deb2ed5faf271c07c65305f55))
- Align error category logging for auth and timeout ([#227](https://github.com/toshiki670/merge-ready/pull/227)) ([`7692847`](https://github.com/toshiki670/merge-ready/commit/76928475a5aa698717f08d2d28e9f2ff33743f39))
### Features
- [**BREAKING**] Split daemon_command into daemon_start/stop/status_command ([#230](https://github.com/toshiki670/merge-ready/pull/230)) ([`396cbea`](https://github.com/toshiki670/merge-ready/commit/396cbeabc3d85d4f85e330c449bd14d5faab0a8b))
- Format フィールドで Starship 風の色・スタイル指定をサポートする ([#231](https://github.com/toshiki670/merge-ready/pull/231)) ([`4b81a93`](https://github.com/toshiki670/merge-ready/commit/4b81a937bd09cef2734db2626bb1e2c5bf3fb773))


## [0.5.3] - 2026-04-30
### Bug Fixes
- Docs.rs ビルドのために [lib] ターゲットを追加し app.rs を lib.rs に統合する ([#219](https://github.com/toshiki670/merge-ready/pull/219)) ([`541b318`](https://github.com/toshiki670/merge-ready/commit/541b318cb0842c09c3a1814de1085c8978871e3f))


## [0.5.2] - 2026-04-29
### Features
- Daemon の定期取得サイクルをリポジトリごとに適応的に制御する ([#198](https://github.com/toshiki670/merge-ready/pull/198)) ([`5e25824`](https://github.com/toshiki670/merge-ready/commit/5e258242a304c2105f35a621a30c73e65aa6b6c0))
### Performance
- リリースバイナリにデバッグシンボルの strip を追加する ([#199](https://github.com/toshiki670/merge-ready/pull/199)) ([`cf9b930`](https://github.com/toshiki670/merge-ready/commit/cf9b930b47480851f2d970ff55dfe3e0020c2dd1))


## [0.5.1] - 2026-04-28
### Features
- BLOCKED かつ理由不明のとき ? Check merge blocker を表示する ([#192](https://github.com/toshiki670/merge-ready/pull/192)) ([`d63f075`](https://github.com/toshiki670/merge-ready/commit/d63f075d11c9e92fe72373eb9391a8ae260244b2))


## [0.5.0] - 2026-04-27
### Features
- ReviewDecision == "REVIEW_REQUIRED" のとき @ assign-reviewer を表示する ([#180](https://github.com/toshiki670/merge-ready/pull/180)) ([`8ea56db`](https://github.com/toshiki670/merge-ready/commit/8ea56dbe0190d62fb8607473903e5b3373115446))
- CI チェック実行中（pending）のとき ⧖ wait-for-ci を表示する ([#181](https://github.com/toshiki670/merge-ready/pull/181)) ([`85a084a`](https://github.com/toshiki670/merge-ready/commit/85a084ae413f8676277097e7f8ebe3f03d321a5b))
- MergeStateStatus が UNKNOWN のとき ⧖ wait-for-status を表示する ([#182](https://github.com/toshiki670/merge-ready/pull/182)) ([`96b6ae8`](https://github.com/toshiki670/merge-ready/commit/96b6ae831c21a586bef62ca2a83cfdcab3caca98))
- [**BREAKING**] ラベルをセンテンスケースのアクション表現に統一し、review キーを changes_requested に改名 ([#185](https://github.com/toshiki670/merge-ready/pull/185)) ([`e2e5746`](https://github.com/toshiki670/merge-ready/commit/e2e5746108945dfd60f48fd2e22b06dd4e631f38))
- [**BREAKING**] [error] を単一セクションに統一し、静的 label をエラーメッセージで置換する ([#189](https://github.com/toshiki670/merge-ready/pull/189)) ([`4503387`](https://github.com/toshiki670/merge-ready/commit/4503387f02d3385c961aae7fff53f7ebe95bc3f5))


## [0.4.3] - 2026-04-27
### Features
- Draft PR のとき ✎ ready-for-review を表示する ([#173](https://github.com/toshiki670/merge-ready/pull/173)) ([`c639ecf`](https://github.com/toshiki670/merge-ready/commit/c639ecfa0f0a1dd73f62f1d677c5105f977c827a))


## [0.4.2] - 2026-04-27
### Features
- PR 未作成時に  を表示する ([#167](https://github.com/toshiki670/merge-ready/pull/167)) ([`c65a91e`](https://github.com/toshiki670/merge-ready/commit/c65a91e491d4c505ee0f9295af890e099747f364))


## [0.4.1] - 2026-04-26
### Bug Fixes
- Stale_delay_ms の初回 gh 呼び出しを即時化してフレーキーを解消 ([#152](https://github.com/toshiki670/merge-ready/pull/152)) ([`9bbd609`](https://github.com/toshiki670/merge-ready/commit/9bbd609e6316ded71ee4c61d04523c2c715af309))
### Features
- PR が closed/merged になったらリフレッシュを停止する ([#150](https://github.com/toshiki670/merge-ready/pull/150)) ([`192f153`](https://github.com/toshiki670/merge-ready/commit/192f153b767dd51939549b5fab324ef7a9a4b7e8))


## [0.4.0] - 2026-04-25
### Features
- [**BREAKING**] Config update サブコマンドと version フィールドの廃止、config edit → config への統合 ([#142](https://github.com/toshiki670/merge-ready/pull/142)) ([`4c8c9be`](https://github.com/toshiki670/merge-ready/commit/4c8c9beaa39c842e37a95aaaa5b20e2191625127))
- [**BREAKING**] Merge-ready-prompt 軽量バイナリの追加と prompt サブコマンドの削除 ([#147](https://github.com/toshiki670/merge-ready/pull/147)) ([`91d6bf4`](https://github.com/toshiki670/merge-ready/commit/91d6bf47cf76a6bfcb41d7737c1b8522d229b69c))


## [0.3.1] - 2026-04-24
### Bug Fixes
- Migrate app-id to client-id in release-prepare workflow ([#130](https://github.com/toshiki670/merge-ready/pull/130)) ([`c2ad532`](https://github.com/toshiki670/merge-ready/commit/c2ad53239efeab4045f0729bb28e2f24bda8fcb3))
### Features
- ErrorCategory / LogRecord の導入と simplelog による構造化ロギング ([#133](https://github.com/toshiki670/merge-ready/pull/133)) ([`da5b8ab`](https://github.com/toshiki670/merge-ready/commit/da5b8ab90f8e86112d57c76fb5ed5f5444ccddf4))


## [0.3.0] - 2026-04-24
### Features
- [**BREAKING**] Remove --no-cache option from prompt subcommand ([#125](https://github.com/toshiki670/merge-ready/pull/125)) ([`a46d89d`](https://github.com/toshiki670/merge-ready/commit/a46d89db104945b1d11986407af02b4a332fcdf8))


## [0.2.1] - 2026-04-22
### Bug Fixes
- Write error logs to merge-ready cache directory ([#115](https://github.com/toshiki670/merge-ready/pull/115)) ([`c344e30`](https://github.com/toshiki670/merge-ready/commit/c344e3003f845f08068dbe9ea6036c1ea093bbe4))


## [0.2.0] - 2026-04-21
### Bug Fixes
- Add timeout to gh command execution to prevent indefinite hang ([#84](https://github.com/toshiki670/merge-ready/pull/84)) ([`ab4f665`](https://github.com/toshiki670/merge-ready/commit/ab4f665aa2bf12d49f16c446e8c9244736055cd9))
- PRのないブランチで「? loading」が永続表示されるバグを修正 ([#90](https://github.com/toshiki670/merge-ready/pull/90)) ([`551080f`](https://github.com/toshiki670/merge-ready/commit/551080f895c607a36158c8c6c09cddfcb625ad18))
- Stabilize daemon startup and no-PR stale cache behavior ([#102](https://github.com/toshiki670/merge-ready/pull/102)) ([`3e95be7`](https://github.com/toshiki670/merge-ready/commit/3e95be70fe6cfdc71f95fcb0874fb1aab14a98ad))
- Restart daemon when prompt detects version mismatch ([#105](https://github.com/toshiki670/merge-ready/pull/105)) ([`cbea420`](https://github.com/toshiki670/merge-ready/commit/cbea42004d71870938297887602f343af24f87f5))
### Features
- [**BREAKING**] デーモン + Unix ソケットによる sub-ms キャッシュ応答（StatusCache BC） ([#75](https://github.com/toshiki670/merge-ready/pull/75)) ([`c2f9935`](https://github.com/toshiki670/merge-ready/commit/c2f99352589eeefb431564952e8e2a21228695d7))


## [0.1.2] - 2026-04-18
### Features
- Config edit / update サブコマンドを追加 ([#72](https://github.com/toshiki670/merge-ready/pull/72)) ([`257cb3e`](https://github.com/toshiki670/merge-ready/commit/257cb3ed51f68719951c09a77ae2ae33a8264fcd))


## [0.1.1] - 2026-04-18
### Bug Fixes
- Add toolchain input to dtolnay/rust-toolchain in CodeQL workflow ([`8074938`](https://github.com/toshiki670/merge-ready/commit/807493850b903e0a7f04b3d21b0d487c2a042e12))
- Change CodeQL Rust build-mode from manual to none ([`31bad17`](https://github.com/toshiki670/merge-ready/commit/31bad175817ce11fc21c7dd84083a0b456e25a01))
- Squash-merge 専用の commit_preprocessors Step 2 を削除 ([`510d2c7`](https://github.com/toshiki670/merge-ready/commit/510d2c7bf3d91c4c282eec4615cc1d18b21fb9c1))
- テスト環境から XDG_CONFIG_HOME を除去して設定読み込みを隔離 ([`cddef71`](https://github.com/toshiki670/merge-ready/commit/cddef71d179c16d19557df65df71cb7e358034c6))
- テスト環境で XDG_CONFIG_HOME を HOME/.config に固定 ([`2b2a3a3`](https://github.com/toshiki670/merge-ready/commit/2b2a3a37190eafdae81fc7cb351854bf2fd8894c))
- Release-plz で feat が minor バンプ・CHANGELOG に反映されない問題を修正 ([`64e4227`](https://github.com/toshiki670/merge-ready/commit/64e4227fee87e0b2528dd75d6aa655821e569ed0))
- Link_parsers を削除 ([#53](https://github.com/toshiki670/merge-ready/pull/53)) ([`2c6224e`](https://github.com/toshiki670/merge-ready/commit/2c6224e6ef16dfcbb53522459b0e52506194c23b))
- バージョンを 0.1.0 に戻す ([#59](https://github.com/toshiki670/merge-ready/pull/59)) ([`68a03a0`](https://github.com/toshiki670/merge-ready/commit/68a03a0809e4127520eeb46ccd1a86eda317b351))
- レイヤー依存違反を設計レベルで修正 ([`70afcef`](https://github.com/toshiki670/merge-ready/commit/70afcef2e3528ea1126eb585c210d1866f91d2e7))
### Features
- ~/.config/merge-ready.toml によるシンボル・フォーマットのカスタマイズ ([`f1b8759`](https://github.com/toshiki670/merge-ready/commit/f1b8759af2e0e45320129a39f97c9aadd32f294c))
- XDG_CONFIG_HOME に対応した設定ファイルパス解決 ([`73252a7`](https://github.com/toshiki670/merge-ready/commit/73252a7e3d4814660a9354c30eed3970fb14decd))
- 内部クレートの changelog_update を有効化 ([#57](https://github.com/toshiki670/merge-ready/pull/57)) ([`a7b7586`](https://github.com/toshiki670/merge-ready/commit/a7b75866ce22f18f085f70851df493159767cde1))
### Performance
- キャッシュパスを tmpfs（/tmp）に変更 ([`a79d6bc`](https://github.com/toshiki670/merge-ready/commit/a79d6bc59b6900907020a9e1f94928da2bcaca57))
- Git 子プロセスを廃止し .git ディレクトリ直接読み取りで repo_id を生成 ([#36](https://github.com/toshiki670/merge-ready/pull/36)) ([`cd86f2a`](https://github.com/toshiki670/merge-ready/commit/cd86f2adbdacce71e479a85bda0bf6425f2439f9))


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
