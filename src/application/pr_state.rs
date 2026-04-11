use crate::domain;
use crate::infra::pr_client::{PrClient, PrViewData};

/// `gh pr view` を呼び出し、ドメイン状態に翻訳して返す。
///
/// 取得またはパースに失敗した場合は `None` を返し、エラー出力を行う。
pub fn fetch(client: &impl PrClient) -> Option<PrViewData> {
    match client.pr_view() {
        Ok(data) => Some(data),
        Err(e) => {
            super::errors::handle(e);
            None
        }
    }
}

/// PR が処理対象（`OPEN`）かどうかを判定する
pub fn is_open(data: &PrViewData) -> bool {
    domain::pr_state::is_open(&data.lifecycle)
}
