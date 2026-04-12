use crate::domain::pr_state::{PrLifecycle, PrStateRepository};

/// ライフサイクル状態を取得する。失敗時は `None` を返してエラー出力する。
pub fn fetch(repo: &impl PrStateRepository) -> Option<PrLifecycle> {
    match repo.fetch_lifecycle() {
        Ok(lifecycle) => Some(lifecycle),
        Err(e) => {
            super::errors::handle(e);
            None
        }
    }
}

/// PR が処理対象（`OPEN`）かどうかを判定する
pub fn is_open(lifecycle: &PrLifecycle) -> bool {
    crate::domain::pr_state::is_open(lifecycle)
}
