# Changelog

All notable changes to this project will be documented in this file.

## [0.1.1] - 2026-04-18
### Bug Fixes
- Add toolchain input to dtolnay/rust-toolchain in CodeQL workflow
- Change CodeQL Rust build-mode from manual to none
- Squash-merge 専用の commit_preprocessors Step 2 を削除
- テスト環境から XDG_CONFIG_HOME を除去して設定読み込みを隔離
- テスト環境で XDG_CONFIG_HOME を HOME/.config に固定
- Release-plz で feat が minor バンプ・CHANGELOG に反映されない問題を修正

### Features
- ~/.config/merge-ready.toml によるシンボル・フォーマットのカスタマイズ
- XDG_CONFIG_HOME に対応した設定ファイルパス解決

### Performance
- キャッシュパスを tmpfs（/tmp）に変更
- Git 子プロセスを廃止し .git ディレクトリ直接読み取りで repo_id を生成 ([[#36](https://github.com/toshiki670/merge-ready/issues/36)](https://github.com/toshiki670/merge-ready/issues/36))




