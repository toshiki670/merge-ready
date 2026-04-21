# merge_readiness context

## 役割

`merge_readiness` コンテキストは、現在のブランチに紐づく Pull Request が
マージ可能かどうかを判定する中核コンテキストです。
GitHub から取得した状態をポリシーで評価し、プロンプト表示用のトークンに変換します。

## 提供する機能

- PR の状態取得 (`pr_state`)
- ブランチ同期状態の判定 (`branch_sync`)
- CI チェック結果の集約 (`ci_checks`)
- レビュー状態の判定 (`review`)
- 最終的なマージ可否評価 (`merge_ready`, `policy`)
- トークンの表示責務 (`interface/presentation.rs`)

## レイヤー構成

- `domain/`: 判定ロジック、ポリシー、状態モデル
- `application/`: ユースケース実行と出力トークン生成
- `infrastructure/`: `gh` 呼び出しやロギング実装
- `interface/`: CLI 入力と表示変換

## 補足

`application::run` では、独立した取得処理を並列実行して待ち時間を短縮しつつ、
評価結果を `OutputToken` に正規化して上位層へ返します。
