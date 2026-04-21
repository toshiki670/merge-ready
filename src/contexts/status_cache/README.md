# status_cache context

## 役割

`status_cache` コンテキストは、`merge-ready prompt` の低遅延応答のために
バックグラウンドデーモンとキャッシュを管理するコンテキストです。
マージ判定結果をキャッシュし、CLI からは軽量に取得できるようにします。

## 提供する機能

- キャッシュモデルと更新ロジック (`domain/cache.rs`, `application/cache.rs`)
- デーモン稼働状態の管理 (`domain/daemon.rs`, `application/lifecycle.rs`)
- デーモンサーバー/クライアント実装 (`infrastructure/daemon_server.rs`, `infrastructure/daemon_client.rs`)
- デーモンの PID・ソケット・パス管理 (`infrastructure/pid.rs`, `infrastructure/paths.rs`)
- CLI からの `daemon` サブコマンド (`interface/cli/daemon.rs`)

## レイヤー構成

- `domain/`: キャッシュとデーモンの状態モデル
- `application/`: キャッシュ運用・ライフサイクル制御
- `infrastructure/`: IPC プロトコルとプロセス実装
- `interface/`: CLI コマンド処理
