# Changelog

All notable changes to this project will be documented in this file.

## [0.1.1] - 2026-04-18
### Bug Fixes
- Add toolchain input to dtolnay/rust-toolchain in CodeQL workflow ([`8074938`](https://github.com/toshiki670/merge-ready/commit/807493850b903e0a7f04b3d21b0d487c2a042e12))
- Change CodeQL Rust build-mode from manual to none ([`31bad17`](https://github.com/toshiki670/merge-ready/commit/31bad175817ce11fc21c7dd84083a0b456e25a01))
- Squash-merge 専用の commit_preprocessors Step 2 を削除 ([#36](https://github.com/toshiki670/merge-ready/issues/36)) ([#38](https://github.com/toshiki670/merge-ready/issues/38)) ([#39](https://github.com/toshiki670/merge-ready/issues/39)) ([#36](https://github.com/toshiki670/merge-ready/issues/36)) ([#38](https://github.com/toshiki670/merge-ready/issues/38)) ([#39](https://github.com/toshiki670/merge-ready/issues/39)) ([`510d2c7`](https://github.com/toshiki670/merge-ready/commit/510d2c7bf3d91c4c282eec4615cc1d18b21fb9c1))
- テスト環境から XDG_CONFIG_HOME を除去して設定読み込みを隔離 ([`cddef71`](https://github.com/toshiki670/merge-ready/commit/cddef71d179c16d19557df65df71cb7e358034c6))
- テスト環境で XDG_CONFIG_HOME を HOME/.config に固定 ([`2b2a3a3`](https://github.com/toshiki670/merge-ready/commit/2b2a3a37190eafdae81fc7cb351854bf2fd8894c))
- Release-plz で feat が minor バンプ・CHANGELOG に反映されない問題を修正 ([`64e4227`](https://github.com/toshiki670/merge-ready/commit/64e4227fee87e0b2528dd75d6aa655821e569ed0))
### Features
- ~/.config/merge-ready.toml によるシンボル・フォーマットのカスタマイズ ([`f1b8759`](https://github.com/toshiki670/merge-ready/commit/f1b8759af2e0e45320129a39f97c9aadd32f294c))
- XDG_CONFIG_HOME に対応した設定ファイルパス解決 ([`73252a7`](https://github.com/toshiki670/merge-ready/commit/73252a7e3d4814660a9354c30eed3970fb14decd))
### Performance
- キャッシュパスを tmpfs（/tmp）に変更 ([#35](https://github.com/toshiki670/merge-ready/issues/35)) ([#35](https://github.com/toshiki670/merge-ready/issues/35)) ([`a79d6bc`](https://github.com/toshiki670/merge-ready/commit/a79d6bc59b6900907020a9e1f94928da2bcaca57))
- Git 子プロセスを廃止し .git ディレクトリ直接読み取りで repo_id を生成 ([#36](https://github.com/toshiki670/merge-ready/pull/36)) ([#36](https://github.com/toshiki670/merge-ready/issues/36)) ([#36](https://github.com/toshiki670/merge-ready/issues/36)) ([#36](https://github.com/toshiki670/merge-ready/issues/36)) ([#36](https://github.com/toshiki670/merge-ready/issues/36)) ([`cd86f2a`](https://github.com/toshiki670/merge-ready/commit/cd86f2adbdacce71e479a85bda0bf6425f2439f9))



