# configuration context

## 役割

`configuration` コンテキストは、`merge-ready` の表示設定やトークン設定を扱うコンテキストです。
ユーザー設定の読み込みと更新を、ドメインに依存しない形で提供します。

## 提供する機能

- 設定値を取得するためのサービス (`application/config_service.rs`)
- 設定値を更新するためのユースケース (`application/config_updater.rs`)
- 設定オブジェクトとリポジトリ境界 (`domain/config.rs`, `domain/repository.rs`)
- TOML ベースの永続化実装 (`infrastructure/toml_loader.rs`)
- CLI からの操作入口 (`interface/cli/`)

## レイヤー構成

- `domain/`: 設定モデルと抽象リポジトリ
- `application/`: 設定取得・更新のユースケース
- `infrastructure/`: TOML 読み書きなどの外部 I/O 実装
- `interface/`: CLI コマンドの受け口
